//! Message types used in communication between the ff client and server.

use std::{fmt::Display, time};

use bincode;
use serde::{Deserialize, Serialize};

/// Maximum size of a single transport frame.
pub const MAXIMUM_SIZE: usize = 65535;

mod encoding {
    use bincode::{
        self,
        config::{
            BigEndian, Bounded, RejectTrailing, WithOtherEndian, WithOtherLimit, WithOtherTrailing,
        },
        DefaultOptions, Options,
    };
    use lazy_static::lazy_static;

    lazy_static! {
        pub static ref BINCODE_OPTS: WithOtherTrailing<
            WithOtherLimit<WithOtherEndian<DefaultOptions, BigEndian>, Bounded>,
            RejectTrailing,
        > = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_limit(super::MAXIMUM_SIZE as u64)
            .reject_trailing_bytes();
    }
}

#[derive(Debug)]
/// Types of communication errors that can occur.
pub enum Error {
    Serialization(bincode::Error),
    V6NotSupported,
    ConnectionTimeout,
    ImpossibleDataLen(u32),
    UnexpectedType,
    WrongChecksum,
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

    /// Length of a file.
    Summary(u32),

    /// Part of a file.
    Part { start_byte: u32, data: Vec<u8> },

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

impl std::error::Error for Error {}

impl From<bincode::Error> for Error {
    fn from(b: bincode::Error) -> Self {
        Self::Serialization(b)
    }
}
