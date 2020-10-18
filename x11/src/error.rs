use std::error::Error as StdError;
use std::fmt;
use std::sync::mpsc::SendError;
use xcb::base::{ConnError, GenericError};
use xcb::Atom;

#[must_use]
#[derive(Debug)]
pub enum Error {
    Set(SendError<Atom>),
    XcbConn(ConnError),
    XcbGeneric(GenericError),
    Lock,
    Timeout,
    Owner,
    UnexpectedType(Atom),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Set(e) => write!(f, "XCB - couldn't set atom: {:?}", e),
            Error::XcbConn(e) => write!(f, "XCB connection error: {:?}", e),
            Error::XcbGeneric(e) => write!(f, "XCB generic error: {:?}", e),
            Error::Lock => write!(f, "XCB: Lock is poisoned"),
            Error::Timeout => write!(f, "Selection timed out"),
            Error::Owner => {
                write!(f, "Failed to set new owner of XCB selection")
            }
            Error::UnexpectedType(target) => {
                write!(f, "Unexpected Reply type: {}", target)
            }
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        use self::Error::*;
        match self {
            Set(e) => Some(e),
            XcbConn(e) => Some(e),
            XcbGeneric(e) => Some(e),
            Lock | Timeout | Owner | UnexpectedType(_) => None,
        }
    }
}

macro_rules! define_from {
    ( $item:ident from $err:ty ) => {
        impl From<$err> for Error {
            fn from(err: $err) -> Error {
                Error::$item(err)
            }
        }
    };
}

define_from!(Set from SendError<Atom>);
define_from!(XcbConn from ConnError);
define_from!(XcbGeneric from GenericError);
