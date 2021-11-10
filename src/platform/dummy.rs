use crate::ClipboardProvider;

use raw_window_handle::HasRawWindowHandle;
use std::error::Error;

pub use clipboard_dummy as dummy;

pub fn connect<W: HasRawWindowHandle>(
    window: &W,
) -> Result<Box<dyn ClipboardProvider>, Box<dyn Error>> {
    let clipboard = Box::new(dummy::Clipboard::connect()?);
    Ok(clipboard)
}

impl ClipboardProvider for dummy::Clipboard {
    fn read(&self) -> Result<String, Box<dyn Error>> {
        self.read()
    }

    fn write(&mut self, contents: String) -> Result<(), Box<dyn Error>> {
        self.write(contents)
    }
}
