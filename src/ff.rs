//! ffd
//!
//! The Fast File client.

use clap::{App, Arg, SubCommand};
use tokio::{
    io::{stdout, AsyncWriteExt},
    time::{timeout, Duration},
};

use std::{io::Cursor, path::PathBuf, process::exit};

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
        .subcommand(
            SubCommand::with_name("ls")
                .about("List contents held remotely")
                .args(&[Arg::with_name("path")
                    .default_value(".")
                    .help("Path of the directory to list.")]),
        )
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

    if let Some(matches) = matches.subcommand_matches("ls") {
        match send_recv_ad_nauseum(
            &mut client,
            Request::List {
                path: matches.value_of("path").unwrap().to_string(),
            },
            Duration::from_secs(3),
        )
        .await
        {
            Some(Response::Directory(files)) => {
                println!("{:<20} | {:<20} | {:<20}", "Path", "Created", "Size");
                println!("{}", "-".repeat(66));
                files.iter().for_each(|f| {
                    println!(
                        "{:<20.20} | {:<20.20} | {:<20.20}",
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

            let mut file = Cursor::new(vec![]);
            while let Some(Response::Part {
                last: false,
                data,
                start_byte,
            }) = client.recv().await
            {
                file.set_position(start_byte as u64);
                file.write_all(data.as_slice()).await;
            }
            stdout.write_all(file.get_ref().as_slice()).await.unwrap();
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
