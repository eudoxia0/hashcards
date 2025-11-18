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
use std::path::Path;
use std::path::PathBuf;

use crate::error::ErrorReport;
use crate::error::Fallible;

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
    /// Path has parent (`..`) components.
    ParentComponent,
    /// Path is outside the collection directory.
    OutsideCollection,
    /// Path is invalid.
    InvalidPath,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            ResolveError::Empty => "path is the empty string.",
            ResolveError::ExternalUrl => "external URLs are not allowed as media paths",
            ResolveError::AbsolutePath => "absolute paths are not allowed as media paths",
            ResolveError::ParentComponent => "path has a parent component",
            ResolveError::OutsideCollection => "path is outside the collection directory",
            ResolveError::InvalidPath => "path is invalid",
        };
        write!(f, "{msg}")
    }
}

impl MediaResolver {
    /// Resolve a path string to a collection-relative file path.
    ///
    /// If the path string starts with `@/`, it will be resolved relative to
    /// the collection root directory.
    ///
    /// If the path string is a relative path, it will be resolved relative to
    /// the deck path. For deck-relative paths, parent (`..`) components are
    /// permitted.
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
            // Reject paths with `..` components.
            if path.components().any(|c| c == Component::ParentDir) {
                return Err(ResolveError::ParentComponent);
            }
            // Check: does it exist? This is done for symmetry with the other
            // branch.
            let abspath: PathBuf = self.collection_path.join(&path);
            if !abspath.exists() {
                return Err(ResolveError::InvalidPath);
            }
            Ok(path)
        } else {
            // Path is deck-relative.
            let path: PathBuf = PathBuf::from(&path);
            if path.is_absolute() {
                return Err(ResolveError::AbsolutePath);
            }
            // Join the collection path and the deck path to get the absolute
            // path to the deck file.
            let deck: PathBuf = self.collection_path.join(&self.deck_path);
            // Get the path of the directory that contains the deck.
            let deck_dir: &Path = deck.parent().ok_or(ResolveError::InvalidPath)?;
            // Join the deck directory path with the file path, to get an
            // absolute path to the deck-relative file.
            let path: PathBuf = deck_dir.join(path);
            // Check: does the file exist?
            if !path.exists() {
                return Err(ResolveError::InvalidPath);
            }
            // Canonicalize the path to resolve `..` components and symbolic
            // links.
            let path: PathBuf = path.canonicalize().map_err(|_| ResolveError::InvalidPath)?;
            // Relativize the path by subtracting the collection root.
            let path: PathBuf = path
                .strip_prefix(&self.collection_path)
                // The only case where `strip_prefix` can fail is where the path
                // does not start with the prefix, i.e., `path` points outside the
                // collection directory.
                .map_err(|_| ResolveError::OutsideCollection)?
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
    pub fn with_collection_path(self, collection_path: PathBuf) -> Fallible<Self> {
        if !collection_path.exists() {
            return Err(ErrorReport::new("Collection path does not exist."));
        }
        if !collection_path.is_absolute() {
            return Err(ErrorReport::new("Collection path is relative."));
        }
        if !collection_path.is_dir() {
            return Err(ErrorReport::new("Collection path is not a directory."));
        }
        Ok(Self {
            collection_path: Some(collection_path),
            deck_path: self.deck_path,
        })
    }

    /// Set a value for `deck_path`.
    pub fn with_deck_path(self, deck_path: PathBuf) -> Fallible<Self> {
        if !deck_path.is_relative() {
            return Err(ErrorReport::new("Deck path is not relative."));
        }
        Ok(Self {
            collection_path: self.collection_path,
            deck_path: Some(deck_path),
        })
    }

    /// Consume the builder and return a [`MediaResolver`].
    pub fn build(self) -> Fallible<MediaResolver> {
        let collection_path = self
            .collection_path
            .ok_or(ErrorReport::new("Missing collection_path."))?;
        let deck_path = self
            .deck_path
            .ok_or(ErrorReport::new("Missing deck_path."))?;
        Ok(MediaResolver {
            collection_path,
            deck_path,
        })
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
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?;
        assert_eq!(r.resolve(""), Err(ResolveError::Empty));
        assert_eq!(r.resolve(" "), Err(ResolveError::Empty));
        Ok(())
    }

    /// Absolute strings are rejected.
    #[test]
    fn test_absolute_paths_are_rejected() -> Fallible<()> {
        let coll_path: PathBuf = create_tmp_directory()?;
        let deck_path: PathBuf = PathBuf::from("deck.md");
        let r: MediaResolver = MediaResolverBuilder::new()
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?;
        assert_eq!(r.resolve("/etc/passwd"), Err(ResolveError::AbsolutePath));
        Ok(())
    }

    /// External URLs are rejected.
    #[test]
    fn test_external_urls_are_rejected() -> Fallible<()> {
        let coll_path: PathBuf = create_tmp_directory()?;
        let deck_path: PathBuf = PathBuf::from("deck.md");
        let r: MediaResolver = MediaResolverBuilder::new()
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?;
        assert_eq!(r.resolve("http://"), Err(ResolveError::ExternalUrl));
        Ok(())
    }

    /// Test collection-relative paths.
    #[test]
    fn test_collection_relative() -> Fallible<()> {
        let coll_path: PathBuf = create_tmp_directory()?;
        std::fs::create_dir_all(coll_path.join("a/b/"))?;
        std::fs::write(coll_path.join("foo.jpg"), "")?;
        std::fs::write(coll_path.join("a/foo.jpg"), "")?;
        std::fs::write(coll_path.join("a/b/foo.jpg"), "")?;
        let deck_path: PathBuf = PathBuf::from("deck.md");
        std::fs::write(coll_path.join("deck.md"), "")?;
        let r: MediaResolver = MediaResolverBuilder::new()
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?;
        assert_eq!(r.resolve("@/foo.jpg"), Ok(PathBuf::from("foo.jpg")));
        assert_eq!(r.resolve("@/a/foo.jpg"), Ok(PathBuf::from("a/foo.jpg")));
        assert_eq!(r.resolve("@/a/b/foo.jpg"), Ok(PathBuf::from("a/b/foo.jpg")));
        Ok(())
    }

    /// Collection-relative absolute paths are rejected.
    #[test]
    fn test_collection_relative_absolute_are_rejected() -> Fallible<()> {
        let coll_path: PathBuf = create_tmp_directory()?;
        let deck_path: PathBuf = PathBuf::from("deck.md");
        let r: MediaResolver = MediaResolverBuilder::new()
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?;
        assert_eq!(r.resolve("@//foo.jpg"), Err(ResolveError::AbsolutePath));
        Ok(())
    }

    /// Collection-relative paths with `..` components are rejected.
    #[test]
    fn test_collection_relative_parent_are_rejected() -> Fallible<()> {
        let coll_path: PathBuf = create_tmp_directory()?;
        let deck_path: PathBuf = PathBuf::from("deck.md");
        let r: MediaResolver = MediaResolverBuilder::new()
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?;
        assert_eq!(
            r.resolve("@/a/b/../foo.jpg"),
            Err(ResolveError::ParentComponent)
        );
        Ok(())
    }

    /// Test deck-relative paths.
    #[test]
    fn test_deck_relative() -> Fallible<()> {
        let coll_path: PathBuf = create_tmp_directory()?;
        let deck_path: PathBuf = PathBuf::from("a/b/c/deck.md");
        std::fs::create_dir_all(coll_path.join("a/b/c"))?;
        std::fs::write(coll_path.join("a/b/c/foo.jpg"), "")?;
        let r: MediaResolver = MediaResolverBuilder::new()
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?;
        assert_eq!(r.resolve("foo.jpg"), Ok(PathBuf::from("a/b/c/foo.jpg")));
        assert_eq!(r.resolve("./foo.jpg"), Ok(PathBuf::from("a/b/c/foo.jpg")));
        assert_eq!(
            r.resolve("../c/foo.jpg"),
            Ok(PathBuf::from("a/b/c/foo.jpg"))
        );
        assert_eq!(
            r.resolve("../../b/c/foo.jpg"),
            Ok(PathBuf::from("a/b/c/foo.jpg"))
        );
        assert_eq!(
            r.resolve("../c/../c/foo.jpg"),
            Ok(PathBuf::from("a/b/c/foo.jpg"))
        );
        assert_eq!(
            r.resolve("../../../a/b/c/foo.jpg"),
            Ok(PathBuf::from("a/b/c/foo.jpg"))
        );
        Ok(())
    }

    /// Ensure deck-relative paths cannot leave the collection root directory.
    #[test]
    fn test_relative_paths_cant_leave_collection_root() -> Fallible<()> {
        let coll_path: PathBuf = create_tmp_directory()?;
        let deck_path: PathBuf = PathBuf::from("deck.md");
        let r: MediaResolver = MediaResolverBuilder::new()
            .with_collection_path(coll_path)?
            .with_deck_path(deck_path)?
            .build()?;
        assert_eq!(
            r.resolve("../../../../../../../../etc/passwd"),
            Err(ResolveError::InvalidPath)
        );
        Ok(())
    }
}
