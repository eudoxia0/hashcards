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
pub struct MediaResolverBuilder {}
