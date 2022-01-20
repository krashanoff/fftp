//! ffd
//!
//! The Fast File Daemon.

use std::{
    collections::HashMap,
    fs::{read_dir, File, OpenOptions},
    io::{Read, Seek},
    net::SocketAddr,
    os::unix::prelude::FileExt,
    path::PathBuf,
    process::exit,
    time::Duration,
};

use clap::Parser;
use mio::{net::UdpSocket, Events, Interest, Poll, Token};

use fftp::{FileData, Request, Response};

const UDP_SOCKET: Token = Token(0);

#[derive(Debug, Parser)]
#[clap(name = "ffd", version, long_version = "ff@20", author)]
struct Args {
    /// Handles for open files in the program
    #[clap(skip)]
    file_handles: HashMap<PathBuf, File>,

    /// Sets the write buffer size for each worker thread in bytes
    #[clap(value_name = "BYTES", short, long, default_value_t = 2048)]
    buffer_size: usize,

    /// Size of the worker thread pool
    #[clap(value_name = "N", short = 'j', long, default_value_t = 4)]
    threads: usize,

    /// Address to serve on
    #[clap()]
    addr: SocketAddr,

    /// Path to the directory to make accessible on the given address
    #[clap(value_name = "PATH")]
    directory: PathBuf,
}

impl Args {
    /// ffd operates optimistically. If a [File] is already open, we just use its handle.
    /// If we haven't touched a file in a sufficiently long period of time, we evict it.
    pub fn get_file<'a>(&'a mut self, path: &String) -> Option<&'a mut File> {
        let path = self.directory.clone().join(path).canonicalize().unwrap();
        eprintln!("Browsing file {}", path.to_str().unwrap());
        if !path.starts_with(self.directory.clone()) {
            eprintln!(
                "Invalid path: {} ---- {}",
                path.to_str().unwrap(),
                self.directory.clone().to_str().unwrap()
            );
            return None;
        }

        if !self.file_handles.contains_key(&path) {
            let handle = OpenOptions::new()
                .read(true)
                .write(false)
                .open(&path)
                .expect("failed to open");
            self.file_handles.insert(path.clone(), handle);
            eprintln!("Inserted");
        }
        self.file_handles.get_mut(&path)
    }
}

fn main() {
    let mut args = Args::parse();
    args.directory = PathBuf::from(&args.directory).canonicalize().unwrap();
    if !args.directory.exists() || !args.directory.is_dir() {
        eprintln!("Path must be to a directory");
        exit(1)
    }
    eprintln!("Using directory {}", &args.directory.to_str().unwrap());

    let mut poll = Poll::new().expect("failed to create poller");
    let mut events = Events::with_capacity(1024);
    let mut socket = UdpSocket::bind(
        format!("0.0.0.0:{}", 8080)
            .parse()
            .expect("valid port number is required"),
    )
    .expect("socket");
    poll.registry()
        .register(&mut socket, UDP_SOCKET, Interest::READABLE)
        .expect("failed to register socket");

    let mut buf = vec![0; args.buffer_size];
    loop {
        poll.poll(&mut events, None).unwrap();

        for event in events.iter() {
            match event.token() {
                UDP_SOCKET => loop {
                    match socket.recv_from(&mut buf) {
                        Ok((len, src_addr)) => {
                            handle_dgram(&mut args, &mut socket, &buf[..len], src_addr)
                        }
                        Err(_) => {
                            break;
                        }
                    }
                },
                _ => {}
            }
        }
    }
}

fn handle_dgram(args: &mut Args, socket: &mut UdpSocket, data: &[u8], src_addr: SocketAddr) {
    eprintln!("{} request", src_addr);

    let mut buf = vec![0; args.buffer_size];
    match bincode::deserialize::<Request>(data) {
        Ok(Request::Download { path }) => {
            let mut pos = 0;
            let handle = args.get_file(&path).unwrap();
            handle.rewind().expect("failed to rewind file cursor");
            while let Ok(amt) = handle.read(&mut buf) {
                eprintln!("Read {} bytes", amt);
                if amt == 0 {
                    socket
                        .send_to(
                            bincode::serialize(&Response::Summary(pos))
                                .unwrap()
                                .as_slice(),
                            src_addr,
                        )
                        .unwrap();
                    eprintln!("Done sending bytes");
                    break;
                }
                socket
                    .send_to(
                        bincode::serialize(&Response::Part {
                            start_byte: pos,
                            data: buf[..amt].to_vec(),
                        })
                        .unwrap()
                        .as_slice(),
                        src_addr,
                    )
                    .unwrap();
                pos += amt as u32;
            }
        }
        Ok(Request::DownloadPart {
            start_byte: mut pos,
            path,
            ..
        }) => {
            let handle = args.get_file(&path).unwrap();
            while let Ok(amt) = handle.read_at(&mut buf, pos as u64) {
                socket
                    .send_to(
                        bincode::serialize(&Response::Part {
                            start_byte: pos,
                            data: buf[..amt].to_vec(),
                        })
                        .unwrap()
                        .as_slice(),
                        src_addr,
                    )
                    .unwrap();
                pos += amt as u32;
            }
        }
        Ok(Request::List { path, .. }) => {
            let mut data = vec![];
            for entry in read_dir(args.directory.join(path).clone()).unwrap() {
                let entry = entry.unwrap();
                let metadata = entry.metadata().unwrap();
                data.push(FileData {
                    path: entry.file_name().into_string().unwrap(),
                    created: Duration::from_millis(2),
                    size: metadata.len(),
                });
            }
            todo!()
        }
        Err(e) => {
            eprintln!("{}", e);
        }
    }
}
