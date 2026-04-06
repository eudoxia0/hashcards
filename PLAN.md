# Commute UX Improvement Plan

Improvements to make hashcards work better for short mobile study sessions.

---

## 1. Button / visual consistency

**Problem:** "Drill" on the landing page is an `<a>` tag (`.drill-link`, 14px, `padding: 6px 16px`). "Drill (N due)" on the browse page is an `<input type="submit">` (`.drill-button`, 16px, `padding: 10px 24px`). The sync/manage buttons also differ in size and color from each other. The visual language is inconsistent.

**Fix:**
- Extract a shared `.btn`, `.btn-primary`, `.btn-secondary` family in `style.css`.
- Apply it consistently to: Drill link (landing), Drill submit (browse), Sync Now, Manage HedgeDoc, Home, Shutdown.
- Touch targets: all buttons ≥ 44px height on mobile (already done for controls; extend to landing/browse).

**Files:** `src/cmd/drill/style.css`, `src/cmd/serve/landing.rs`, `src/cmd/serve/browse.rs`, `src/cmd/drill/template.rs`

---

## 2. Session persistence (incremental DB writes)

**Problem:** Reviews are batched in an in-memory cache and only flushed to SQLite when `finish_session()` is called at session end. If the browser tab closes, the phone locks, or the connection drops mid-session, all progress is lost.

**Fix:** Write each review to the database immediately when a grade action is processed, rather than at session end. `finish_session()` then becomes a lightweight cleanup (mark session complete, clear cache).

**Design notes:**
- In `post.rs::handle_action()`, after computing the updated FSRS state for a graded card, write it to `db` immediately via `db.insert_review(...)`.
- Keep the in-memory `PerformanceCache` for within-session lookups (undo, re-scheduling repeats) but treat the DB as the source of truth.
- Undo: mark the last-written review as voided (add a `voided` boolean to the reviews table) rather than deleting it. The cache already tracks undo state; the DB just needs to reflect it.
- Session record: still written at end, but individual card reviews are safe throughout.

**Files:** `src/cmd/drill/post.rs`, `src/cmd/drill/state.rs`, `src/db.rs`, `src/schema.sql`

---

## 3. Quick session sizing

**Problem:** No way to say "give me 10 cards." The server supports `--card-limit` as a CLI flag only.

**Fix:** Add an optional `limit` query parameter to `POST /collection/{slug}/start`. Expose this on the browse page as a small select or number input ("Limit: [ 10 | 20 | 50 | All ]") next to the Drill button.

**Design notes:**
- The `StartParams` form struct (in `src/cmd/serve/handlers.rs`) gains an optional `limit: Option<usize>` field.
- If set, it overrides (or caps) the server-configured `--card-limit`.
- UI: a `<select name="limit">` with options `10 / 20 / 50 / 0 (all)`, defaulting to `0`. Keep it compact — single row with the existing Drill button.
- On mobile, this makes it easy to start a 10-card session while waiting for a bus.

**Files:** `src/cmd/serve/handlers.rs`, `src/cmd/serve/browse.rs`, `src/cmd/drill/style.css`

---

## 4. Offline support (PWA)

**Problem:** The app requires an active server connection. Static assets re-download on every visit; there is no "add to home screen" support.

**Realistic scope (two phases):**

### Phase A — PWA shell (low effort, high value)
- Add `manifest.json` (name, short_name, icons, display: `standalone`, theme_color).
- Serve it at `/manifest.json` from the Axum router.
- Add `<link rel="manifest">` in `page_template`.
- This alone enables "Add to Home Screen" and full-screen mode (no browser chrome).

### Phase B — Service worker for static assets
- Register a service worker (`/sw.js`) that caches the CSS, JS, and fonts on first load.
- Use a cache-first strategy for static assets, network-first for HTML.
- Provide a minimal offline page ("You're offline — reconnect to continue drilling.").
- Does not enable fully offline drilling (state is server-side), but makes the app resilient to patchy mobile connections for assets.

**Files:** new `manifest.json`, new `sw.js`, `src/cmd/drill/template.rs`, router in `src/cmd/serve/server.rs` or `src/cmd/drill/server.rs`

---

## 5. Session completion UX

**Problem:** The completion screen blocks exit with a stats table and requires an explicit tap to go home. On a commute this is friction after the work is done.

**Fix:**
- Auto-redirect to `/` after 5 seconds (with a visible countdown so users can cancel).
- Reduce the stats table to a single summary line: "Done — 42 cards in 6 min (8 s/card)." The full table stays but is collapsed by default (`<details>`).
- Remove the "Shutdown" button in serve mode (it was added for drill mode); replace with a plain "Home" link.

**Files:** `src/cmd/drill/get.rs` (completion page render), `src/cmd/drill/style.css`

---

## 6. Per-card timing

**Problem:** The session shows only aggregate pace. No visibility into which cards or decks are slow.

**Fix:**
- Add `card_shown_at: Option<Timestamp>` to `SessionState` (set when a card is first shown, reset after grading).
- When a grade action is processed, compute `elapsed = now - card_shown_at` and store it alongside the review (new `duration_ms` column in `reviews`).
- Display on the completion page: a second stats table row "Slowest card" (deck + elapsed), and update "Pace" to show median instead of mean (more informative when a few cards dominate).
- Future: per-deck breakdown (out of scope for this iteration).

**Files:** `src/cmd/drill/state.rs`, `src/cmd/drill/post.rs`, `src/db.rs`, `src/schema.sql`, `src/cmd/drill/get.rs`

---

## Suggested implementation order

| # | Item | Effort | Value |
|---|------|--------|-------|
| 1 | Button consistency | S | Medium |
| 5 | Completion UX | S | High |
| 3 | Quick session sizing | M | High |
| 6 | Per-card timing | M | Medium |
| 2 | Session persistence | M–L | High |
| 4a | PWA manifest | S | High |
| 4b | Service worker | M | Medium |

Start with 1, 5, 4a (quick wins), then 3, 6, 2, 4b.
