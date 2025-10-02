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

use crate::collection::Collection;
use crate::error::Fallible;

pub fn check_collection(directory: Option<String>) -> Fallible<()> {
    let _ = Collection::new(directory)?;
    println!("ok");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_collection;
    use crate::error::Fallible;
    use crate::helper::create_tmp_copy_of_test_directory;

    #[test]
    fn test_non_existent_directory() {
        assert!(check_collection(Some("./derpherp".to_string())).is_err());
    }

    #[test]
    fn test_directory() -> Fallible<()> {
        let directory = create_tmp_copy_of_test_directory()?;
        assert!(check_collection(Some(directory)).is_ok());
        Ok(())
    }
}
