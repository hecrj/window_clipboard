use x11rb::errors::{ConnectError, ConnectionError, ReplyError};
use x11rb::protocol::xproto::Atom;

use std::sync::mpsc;

#[must_use]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("connection failed: {0}")]
    ConnectionFailed(#[from] ConnectError),
    #[error("connection errored: {0}")]
    ConnectionErrored(#[from] ConnectionError),
    #[error("reply failed: {0}")]
    ReplyError(#[from] ReplyError),
    #[error("timeout")]
    Timeout,
    #[error("unexpected type: {0}")]
    UnexpectedType(Atom),
    #[error("invalid utf8 string: {0}")]
    InvalidUtf8(std::string::FromUtf8Error),
    #[error("deadlock")]
    SelectionLocked,
    #[error("invalid selection owner")]
    InvalidOwner,
    #[error("worker communication error")]
    SendError(#[from] mpsc::SendError<Atom>),
}
