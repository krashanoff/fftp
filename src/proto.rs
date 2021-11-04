//! Message types used in communication between the FF client and server.

use std::{fmt::Display, net::SocketAddr, time};

use bincode::{self, Options};
use igd::aio;
use serde::{Deserialize, Serialize};
use tokio::{
    io,
    net::{ToSocketAddrs, UdpSocket},
    sync::mpsc,
    task::JoinHandle,
};

/// Suggested size for data field of FFTP frame.
pub const DATA_SIZE: usize = 4096;

/// Maximum size of a single transport frame.
const MAXIMUM_SIZE: usize = 65535;

mod encoding {
    use bincode::{
        self,
        config::{BigEndian, Bounded, WithOtherEndian, WithOtherLimit},
        DefaultOptions, Options,
    };
    use lazy_static::lazy_static;

    lazy_static! {
        pub static ref BINCODE_OPTS: WithOtherLimit<WithOtherEndian<DefaultOptions, BigEndian>, Bounded> =
            bincode::DefaultOptions::new()
                .with_big_endian()
                .with_limit(super::MAXIMUM_SIZE as u64);
    }
}

#[derive(Debug)]
/// Types of communication errors that can occur.
pub enum Error {
    IO(io::Error),
    MPSC(mpsc::error::SendError<Response>),
    Serialization(bincode::Error),
    V6NotSupported,
    ConnectionTimeout,
    ImpossibleDataLen(u32),
    UnexpectedType,
    WrongChecksum,
}

/// Transport wrapper for FF servers and clients.
pub struct Transport {
    /// Communication socket.
    sock: UdpSocket,
}

/// Client for making [Requests](Request) and receiving [Responses](Response).
pub struct Client {
    receiver: mpsc::Receiver<Response>,
    sender: mpsc::Sender<Request>,
}

/// Server for receiving [Requests](Request) and sending [Responses](Response).
pub struct Listener {
    receiver: mpsc::Receiver<(Request, SocketAddr)>,
    sender: mpsc::Sender<(Response, SocketAddr)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u8)]
/// Types of requests that may be sent from a client.
pub enum Request {
    /// List files available for download.
    List { path: String },

    /// Download a file.
    Download { path: String },

    /// Download a *part* of a file.
    DownloadPart {
        /// Path of the file.
        path: String,

        /// The byte to start at.
        start_byte: u32,

        /// The length of the data we are missing.
        len: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u8)]
/// Types of responses that may be sent by a server.
pub enum Response {
    /// Directory listing.
    Directory(Vec<FileData>),

    /// Part of a file.
    Part {
        start_byte: u32,
        /// Is this the last chunk of bytes?
        last: bool,
        data: Vec<u8>,
    },

    /// Operation is not allowed.
    NotAllowed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Simple representation of a file on the server.
pub struct FileData {
    /// Path of the file on the server.
    pub path: String,

    /// When was the file created?
    pub created: time::Duration,

    /// How large is the file?
    pub size: u64,
}

impl Listener {
    /// Receive a [Request].
    pub async fn recv(&mut self) -> Option<(Request, SocketAddr)> {
        self.receiver.recv().await
    }

    /// Queue a [Response] to send.
    pub async fn send(&self, r: (Response, SocketAddr)) -> Result<(), Error> {
        Ok(self.sender.send(r).await.unwrap())
    }
}

impl Client {
    pub async fn recv(&mut self) -> Option<Response> {
        self.receiver.recv().await
    }

    pub async fn send(&self, r: Request) -> Result<(), Error> {
        Ok(self.sender.send(r).await.unwrap())
    }
}

impl Transport {
    /// Bind to some port, forwarding with uPNP if requested.
    async fn bind_to(port: u16, forward: bool) -> Result<Self, Error> {
        let local_addr = SocketAddr::new("0.0.0.0".parse().unwrap(), port);

        let sock = UdpSocket::bind(local_addr).await.unwrap();
        let local_addr = match sock.local_addr().unwrap() {
            SocketAddr::V4(addr) => addr,
            SocketAddr::V6(_) => {
                return Err(Error::V6NotSupported);
            }
        };

        if forward {
            let re = aio::search_gateway(Default::default()).await.unwrap();
            re.get_any_address(igd::PortMappingProtocol::UDP, local_addr, 0, "ff")
                .await
                .expect("failed to acquire forwarded port from gateway");
        }

        Ok(Self { sock })
    }

    /// Bind to an external port.
    pub async fn bind_ext(port: u16) -> Result<Self, Error> {
        Self::bind_to(port, true).await
    }

    /// Bind to a port, but do **not** attempt to forward with uPNP.
    pub async fn bind(port: u16) -> Result<Self, Error> {
        Self::bind_to(port, false).await
    }

    /// Spin up the [Transport] to handle queueing of requests and responses.
    pub async fn start_server(self) -> (Listener, JoinHandle<()>) {
        let (resp_tx, mut resp_rx) = mpsc::channel(50);
        let (req_tx, req_rx) = mpsc::channel(50);
        (
            Listener {
                receiver: req_rx,
                sender: resp_tx,
            },
            tokio::spawn(async move {
                let mut buf = [0; MAXIMUM_SIZE];
                loop {
                    tokio::select! {
                        Ok((len, src_addr)) = self.sock.recv_from(&mut buf) => {
                            req_tx.send((encoding::BINCODE_OPTS.deserialize(&buf[..len]).unwrap(), src_addr)).await.expect("channel closed");
                        }
                        Some((resp, src_addr)) = resp_rx.recv() => {
                            self.sock.send_to(encoding::BINCODE_OPTS.serialize(&resp).unwrap().as_slice(), src_addr).await.expect("channel closed");
                        }
                    }
                }
            }),
        )
    }

    /// Spin up the [Transport] to handle queueing of requests and responses to the given
    /// address.
    pub async fn start_client<A: ToSocketAddrs>(
        self,
        addr: A,
    ) -> Result<(Client, JoinHandle<Result<(), Error>>), Error> {
        self.sock.connect(&addr).await?;
        let (resp_tx, resp_rx) = mpsc::channel(50);
        let (req_tx, mut req_rx) = mpsc::channel(50);
        Ok((
            Client {
                receiver: resp_rx,
                sender: req_tx,
            },
            tokio::spawn(async move {
                let mut buf = vec![0; MAXIMUM_SIZE];
                loop {
                    tokio::select! {
                        Ok(count) = self.sock.recv(&mut buf) => {
                            eprintln!("Received {} bytes", &count);
                            resp_tx.send(encoding::BINCODE_OPTS.deserialize(&buf[..count]).expect("failed to deserialize")).await.expect("channel closed");
                        }
                        Some(req) = req_rx.recv() => {
                            self.sock.send(encoding::BINCODE_OPTS.serialize(&req).unwrap().as_slice()).await?;
                        }
                    }
                }
            }),
        ))
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO(e) => e.fmt(f),
            Self::Serialization(e) => e.fmt(f),
            Self::MPSC(e) => e.fmt(f),
            Self::V6NotSupported => write!(f, "IPv6 is not supported yet"),
            Self::ConnectionTimeout => write!(f, "timed out connecting"),
            Self::ImpossibleDataLen(len) => write!(f, "data length '{}' is impossible", len),
            Self::UnexpectedType => write!(f, "expected request/response or vice versa"),
            Self::WrongChecksum => write!(f, "wrong checksum"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::IO(e)
    }
}

impl From<bincode::Error> for Error {
    fn from(b: bincode::Error) -> Self {
        Self::Serialization(b)
    }
}

impl From<mpsc::error::SendError<Response>> for Error {
    fn from(e: mpsc::error::SendError<Response>) -> Self {
        Self::MPSC(e)
    }
}
