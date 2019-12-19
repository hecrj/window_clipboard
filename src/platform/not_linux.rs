use crate::ClipboardProvider;

use raw_window_handle::HasRawWindowHandle;
use std::error::Error;

pub fn new_clipboard<W: HasRawWindowHandle>(
    _window: &W,
) -> Result<Box<dyn ClipboardProvider>, Box<dyn Error>> {
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(window_clipboard_windows::Clipboard::new()?))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(window_clipboard_macos::Clipboard::new()?))
    }
}

#[cfg(target_os = "windows")]
impl ClipboardProvider for window_clipboard_windows::Clipboard {
    fn read(&self) -> Result<String, Box<dyn Error>> {
        self.read()
    }
}

#[cfg(target_os = "macos")]
impl ClipboardProvider for window_clipboard_macos::Clipboard {
    fn read(&self) -> Result<String, Box<dyn Error>> {
        self.read()
    }
}
