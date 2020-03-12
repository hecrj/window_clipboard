use crate::ClipboardProvider;

use clipboard_win::get_clipboard_string;
use raw_window_handle::HasRawWindowHandle;

use std::error::Error;

pub fn new_clipboard<W: HasRawWindowHandle>(
    _window: &W,
) -> Result<Box<dyn ClipboardProvider>, Box<dyn Error>> {
    Ok(Box::new(Clipboard))
}

pub struct Clipboard;

impl ClipboardProvider for Clipboard {
    fn read(&self) -> Result<String, Box<dyn Error>> {
        Ok(get_clipboard_string()?)
    }
}
