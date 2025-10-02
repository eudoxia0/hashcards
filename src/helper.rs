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

use std::fs::copy;
use std::fs::create_dir_all;
use std::path::PathBuf;

use tempfile::tempdir;

use crate::error::Fallible;

pub fn create_tmp_copy_of_test_directory() -> Fallible<String> {
    let source: PathBuf = PathBuf::from("./test").canonicalize()?;
    let target: PathBuf = tempdir()?.path().to_path_buf().canonicalize()?;
    create_dir_all(&target)?;
    for entry in source.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(file_name) = path.file_name() {
                if file_name != "db.sqlite3" {
                    let target_path = target.join(file_name);
                    copy(&path, &target_path)?;
                }
            }
        }
    }
    Ok(target.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_tmp_copy_of_test_directory() -> Fallible<()> {
        let result = create_tmp_copy_of_test_directory();
        assert!(result.is_ok());
        Ok(())
    }
}
