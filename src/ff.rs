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
        .args(&[
            Arg::with_name("ext")
                .short("e")
                .long("bind-ext")
                .takes_value(false)
                .help("Connect from an external IP using UPnP on supported gateways."),
            Arg::with_name("addr")
                .required(true)
                .help("address to connect to"),
        ])
        .subcommand(
            SubCommand::with_name("ls")
                .about("List contents held remotely")
                .args(&[
                    Arg::with_name("path")
                        .default_value(".")
                        .help("Path of the directory to list."),
                    Arg::with_name("csv")
                        .short("c")
                        .takes_value(false)
                        .help("Print directory information as a CSV."),
                ]),
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
    let transport = match matches.is_present("ext") {
        true => Transport::bind_ext(0)
            .await
            .expect("failed to bind to external port"),
        false => Transport::bind(0)
            .await
            .expect("failed to bind to internal port"),
    };

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
                if let Err(e) = file.write_all(data.as_slice()).await {
                    eprintln!("Failed to make a write to disk: {}", e);
                    exit(1)
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
