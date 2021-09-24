//! ffd
//!
//! The Fast File client.

use clap::{App, Arg, SubCommand};
use tokio::{
    io::{stdout, AsyncWriteExt},
    net::TcpStream,
};

use std::{path::PathBuf, process::exit};

mod proto;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let matches = App::new("ff")
        .version("v0.1.0")
        .long_version("v0.1.0 ff@20")
        .args(&[Arg::with_name("addr")
            .required(true)
            .help("address to connect to")])
        .subcommand(SubCommand::with_name("ls").about("List contents held remotely"))
        .subcommand(
            SubCommand::with_name("get")
                .args(&[Arg::with_name("path")
                    .required(true)
                    .help("Path of the file to download")])
                .about("Download a file"),
        )
        .get_matches();

    let mut conn =
        match TcpStream::connect(matches.value_of("addr").expect("an address is required")).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to connect: {}", e);
                exit(1)
            }
        };

    if let Some(_) = matches.subcommand_matches("ls") {
        if let Err(e) = proto::Message::List.send(&mut conn).await {
            eprintln!("{}", e);
            exit(1)
        }

        match proto::Message::recv(&mut conn).await {
            Ok(proto::Message::Directory(files)) => {
                println!("{:<20} | {:<20} | {:<20}", "Path", "Created", "Size");
                println!("{}", "-".repeat(66));
                files.iter().for_each(|f| {
                    println!(
                        "{:<20} | {:<20} | {:<20}",
                        f.path,
                        f.created.as_millis(),
                        f.size
                    )
                });
            }
            _ => exit(1),
        }
    }
    if let Some(matches) = matches.subcommand_matches("get") {
        let path = PathBuf::from(matches.value_of("path").unwrap());

        if let Err(e) = (proto::Message::Download {
            path: path.to_str().unwrap().to_string(),
        })
        .send(&mut conn)
        .await
        {
            eprintln!("{}", e);
            exit(1);
        }

        let mut stdout = stdout();
        loop {
            match proto::Message::recv(&mut conn).await {
                Ok(proto::Message::Part { end, data, .. }) => {
                    if let Err(e) = stdout.write_all(data.as_slice()).await {
                        eprintln!("Failed to write to stdout: {}", e);
                        exit(1)
                    }
                    if end {
                        exit(0)
                    }
                }
                Ok(proto::Message::NotAllowed) => {
                    eprintln!("Operation not allowed");
                    exit(1)
                }
                Err(proto::Error::IO(e)) => {
                    eprintln!("Encountered IO error: {}", e);
                    exit(1)
                }
                Err(proto::Error::Serialization(e)) => {
                    eprintln!("Encountered serialization error: {}", e);
                    exit(1)
                }
                _ => {}
            }
        }
    }
}
