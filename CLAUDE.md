Dear Claude: this document is to help you in your work.

# Overview

hashcards is a plain-text spaced repetition system written in Rust. It parses Markdown files containing flashcards, stores performance data in SQLite, and presents cards through a web interface using the FSRS algorithm for scheduling.

# Design and Internals

- Cards are content addressed.
- Media files are referenced in markdown using standard image syntax: `![](path/to/file.ext)`. Standard image and AV formats are supported.
- We use `pulldown-cmark` to parse/process/render Markdown.
- In `markdown.rs`: URLs are rewritten to `/file/{url}` endpoints for serving.
- In `media.rs`: Image references are extracted and validated during collection loading.
- Files are served via `/file/*path` endpoint, resolved relative to collection directory.
- Path validation (in `cmd/drill/file.rs`) prevents directory traversal attacks.

# Rules

- Use newtypes for domain concepts.
- When fixing bugs, add a failing regression test first.
- No `unwrap()` calls in production code. Tests are ok.
- Use `Fallible` and `?` for error handling.
- Use `fail()` function for creating custom errors.
- All errors are user-facing, so messages should be clear.
- Keep functions small and focused.
- Module files should re-export what's needed, hide implementation details.
- Prefer imports to fully qualified names: e.g. instead of writing `foo::bar()`, add a `use foo::bar;` statement at the top of the module.
- Don't persist changes to the database during drilling. Use the cache.
- Don't use timezones: dates are naive for a reason. Due dates etc. are more like the dates in a journal entry than precise points in time.
- When relevant, update `CHANGELOG.xml`.
- When updating this file, be terse.

# Watch Out

- Cloze deletion positions are _byte_ positions, not _character_ positions. Therefore: when working with cloze positions, always use `.bytes()` not `.chars()`.

Thank you. Good luck little buddy.
