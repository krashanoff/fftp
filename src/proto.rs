//! Message types used in communication between the FF client and server.

use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    fmt::Display,
    io::Read,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    time,
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use either::Either;
use igd::{aio, SearchOptions};
use ring::digest::{self, digest, SHA1_OUTPUT_LEN};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{self},
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
    ImpossibleDataLen(u32),
    UnexpectedType,
    WrongChecksum,
}

/// Transport wrapper for FF servers and clients.
pub struct Transport {
    /// Communication socket.
    sock: UdpSocket,

    /// Size used for handling file sends.
    preferred_chunk_size: usize,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
/// A [Frame] for transmitting content over UDP. The data field may not exceed
/// `MAXIMUM_DATA_SIZE` bytes.
struct Frame {
    /// Length of our data field.
    len: u32,

    /// Our data field.
    data: Vec<u8>,

    /// Our checksum.
    checksum: [u8; SHA1_OUTPUT_LEN],
}

/// Client for making [Requests](Request) and receiving [Responses](Response).
pub struct Client {
    receiver: mpsc::Receiver<Response>,
    sender: mpsc::Sender<Request>,
}

/// Server for receiving [Requests](Request) and sending [Responses](Response).
pub struct Listener {
    receiver: mpsc::Receiver<Request>,
    sender: mpsc::Sender<Response>,
    chunk_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u8)]
/// Types of requests that may be sent from a client.
pub enum Request {
    /// List files available for download.
    List,

    /// Download a file.
    Download { path: String },

    /// Download a *part* of a file.
    DownloadPart { path: String, part: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u8)]
/// Types of responses that may be sent by a server.
pub enum Response {
    /// Directory listing.
    Directory(Vec<FileData>),

    /// Part of a file.
    Part { num: u32, last: bool, data: Vec<u8> },

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

impl Frame {
    /// Maximum amount of data transmittable in a single [Frame].
    pub const MAXIMUM_DATA_SIZE: usize = 65535 - 32 - SHA1_OUTPUT_LEN;

    /// Maximum size of a single [Frame].
    pub const MAXIMUM_SIZE: usize = 65535;

    /// Recalculates the checksum of our [Frame].
    fn compute_checksum(&self) -> digest::Digest {
        let mut frame_copy = self.clone();
        frame_copy.checksum = [0; digest::SHA1_OUTPUT_LEN];
        digest(&digest::SHA1_FOR_LEGACY_USE_ONLY, self.data.as_slice())
    }

    /// Recalculates the checksum of our [Frame] and compares it to the current value.
    pub fn valid(&self) -> bool {
        self.compute_checksum().as_ref() == self.checksum
    }

    /// Outputs the data of this struct as a [Vec] transmittable on the wire.
    pub fn to_vec(&self) -> Vec<u8> {
        let mut buf = vec![];
        buf.write_u32::<BigEndian>(self.len).unwrap();
        self.data.iter().for_each(|&b| buf.write_u8(b).unwrap());
        self.checksum.iter().for_each(|&b| buf.write_u8(b).unwrap());
        buf
    }
}

impl TryFrom<&[u8]> for Frame {
    type Error = Error;

    // Deserialize a [Frame] from some bytes.
    fn try_from(mut value: &[u8]) -> Result<Self, Self::Error> {
        let len = value.read_u32::<BigEndian>()?;

        if len > Self::MAXIMUM_DATA_SIZE as u32 {
            return Err(Error::ImpossibleDataLen(len));
        }

        let mut data = vec![0; len as usize];
        value.read_exact(&mut data)?;

        let mut checksum = [0; digest::SHA1_OUTPUT_LEN];
        value.read_exact(&mut checksum)?;

        let current = Self {
            len,
            data,
            checksum,
        };

        if !current.valid() {
            return Err(Error::WrongChecksum);
        }

        Ok(current)
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

impl From<Request> for Frame {
    fn from(r: Request) -> Self {
        Self::from(Either::Left(r))
    }
}

impl From<Response> for Frame {
    fn from(r: Response) -> Self {
        Self::from(Either::Right(r))
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

impl TryInto<Request> for Frame {
    type Error = Error;
    fn try_into(self) -> Result<Request, Self::Error> {
        match self.try_into()? {
            Either::Left(r) => Ok(r),
            _ => Err(Error::UnexpectedType),
        }
    }
}

impl TryInto<Response> for Frame {
    type Error = Error;
    fn try_into(self) -> Result<Response, Self::Error> {
        match self.try_into()? {
            Either::Right(r) => Ok(r),
            _ => Err(Error::UnexpectedType),
        }
    }
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
                    // Handle frame acquisition.
                    let mut buf = [0; Frame::MAXIMUM_DATA_SIZE];
                    let (amt_read, src_addr) = self.sock.recv_from(&mut buf).await.unwrap();
                    req_tx
                        .send(Frame::try_from(&buf[..]).unwrap().try_into().unwrap())
                        .await;

                        // Handle sending of responses.
                    match resp_rx.recv().await {
                        Some(resp) => {
                            self.sock
                                .send(
                                    bincode::serialize(&Frame::from(resp).to_vec())
                                        .unwrap()
                                        .as_slice(),
                                )
                                .await
                                .unwrap();
                        }
                        _ => {}
                    }
                }
            }),
        )
    }

    /// Spin up the [Transport] to handle queueing of requests and responses.
    pub async fn start_client(self) -> (Client, JoinHandle<()>) {
        let (resp_tx, mut resp_rx) = mpsc::channel(50);
        let (req_tx, req_rx) = mpsc::channel(50);
        (
            Client {
                receiver: req_rx,
                sender: resp_tx,
            },
            tokio::spawn(async move {
                loop {
                    match resp_rx.recv().await {
                        Some(resp) => {
                            self.sock
                                .send(
                                    bincode::serialize(&Frame::from(resp).to_vec())
                                        .unwrap()
                                        .as_slice(),
                                )
                                .await
                                .unwrap();
                        }
                        _ => {}
                    }
                }
            }),
        )
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO(e) => e.fmt(f),
            Self::Serialization(e) => e.fmt(f),
            Self::MPSC(e) => e.fmt(f),
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
