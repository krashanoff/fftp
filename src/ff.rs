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
    let transport = Transport::bind(0)
        .await
        .expect("failed to bind to internal port");

    let (mut client, _handle) = match transport
        .start_client(matches.value_of("addr").unwrap())
        .await
    {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };

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
                print_filedata(files, matches.is_present("csv"));
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
            eprintln!("Sending request");
            if let Err(e) = client
                .send(Request::Download {
                    path: path.to_str().unwrap().to_string(),
                })
                .await
            {
                eprintln!("{}", e);
                exit(1)
            }
            eprintln!("Request sent");

            let mut file_len = 0;
            let mut file_pos_recvd = false;
            let mut file = Cursor::new(vec![]);
            loop {
                if file_pos_recvd && file.position() == file_len {
                    break;
                }

                match client.recv().await {
                    Some(Response::Part { data, start_byte }) => {
                        eprintln!("Received {} bytes", data.len());
                        file.set_position(start_byte as u64);
                        if let Err(e) = file.write_all(data.as_slice()).await {
                            eprintln!("Failed to make a write to disk: {}", e);
                            exit(1)
                        }
                    }
                    Some(Response::Summary(len)) => {
                        eprintln!("File is {} bytes long", len);
                        file_len = len as u64;
                        file_pos_recvd = true;
                    }
                    None => break,
                    _ => {}
                }
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
        client.send(msg.clone()).await.expect("channel closed");
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

/// Prints a [Vec<FileData>] nicely.
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
