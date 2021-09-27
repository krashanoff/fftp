//! ffd
//!
//! The Fast File Daemon.

use std::{io::SeekFrom, net::SocketAddr, path::PathBuf, process::exit};

use clap::{App, Arg};
use fork::{daemon, Fork};
use proto::*;
use tokio::{
    fs::{read_dir, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt},
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
    let directory_path = PathBuf::from(matches.value_of("directory").unwrap())
        .canonicalize()
        .unwrap();

    if !directory_path.exists() || !directory_path.is_dir() {
        eprintln!("Path must be to a directory");
        exit(1)
    }

    let transport = proto::Transport::bind(8080).await;
    let (mut listener, _handle) = transport.start_server().await;

    if matches.is_present("daemon") {
        match daemon(false, false) {
            Ok(Fork::Parent(pid)) => {
                eprintln!("Spawned process {}", pid);
                exit(0)
            }
            Ok(Fork::Child) => loop {
                if let Some((req, src_addr)) = listener.recv().await {
                    handle_request(req, src_addr, &mut listener, directory_path.clone()).await;
                }
            },
            Err(e) => {
                eprintln!("{}", e);
                exit(1)
            }
        }
    }

    loop {
        if let Some((req, src_addr)) = listener.recv().await {
            println!("Received a request {:?} from {}", req, src_addr);
            handle_request(req, src_addr, &mut listener, directory_path.clone()).await;
        }
    }
}

async fn handle_request(
    req: Request,
    src_addr: SocketAddr,
    listener: &mut Listener,
    directory_path: PathBuf,
) {
    match req {
        proto::Request::List { path } => {
            let full_path = directory_path.clone().join(path);

            if !full_path
                .canonicalize()
                .unwrap()
                .starts_with(directory_path)
            {
                eprintln!("Invalid path");
                if let Err(e) = listener.send((Response::NotAllowed, src_addr)).await {
                    eprintln!("{}", e);
                }
                return;
            }

            if let Err(e) = listener
                .send((Response::Directory(dir_data(full_path).await), src_addr))
                .await
            {
                eprintln!("{}", e);
            }
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

            let mut byte_count = 0u32;
            let mut buf = vec![0; listener.preferred_chunk_size()];

            while let Ok(size) = file.read(&mut buf).await {
                let last = size == 0;

                if let Err(e) = listener
                    .send((
                        Response::Part {
                            start_byte: byte_count,
                            last,
                            data: buf[..size].to_vec(),
                        },
                        src_addr,
                    ))
                    .await
                {
                    eprintln!("{}", e);
                    return;
                }
                eprintln!("Sent {}", byte_count);

                byte_count += size as u32;

                if last {
                    break;
                }
            }
        }
        proto::Request::DownloadPart {
            path,
            start_byte,
            len,
        } => {
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

            file.seek(SeekFrom::Start(start_byte as u64)).await;

            let mut data = vec![0; len as usize];
            file.read_exact(&mut data).await;

            listener
                .send((
                    Response::Part {
                        start_byte,
                        data,
                        last: false,
                    },
                    src_addr,
                ))
                .await;
        }
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
