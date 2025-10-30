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

use crate::error::ErrorReport;
use crate::error::Fallible;
use crate::error::fail;

/// Resolves a media file path according to hashcards path resolution rules:
/// - Paths starting with "@/" are resolved relative to the collection root (with @ stripped)
/// - All other paths are resolved relative to the deck file's directory
///
/// Returns a path relative to the collection root, suitable for use in URLs and file serving.
pub fn resolve_media_path(
    deck_file_path: &Path,
    collection_root: &Path,
    media_path: &str,
) -> Fallible<PathBuf> {
    if media_path.is_empty() {
        return fail("Media path cannot be empty");
    }

    // Normalize the paths
    let collection_root = collection_root
        .canonicalize()
        .map_err(|_| ErrorReport::new("Failed to canonicalize collection root"))?;

    let deck_file_path = deck_file_path
        .canonicalize()
        .map_err(|_| ErrorReport::new("Failed to canonicalize deck file path"))?;

    let deck_dir = deck_file_path
        .parent()
        .ok_or_else(|| ErrorReport::new("Deck file has no parent directory"))?;

    // Handle collection-relative paths (starting with @/)
    let absolute_path = if let Some(relative_path) = media_path.strip_prefix("@/") {
        // Strip the @ and resolve relative to collection root
        collection_root.join(relative_path)
    } else {
        // Resolve relative to deck file's directory
        deck_dir.join(media_path)
    };

    // Canonicalize to resolve .. and . components
    let canonical_path = absolute_path
        .canonicalize()
        .map_err(|_| ErrorReport::new(format!("Failed to resolve media path: {media_path}")))?;

    // Ensure the resolved path is within the collection root
    if !canonical_path.starts_with(&collection_root) {
        return fail(format!(
            "Media path '{media_path}' resolves outside the collection directory"
        ));
    }

    // Return the path relative to collection root
    canonical_path
        .strip_prefix(&collection_root)
        .map(|p| p.to_path_buf())
        .map_err(|_| ErrorReport::new("Failed to strip collection root prefix"))
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::fs::create_dir_all;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_resolve_media_path_deck_relative() {
        // Create a temporary directory structure:
        // collection/
        //   subdir/
        //     deck.md
        //     image.png
        let temp_dir = tempdir().unwrap();
        let collection_root = temp_dir.path();
        let subdir = collection_root.join("subdir");
        create_dir_all(&subdir).unwrap();

        let deck_file = subdir.join("deck.md");
        File::create(&deck_file).unwrap();
        let image_file = subdir.join("image.png");
        File::create(&image_file).unwrap();

        // Deck-relative path should resolve to subdir/image.png
        let resolved = resolve_media_path(&deck_file, collection_root, "image.png").unwrap();
        assert_eq!(resolved, PathBuf::from("subdir/image.png"));
    }

    #[test]
    fn test_resolve_media_path_deck_relative_with_parent() {
        // Create a temporary directory structure:
        // collection/
        //   images/
        //     photo.jpg
        //   decks/
        //     chapter1.md
        let temp_dir = tempdir().unwrap();
        let collection_root = temp_dir.path();
        let images_dir = collection_root.join("images");
        let decks_dir = collection_root.join("decks");
        create_dir_all(&images_dir).unwrap();
        create_dir_all(&decks_dir).unwrap();

        let deck_file = decks_dir.join("chapter1.md");
        File::create(&deck_file).unwrap();
        let image_file = images_dir.join("photo.jpg");
        File::create(&image_file).unwrap();

        // Deck-relative path with .. should resolve to images/photo.jpg
        let resolved =
            resolve_media_path(&deck_file, collection_root, "../images/photo.jpg").unwrap();
        assert_eq!(resolved, PathBuf::from("images/photo.jpg"));
    }

    #[test]
    fn test_resolve_media_path_collection_relative() {
        // Create a temporary directory structure:
        // collection/
        //   shared/
        //     logo.png
        //   decks/
        //     subdeck/
        //       notes.md
        let temp_dir = tempdir().unwrap();
        let collection_root = temp_dir.path();
        let shared_dir = collection_root.join("shared");
        let subdeck_dir = collection_root.join("decks/subdeck");
        create_dir_all(&shared_dir).unwrap();
        create_dir_all(&subdeck_dir).unwrap();

        let deck_file = subdeck_dir.join("notes.md");
        File::create(&deck_file).unwrap();
        let image_file = shared_dir.join("logo.png");
        File::create(&image_file).unwrap();

        // Collection-relative path (with @/) should resolve to shared/logo.png
        let resolved =
            resolve_media_path(&deck_file, collection_root, "@/shared/logo.png").unwrap();
        assert_eq!(resolved, PathBuf::from("shared/logo.png"));
    }

    #[test]
    fn test_resolve_media_path_escapes_collection() {
        // Create a temporary directory structure with two separate directories
        let temp_dir = tempdir().unwrap();
        let collection_root = temp_dir.path().join("collection");
        create_dir_all(&collection_root).unwrap();
        let deck_file = collection_root.join("deck.md");
        File::create(&deck_file).unwrap();

        // Try to escape the collection directory
        let result = resolve_media_path(&deck_file, &collection_root, "../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_media_path_nested_subdirectories() {
        // Create a deeply nested structure:
        // collection/
        //   biology/
        //     cell/
        //       images/
        //         mitochondria.png
        //       notes.md
        let temp_dir = tempdir().unwrap();
        let collection_root = temp_dir.path();
        let images_dir = collection_root.join("biology/cell/images");
        let notes_dir = collection_root.join("biology/cell");
        create_dir_all(&images_dir).unwrap();

        let deck_file = notes_dir.join("notes.md");
        File::create(&deck_file).unwrap();
        let image_file = images_dir.join("mitochondria.png");
        File::create(&image_file).unwrap();

        // Deck-relative path
        let resolved =
            resolve_media_path(&deck_file, collection_root, "images/mitochondria.png").unwrap();
        assert_eq!(
            resolved,
            PathBuf::from("biology/cell/images/mitochondria.png")
        );
    }

    #[test]
    fn test_resolve_media_path_empty_path() {
        let temp_dir = tempdir().unwrap();
        let collection_root = temp_dir.path();
        let deck_file = collection_root.join("deck.md");
        File::create(&deck_file).unwrap();

        let result = resolve_media_path(&deck_file, collection_root, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_media_path_missing_file() {
        let temp_dir = tempdir().unwrap();
        let collection_root = temp_dir.path();
        let deck_file = collection_root.join("deck.md");
        File::create(&deck_file).unwrap();

        // Try to resolve a path to a file that doesn't exist
        let result = resolve_media_path(&deck_file, collection_root, "nonexistent.png");
        assert!(result.is_err());
    }
}
