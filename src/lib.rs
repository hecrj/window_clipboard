#[cfg(all(
    unix,
    not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
        target_os = "emscripten",
        target_os = "redox"
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

#[cfg(target_os = "android")]
#[path = "platform/android.rs"]
mod platform;

#[cfg(not(any(
    all(
        unix,
        not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "android",
            target_os = "emscripten",
            target_os = "redox"
        ))
    ),
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
)))]
#[path = "platform/dummy.rs"]
mod platform;

use raw_window_handle::HasDisplayHandle;
use std::error::Error;

pub struct Clipboard {
    raw: Box<dyn ClipboardProvider>,
}

impl Clipboard {
    /// Safety: the display handle must be valid for the lifetime of `Clipboard`
    pub unsafe fn connect<W: HasDisplayHandle>(
        window: &W,
    ) -> Result<Self, Box<dyn Error>> {
        let raw = platform::connect(window)?;

        Ok(Clipboard { raw })
    }

    pub fn read(&self) -> Result<String, Box<dyn Error>> {
        self.raw.read()
    }

    pub fn write(&mut self, contents: String) -> Result<(), Box<dyn Error>> {
        self.raw.write(contents)
    }
}

impl Clipboard {
    pub fn read_primary(&self) -> Option<Result<String, Box<dyn Error>>> {
        self.raw.read_primary()
    }

    pub fn write_primary(&mut self, contents: String) -> Option<Result<(), Box<dyn Error>>> {
        self.raw.write_primary(contents)
    }
}

pub trait ClipboardProvider {
    fn read(&self) -> Result<String, Box<dyn Error>>;

    fn write(&mut self, contents: String) -> Result<(), Box<dyn Error>>;

    fn read_primary(&self) -> Option<Result<String, Box<dyn Error>>> {
        None
    }

    fn write_primary(&mut self, _contents: String) -> Option<Result<(), Box<dyn Error>>> {
        None
    }
}
