//! ffd
//!
//! The Fast File client.

use clap::{App, Arg, SubCommand};
use mio::{net::UdpSocket, Events, Interest, Poll, Token};

use std::{
    io::{stdout, Cursor, Write},
    path::PathBuf,
    process::exit,
    thread::sleep,
    time::Duration,
};

mod proto;

use proto::*;

const UDP_SOCKET: Token = Token(0);

fn main() {
    let matches = App::new("ff")
        .version("v0.2.0")
        .long_version("v0.2.0 ff@15")
        .args(&[Arg::with_name("addr")
            .required(true)
            .help("address to connect to")])
        .subcommand(
            SubCommand::with_name("ls")
                .about("List contents held remotely")
                .args(&[
                    Arg::with_name("path")
                        .default_value(".")
                        .help("Path of the directory to list"),
                    Arg::with_name("csv")
                        .short("c")
                        .takes_value(false)
                        .help("Print directory information as a CSV"),
                ]),
        )
        .subcommand(
            SubCommand::with_name("get")
                .alias("g")
                .arg(
                    Arg::with_name("path")
                        .value_name("PATH")
                        .required(true)
                        .multiple(true)
                        .help("Path(s) of the file(s) to download"),
                )
                .about("Download files"),
        )
        .get_matches();

    // Create our transport.
    let mut sock = match UdpSocket::bind("0.0.0.0".parse().unwrap()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };
    sock.connect(
        matches
            .value_of("addr")
            .unwrap()
            .parse()
            .expect("valid addr required"),
    )
    .expect("valid addr required");

    let poll = match Poll::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };
    poll.registry()
        .register(&mut sock, UDP_SOCKET, Interest::READABLE)
        .unwrap();
    let mut events = Events::with_capacity(5);

    if let Some(matches) = matches.subcommand_matches("ls") {
        match send_recv_ad_nauseum(
            &mut sock,
            Request::List {
                path: matches.value_of("path").unwrap().to_string(),
            },
            Duration::from_secs(3),
        ) {
            Some(Response::Directory(files)) => {
                print_filedata(files, matches.is_present("csv"));
                exit(0)
            }
            Some(Response::NotAllowed) => {
                eprintln!("Not allowed");
            }
            None => {
                eprintln!("Missing response");
                exit(1)
            }
            _ => {
                eprintln!("Wrong response");
                exit(1)
            }
        }
        exit(0)
    }
    if let Some(matches) = matches.subcommand_matches("get") {
        let paths = matches.values_of("path").unwrap().map(PathBuf::from);

        for path in paths {
            eprintln!("Sending request");
            if let Err(e) = sock.send(
                bincode::serialize(&Request::Download {
                    path: path.to_str().unwrap().to_string(),
                })
                .unwrap()
                .as_slice(),
            ) {
                eprintln!("{}", e);
                exit(1)
            }
            eprintln!("Request sent");

            let mut buf = [0; proto::MAXIMUM_SIZE];
            let mut file_len = 0;
            let mut file_pos_recvd = false;
            let mut file = Cursor::new(vec![]);
            loop {
                if file_pos_recvd && file.position() == file_len {
                    break;
                }

                match sock.recv(&mut buf) {
                    Ok(len) => match bincode::deserialize(&buf[..len]) {
                        Ok(Response::Part { data, start_byte }) => {
                            eprintln!("Received {} bytes", data.len());
                            file.set_position(start_byte as u64);
                        }
                        Ok(Response::Summary(len)) => {
                            eprintln!("File is {} bytes long", len);
                            file_len = len as u64;
                            file_pos_recvd = true;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            stdout().write_all(file.get_ref().as_slice()).unwrap();
        }
    }
}

/// Send a packet at the given interval until a response is received.
fn send_recv_ad_nauseum(
    transport: &mut UdpSocket,
    msg: Request,
    duration: Duration,
) -> Option<Response> {
    let mut buf = [0; proto::MAXIMUM_SIZE];
    let mut poll = Poll::new().unwrap();
    poll.registry()
        .register(transport, UDP_SOCKET, Interest::READABLE);
    let mut events = Events::with_capacity(1);
    match transport.send(bincode::serialize(&msg).unwrap().as_slice()) {
        Ok(amt) => {
            eprintln!("Wrote {} bytes", amt);
        }
        Err(e) => {
            eprintln!("Failed sending request ({}). Trying again...", e);
        }
    }

    let mut buf = [0; proto::MAXIMUM_SIZE];
    loop {
        poll.poll(&mut events, Some(duration));
        for event in events.iter() {
            match event.token() {
                UDP_SOCKET => {
                    let amt = transport.recv(&mut buf).unwrap();
                    return Some(bincode::deserialize(&buf[..amt]).unwrap());
                }
                _ => {}
            }
        }
        sleep(duration);
    }
}

/// Prints a [Vec] of [FileData] nicely.
fn print_filedata(data: Vec<FileData>, csv: bool) {
    if !csv {
        let longest_name = data.iter().fold(20, |acc, data| acc.max(data.path.len()));
        println!(
            "{:<longest_name$} | {:<12} | {:<12}",
            "Path",
            "Created",
            "Size",
            longest_name = longest_name as usize,
        );
        println!("{}", "-".repeat(66));
        data.iter().for_each(|f| {
            println!(
                "{:<longest_name$} | {:<12} | {:<12}",
                f.path,
                f.created.as_millis(),
                f.size,
                longest_name = longest_name as usize,
            )
        });
    } else {
        println!("path,created,size");
        data.iter()
            .for_each(|f| println!("{},{},{}", f.path, f.created.as_millis(), f.size));
    }
}
