mod clipboard;
mod error;
mod run;

use std::ffi::c_void;
pub use xcb::*;

use std::error::Error;

pub struct Clipboard(clipboard::Clipboard);

impl Clipboard {
    /// Create Clipboard from an XLib display and window
    pub unsafe fn new_xlib(
        display: *mut c_void,
    ) -> Result<Clipboard, Box<dyn Error>> {
        Ok(Clipboard(clipboard::Clipboard::new_xlib(display)?))
    }

    /// Create Clipboard from an XCB connection and window
    pub unsafe fn new_xcb(
        connection: *mut c_void,
    ) -> Result<Clipboard, Box<dyn Error>> {
        Ok(Clipboard(clipboard::Clipboard::new_xcb(connection)?))
    }

    /// Read clipboard contents as a String
    pub fn read(&self) -> Result<String, Box<dyn Error>> {
        Ok(String::from_utf8(self.0.read(
            self.0.atoms.clipboard,
            self.0.atoms.utf8_string,
            std::time::Duration::from_secs(3),
        )?)?)
    }

    /// Write clipboard contents from a String
    pub fn write(&self, text: String) -> Result<(), Box<dyn Error>> {
        Ok(self.0.write(
            self.0.atoms.clipboard,
            self.0.atoms.utf8_string,
            text,
        )?)
    }
}
