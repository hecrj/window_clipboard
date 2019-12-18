use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::error::Error;

pub struct Clipboard {
    raw: RefCell<Box<dyn copypasta::ClipboardProvider>>,
}

impl Clipboard {
    pub fn new<W: HasRawWindowHandle>(
        window: &W,
    ) -> Result<Self, Box<dyn Error>> {
        let raw = match window.raw_window_handle() {
            #[cfg(all(
                unix,
                not(any(
                    target_os = "macos",
                    target_os = "android",
                    target_os = "emscripten"
                ))
            ))]
            RawWindowHandle::Wayland(handle) => {
                assert!(!handle.display.is_null());

                Box::new(unsafe {
                    let (_, raw) = copypasta::wayland_clipboard::create_clipboards_from_external(
                        handle.display as *mut _,
                    );

                    raw
                }) as _
            }
            _ => Box::new(copypasta::ClipboardContext::new()?) as _,
        };

        Ok(Clipboard {
            raw: RefCell::new(raw),
        })
    }

    pub fn read(&self) -> Result<String, Box<dyn Error>> {
        // TODO: Think about use of `RefCell`
        // Maybe we should make `read` mutable (?)
        self.raw.borrow_mut().get_contents()
    }

    pub fn write(
        &mut self,
        contents: impl Into<String>,
    ) -> Result<(), Box<dyn Error>> {
        self.raw.borrow_mut().set_contents(contents.into())
    }
}
