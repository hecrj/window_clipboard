use crate::ClipboardProvider;

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use std::error::Error;
use std::fmt;

pub use clipboard_wayland as wayland;
pub use clipboard_x11 as x11;

#[derive(Debug)]
struct Unsupported(RawWindowHandle);

impl std::error::Error for Unsupported {}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unsupported window handle: {:?}", self.0)
    }
}

pub fn new_clipboard<W: HasRawWindowHandle>(
    window: &W,
) -> Result<Box<dyn ClipboardProvider>, Box<dyn Error>> {
    match window.raw_window_handle() {
        RawWindowHandle::Wayland(handle) => {
            assert!(!handle.display.is_null());
            let clipboard =
                unsafe { wayland::Clipboard::new(handle.display as *mut _) };
            Ok(Box::new(clipboard))
        }
        RawWindowHandle::Xcb(handle) => {
            assert!(!handle.connection.is_null());
            let clipboard =
                unsafe { x11::Clipboard::new_xcb(handle.connection)? };
            Ok(Box::new(clipboard))
        }
        RawWindowHandle::Xlib(handle) => {
            assert!(!handle.display.is_null());
            let clipboard =
                unsafe { x11::Clipboard::new_xlib(handle.display)? };
            Ok(Box::new(clipboard))
        }
        h => Err(Box::new(Unsupported(h))),
    }
}

impl ClipboardProvider for wayland::Clipboard {
    fn read(&self) -> Result<String, Box<dyn Error>> {
        self.read()
    }

    fn write(
        &mut self,
        string: std::borrow::Cow<str>,
    ) -> Result<(), Box<dyn Error>> {
        wayland::Clipboard::write(self, string.to_string())
    }
}

impl ClipboardProvider for x11::Clipboard {
    fn read(&self) -> Result<String, Box<dyn Error>> {
        self.read()
    }

    fn write(
        &mut self,
        string: std::borrow::Cow<str>,
    ) -> Result<(), Box<dyn Error>> {
        x11::Clipboard::write(self, string.to_string())
    }
}
