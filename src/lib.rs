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

use raw_window_handle::HasRawDisplayHandle;
use std::error::Error;

pub struct Clipboard {
    raw: Box<dyn ClipboardProvider>,
}

impl Clipboard {
    pub fn connect<W: HasRawDisplayHandle>(
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

    #[cfg(target_family = "unix")]
    pub fn read_primary(&self) -> Result<String, Box<dyn Error>> {
        self.raw.read_primary()
    }

    #[cfg(target_family = "unix")]
    pub fn write_primary(
        &mut self,
        contents: String,
    ) -> Result<(), Box<dyn Error>> {
        self.raw.write_primary(contents)
    }
}

pub trait ClipboardProvider {
    fn read(&self) -> Result<String, Box<dyn Error>>;

    fn write(&mut self, contents: String) -> Result<(), Box<dyn Error>>;

    #[cfg(target_family = "unix")]
    fn read_primary(&self) -> Result<String, Box<dyn Error>>;

    #[cfg(target_family = "unix")]
    fn write_primary(&mut self, contents: String)
        -> Result<(), Box<dyn Error>>;
}
