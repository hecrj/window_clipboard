mod clipboard;
mod error;

pub use xcb::*;

use std::error::Error;

pub struct Clipboard(clipboard::Clipboard);

impl Clipboard {
    pub fn new() -> Result<Clipboard, Box<dyn Error>> {
        Ok(Clipboard(clipboard::Clipboard::new(None)?))
    }

    pub fn read(&self) -> Result<String, Box<dyn Error>> {
        Ok(String::from_utf8(self.0.load(
            self.0.atoms.clipboard,
            self.0.atoms.utf8_string,
            self.0.atoms.property,
            std::time::Duration::from_secs(3),
        )?)?)
    }
}
