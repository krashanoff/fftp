//! ffd
//!
//! The Fast File Daemon.

use std::{path::PathBuf, process::exit};

use clap::{App, Arg};
use fork::{daemon, Fork};
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

    if matches.is_present("daemon") {
        if let Ok(Fork::Child) = daemon(false, false) {
            let tcp = TcpListener::bind(addr).await.unwrap();

            loop {
                if let Ok((stream, _)) = tcp.accept().await {
                    tokio::spawn(handle_conn(stream, directory_path.clone(), buffer_size));
                }
            }
        } else {
            eprintln!("Failed");
            exit(1)
        }
    } else {
        let tcp = TcpListener::bind(addr).await.unwrap();

        loop {
            if let Ok((stream, _)) = tcp.accept().await {
                tokio::spawn(handle_conn(stream, directory_path.clone(), buffer_size));
            }
        }
    }
}

async fn handle_conn(mut stream: TcpStream, mut base_path: PathBuf, buffer_size: usize) {
    match proto::Message::recv(&mut stream).await {
        Ok(proto::Message::List) => {
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

            if let Err(e) = proto::Message::Directory(dir_info).send(&mut stream).await {
                eprintln!("{}", e);
                exit(1)
            }
        }
        Ok(proto::Message::Download { path: dl_path }) => {
            base_path.push(dl_path);

            if !base_path.exists() {
                if let Err(e) = proto::Message::NotAllowed.send(&mut stream).await {
                    eprintln!("{}", e);
                    return;
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

            let mut part = 0u64;
            let mut buf = vec![0; buffer_size];

            while let Ok(size) = file.read(&mut buf).await {
                let end = size == 0;

                if let Err(e) = (proto::Message::Part {
                    num: part,
                    end: size == 0,
                    data: buf[..size].to_vec(),
                })
                .send(&mut stream)
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
        _ => {
            return;
        }
    }
}
