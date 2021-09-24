//! Message types used in communication between the FF client and server.

use std::{fmt::Display, time};

use serde::{Deserialize, Serialize};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    Serialization(bincode::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// List what's available.
    List,

    /// Files found.
    Directory(Vec<FileData>),

    /// Download a file.
    Download { path: String },

    /// Writing a file part.
    Part { num: u64, end: bool, data: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
