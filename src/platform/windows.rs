use crate::ClipboardProvider;

use raw_window_handle::HasRawWindowHandle;
use clipboard_win::{get_clipboard_string, set_clipboard_string};
use std::error::Error;

pub fn new_clipboard<W: HasRawWindowHandle>(
    _window: &W,
) -> Result<Box<dyn ClipboardProvider>, Box<dyn Error>> {
        Ok(Box::new(clipboard_windows::Clipboard::new()?))
}

impl ClipboardProvider for clipboard_windows::Clipboard {
    fn fn read(&self) -> Result<String, Box<dyn Error>> {
        Ok(get_clipboard_string()?)
    }
}

