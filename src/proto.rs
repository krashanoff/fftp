//! Message types used in communication between the FF client and server.

use std::{
    collections::HashMap,
    convert::TryInto,
    fmt::Display,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    time,
};

use byteorder::{BigEndian, ReadBytesExt};
use either::Either;
use igd::{aio, SearchOptions};
use ring::digest::{self, digest, SHA1_OUTPUT_LEN};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpSocket, ToSocketAddrs, UdpSocket},
    sync::mpsc,
    task::JoinHandle,
};

#[derive(Debug)]
/// Types of communication errors that can occur.
pub enum Error {
    IO(io::Error),
    MPSC(mpsc::error::SendError<Response>),
    Serialization(bincode::Error),
    WrongChecksum,
}

pub struct Transport {
    /// Communication socket.
    sock: UdpSocket,

    /// Size used for handling file sends.
    preferred_chunk_size: usize,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
struct Frame {
    len: u32,
    data: Vec<u8>,
    checksum: [u8; SHA1_OUTPUT_LEN],
}

impl Frame {
    /// Recalculates the checksum of our frame.
    pub fn valid(&self) -> bool {
        let mut frame_copy = self.clone();
        frame_copy.checksum = [0; digest::SHA1_OUTPUT_LEN];
        digest(&digest::SHA1_FOR_LEGACY_USE_ONLY, self.data.as_slice()).as_ref()
            == frame_copy.checksum
    }

    /// Outputs the data of this struct as a [Vec].
    pub fn to_vec(&self) -> Vec<u8> {
        let mut buf = vec![];
        buf.extend(&self.len.to_be_bytes()[..]);
        buf.extend(self.data.as_slice());
        buf.extend(self.checksum.as_ref());
        buf
    }
}

impl From<Either<Request, Response>> for Frame {
    // Build our frame.
    fn from(r: Either<Request, Response>) -> Self {
        let data = bincode::serialize(&r).unwrap();
        let mut s: Frame = Default::default();
        s.len = data.len() as u32;
        s.data = data;
        s.checksum = digest::digest(&digest::SHA1_FOR_LEGACY_USE_ONLY, s.to_vec().as_slice())
            .as_ref()
            .try_into()
            .unwrap();
        s
    }
}

impl TryInto<Either<Request, Response>> for Frame {
    type Error = Error;
    fn try_into(self) -> Result<Either<Request, Response>, Self::Error> {
        if !self.valid() {
            return Err(Error::WrongChecksum);
        }
        Ok(bincode::deserialize(self.data.as_slice())?)
    }
}

pub struct Client {
    receiver: mpsc::Receiver<Response>,
    sender: mpsc::Sender<Request>,
}

/// Server.
pub struct Listener {
    receiver: mpsc::Receiver<Request>,
    sender: mpsc::Sender<Response>,
    chunk_size: usize,
}

impl Listener {
    /// Receive a [Request].
    pub async fn recv(&mut self) -> Option<Request> {
        self.receiver.recv().await
    }

    /// Queue a [Response] to send.
    pub async fn send(&self, r: Response) -> Result<(), Error> {
        Ok(self.sender.send(r).await?)
    }

    /// Get the preferred chunk size of transfer.
    pub fn preferred_chunk_size(&self) -> usize {
        self.chunk_size
    }
}

impl Transport {
    async fn bind_to(port: u16, forward: bool) -> Self {
        let local_addr = SocketAddrV4::new("0.0.0.0".parse().unwrap(), port);

        let external_addr = {
            if forward {
                let re = aio::search_gateway(Default::default()).await.unwrap();
                Some(
                    re.get_any_address(igd::PortMappingProtocol::UDP, local_addr, 0, "ff")
                        .await
                        .expect("failed to acquire forwarded port from gateway"),
                )
            } else {
                None
            }
        };

        let sock = UdpSocket::bind(&match external_addr {
            Some(a) => a,
            None => local_addr,
        })
        .await
        .unwrap();

        Self {
            sock,
            preferred_chunk_size: 0,
        }
    }

    /// Bind to an external port.
    pub async fn bind_ext(port: u16) -> Self {
        Self::bind_to(port, true).await
    }

    /// Bind to a port, but do **not** attempt to forward with uPNP.
    pub async fn bind(port: u16) -> Self {
        Self::bind_to(port, false).await
    }

    /// Set the preferred chunk size.
    pub fn chunk_size(mut self, size: usize) -> Self {
        self.preferred_chunk_size = size;
        self
    }

    /// Spin up the [Transport] to handle queueing of requests and responses.
    pub async fn start_server(self) -> (Listener, JoinHandle<()>) {
        let (resp_tx, mut resp_rx) = mpsc::channel(50);
        let (req_tx, req_rx) = mpsc::channel(50);
        (
            Listener {
                receiver: req_rx,
                sender: resp_tx,
                chunk_size: self.preferred_chunk_size,
            },
            tokio::spawn(async move {
                loop {
                    match resp_rx.recv().await {
                        _ => {}
                    }
                }
            }),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u8)]
pub enum Request {
    /// Get a file.
    List,

    /// Download a file.
    Download { path: String },

    /// Download a *part* of a file.
    DownloadPart { path: String, part: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u8)]
pub enum Response {
    /// Directory listing.
    Directory(Vec<FileData>),

    /// Part of a file.
    Part { num: u32, last: bool, data: Vec<u8> },

    /// Operation is not allowed.
    NotAllowed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u8)]
/// Message types exchanged between clients and servers.
pub enum Message {
    /// List what's available.
    List,

    /// Files found.
    Directory(Vec<FileData>),

    /// Download a file.
    Download { path: String },

    /// Writing a file part.
    Part { num: u64, end: bool, data: Vec<u8> },

    /// Operation is not permitted.
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

impl Message {
    pub async fn send<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<(), Error> {
        let buf = bincode::serialize(self).unwrap();
        writer.write_u32(buf.len() as u32).await?;
        writer.write_all(buf.as_slice()).await?;
        Ok(())
    }

    pub async fn recv<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Self, Error> {
        let len = reader.read_u32().await?;
        let mut buf = vec![0; len as usize];
        reader.read_exact(&mut buf).await?;
        Ok(bincode::deserialize(buf.as_slice())?)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO(e) => e.fmt(f),
            Self::Serialization(e) => e.fmt(f),
            Self::MPSC(e) => e.fmt(f),
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
