//! ffd
//!
//! The Fast File client.

use clap::{App, Arg, SubCommand};
use mio::{net::UdpSocket, Events, Interest, Poll, Token};

use std::{
    fmt::Display,
    io::{stdout, Cursor, Write},
    path::PathBuf,
    process::exit,
    time::Duration,
};

mod proto;

use proto::{FileData, Request, Response};

const UDP_SOCKET: Token = Token(0);

fn die<T: Display>(msg: T) -> ! {
    eprintln!("{}", msg);
    exit(1)
}

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
    let mut sock = match UdpSocket::bind("0.0.0.0:0".parse().unwrap()) {
        Ok(t) => t,
        Err(e) => {
            die(e);
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

    let mut poll = match Poll::new() {
        Ok(p) => p,
        Err(e) => {
            die(e);
        }
    };
    poll.registry()
        .register(&mut sock, UDP_SOCKET, Interest::READABLE)
        .unwrap();
    let mut events = Events::with_capacity(5);

    if let Some(matches) = matches.subcommand_matches("ls") {
        match send_recv_ad_nauseum(
            &mut sock,
            &mut poll,
            &mut events,
            &Request::List {
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
                die("Missing response");
            }
            _ => {
                die("Wrong response");
            }
        }
        exit(0)
    }
    if let Some(matches) = matches.subcommand_matches("get") {
        let paths = matches.values_of("path").unwrap().map(PathBuf::from);
        get_files(
            &mut sock,
            &mut poll,
            &mut events,
            Duration::from_micros(50),
            paths,
        );
    }
}

/// Send a message.
fn send_msg(transport: &mut UdpSocket, msg: &Request) {
    transport
        .send(bincode::serialize(msg).unwrap().as_slice())
        .expect("Failed sending request");
}

/// Send a single packet at the given interval until a response is received.
fn send_recv_ad_nauseum(
    transport: &mut UdpSocket,
    poll: &mut Poll,
    events: &mut Events,
    msg: &Request,
    duration: Duration,
) -> Option<Response> {
    let mut buf = [0; proto::MAXIMUM_SIZE];
    loop {
        send_msg(transport, msg);
        poll.poll(events, Some(duration))
            .expect("Failed to poll socket");
        for event in events.iter() {
            if let UDP_SOCKET = event.token() {
                let amt = transport.recv(&mut buf).unwrap();
                return Some(bincode::deserialize(&buf[..amt]).unwrap());
            }
        }
    }
}

/// Receive multiple files.
fn get_files<T: IntoIterator<Item = PathBuf>>(
    transport: &mut UdpSocket,
    poll: &mut Poll,
    events: &mut Events,
    duration: Duration,
    paths: T,
) {
    for path in paths {
        if let Err(e) = transport.send(
            bincode::serialize(&Request::Download {
                path: path.to_str().unwrap().to_string(),
            })
            .unwrap()
            .as_slice(),
        ) {
            die(e);
        }

        let mut buf = [0; proto::MAXIMUM_SIZE];
        let mut file = Cursor::new(vec![]);
        let mut file_len = 0;
        let mut file_pos_recvd = false;
        let mut written = 0;
        loop {
            if file_pos_recvd && written == file_len {
                break;
            }

            match transport.recv(&mut buf) {
                Ok(len) => match bincode::deserialize(&buf[..len]) {
                    Ok(Response::Part { data, start_byte }) => {
                        file.set_position(start_byte as u64);
                        file.write_all(data.as_slice()).expect("failed to write");
                        written += data.len();
                    }
                    Ok(Response::Summary(len)) => {
                        eprintln!("File is {} bytes long", len);
                        file_len = len as usize;
                        file_pos_recvd = true;
                    }
                    Err(e) => {
                        die(e);
                    }
                    _ => {
                        eprintln!("Unexpected response");
                        break;
                    }
                },
                _ => {}
            }
        }
        let mut stdout = stdout();
        stdout
            .write_all(file.get_ref().as_slice())
            .expect("failed to write file");
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
