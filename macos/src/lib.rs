// Copyright 2016 Avraham Weinstock
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{msg_send, ClassType};
use objc2_app_kit::{NSPasteboard, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSString};
use std::error::Error;
use std::panic::{RefUnwindSafe, UnwindSafe};

pub struct Clipboard {
    pasteboard: Retained<NSPasteboard>,
}

unsafe impl Send for Clipboard {}
unsafe impl Sync for Clipboard {}
impl UnwindSafe for Clipboard {}
impl RefUnwindSafe for Clipboard {}

impl Clipboard {
    pub fn new() -> Result<Clipboard, Box<dyn Error>> {
        // Use `msg_send_id!` instead of `NSPasteboard::generalPasteboard()`
        // in the off case that it will return NULL (even though it's
        // documented not to).
        let pasteboard: Option<Retained<NSPasteboard>> =
            unsafe { msg_send![NSPasteboard::class(), generalPasteboard] };
        let pasteboard =
            pasteboard.ok_or("NSPasteboard#generalPasteboard returned null")?;
        Ok(Self { pasteboard })
    }

    pub fn read(&self) -> Result<String, Box<dyn Error>> {
        // The NSPasteboard API is a bit weird, it requires you to pass
        // classes as objects, which `objc2_foundation::NSArray` was not really
        // made for - so we convert the class to an `AnyObject` type instead.
        //
        // TODO: Use the NSPasteboard helper APIs (`stringForType`).
        let string_class = NSString::class();
        let classes = NSArray::arrayWithObject(string_class);
        let string_array = unsafe {
            self.pasteboard
                .readObjectsForClasses_options(&classes, None)
        }
        .ok_or("pasteboard#readObjectsForClasses:options: returned null")?;

        let obj = string_array.firstObject().ok_or(
            "pasteboard#readObjectsForClasses:options: returned empty",
        )?;
        // `readObjectsForClasses:options:` returns instances of the classes we pass in.
        // Since we requested `NSString::class()`, this object is guaranteed to be an `NSString`.
        let obj: Retained<NSString> = unsafe { Retained::cast_unchecked(obj) };
        Ok(obj.to_string())
    }

    pub fn write(&mut self, data: String) -> Result<(), Box<dyn Error>> {
        let obj: Retained<ProtocolObject<dyn NSPasteboardWriting>> =
            ProtocolObject::from_retained(NSString::from_str(&data));
        let string_array = NSArray::arrayWithObject(&*obj);
        self.pasteboard.clearContents();
        let success = self.pasteboard.writeObjects(&string_array);
        if success {
            Ok(())
        } else {
            Err("NSPasteboard#writeObjects: returned false".into())
        }
    }
}
