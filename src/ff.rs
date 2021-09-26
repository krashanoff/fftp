//! ffd
//!
//! The Fast File client.

use clap::{App, Arg, SubCommand};
use tokio::{
    io::{stdout, AsyncWriteExt},
    time::{timeout, Duration},
};

use std::{path::PathBuf, process::exit};

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
                    .multiple(true)
                    .help("Path(s) of the file(s) to download")])
                .about("Download a file"),
        )
        .get_matches();

    // Create our transport.
    let transport = Transport::bind(0).await;
    let (mut client, _handle) = transport
        .start_client(matches.value_of("addr").unwrap())
        .await;

    if let Some(_) = matches.subcommand_matches("ls") {
        match send_recv_ad_nauseum(&mut client, Request::List, Duration::from_secs(3)).await {
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
        let mut stdout = stdout();

        for path in paths {
            if let Err(e) = client
                .send(Request::Download {
                    path: path.to_str().unwrap().to_string(),
                })
                .await
            {
                eprintln!("{}", e);
                exit(1)
            }

            let mut file = vec![];
            while let Some(Response::Part {
                last: false,
                data,
                start_byte,
            }) = client.recv().await
            {
                eprintln!("Received {} bytes starting at {}", &data.len(), &start_byte);
                data.iter().cloned().fold(0, |acc, byte| {
                    file.insert(start_byte as usize + acc, byte);
                    acc + 1
                });
            }
            stdout.write_all(file.as_slice()).await.unwrap();
        }
    }
}

/// Send a packet at the given interval until a response is received.
async fn send_recv_ad_nauseum(
    client: &mut Client,
    msg: Request,
    duration: Duration,
) -> Option<Response> {
    loop {
        client.send(msg.clone()).await;
        match timeout(duration, client.recv()).await {
            Ok(resp) => {
                return resp;
            }
            Err(e) => {
                eprintln!("Timed out waiting for response ({}). Trying again...", e);
            }
        }
    }
}
