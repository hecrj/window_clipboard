use crate::ClipboardProvider;

use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
use std::error::Error;

pub use clipboard_wayland as wayland;
pub use clipboard_x11 as x11;

pub unsafe fn connect<W: HasDisplayHandle>(
    window: &W,
) -> Result<Box<dyn ClipboardProvider>, Box<dyn Error>> {
    let clipboard = match window.display_handle()?.as_raw() {
        RawDisplayHandle::Wayland(handle) => {
            Box::new(wayland::Clipboard::connect(handle.display.as_ptr())) as _
        }
        _ => Box::new(x11::Clipboard::connect()?) as _,
    };

    Ok(clipboard)
}

impl ClipboardProvider for wayland::Clipboard {
    fn read(&self) -> Result<String, Box<dyn Error>> {
        self.read()
    }

    fn read_primary(&self) -> Option<Result<String, Box<dyn Error>>> {
        Some(self.read_primary())
    }

    fn write(&mut self, contents: String) -> Result<(), Box<dyn Error>> {
        self.write(contents)
    }

    fn write_primary(&mut self, contents: String) -> Option<Result<(), Box<dyn Error>>> {
        Some(self.write_primary(contents))
    }
}

impl ClipboardProvider for x11::Clipboard {
    fn read(&self) -> Result<String, Box<dyn Error>> {
        self.read().map_err(Box::from)
    }

    fn read_primary(&self) -> Option<Result<String, Box<dyn Error>>> {
        Some(self.read_primary().map_err(Box::from))
    }

    fn write(&mut self, contents: String) -> Result<(), Box<dyn Error>> {
        self.write(contents).map_err(Box::from)
    }

    fn write_primary(&mut self, contents: String) -> Option<Result<(), Box<dyn Error>>> {
        Some(self.write_primary(contents).map_err(Box::from))
    }
}
