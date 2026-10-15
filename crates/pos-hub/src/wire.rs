//! Messages between a paired device and the shop hub: length-prefixed JSON
//! frames (4-byte big-endian length, then UTF-8 JSON).
use pos_service::{Reply, Request, Versions};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use uuid::Uuid;

/// Larger frames are refused (product pictures are at most 512 KB).
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    /// First message of every connection.
    Hello {
        protocol: u32,
        device: Uuid,
        token: String,
    },
    /// One request; `id` is echoed back and is the device's retry key.
    Call { id: Uuid, request: Box<Request> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    Welcome {
        register: Uuid,
        versions: Versions,
    },
    /// The connection is closed after this.
    Refused(Refusal),
    Answer {
        id: Uuid,
        result: Result<Box<Reply>, WireError>,
    },
    /// Data changed (by any device); reload the listed topics.
    Changed(Versions),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// The device runs another protocol version: update the app.
    Version {
        hub: u32,
    },
    /// Unknown, revoked or wrong token: pair again.
    NotPaired,
    BadMessage,
}

/// A business error as text: the kind for code, the message for people.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireError {
    pub kind: String,
    pub message: String,
}
impl From<&pos_core::Error> for WireError {
    fn from(e: &pos_core::Error) -> Self {
        let debug = format!("{e:?}");
        let kind = debug
            .split(|c: char| !c.is_alphanumeric())
            .next()
            .unwrap_or_default()
            .to_owned();
        Self {
            kind,
            message: e.to_string(),
        }
    }
}

#[derive(Debug)]
pub enum FrameError {
    Io(std::io::Error),
    TooLarge(usize),
    Json(serde_json::Error),
}
impl From<std::io::Error> for FrameError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for FrameError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::TooLarge(n) => write!(f, "frame of {n} bytes is too large"),
            Self::Json(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for FrameError {}

pub fn write_frame<T: Serialize>(out: &mut impl Write, message: &T) -> Result<(), FrameError> {
    let body = serde_json::to_vec(message)?;
    if body.len() > MAX_FRAME {
        return Err(FrameError::TooLarge(body.len()));
    }
    let len = u32::try_from(body.len()).map_err(|_| FrameError::TooLarge(body.len()))?;
    out.write_all(&len.to_be_bytes())?;
    out.write_all(&body)?;
    out.flush()?;
    Ok(())
}
pub fn read_frame<T: for<'de> Deserialize<'de>>(input: &mut impl Read) -> Result<T, FrameError> {
    let mut len = [0u8; 4];
    input.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge(len));
    }
    let mut body = vec![0u8; len];
    input.read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}
