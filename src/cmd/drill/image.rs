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

use std::path::Path;
use std::path::PathBuf;

/// Errors that can occur when validating an image path.
#[derive(Debug, PartialEq)]
pub enum ImagePathError {
    /// Path is the empty string.
    Empty,
    /// Path is a symbolic link.
    Symlink,
    /// Path contains invalid components (e.g., ".." or is absolute).
    InvalidPath,
    /// File does not exist or cannot be accessed
    NotFound,
    /// Path resolves outside the collection directory
    OutsideDirectory,
}

/// Validates a user-provided path and returns a canonicalized path that is
/// guaranteed to be within the base directory.
///
/// This function prevents directory traversal attacks by:
/// 1. Rejecting paths containing ".." or absolute paths
/// 2. Canonicalizing the path to resolve symlinks
/// 3. Verifying the canonicalized path is within the base directory
pub fn validate_image_path(base_dir: &Path, user_path: String) -> Result<PathBuf, ImagePathError> {
    // The empty string is an invalid path.
    if user_path.trim().is_empty() {
        return Err(ImagePathError::Empty);
    }

    let requested_path = PathBuf::from(&user_path);

    // Reject paths containing ".." or absolute paths.
    if user_path.contains("..") || requested_path.is_absolute() {
        return Err(ImagePathError::InvalidPath);
    }

    // Join the path with the base directory.
    let full_path = base_dir.join(&requested_path);

    // Is the path a symbolic link? Reject it.
    if full_path.is_symlink() {
        return Err(ImagePathError::Symlink);
    }

    // Canonicalize the full path (validates existence).
    let canonical_full = full_path
        .canonicalize()
        .map_err(|_| ImagePathError::NotFound)?;

    // Canonicalize the base directory (should always succeed since it was validated at startup).
    let canonical_dir = base_dir
        .canonicalize()
        .map_err(|_| ImagePathError::NotFound)?;

    // Ensure the resolved path is within the base directory. This should be
    // caught by the symlink check, but nevertheless.
    if !canonical_full.starts_with(&canonical_dir) {
        return Err(ImagePathError::OutsideDirectory);
    }

    Ok(canonical_full)
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::fs::create_dir;
    use std::fs::create_dir_all;
    use std::os::unix::fs::symlink;

    use super::*;
    use crate::error::Fallible;
    use crate::helper::create_tmp_directory;

    /// Create a directory, and an image in it, and test the "normal" path to
    /// the image works.
    #[test]
    fn test_validate_image_path_valid() -> Fallible<()> {
        // Test data.
        let dir: PathBuf = create_tmp_directory()?;
        let image = dir.join("test.jpg");
        File::create(&image)?;

        // Assertions.
        let result = validate_image_path(&dir, "test.jpg".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), image.canonicalize().unwrap());
        Ok(())
    }

    /// Create a directory, a subdirectory, and an image in the subdirectory,
    /// and test that the path to the image works.
    #[test]
    fn test_validate_image_path_in_subdirectory() -> Fallible<()> {
        // Test data.
        let dir: PathBuf = create_tmp_directory()?;
        let sub_dir: PathBuf = dir.join("images");
        create_dir(&sub_dir)?;
        let image_path = sub_dir.join("photo.png");
        File::create(&image_path).unwrap();

        // Assertions.
        let result = validate_image_path(&dir, "images/photo.png".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), image_path.canonicalize().unwrap());
        Ok(())
    }

    /// Requesting a nonexistent image should return NotFound.
    #[test]
    fn test_validate_image_path_not_found() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let result = validate_image_path(&dir, "nonexistent.jpg".to_string());
        assert_eq!(result, Err(ImagePathError::NotFound));
        Ok(())
    }

    /// Paths starting with ".." should be rejected.
    #[test]
    fn test_validate_image_path_with_dot_dot() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let result = validate_image_path(&dir, "../etc/passwd".to_string());
        assert_eq!(result, Err(ImagePathError::InvalidPath));
        Ok(())
    }

    /// Paths with ".." in the middle should be rejected.
    #[test]
    fn test_validate_image_path_with_dot_dot_middle() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let result = validate_image_path(&dir, "images/../../../etc/passwd".to_string());
        assert_eq!(result, Err(ImagePathError::InvalidPath));
        Ok(())
    }

    /// Absolute paths should be rejected.
    #[test]
    fn test_validate_image_path_absolute() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let result = validate_image_path(&dir, "/etc/passwd".to_string());
        assert_eq!(result, Err(ImagePathError::InvalidPath));
        Ok(())
    }

    /// Symlinks pointing to files within the base directory should be rejected.
    #[test]
    fn test_validate_image_path_symlink_inside() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;
        let target = dir.join("target.jpg");
        File::create(&target)?;
        let link = dir.join("link.jpg");
        symlink(&target, &link)?;

        // Assertions.
        let result = validate_image_path(&dir, "link.jpg".to_string());
        assert_eq!(result, Err(ImagePathError::Symlink));
        Ok(())
    }

    /// Symlinks pointing outside the base directory should be rejected.
    #[test]
    fn test_validate_image_path_symlink_outside() -> Fallible<()> {
        // Test data.
        let dir1 = create_tmp_directory()?;
        let dir2 = create_tmp_directory()?;
        create_dir_all(&dir1)?;
        create_dir_all(&dir2)?;
        let outside_file = dir2.join("outside.txt");
        File::create(&outside_file)?;
        let link = dir1.join("evil_link.jpg");
        symlink(&outside_file, &link)?;

        // Assertions.
        let result = validate_image_path(&dir1, "evil_link.jpg".to_string());
        assert_eq!(result, Err(ImagePathError::Symlink));
        Ok(())
    }

    /// URL-encoded ".." sequences should still be caught by string check.
    #[test]
    fn test_validate_image_path_url_encoded_dot_dot() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let result = validate_image_path(&dir, "..%2F..%2Fetc%2Fpasswd".to_string());
        assert_eq!(result, Err(ImagePathError::InvalidPath));
        Ok(())
    }

    /// The empty string should be rejected.
    #[test]
    fn test_validate_image_path_empty_string() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let result = validate_image_path(&dir, "".to_string());
        assert_eq!(result, Err(ImagePathError::Empty));
        Ok(())
    }

    /// File names with spaces should be handled correctly.
    #[test]
    fn test_validate_image_path_with_spaces() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;
        let image_path = dir.join("my image.jpg");
        File::create(&image_path)?;

        // Assertions.
        let result = validate_image_path(&dir, "my image.jpg".to_string());
        assert!(result.is_ok());
        Ok(())
    }

    /// File names with Unicode characters should be handled correctly.
    #[test]
    fn test_validate_image_path_unicode() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;
        let image_path = dir.join("画像.jpg");
        File::create(&image_path)?;

        // Assertions.
        let result = validate_image_path(&dir, "画像.jpg".to_string());
        assert!(result.is_ok());
        Ok(())
    }
}
