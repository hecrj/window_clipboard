use crate::ClipboardProvider;

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use std::error::Error;

pub use clipboard_wayland as wayland;
pub use clipboard_x11 as x11;

pub fn new_clipboard<W: HasRawWindowHandle>(
    window: &W,
) -> Result<Box<dyn ClipboardProvider>, Box<dyn Error>> {
    let clipboard = match window.raw_window_handle() {
        RawWindowHandle::Wayland(handle) => {
            assert!(!handle.display.is_null());

            Box::new(unsafe {
                wayland::Clipboard::new(handle.display as *mut _)
            }) as _
        }
        _ => Box::new(x11::Clipboard::new()?) as _,
    };

    Ok(clipboard)
}

impl ClipboardProvider for wayland::Clipboard {
    fn read(&self) -> Result<String, Box<dyn Error>> {
        self.read()
    }

    fn write(&self, s: String) -> Result<(), Box<dyn Error>> {
        self.write(s)
    }
}

impl ClipboardProvider for x11::Clipboard {
    fn read(&self) -> Result<String, Box<dyn Error>> {
        self.read()
    }

    fn write(&self, s: String) -> Result<(), Box<dyn Error>> {
        self.write(s)
    }
}
