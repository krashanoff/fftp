//! ffd
//!
//! The Fast File client.

use clap::{App, Arg, SubCommand};
use tokio::{
    fs::{metadata, OpenOptions},
    io::AsyncWriteExt,
    net::TcpStream,
};

use std::process::exit;

mod proto;

#[tokio::main]
async fn main() {
    let matches = App::new("ff")
        .version("v0.1.0")
        .long_version("v0.1.0 ff@20")
        .args(&[Arg::with_name("addr")
            .required(true)
            .help("address to connect to")])
        .subcommand(SubCommand::with_name("ls"))
        .subcommand(
            SubCommand::with_name("get").args(&[
                Arg::with_name("path")
                    .required(true)
                    .help("path of the file to download"),
                Arg::with_name("force")
                    .short("f")
                    .long("force")
                    .takes_value(false)
                    .required(false)
                    .help("enable file overwrite"),
            ]),
        )
        .get_matches();

    let mut conn = TcpStream::connect(matches.value_of("addr").expect("an address is required"))
        .await
        .expect("failed connecting");

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
        let force = matches.is_present("force");
        let path = matches.value_of("path").unwrap().to_string();
        if let (Ok(_), false) = (metadata(&path).await, &force) {
            eprintln!("File exists");
            exit(1)
        }

        if let Err(e) = (proto::Message::Download { path: path.clone() })
            .send(&mut conn)
            .await
        {
            eprintln!("{}", e);
            exit(1);
        }

        let mut file = OpenOptions::new()
            .write(true)
            .read(false)
            .create(true)
            .open(path)
            .await
            .expect("failed to open file");

        while let Ok(proto::Message::Part {
            end: false, data, ..
        }) = proto::Message::recv(&mut conn).await
        {
            file.write_all(data.as_slice())
                .await
                .expect("failed to write data");
        }
    }
}
