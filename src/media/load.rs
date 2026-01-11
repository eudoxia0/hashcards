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

use std::path::Component;
use std::path::PathBuf;

/// The media loader takes collection-relative file paths and returns the
/// absolute path to the file, if it exists.
///
/// This takes unsafe strings from the client, so we have to ensure there's
/// no possibility of directory traversals.
pub struct MediaLoader {
    /// Absolute path to the collection root directory.
    root: PathBuf,
}

/// Errors that can occur when loading a path.
#[derive(Debug, PartialEq)]
pub enum MediaLoaderError {
    /// Path is absolute.
    Absolute,
    /// Path does not exist.
    NotFound,
    /// Path is not a file.
    NotFile,
    /// Path points to a symbolic link.
    SymbolicLink,
    /// Path contains parent (`..`) components.
    ParentComponent,
}

impl MediaLoader {
    /// Construct a new [`MediaLoader`].
    pub fn new(path: PathBuf) -> Self {
        assert!(path.is_absolute());
        Self { root: path }
    }

    /// Given a path string from the client, check that a file exists at that
    /// location within the collection root directory.
    ///
    /// Symbolic links and absolute paths are rejected.
    pub fn validate(&self, path: &str) -> Result<PathBuf, MediaLoaderError> {
        let path: PathBuf = PathBuf::from(path);
        if path.components().any(|c| c == Component::ParentDir) {
            return Err(MediaLoaderError::ParentComponent);
        }
        if path.is_absolute() {
            return Err(MediaLoaderError::Absolute);
        }
        let path: PathBuf = self.root.join(path);
        if !path.exists() {
            return Err(MediaLoaderError::NotFound);
        }
        if path.is_symlink() {
            return Err(MediaLoaderError::SymbolicLink);
        }
        if !path.is_file() {
            return Err(MediaLoaderError::NotFile);
        }
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Fallible;
    use crate::helper::create_tmp_directory;

    /// Absolute paths are rejected.
    #[test]
    fn test_abs_rejected() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let loader = MediaLoader::new(root);
        assert_eq!(
            loader.validate("/etc/passwd"),
            Err(MediaLoaderError::Absolute)
        );
        Ok(())
    }

    /// Paths with parent components are rejected.
    #[test]
    fn test_parent() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let loader = MediaLoader::new(root);
        assert_eq!(
            loader.validate("../../../../../../../../../../etc/passwd"),
            Err(MediaLoaderError::ParentComponent)
        );
        Ok(())
    }

    /// Paths to non-existent files are rejected.
    #[test]
    fn test_non_existent() -> Fallible<()> {
        let root = create_tmp_directory()?;
        let loader = MediaLoader::new(root);
        assert_eq!(
            loader.validate("does_not_exist.txt"),
            Err(MediaLoaderError::NotFound)
        );
        Ok(())
    }

    /// Paths to symlinks are rejected.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_symlink() -> Fallible<()> {
        use std::fs::File;
        use std::os::unix::fs::symlink;

        let root = create_tmp_directory()?;
        let loader = MediaLoader::new(root.clone());

        // Create a real file
        let real_file = root.join("real.txt");
        File::create(&real_file)?;

        // Create a symlink to it
        let link_path = root.join("link.txt");
        symlink(&real_file, &link_path)?;

        // Try to validate the symlink path
        assert_eq!(
            loader.validate("link.txt"),
            Err(MediaLoaderError::SymbolicLink)
        );
        Ok(())
    }

    /// Paths to directories.
    #[test]
    fn test_dir() -> Fallible<()> {
        use std::fs::create_dir;

        let root = create_tmp_directory()?;
        let loader = MediaLoader::new(root.clone());

        // Create a subdirectory
        let subdir = root.join("subdir");
        create_dir(&subdir)?;

        // Try to validate the directory path
        assert_eq!(loader.validate("subdir"), Err(MediaLoaderError::NotFile));
        Ok(())
    }
}
