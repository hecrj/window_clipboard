// Copyright 2017 Avraham Weinstock
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

use std::error::Error;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};

pub struct Clipboard {
    context: Arc<Mutex<smithay_clipboard::Clipboard>>,
}

impl Clipboard {
    pub unsafe fn connect(display: *mut c_void) -> Clipboard {
        let context = Arc::new(Mutex::new(smithay_clipboard::Clipboard::new(
            display as *mut _,
        )));

        Clipboard { context }
    }

    pub fn read(&self) -> Result<String, Box<dyn Error>> {
        Ok(self.context.lock().unwrap().load()?)
    }

    pub fn write(&mut self, data: String) -> Result<(), Box<dyn Error>> {
        self.context.lock().unwrap().store(data);

        Ok(())
    }
}
