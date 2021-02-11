mod clipboard;
mod error;

use std::error::Error;

pub struct Clipboard(clipboard::Clipboard);

impl Clipboard {
    pub fn new() -> Result<Clipboard, Box<dyn Error>> {
        Ok(Clipboard(clipboard::Clipboard::new()?))
    }

    pub fn read(&self) -> Result<String, Box<dyn Error>> {
        Ok(String::from_utf8(self.0.load(
            self.0.getter.atoms.clipboard,
            self.0.getter.atoms.utf8_string,
            self.0.getter.atoms.property,
            std::time::Duration::from_secs(3),
        )?)?)
    }
}
