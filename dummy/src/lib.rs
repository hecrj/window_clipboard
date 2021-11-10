#[forbid(unsafe_code)]
use std::error::Error;

/// A connection to an Dummy [`Clipboard`].
pub struct Clipboard;

impl Clipboard {
    /// Connect to the Dummy and obtain a [`Clipboard`].
    pub fn connect() -> Result<Self, Box<dyn Error>> {
        Ok(Clipboard)
    }

    /// Read the current [`Clipboard`] value.
    pub fn read(&self) -> Result<String, Box<dyn Error>> {
        Ok(String::new())
    }

    /// Write a new value to the [`Clipboard`].
    pub fn write(&mut self, contents: String) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}
