// Copyright 2025 Fernando Borretti
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Browser localStorage wrapper for persisting data.

use wasm_bindgen::prelude::*;
use web_sys::Storage as WebStorage;

pub struct Storage {
    inner: Option<WebStorage>,
}

impl Storage {
    pub fn new() -> Self {
        let inner = web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten();
        Self { inner }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, JsValue> {
        match &self.inner {
            Some(storage) => storage.get_item(key),
            None => Ok(None),
        }
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), JsValue> {
        match &self.inner {
            Some(storage) => storage.set_item(key, value),
            None => Ok(()),
        }
    }

    #[allow(dead_code)]
    pub fn remove(&self, key: &str) -> Result<(), JsValue> {
        match &self.inner {
            Some(storage) => storage.remove_item(key),
            None => Ok(()),
        }
    }
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}
