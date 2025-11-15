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
    pub fn resolve(self, path: &str) -> Result<PathBuf, ResolveError> {
        // Trim the path.
        let path: &str = path.trim();
        // Is the path empty?
        if path.is_empty() {
            return Err(ResolveError::Empty);
        }
        // Is the path an external URL?
        if path.contains("://") {
            return Err(ResolveError::ExternalUrl);
        }
        todo!()
    }
}

impl MediaResolverBuilder {
    pub fn new() -> Self {
        Self {
            collection_path: None,
            deck_path: None,
        }
    }

    pub fn with_collection_path(self, collection_path: PathBuf) -> Self {
        assert!(collection_path.is_dir());
        assert!(collection_path.is_absolute());
        Self {
            collection_path: Some(collection_path),
            deck_path: self.deck_path,
        }
    }

    pub fn with_deck_path(self, deck_path: PathBuf) -> Self {
        assert!(deck_path.is_dir());
        assert!(deck_path.is_relative());
        Self {
            collection_path: self.collection_path,
            deck_path: Some(deck_path),
        }
    }

    pub fn build(self) -> MediaResolver {
        let collection_path = self.collection_path.unwrap();
        let deck_path = self.deck_path.unwrap();
        MediaResolver {
            collection_path,
            deck_path,
        }
    }
}
