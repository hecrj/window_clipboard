use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::error::Error;

pub struct Clipboard {
    raw: Raw,
}

enum Raw {
    #[cfg(all(
        unix,
        not(any(
            target_os = "macos",
            target_os = "android",
            target_os = "emscripten"
        ))
    ))]
    Wayland(RefCell<smithay_clipboard::WaylandClipboard>),

    NotWayland(RefCell<clipboard::ClipboardContext>),
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

                Raw::Wayland(RefCell::new(unsafe {
                    smithay_clipboard::WaylandClipboard::new_from_external(
                        handle.display as *mut _,
                    )
                }))
            }
            _ => {
                use clipboard::ClipboardProvider as _;

                Raw::NotWayland(RefCell::new(
                    clipboard::ClipboardContext::new()?
                ))
            }
        };

        Ok(Clipboard { raw })
    }

    pub fn read(&self) -> Result<String, Box<dyn Error>> {
        // TODO: Think about use of `RefCell`
        // Maybe we should make `read` mutable (?)
        use clipboard::ClipboardProvider as _;

        match &self.raw {
            #[cfg(all(
                unix,
                not(any(
                    target_os = "macos",
                    target_os = "android",
                    target_os = "emscripten"
                ))
            ))]
            Raw::Wayland(clipboard) => Ok(clipboard.borrow_mut().load(None)),
            Raw::NotWayland(clipboard) => clipboard.borrow_mut().get_contents(),
        }
    }

    pub fn write(
        &mut self,
        contents: impl Into<String>,
    ) -> Result<(), Box<dyn Error>> {
        use clipboard::ClipboardProvider as _;

        match &self.raw {
            #[cfg(all(
                unix,
                not(any(
                    target_os = "macos",
                    target_os = "android",
                    target_os = "emscripten"
                ))
            ))]
            Raw::Wayland(clipboard) => {
                clipboard.borrow_mut().store(None, contents);

                Ok(())
            }
            Raw::NotWayland(clipboard) => {
                clipboard.borrow_mut().set_contents(contents.into())
            }
        }
    }
}
