//! ffd
//!
//! The Fast File client.

use clap::Parser;
use mio::{net::UdpSocket, Events, Interest, Poll, Token};

use std::{
    fmt::Display,
    io::{stdout, Cursor, Write},
    net::SocketAddr,
    path::PathBuf,
    process::exit,
    time::Duration,
};

use fftp::files::{FileData, Request, Response};

const UDP_SOCKET: Token = Token(0);

/// Client for the Fast File Transport Protocol.
#[derive(Debug, Parser)]
#[clap(name = "ff", author, version, long_version = "ff@20")]
struct Args {
    /// How to handle directories or files encountered
    #[clap(value_name = "MODE", possible_values = ["ls", "get"])]
    mode: String,

    /// Address of the computer you're trying to reach
    #[clap(value_name = "ADDR")]
    addr: SocketAddr,

    /// Path of the directory or file to retrieve
    #[clap(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

/// Kill the program with an error message.
fn die<T: Display>(msg: T) -> ! {
    eprintln!("{}", msg);
    exit(1)
}

fn main() {
    let args = Args::parse();

    let mut sock = UdpSocket::bind("0.0.0.0:0".parse().unwrap()).unwrap_or_else(|e| die(e));
    sock.connect(args.addr).expect("valid addr required");

    match args.mode.as_str() {
        "ls" => {
            match send_recv_ad_nauseum(
                &mut sock,
                &Request::List {
                    path: args.paths.first().unwrap().display().to_string(),
                    recursive: false,
                },
                Duration::from_secs(3),
            ) {
                Some(Response::Directory(files)) => {
                    print_filedata(files, false);
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
        "get" => {
            get_files(
                &mut sock,
                Duration::from_micros(50),
                args.paths,
            );
        }
        _ => {
            eprintln!("How the hell");
        }
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
    msg: &Request,
    duration: Duration,
) -> Option<Response> {
    let mut buf = [0; fftp::MAXIMUM_SIZE];
    loop {
        if let Ok((amt, src_addr)) = transport.recv_from(&mut buf) {
            return Some(bincode::deserialize(&buf[..amt]).unwrap());
        }
    }
}

/// Receive multiple files.
fn get_files<T: IntoIterator<Item = PathBuf>>(
    transport: &mut UdpSocket,
    _duration: Duration,
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

        let mut buf = [0; fftp::MAXIMUM_SIZE];
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
