#![doc = include_str!("../proto.md")]

use std::{
    collections::{HashMap, HashSet},
    convert::{TryFrom, TryInto},
    error,
    fmt::Display,
    hash::Hash,
    io,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    result,
    time::{self, Duration},
};

use bincode::Options;
use ring::{
    agreement, digest,
    rand::{self, SystemRandom},
};

/// Maximum size of a single transport frame.
pub const MAXIMUM_SIZE: usize = 65535;

/// Possible states of an FFTP connection.
enum State {
    /// No attempt to connect has been made.
    Disconnected,

    /// Waiting for peer to reply with publickey.
    WaitPeer,

    /// Connected to the peer.
    Connected,
}

/// Any type that can distinguish addresses and send to addresses.
///
/// The transport trait is highly permissive by design to keep the
/// protocol framework flexible.
pub trait Transport {
    type PeerId: Eq + Hash;
    type Error: error::Error;

    /// Given an arbitrary buffer of bytes, send some data to the given
    /// address.
    fn send_to<A: Into<Self::PeerId>, D: AsRef<[u8]>>(
        &mut self,
        buf: D,
        addr: &A,
    ) -> Result<usize, Self::Error>;

    /// Given a reference to a mutable buffer of bytes, receive data into
    /// the buffer, returning the amount of data read and the [Self::PeerId]
    /// of the associated sender.
    fn recv_from<D: AsMut<[u8]>>(&mut self, buf: D) -> Result<(usize, Self::PeerId), Self::Error>;
}

impl Transport for UdpSocket {
    type PeerId = SocketAddr;
    type Error = io::Error;

    fn send_to<A: Into<Self::PeerId>, D: AsRef<[u8]>>(
        &mut self,
        buf: D,
        addr: &A,
    ) -> Result<usize, Self::Error> {
        self.send_to(buf.as_ref(), addr.into())
    }

    fn recv_from<D: AsMut<[u8]>>(
        &mut self,
        mut buf: D,
    ) -> Result<(usize, Self::PeerId), Self::Error> {
        self.recv_from(buf.as_mut())
    }
}

/// An FFTP socket.
pub struct Socket<T: Transport> {
    /// Underlying transport.
    transport: T,

    /// Addresses that are currently being connected with.
    handshaking: HashMap<T::PeerId, (agreement::EphemeralPrivateKey, State)>,

    /// Addresses that are considered "connected".
    destination_addrs: HashMap<T::PeerId, (agreement::EphemeralPrivateKey, agreement::PublicKey)>,
}

impl<T> Socket<T>
where
    T: Transport,
{
    /// Create a new FFTP [Socket] using the underlying [Transport].
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            handshaking: HashMap::new(),
            destination_addrs: HashMap::new(),
        }
    }

    /// Initiate a handshake with a peer.
    pub fn connect<'a, A: Into<T::PeerId>>(&'a mut self, addr: A) -> &'a Self {
        // Generate and send public key to peer.
        let private_key = agreement::EphemeralPrivateKey::generate(
            &agreement::X25519,
            &rand::SystemRandom::new(),
        )
        .unwrap();
        let public_key = private_key.compute_public_key().unwrap();
        self.transport.send_to(public_key.as_ref(), &addr);

        // Insert into pending handshakes for future reference.
        self.handshaking
            .insert(addr.into(), (private_key, State::Disconnected));
        self
    }

    /// Receive a message from a peer.
    pub fn recv<D: AsRef<[u8]>>(&mut self) -> Result<(Frame<D>, T::PeerId), Error> {
        let mut buf = [0; MAXIMUM_SIZE];
        if let Ok((size, sender)) = self.transport.recv_from(&mut buf) {
            return Ok((
                encoding::BINCODE_OPTS.deserialize(&buf[..size]).unwrap(),
                sender,
            ));
        }
        Err(Error::UnexpectedType)
    }

    /// Send a message to a peer.
    pub fn send<D: AsRef<[u8]>>(&mut self, r: &Frame<D>, peer: T::PeerId) {
        todo!()
    }
}

/// A frame for FFTP communications.
#[derive(Debug, Clone)]
pub struct Frame<D: AsRef<[u8]> + TryFrom<Vec<u8>>> {
    data: D,
    checksum: [u8; digest::SHA256_OUTPUT_LEN],
}

impl<D> Frame<D>
where
    D: AsRef<[u8]> + TryFrom<Vec<u8>>,
{
    fn deserialize(buf: &[u8]) -> Self {
        let checksum = &buf[buf.len() - digest::SHA256_OUTPUT_LEN..]
            .try_into()
            .unwrap();
        let data = buf[..buf.len() - digest::SHA256_OUTPUT_LEN]
            .try_into()
            .unwrap();
        Self { data, checksum }
    }
}

/// An initiating frame.
pub type Initiate = Frame<agreement::PublicKey>;

/// Handshake reply frame. Contains encrypted data.
pub type First<D> = Frame<(agreement::PublicKey, D)>;

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialization(e) => e.fmt(f),
            Self::V6NotSupported => write!(f, "IPv6 is not supported yet"),
            Self::ConnectionTimeout => write!(f, "timed out connecting"),
            Self::ImpossibleDataLen(len) => write!(f, "data length '{}' is impossible", len),
            Self::UnexpectedType => write!(f, "expected request/response or vice versa"),
            Self::WrongChecksum => write!(f, "wrong checksum"),
        }
    }
}

/// Communication errors that can occur.
#[derive(Debug)]
pub enum Error {
    Serialization(bincode::Error),
    V6NotSupported,
    ConnectionTimeout,
    ImpossibleDataLen(u32),
    UnexpectedType,
    WrongChecksum,
}

impl std::error::Error for Error {}

impl From<bincode::Error> for Error {
    fn from(b: bincode::Error) -> Self {
        Self::Serialization(b)
    }
}

mod files {
    use super::*;

    /// Requests that may be sent from a client.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[repr(u8)]
    pub enum Request {
        /// List files available for download. Recursive listing requests may or may not be ignored.
        List { path: String, recursive: bool },

        /// Download an entire file.
        Download { path: String },

        /// Download a specific *part* of a file.
        DownloadPart {
            /// Path of the file.
            path: String,

            /// The byte to start at.
            start_byte: u32,

            /// The amount of data to request.
            len: u32,
        },
    }

    impl<'a> From<&'a [u8]> for Request {
        fn from(v: &'a [u8]) -> Self {
            encoding::BINCODE_OPTS.deserialize(v).unwrap()
        }
    }

    impl Into<Vec<u8>> for Request {
        fn into(self) -> Vec<u8> {
            encoding::BINCODE_OPTS.serialize(&self).unwrap()
        }
    }

    /// Responses that may be sent by a server.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[repr(u8)]
    pub enum Response {
        /// Directory listing.
        Directory(Vec<FileData>),

        /// Length of a file.
        Summary(u32),

        /// Part of a file.
        Part { start_byte: u32, data: Vec<u8> },

        /// Operation is not allowed.
        NotAllowed,
    }

    impl<'a> From<&'a [u8]> for Response {
        fn from(v: &'a [u8]) -> Self {
            encoding::BINCODE_OPTS.deserialize(v).unwrap()
        }
    }

    impl Into<Vec<u8>> for Response {
        fn into(self) -> Vec<u8> {
            encoding::BINCODE_OPTS.serialize(&self).unwrap()
        }
    }

    /// Simple representation of a file on the server.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FileData {
        /// Path of the file on the server.
        pub path: String,

        /// Creation date of the file measured in nanoseconds from the epoch.
        pub created: time::Duration,

        /// Size of the file on disk.
        pub size: u64,
    }
}
