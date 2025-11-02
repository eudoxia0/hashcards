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

/// The media resolver takes media paths as entered in the Markdown text of the
/// flashcards, and resolves them to absolute file paths on disk, based on the
/// resolution rules.
pub struct MediaResolver {
    /// Path to the collection root directory.
    pub root: PathBuf,
}

/// Errors that can occur when resolve a file path.
#[derive(Debug, PartialEq)]
pub enum ResolveError {
    /// Path is the empty string.
    Empty,
    /// Path is an external URL.
    ExternalUrl,
    /// Path is a symbolic link.
    Symlink,
    /// Path contains invalid components (e.g., "..").
    InvalidPath,
    /// Path is absolute.
    AbsolutePath,
    /// File does not exist or cannot be accessed
    NotFound,
    /// Path resolves outside the collection directory
    OutsideDirectory,
}

impl MediaResolver {
    /// Normalize a media path from markdown to a collection-relative path.
    ///
    /// This handles two types of paths:
    /// 1. Paths starting with `@/` are collection-relative (strip the `@`)
    /// 2. Other paths are deck-relative (resolve relative to deck file)
    ///
    /// The deck_path must be an absolute path to the deck file.
    ///
    /// Returns a collection-relative path string.
    pub fn normalize_path(&self, path: &str, deck_path: &Path) -> Result<String, ResolveError> {
        // The empty string is an invalid path.
        if path.trim().is_empty() {
            return Err(ResolveError::Empty);
        }

        // External URLs cannot be normalized.
        if path.contains("://") {
            return Err(ResolveError::ExternalUrl);
        }

        // Parse the string as a PathBuf.
        let path_buf = PathBuf::from(path);

        // Absolute paths are forbidden.
        if path_buf.is_absolute() {
            return Err(ResolveError::AbsolutePath);
        }

        // Check if this is a collection-relative path (starts with @)
        let collection_relative = if path.starts_with('@') {
            // Strip the @ prefix and any leading slashes
            let stripped = path.trim_start_matches('@').trim_start_matches('/');
            PathBuf::from(stripped)
        } else {
            // Deck-relative path: resolve relative to the deck file's directory
            let deck_dir = deck_path.parent().ok_or(ResolveError::InvalidPath)?;

            // Make deck_dir relative to collection root if it's absolute
            let deck_dir_relative = if deck_dir.starts_with(&self.root) {
                deck_dir
                    .strip_prefix(&self.root)
                    .map_err(|_| ResolveError::InvalidPath)?
            } else {
                deck_dir
            };

            // Join with the media path
            deck_dir_relative.join(&path_buf)
        };

        // Normalize the path (resolve .. components)
        let normalized = self.normalize_path_components(&collection_relative)?;

        // Convert to string
        normalized
            .to_str()
            .map(|s| s.to_string())
            .ok_or(ResolveError::InvalidPath)
    }

    /// Normalize path components, resolving ".." while ensuring we don't escape
    /// the collection directory.
    fn normalize_path_components(&self, path: &Path) -> Result<PathBuf, ResolveError> {
        let mut components = Vec::new();

        for component in path.components() {
            match component {
                std::path::Component::Normal(c) => components.push(c),
                std::path::Component::ParentDir => {
                    if components.is_empty() {
                        // Trying to escape the collection directory
                        return Err(ResolveError::OutsideDirectory);
                    }
                    components.pop();
                }
                std::path::Component::CurDir => {
                    // Skip "." components
                }
                _ => {
                    // Reject any other component types (Prefix, RootDir)
                    return Err(ResolveError::InvalidPath);
                }
            }
        }

        let mut result = PathBuf::new();
        for component in components {
            result.push(component);
        }
        Ok(result)
    }

    /// Resolve the given media path to an absolute file path on disk.
    ///
    /// Rules:
    ///
    /// 1. Absolute paths are forbidden.
    /// 2. Relative paths are resolved relative to the collection root
    ///    directory.
    /// 3. Paths containing ".." segments are forbidden (use normalize_path
    ///    for paths that may contain ..).
    pub fn resolve(&self, path: &str) -> Result<PathBuf, ResolveError> {
        // The empty string is an invalid path.
        if path.trim().is_empty() {
            return Err(ResolveError::Empty);
        }

        // External URLs (e.g. `http`, `https`) cannot be resolved.
        if path.contains("://") {
            return Err(ResolveError::ExternalUrl);
        }

        // Reject paths containing "..".
        if path.contains("..") {
            return Err(ResolveError::InvalidPath);
        }

        // Parse the string as a PathBuf.
        let requested_path = PathBuf::from(&path);

        // Absolute paths are forbidden.
        if requested_path.is_absolute() {
            return Err(ResolveError::AbsolutePath);
        }

        // Join the path with the base directory.
        let full_path = self.root.join(&requested_path);

        // Is the path a symbolic link? Reject it.
        if full_path.is_symlink() {
            return Err(ResolveError::Symlink);
        }

        // Canonicalize the full path (validates existence).
        let canonical_full = full_path
            .canonicalize()
            .map_err(|_| ResolveError::NotFound)?;

        // Canonicalize the base directory (should always succeed since it was
        // validated at startup).
        let canonical_dir = self
            .root
            .canonicalize()
            .map_err(|_| ResolveError::NotFound)?;

        // Ensure the resolved path is within the base directory. This should be
        // caught by the symlink check, but nevertheless.
        if !canonical_full.starts_with(&canonical_dir) {
            return Err(ResolveError::OutsideDirectory);
        }

        Ok(canonical_full)
    }

    /// Resolve a deck-relative path to an absolute file path on disk.
    ///
    /// This first normalizes the path (handling @ prefix and .. components),
    /// then resolves it to an absolute path.
    pub fn resolve_for_deck(&self, path: &str, deck_path: &Path) -> Result<PathBuf, ResolveError> {
        let normalized = self.normalize_path(path, deck_path)?;
        self.resolve(&normalized)
    }
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

    /// Test normalizing a deck-relative path.
    #[test]
    fn test_normalize_deck_relative_path() -> Fallible<()> {
        let dir = create_tmp_directory()?;
        let resolver = MediaResolver { root: dir.clone() };

        // Deck at foo/bar/deck.md with image at img/test.jpg
        let deck_path = dir.join("foo/bar/deck.md");
        let result = resolver.normalize_path("img/test.jpg", &deck_path)?;
        assert_eq!(result, "foo/bar/img/test.jpg");
        Ok(())
    }

    /// Test normalizing a collection-relative path with @ prefix.
    #[test]
    fn test_normalize_collection_relative_path() -> Fallible<()> {
        let dir = create_tmp_directory()?;
        let resolver = MediaResolver { root: dir.clone() };

        // Deck at foo/bar/deck.md with image at @/img/test.jpg
        let deck_path = dir.join("foo/bar/deck.md");
        let result = resolver.normalize_path("@/img/test.jpg", &deck_path)?;
        assert_eq!(result, "img/test.jpg");
        Ok(())
    }

    /// Test normalizing a path with .. that stays within bounds.
    #[test]
    fn test_normalize_path_with_parent_dir() -> Fallible<()> {
        let dir = create_tmp_directory()?;
        let resolver = MediaResolver { root: dir.clone() };

        // Deck at foo/bar/deck.md with image at ../img/test.jpg
        let deck_path = dir.join("foo/bar/deck.md");
        let result = resolver.normalize_path("../img/test.jpg", &deck_path)?;
        assert_eq!(result, "foo/img/test.jpg");
        Ok(())
    }

    /// Test normalizing a path with .. that tries to escape.
    #[test]
    fn test_normalize_path_escaping() -> Fallible<()> {
        let dir = create_tmp_directory()?;
        let resolver = MediaResolver { root: dir.clone() };

        // Deck at deck.md with image at ../../../etc/passwd
        let deck_path = dir.join("deck.md");
        let result = resolver.normalize_path("../../../etc/passwd", &deck_path);
        assert_eq!(result, Err(ResolveError::OutsideDirectory));
        Ok(())
    }

    /// Test resolve_for_deck with an existing file.
    #[test]
    fn test_resolve_for_deck_valid() -> Fallible<()> {
        let dir = create_tmp_directory()?;
        let sub_dir = dir.join("foo/bar");
        create_dir_all(&sub_dir)?;
        let img_dir = sub_dir.join("img");
        create_dir(&img_dir)?;
        let image_path = img_dir.join("test.jpg");
        File::create(&image_path)?;

        let resolver = MediaResolver { root: dir.clone() };
        let deck_path = sub_dir.join("deck.md");
        let result = resolver.resolve_for_deck("img/test.jpg", &deck_path)?;
        assert_eq!(result, image_path.canonicalize()?);
        Ok(())
    }

    /// Test resolve_for_deck with @ prefix.
    #[test]
    fn test_resolve_for_deck_collection_relative() -> Fallible<()> {
        let dir = create_tmp_directory()?;
        let deck_dir = dir.join("foo/bar");
        create_dir_all(&deck_dir)?;
        let img_dir = dir.join("img");
        create_dir(&img_dir)?;
        let image_path = img_dir.join("test.jpg");
        File::create(&image_path)?;

        let resolver = MediaResolver { root: dir.clone() };
        let deck_path = deck_dir.join("deck.md");
        let result = resolver.resolve_for_deck("@/img/test.jpg", &deck_path)?;
        assert_eq!(result, image_path.canonicalize()?);
        Ok(())
    }

    /// Create a directory, and an image in it, and test the "normal" path to
    /// the image works.
    #[test]
    fn test_validate_file_path_valid() -> Fallible<()> {
        // Test data.
        let dir: PathBuf = create_tmp_directory()?;
        let image = dir.join("test.jpg");
        File::create(&image)?;

        // Assertions.
        let resolver = MediaResolver { root: dir.clone() };
        let result = resolver.resolve("test.jpg");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), image.canonicalize().unwrap());
        Ok(())
    }

    /// Create a directory, a subdirectory, and an image in the subdirectory,
    /// and test that the path to the image works.
    #[test]
    fn test_validate_file_path_in_subdirectory() -> Fallible<()> {
        // Test data.
        let dir: PathBuf = create_tmp_directory()?;
        let sub_dir: PathBuf = dir.join("images");
        create_dir(&sub_dir)?;
        let image_path = sub_dir.join("photo.png");
        File::create(&image_path).unwrap();

        // Assertions.
        let resolver = MediaResolver { root: dir.clone() };
        let result = resolver.resolve("images/photo.png");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), image_path.canonicalize().unwrap());
        Ok(())
    }

    /// Requesting a nonexistent image should return NotFound.
    #[test]
    fn test_validate_file_path_not_found() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let resolver = MediaResolver { root: dir.clone() };
        let result = resolver.resolve("nonexistent.jpg");
        assert_eq!(result, Err(ResolveError::NotFound));
        Ok(())
    }

    /// Paths starting with ".." should be rejected.
    #[test]
    fn test_validate_file_path_with_dot_dot() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let resolver = MediaResolver { root: dir.clone() };
        let result = resolver.resolve("../etc/passwd");
        assert_eq!(result, Err(ResolveError::InvalidPath));
        Ok(())
    }

    /// Paths with ".." in the middle should be rejected.
    #[test]
    fn test_validate_file_path_with_dot_dot_middle() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let resolver = MediaResolver { root: dir.clone() };
        let result = resolver.resolve("images/../../../etc/passwd");
        assert_eq!(result, Err(ResolveError::InvalidPath));
        Ok(())
    }

    /// Absolute paths should be rejected.
    #[test]
    fn test_validate_file_path_absolute() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let resolver = MediaResolver { root: dir.clone() };
        let result = resolver.resolve("/etc/passwd");
        assert_eq!(result, Err(ResolveError::AbsolutePath));
        Ok(())
    }

    /// Symlinks pointing to files within the base directory should be rejected.
    #[test]
    fn test_validate_file_path_symlink_inside() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;
        let target = dir.join("target.jpg");
        File::create(&target)?;
        let link = dir.join("link.jpg");
        symlink(&target, &link)?;

        // Assertions.
        let resolver = MediaResolver { root: dir.clone() };
        let result = resolver.resolve("link.jpg");
        assert_eq!(result, Err(ResolveError::Symlink));
        Ok(())
    }

    /// Symlinks pointing outside the base directory should be rejected.
    #[test]
    fn test_validate_file_path_symlink_outside() -> Fallible<()> {
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
        let resolver = MediaResolver { root: dir1.clone() };
        let result = resolver.resolve("evil_link.jpg");
        assert_eq!(result, Err(ResolveError::Symlink));
        Ok(())
    }

    /// URL-encoded ".." sequences should still be caught by string check.
    #[test]
    fn test_validate_file_path_url_encoded_dot_dot() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let resolver = MediaResolver { root: dir.clone() };
        let result = resolver.resolve("..%2F..%2Fetc%2Fpasswd");
        assert_eq!(result, Err(ResolveError::InvalidPath));
        Ok(())
    }

    /// The empty string should be rejected.
    #[test]
    fn test_validate_file_path_empty_string() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;

        // Assertions.
        let resolver = MediaResolver { root: dir.clone() };
        let result = resolver.resolve("");
        assert_eq!(result, Err(ResolveError::Empty));
        Ok(())
    }

    /// File names with spaces should be handled correctly.
    #[test]
    fn test_validate_file_path_with_spaces() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;
        let image_path = dir.join("my image.jpg");
        File::create(&image_path)?;

        // Assertions.
        let resolver = MediaResolver { root: dir.clone() };
        let result = resolver.resolve("my image.jpg");
        assert!(result.is_ok());
        Ok(())
    }

    /// File names with Unicode characters should be handled correctly.
    #[test]
    fn test_validate_file_path_unicode() -> Fallible<()> {
        // Test data.
        let dir = create_tmp_directory()?;
        let image_path = dir.join("画像.jpg");
        File::create(&image_path)?;

        // Assertions.
        let resolver = MediaResolver { root: dir.clone() };
        let result = resolver.resolve("画像.jpg");
        assert!(result.is_ok());
        Ok(())
    }
}
