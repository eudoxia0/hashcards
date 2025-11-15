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

/// The media resolver takes media paths as entered in the Markdown text of the
/// flashcards, and resolves them to collection-relative paths.
pub struct MediaResolver {
    /// Absolute path to the collection root directory.
    collection_path: PathBuf,
    /// Collection-relative path to the deck. The resolver must only be used
    /// with flashcards parsed from this deck.
    deck_path: PathBuf,
}

/// Builder to construct a [`MediaResolver`].
pub struct MediaResolverBuilder {
    collection_path: Option<PathBuf>,
    deck_path: Option<PathBuf>,
}

/// Errors that can occur when resolving a file path.
#[derive(Debug, PartialEq)]
pub enum ResolveError {
    /// Path is the empty string.
    Empty,
    /// Path is an external URL.
    ExternalUrl,
    /// Path is absolute.
    AbsolutePath,
    /// Path is invalid.
    InvalidPath,
    /// Path resolves outside the collection directory.
    OutsideDirectory,
}

impl MediaResolver {
    /// Resolve a path string to a collection-relative file path.
    ///
    /// If the path string starts with `@/`, it will be resolved relative to
    /// the collection root directory.
    ///
    /// If the path string is a relative path, it will be resolved relative to
    /// the deck path.
    pub fn resolve(&self, path: &str) -> Result<PathBuf, ResolveError> {
        // Trim the path.
        let path: &str = path.trim();

        // Reject the empty string.
        if path.is_empty() {
            return Err(ResolveError::Empty);
        }

        // Reject external URLs.
        if path.contains("://") {
            return Err(ResolveError::ExternalUrl);
        }

        if let Some(stripped) = path.strip_prefix("@/") {
            // Path is collection-relative, leave it as-is.
            let path: PathBuf = PathBuf::from(&stripped);
            // Reject absolute paths.
            if path.is_absolute() {
                return Err(ResolveError::AbsolutePath);
            }
            Ok(path)
        } else {
            // Path is deck-relative.
            let path: PathBuf = PathBuf::from(&path);
            // Join the deck path and the file path, and canonicalize them to
            // eliminate `..` components.
            let path: PathBuf = self
                .deck_path
                .join(path)
                .canonicalize()
                .map_err(|_| ResolveError::InvalidPath)?;
            // Relativize the path by subtracting the collection directory.
            let path: PathBuf = path
                .strip_prefix(&self.collection_path)
                .map_err(|_| ResolveError::InvalidPath)?
                .to_path_buf();
            Ok(path)
        }
    }
}

impl MediaResolverBuilder {
    /// Construct a new [`MediaResolverBuilder`].
    pub fn new() -> Self {
        Self {
            collection_path: None,
            deck_path: None,
        }
    }

    /// Set a value for `collection_path`.
    pub fn with_collection_path(self, collection_path: PathBuf) -> Self {
        assert!(collection_path.is_absolute());
        assert!(collection_path.is_dir());
        Self {
            collection_path: Some(collection_path),
            deck_path: self.deck_path,
        }
    }

    /// Set a value for `deck_path`.
    pub fn with_deck_path(self, deck_path: PathBuf) -> Self {
        assert!(deck_path.is_relative());
        Self {
            collection_path: self.collection_path,
            deck_path: Some(deck_path),
        }
    }

    /// Consume the builder and return a [`MediaResolver`].
    pub fn build(self) -> MediaResolver {
        let collection_path = self.collection_path.unwrap();
        let deck_path = self.deck_path.unwrap();
        MediaResolver {
            collection_path,
            deck_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Fallible;
    use crate::helper::create_tmp_directory;

    /// Empty strings are rejected.
    #[test]
    fn test_empty_strings_are_rejected() -> Fallible<()> {
        let coll_path: PathBuf = create_tmp_directory()?;
        let deck_path: PathBuf = PathBuf::from("deck.md");
        let r: MediaResolver = MediaResolverBuilder::new()
            .with_collection_path(coll_path)
            .with_deck_path(deck_path)
            .build();
        assert_eq!(r.resolve(""), Err(ResolveError::Empty));
        assert_eq!(r.resolve(" "), Err(ResolveError::Empty));
        Ok(())
    }

    /// External URLs are rejected.
    #[test]
    fn test_external_urls_are_rejected() -> Fallible<()> {
        let coll_path: PathBuf = create_tmp_directory()?;
        let deck_path: PathBuf = PathBuf::from("deck.md");
        let r: MediaResolver = MediaResolverBuilder::new()
            .with_collection_path(coll_path)
            .with_deck_path(deck_path)
            .build();
        assert_eq!(r.resolve("http://"), Err(ResolveError::ExternalUrl));
        Ok(())
    }

    /// Test collection-relative paths.
    #[test]
    fn test_collection_relative() -> Fallible<()> {
        let coll_path: PathBuf = create_tmp_directory()?;
        std::fs::create_dir_all(coll_path.join("a/b/c"))?;
        let deck_path: PathBuf = PathBuf::from("a/b/c/deck.md");
        let r: MediaResolver = MediaResolverBuilder::new()
            .with_collection_path(coll_path)
            .with_deck_path(deck_path)
            .build();
        assert_eq!(r.resolve("@/foo.jpg").unwrap(), PathBuf::from("foo.jpg"));
        Ok(())
    }
}
