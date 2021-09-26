//! ffd
//!
//! The Fast File client.

use clap::{App, Arg, SubCommand};
use igd::aio;
use tokio::{
    io::{stdout, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

use std::{path::PathBuf, process::exit, time::Duration};

mod proto;

use proto::*;

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

    // Create our transport.
    let transport = Transport::bind(0).await;
    let (mut client, handle) = transport
        .start_client(matches.value_of("addr").unwrap())
        .await;

    if let Some(_) = matches.subcommand_matches("ls") {
        client.send(Request::List).await;
        println!("Sent list request");

        match client.recv().await {
            Some(Response::Directory(files)) => {
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
                exit(0)
            }
            Some(Response::NotAllowed) => {
                eprintln!("Not allowed");
            }
            _ => {}
        }
        exit(1)
    }
    if let Some(matches) = matches.subcommand_matches("get") {
        let path = PathBuf::from(matches.value_of("path").unwrap());

        if let Err(e) = client
            .send(Request::Download {
                path: path.to_str().unwrap().to_string(),
            })
            .await
        {
            eprintln!("{}", e);
            exit(1)
        }

        let mut stdout = stdout();
        let mut pieces = vec![];
        loop {
            while let Some(Response::Part { last, data, num }) = client.recv().await {
                pieces.push(Response::Part { last, data, num });
            }
        }
    }
}
