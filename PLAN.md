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

> **Implemented in PR #8.** `.btn`, `.btn-primary`, `.btn-secondary`, `.btn-danger` added to `style.css`. All listed buttons updated to use the shared classes. Mobile `min-height: 44px` applied via media query. ✅ Goal reached.

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

> **Phase A implemented in PR #8.** Manifest served at `/manifest.json` from both drill and serve servers. `<link rel="manifest">` added to `page_template`. Has `name`, `short_name`, `display: standalone`, `theme_color`, `background_color`, `start_url`. ✅ Standalone mode and "Add to Home Screen" are enabled.
>
> ⚠️ **Gap — no icons:** the manifest does not specify an `icons` field. Without a proper icon, mobile browsers will either use a screenshot or a generic placeholder on the home screen. For real-world use the app needs at least one 192×192 and one 512×512 PNG icon served at a stable path (e.g., `/icons/icon-192.png`). This is the main thing missing before Phase A is genuinely complete.
>
> Phase B (service worker) not yet started.

---

## 5. Session completion UX

**Problem:** The completion screen blocks exit with a stats table and requires an explicit tap to go home. On a commute this is friction after the work is done.

**Fix:**
- Auto-redirect to `/` after 5 seconds (with a visible countdown so users can cancel).
- Reduce the stats table to a single summary line: "Done — 42 cards in 6 min (8 s/card)." The full table stays but is collapsed by default (`<details>`).
- In serve mode, replace the "Shutdown" button (which only appears in drill mode) with a "Home" POST form that tears down the session and returns to the collection list.

**Files:** `src/cmd/drill/get.rs` (completion page render), `src/cmd/drill/style.css`

> **Implemented in PR #8.** Auto-redirect with 5-second countdown and cancel link added (serve mode only). Stats wrapped in `<details>` (collapsed by default). Sub-minute durations display in seconds (e.g., "Done — 5 cards in 45 s"). The `Home` action remains a POST form submit in serve mode so the session is torn down correctly (the Shutdown button was never shown in serve mode — `CompletionAction` branching already handled that). ✅ Done.

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

---

## 7. Multi-user support with authentication *(larger task — later)*

**Problem:** hashcards assumes a single user. Collections, review history, and session state are global. To share an instance (family, study group, multiple devices) users need isolated data and access control.

### User model
- A `users` table: `id`, `username`, `password_hash` (argon2), `role` (`admin` | `user`), `created_at`.
- First registered account is automatically `admin`; subsequent accounts require admin approval or an invite token.
- All existing tables (`cards`, `reviews`, `sessions`) gain a `user_id` foreign key. Existing data is migrated to a seed admin account on upgrade.

### Session / auth
- Server-side sessions: a `sessions` table (`token`, `user_id`, `expires_at`). Token stored in an `HttpOnly` cookie.
- Axum middleware extracts the session token, loads the user, and injects it into request extensions. Unauthenticated requests redirect to `/login`.
- Login page: username + password form, issues a session token on success.

### User scopes / roles
- `admin`: create/delete accounts, assign collections to users, view all activity.
- `user`: sees and drills only their own assigned collections; no admin UI.
- Collections are either shared (visible to all) or assigned per-user by admin.

### Admin UI
- `/admin` route (admin only): list users, create accounts (username + temporary password), deactivate accounts.
- Plain HTML table — no framework, consistent with the rest of the UI.

### OIDC (optional)
- Support a single configured OIDC provider (e.g. Authentik, Keycloak, Google) as an alternative login path.
- Config: `[oidc]` section in config file with `client_id`, `client_secret`, `issuer_url`.
- On first OIDC sign-in, auto-provision a `user`-role account (email as username); admin role granted manually.
- Use the `openidconnect` crate. Local password auth stays available alongside OIDC.

### Architectural notes
- Keep the single-binary deployment model — no external service required for basic use.
- Evaluate per-user SQLite files (simpler backup/isolation) vs. a shared DB with `user_id` columns everywhere before starting.
- Rate-limit login attempts (in-memory token bucket is sufficient).

**Files (new):** `src/auth/`, `src/cmd/serve/admin.rs`, `src/cmd/serve/login.rs`  
**Files (modified):** `src/schema.sql`, `src/db.rs`, `src/cmd/serve/server.rs`, `src/cmd/serve/handlers.rs`, config structs

---

## Status summary

| # | Item | Effort | Value | Status |
|---|------|--------|-------|--------|
| 1 | Button consistency | S | Medium | ✅ Done (PR #8) |
| 5 | Completion UX | S | High | ✅ Done (PR #8) |
| 4a | PWA manifest | S | High | ⚠️ Partial — icons missing |
| 3 | Quick session sizing | M | High | ⬜ Not started |
| 6 | Per-card timing | M | Medium | ⬜ Not started |
| 2 | Session persistence | M–L | High | ⬜ Not started |
| 4b | Service worker | M | Medium | ⬜ Not started |
| 7 | Multi-user + auth | XL | High | ⬜ Later |

### Remaining work (in order)

1. **4a — Icons** (gap before Phase A is complete): add at least one 192×192 and one 512×512 PNG icon, serve from `/icons/`, reference in manifest.
2. **3 — Quick session sizing**: `limit` query param on session start; `<select>` on browse page.
3. **6 — Per-card timing**: `card_shown_at` in `SessionState`; `duration_ms` in reviews; slowest-card row on completion page.
4. **2 — Session persistence**: write each review to DB immediately on grade; undo via `voided` flag.
5. **4b — Service worker**: cache static assets; minimal offline page.
6. **7 — Multi-user auth**: separate project.
