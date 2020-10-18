#[cfg(all(
    unix,
    not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
        target_os = "emscripten"
    ))
))]
#[path = "platform/linux.rs"]
mod platform;

#[cfg(target_os = "windows")]
#[path = "platform/windows.rs"]
mod platform;

#[cfg(target_os = "macos")]
#[path = "platform/macos.rs"]
mod platform;

#[cfg(target_os = "ios")]
#[path = "platform/ios.rs"]
mod platform;

use raw_window_handle::HasRawWindowHandle;
use std::error::Error;

pub struct Clipboard {
    raw: Box<dyn ClipboardProvider>,
}

impl Clipboard {
    pub fn new<W: HasRawWindowHandle>(
        window: &W,
    ) -> Result<Self, Box<dyn Error>> {
        let raw = platform::new_clipboard(window)?;

        Ok(Clipboard { raw })
    }

    pub fn read(&self) -> Result<String, Box<dyn Error>> {
        // TODO: Think about use of `RefCell`
        // Maybe we should make `read` mutable (?)
        self.raw.read()
    }

    pub fn write(&self, s: String) -> Result<(), Box<dyn Error>> {
        self.raw.write(s)
    }
}

pub trait ClipboardProvider {
    fn read(&self) -> Result<String, Box<dyn Error>>;
    fn write(&self, s: String) -> Result<(), Box<dyn Error>>;
}
