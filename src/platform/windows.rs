use crate::ClipboardProvider;

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
        Ok(clipboard_win::get_clipboard_string()?)
    }

    fn write(
        &mut self,
        string: std::borrow::Cow<str>,
    ) -> Result<(), Box<dyn Error>> {
        Ok(clipboard_win::set_clipboard_string(&string)?)
    }
}
