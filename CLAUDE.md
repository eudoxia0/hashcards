This document describes the design patterns, coding conventions, and architectural decisions in the hashcards codebase.

# Overview

hashcards is a plain-text spaced repetition system written in Rust. It parses Markdown files containing flashcards, stores performance data in SQLite, and presents cards through a web interface using the FSRS algorithm for scheduling.

# Design Patterns

## Content-Addressable Cards

Cards are identified by the BLAKE3 hash of their content. This means:

- Editing a card resets its progress (new hash = new card)
- Duplicate cards across files are automatically deduplicated
- No need for explicit card IDs, which is ideal since one of the design goals is low-friction card entry.

## Newtype Wrappers for Domain Types

Most important domain objects are wrapped in newtype structs.

When adding new domain concepts, wrap them in newtypes even if they seem trivial.

## Error Handling

Custom `ErrorReport` type and an alias of the `Result` type:

```rust
pub type Fallible<T> = Result<T, ErrorReport>;

```

**Patterns:**

- Use `?` operator freely for error propagation
- Use `fail()` function for creating custom errors
- All errors are user-facing, so messages should be clear

# Implementation Details

## Byte-Level Processing for Cloze Deletions

Cloze deletion positions are stored as **byte offsets**, not character offsets:

```rust
CardContent::Cloze {
    text: String,      // Text without brackets
    start: usize,      // Byte position
    end: usize,        // Byte position
}
```

**Rationale:** Bytes are concrete and well-defined; Unicode "characters" are ambiguous. This keeps the implementation tractable.

**Pattern:** When working with cloze positions, always use `.bytes()` not `.chars()`.

# Web Interface

The drill interface (`cmd/drill/`) is built with:

- **Axum** for the web server
- **Maud** for HTML templating (Rust macros that generate HTML)
- Plain CSS and JavaScript (no frameworks)

**Pattern:** HTML is generated server-side. The JavaScript is minimal (form submissions).

## Server State

```rust
pub struct ServerState {
    pub port: u16,
    pub directory: PathBuf,
    pub macros: Vec<(String, String)>,
    pub total_cards: usize,
    pub session_started_at: Timestamp,
    pub mutable: Arc<Mutex<MutableState>>,
}
```

**Pattern:** Use `Arc<Mutex<MutableState>>` for state that changes during a session.

## Card Sorting

Cards are sorted by hash before drilling:

```rust
all_cards.sort_by_key(|c| c.hash());
```

**Rationale:** Deterministic but appears random to the user. Mixes cards from different decks without needing an RNG.

## Hash-Based Deduplication

Cards are deduplicated in two places:
- Within a file during parsing
- Across all files after parsing

## Timestamp Handling

- All timestamps stored in UTC (`Timestamp` wraps `DateTime<Utc>`).
- Converted to local time only for display/due date calculation.

## Media Files (Images and Audio)

Media files are referenced in markdown using standard image syntax: `![](path/to/file.ext)`

**Supported formats:**
- Images: PNG, JPG, GIF, SVG, WEBP
- Audio: MP3, WAV, OGG (auto-detected and rendered as HTML5 `<audio>` elements)
- Video: MP4, WEBM

**Processing:**
- Markdown is parsed using `pulldown-cmark` library
- In `markdown.rs`: URLs are rewritten to `/file/{url}` endpoints for serving
- In `media.rs`: Image references are extracted and validated during collection loading
- Files are served via `/file/*path` endpoint, resolved relative to collection directory
- Path validation (in `cmd/drill/file.rs`) prevents directory traversal attacks

**Validation:**
- `Collection::new()` calls `validate_media_files()` to check all referenced files exist
- Both `drill` and `check` commands validate media on startup
- External URLs (containing `://`) are skipped during validation

## TeX Macros

LaTeX macros can be defined in `macros.tex` in the collection root:

```
\command definition
```

e.g.:

```
\foo \text{foo}
```

These are passed on to KaTeX for rendering.

# Pattern: Adding a New Database Field

- Update schema in `schema.sql`.
- Update relevant methods in the `Database` struct.
- Update tests.

# Pattern: Adding a New CLI Command

1. Add variant to `Command` enum in `cli.rs`.
2. Create new module in `cmd/`.
3. Implement command logic.
4. Add to `entrypoint()` match statement.
5. Add tests if possible.

# Pattern: Adding Collection-Wide Validation

If you need to validate something about all cards or the collection as a whole:

1. Add validation function to appropriate module (e.g., `media.rs`)
2. Call it from `Collection::new()` in `collection.rs`
3. This ensures both `drill` and `check` commands validate automatically

**Why:** Both commands use `Collection::new()` as their entry point, so validation added there runs for both.

# Testing Strategy

- Unit tests for individual functions/methods. E2E tests simulate a full drilling session via HTTP requests.
- When fixing bugs, add a failing regression test first.

## Testing with Parsed Cards

When testing functionality that operates on cards:

```rust
use crate::parser::Parser;
use std::env::temp_dir;

let parser = Parser::new("test_deck".to_string(), PathBuf::from("test.md"));
let cards = parser.parse(markdown).expect("Failed to parse");
// Now test your functionality with real parsed cards
```

This pattern tests the full pipeline from markdown → parsed cards → your feature.

# Things to Avoid

- **Don't use character positions for cloze deletions** - always use byte positions
- **Don't write to the database during drilling** - use the cache
- **Don't expose internal hash representations** - use `.to_hex()` for display
- **Don't skip foreign key checks** - they're enabled for a reason
- **Don't mix local and UTC times** - convert only at boundaries

# Code Quality

- No unwrap() calls in production code (except in tests where failure is okay)
- Use `?` for error propagation
- Prefer explicit types over inference for public APIs
- Keep functions small and focused
- Module files should re-export what's needed, hide implementation details
