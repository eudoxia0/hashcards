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

use std::path::PathBuf;

use crate::error::Fallible;
use crate::error::fail;
use crate::parser::parse_deck;

pub fn check_deck(directory: &PathBuf) -> Fallible<()> {
    if !directory.exists() {
        return fail("directory does not exist.");
    }
    let _ = parse_deck(directory)?;
    println!("ok");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::check::check_deck;

    #[test]
    fn test_non_existent_directory() {
        let directory = PathBuf::from("./derpherp");
        assert!(check_deck(&directory).is_err());
    }

    #[test]
    fn test_directory() {
        let directory = PathBuf::from("./test");
        assert!(check_deck(&directory).is_ok());
    }

    #[test]
    fn test_example_directory() {
        let directory = PathBuf::from("./example");
        assert!(check_deck(&directory).is_ok());
    }
}
