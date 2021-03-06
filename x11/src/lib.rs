#[forbid(unsafe_code)]
mod clipboard;
mod error;

pub use clipboard::Clipboard;
pub use error::Error;
