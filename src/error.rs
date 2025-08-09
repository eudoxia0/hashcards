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

use std::fmt::Display;
use std::fmt::Formatter;

pub struct ErrorReport {
    message: String,
}

impl From<std::io::Error> for ErrorReport {
    fn from(value: std::io::Error) -> Self {
        ErrorReport {
            message: format!("I/O error: {value:#?}"),
        }
    }
}

impl Display for ErrorReport {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "error: {}", self.message)
    }
}

pub type Fallible<T> = Result<T, ErrorReport>;

pub fn fail<T>(msg: impl Into<String>) -> Fallible<T> {
    Err(ErrorReport {
        message: msg.into(),
    })
}
