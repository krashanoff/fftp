//! ffd
//!
//! The Fast File Daemon.

use std::{net::SocketAddr, path::PathBuf, process::exit};

use clap::{App, Arg};
use fork::{daemon, Fork};
use proto::*;
use tokio::{
    fs::{read_dir, OpenOptions},
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
};

mod proto;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let matches = App::new("ffd")
        .version("v0.1.0")
        .long_version("v0.1.0 ff@20")
        .args(&[
            Arg::with_name("daemon")
                .short("d")
                .long("daemon")
                .takes_value(false)
                .help("detach the process from the terminal"),
            Arg::with_name("buffer-size")
                .short("b")
                .long("buffer-size")
                .default_value("4096")
                .help("sets the size of the buffer allocated to each file transfer transaction"),
            Arg::with_name("addr")
                .takes_value(true)
                .required(true)
                .help("address to listen for new connections on"),
            Arg::with_name("directory")
                .required(true)
                .help("directory to serve files from"),
        ])
        .get_matches();

    let addr = matches.value_of("addr").expect("address expected");
    let buffer_size: usize = matches
        .value_of("buffer-size")
        .expect("a buffer size is required")
        .parse()
        .expect("a valid buffer size is required");
    let directory_path: PathBuf = matches.value_of("directory").unwrap().into();

    if !directory_path.exists() || !directory_path.is_dir() {
        eprintln!("Path must be to a directory");
        exit(1)
    }

    let transport = proto::Transport::bind(8080).await;
    let (mut listener, handle) = transport.start_server().await;

    loop {
        if let Some((req, src_addr)) = listener.recv().await {
            println!("Got a request: {:?}", req);
            run_server(req, src_addr, &mut listener, directory_path.clone()).await;
        }
    }
}

async fn run_server(
    req: Request,
    src_addr: SocketAddr,
    listener: &mut Listener,
    directory_path: PathBuf,
) {
    match req {
        proto::Request::List => {
            println!("Servicing request for list");
            if let Err(e) = listener
                .send((
                    Response::Directory(dir_data(directory_path.clone()).await),
                    src_addr,
                ))
                .await
            {
                eprintln!("{}", e);
                exit(1)
            }
            println!("Done.");
        }
        proto::Request::Download { path } => {
            let mut base_path = directory_path.clone();
            base_path.push(path);

            if !base_path.exists() {
                if let Err(e) = listener.send((Response::NotAllowed, src_addr)).await {
                    eprintln!("{}", e);
                }
                return;
            }

            let mut file = match OpenOptions::new()
                .read(true)
                .write(false)
                .open(base_path)
                .await
            {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("{}", e);
                    return;
                }
            };

            let mut part = 0u32;
            let mut buf = vec![0; listener.preferred_chunk_size()];

            while let Ok(size) = file.read(&mut buf).await {
                let end = size == 0;

                if let Err(e) = listener
                    .send((
                        Response::Part {
                            num: part,
                            last: size == 0,
                            data: buf[..size].to_vec(),
                        },
                        src_addr,
                    ))
                    .await
                {
                    eprintln!("{}", e);
                    return;
                }

                part += 1;

                if end {
                    break;
                }
            }
        }
        proto::Request::DownloadPart { .. } => {}
    }
}

async fn dir_data(base_path: PathBuf) -> Vec<proto::FileData> {
    let mut dir = read_dir(&base_path).await.unwrap();
    let mut dir_info = vec![];
    while let Ok(Some(entry)) = dir.next_entry().await {
        let meta = entry.metadata().await.unwrap();
        dir_info.push(proto::FileData {
            path: entry
                .path()
                .strip_prefix(&base_path)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            created: meta.created().unwrap().elapsed().unwrap(),
            size: meta.len(),
        });
    }
    dir_info
}
