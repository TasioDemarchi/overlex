# Plan: Per-profile history cache, dedicated history window, and engine indicator fix (v0.9.12)

## Goal

Three related improvements to the history feature:

1. **Per-profile cache**: The cache currently ignores game profiles — translating "attack" in the browser returns the same cached result as in a JRPG. Fix this by scoping the cache to the active game profile, so each game context has its own translations.

2. **Dedicated history window**: The history panel currently lives inside Settings, taking up significant space. Move it to its own Tauri window that can be opened from a button in Settings. This declutters Settings and lets the user keep the history open while playing.

3. **Engine indicator fix**: When a translation comes from the cache, the engine name is irrelevant (no engine was called). Show only "cached · timestamp" in that case. After a force re-translate, the engine name reappears because an engine was actually called.

## Why these together

All three are UX/data improvements to the history feature shipped in v0.9.11. The user explicitly requested them in the same session, and they're tightly related:
- Improvement 1 fixes a real bug (cache pollution across contexts)
- Improvement 2 is UX cleanup (history takes too much space in Settings)
- Improvement 3 is UX cleanup (engine shown when it wasn't called)

Doing them together avoids touching the history code 3 separate times.

## Files to Touch

| File | Change |
|------|--------|
| `src-tauri/src/history.rs` | Add `profile_id` column to `HistoryEntry`; modify `init` (migration), `insert`, `get_all`, `search`, `find_cached`, `update_translation`, `export`, `clear` |
| `src-tauri/src/commands.rs` | Pass `profile_id` to `find_cached` in 3 places; add `open_history_window` command; pass `profile_id` to `force_retranslate` |
| `src-tauri/src/lib.rs` | Register new `history` window from config; add `get_current_profile_id` helper or inline |
| `src-tauri/tauri.conf.json` | Add `history` window config |
| `src/history/index.html` | **NEW** — history window HTML |
| `src/history/history.js` | **NEW** — history window JS (render, search, export, delete, clear, load more) |
| `src/history/history.css` | **NEW** — history window styles (terminal aesthetic) |
| `src/settings/index.html` | Remove the inline history panel; replace with "Open History" button |
| `src/settings/settings.js` | Remove history list rendering, search, export, clear, delete logic; add button click handler |
| `src/result/result.js` | Fix engine indicator: hide engine name when `from_cache == true`; show only "cached · timestamp" |
| `src/result/index.html` | Verify retranslate button visibility logic (no change to HTML, just the JS that controls it) |
| `src/write/write.js` | Fix per-message engine indicator: hide engine when cached; hide ↻ button after force re-translate |
| `docs/decisions.md` | Add ADR-030 |
| `CHANGELOG.md` | Add v0.9.12 entry |
| 3 version files | Bump 0.9.11 → 0.9.12 |

## Implementation Detail

### Part 1: Per-profile cache (backend)

#### 1.1 `history.rs`: Add `profile_id` to `HistoryEntry`

```rust
pub struct HistoryEntry {
    pub id: i64,
    pub original_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub engine: String,
    pub created_at: String,
    /// Profile ID this entry is scoped to. NULL for entries not associated
    /// with a game profile (e.g. browser, no game detected).
    /// Stored as Option<String> for backward compatibility (old entries have NULL).
    pub profile_id: Option<String>,
}
```

#### 1.2 `history.rs`: Schema migration in `init()`

Add a `profile_id` column to the `translations` table. Use `ALTER TABLE` with `IF NOT EXISTS` semantics (SQLite doesn't support `IF NOT EXISTS` on columns natively, so we check via `PRAGMA table_info`):

```rust
// After CREATE TABLE IF NOT EXISTS translations (...), add:
let has_profile_id: bool = conn
    .prepare("PRAGMA table_info(translations)")
    .map_err(|e| format!("Failed to check schema: {}", e))?
    .query_map([], |row| Ok(row.get::<_, String>(1)?))
    .map_err(|e| format!("Failed to query schema: {}", e))?
    .filter_map(|r| r.ok())
    .any(|col| col == "profile_id");

if !has_profile_id {
    conn.execute("ALTER TABLE translations ADD COLUMN profile_id TEXT", [])
        .map_err(|e| format!("Failed to add profile_id column: {}", e))?;
}
```

Also update the `CREATE TABLE` to include `profile_id TEXT` (so new installs have it from the start).

#### 1.3 `history.rs`: Update all CRUD functions

**`insert`**: add `profile_id` parameter; INSERT includes the column
**`get_all`**: SELECT includes `profile_id`; row.get(7) for the new column
**`search`**: same
**`find_cached`**: add `profile_id: Option<&str>` parameter; WHERE clause includes `AND profile_id IS ?4` (SQLite uses `IS` for NULL comparison)
**`update_translation`**: unchanged (updates translated_text, engine, created_at — profile_id stays the same)
**`export`**: add `profile_id` to CSV header and JSON output
**`clear`**: unchanged

#### 1.4 `commands.rs`: Pass `profile_id` to `find_cached`

The 3 calls to `find_cached` (lines 676, 997, 1178) need to pass the active profile. The active profile is in `active_game_state.info.lock().unwrap().matched_profile.clone()`.

Extract a helper to avoid repetition:
```rust
fn get_active_profile_id(active_game_state: &ActiveGameState) -> Option<String> {
    active_game_state.info.lock().unwrap().matched_profile.clone()
}
```

Each `find_cached` call becomes:
```rust
let profile_id = get_active_profile_id(active_game_state);
history::HistoryDb::find_cached(&text, &settings.source_lang, &settings.target_lang, profile_id.as_deref())
```

#### 1.5 `commands.rs`: Pass `profile_id` to `force_retranslate`

The `force_retranslate` command (line 1704) needs the same:
```rust
let profile_id = get_active_profile_id(active_game_state);
history::HistoryDb::find_cached(&original_text, &source_lang, &target_lang, profile_id.as_deref())
```

### Part 2: Dedicated history window (frontend + backend)

#### 2.1 `tauri.conf.json`: Add `history` window

```json
{
    "label": "history",
    "title": "Translation History",
    "width": 800,
    "height": 600,
    "minWidth": 600,
    "minHeight": 400,
    "resizable": true,
    "visible": false,
    "center": true,
    "decorations": true,
    "url": "history/index.html"
}
```

#### 2.2 `commands.rs`: New command `open_history_window`

```rust
#[tauri::command]
pub async fn open_history_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("history") {
        window.show().map_err(|e| e.to_string())?;
        let _ = window.set_focus();
    } else {
        // Window not yet created (config-loaded but not instantiated)
        return Err("History window not available".to_string());
    }
    Ok(())
}
```

Register in `lib.rs` invoke_handler.

#### 2.3 `src/history/index.html`: New window HTML

Structure:
- Header: title + close button
- Toolbar: search input, "Load More" button, "Export JSON", "Export CSV", "Clear All"
- Main: scrolleable list of entries (reuse the same `history-entry` CSS class from Settings)
- Empty state: "No history yet" or "History disabled"

Reuse the existing `history-entry` CSS by inlining the relevant styles from `src/settings/index.html` (or import the settings CSS — but that pulls in too much. Just copy the necessary styles).

#### 2.4 `src/history/history.js`: New window JS

Functions needed (mirror what was in `settings.js`):
- `renderHistoryEntry(entry)` — same as before, with profile_id display
- `renderHistory()` — fetch and render list
- `searchHistory()` — search with debounce
- `exportHistory(format)` — JSON or CSV
- `clearHistory()` — with confirmation
- `deleteEntry(id)` — with confirmation, then refresh list
- `loadMore()` — pagination

**Display profile_id**: Each entry shows its profile (if any) below the meta line:
```html
<div class="entry-meta">[profile name if any] · [engine or "cached"] · [timestamp]</div>
```

Actually, the user said no re-translate in history window, but the per-profile distinction should be visible. So just show the profile as a small label like `[Profile: GameName]` next to the engine/timestamp.

#### 2.5 `src/settings/index.html`: Remove inline history panel

Replace the entire history panel section (lines ~979-1000) with:
```html
<label class="terminal-checkbox">
    <input type="checkbox" id="history-enabled" />
    <span class="cb-display"></span>
    <span class="cb-label">Save translation history</span>
</label>
<button id="open-history-btn" class="small-btn" style="margin-top: 8px;">OPEN HISTORY</button>
```

#### 2.6 `src/settings/settings.js`: Remove history logic, add button handler

- Remove `historyList`, `historySearchInput`, `historyLoadMoreBtn`, `historyExportJsonBtn`, `historyExportCsvBtn`, `historyClearBtn` element references
- Remove `renderHistory`, `searchHistory`, `exportHistory`, `clearHistory`, `deleteEntry` functions
- Add click handler for `open-history-btn`:
```javascript
openHistoryBtn.addEventListener('click', async () => {
    try {
        await invoke('open_history_window');
    } catch (err) {
        showMessage('Failed to open history: ' + err, true);
    }
});
```
- Keep the `historyEnabledCheckbox` logic (still needed for the setting)

### Part 3: Engine indicator fix (frontend)

#### 3.1 `src/result/result.js`: Hide engine when from_cache

In `updateEngineUsedIndicator` or wherever the engine display happens, add a branch:
```javascript
if (payload.from_cache) {
    engineEl.textContent = `cached · ${formatTimestamp(cached_at)}`;
    engineEl.style.color = '#69db7c'; // green
    // ↻ button stays visible (retranslate still possible)
} else {
    // normal engine display (existing logic)
    updateEngineUsedIndicator(payload.engine_used, payload.fallback);
}
```

The `cached_at` is the `created_at` of the cache entry. The backend should include it in the payload.

Add `cached_at: Option<String>` to `ResultPayload` (and `TranslationResult` for write window):
```rust
pub struct ResultPayload {
    // ... existing fields ...
    pub from_cache: bool,
    /// If from_cache, the created_at of the cache entry. Used by the frontend
    /// to display the cache age in the indicator.
    #[serde(default)]
    pub cached_at: Option<String>,
}
```

#### 3.2 `src/write/write.js`: Same fix per message

When rendering a chat message:
- If `from_cache == true`: show `cached · ${cached_at}` in the meta line, green color
- If `from_cache == false`: show `engine · ${timestamp}` as before

#### 3.3 Hide ↻ button after force re-translate (both windows)

When the user clicks ↻, the backend updates the cache entry with the new translation. The new entry has `from_cache = false` (just re-translated, not cached). So the frontend should:
- Update the displayed translation with the new text
- Update the indicator to show the new engine + new timestamp (no "cached" anymore)
- **Hide the ↻ button** (the entry is no longer cached, doesn't need re-translate)

This logic already exists in v0.9.11 partially — the apply agent set `from_cache` after the update. We just need to make sure the button visibility reflects the new state.

## How it Works (Flow)

### Per-profile cache example

1. User translates "attack" in browser (no game) → `find_cached("attack", en, es, None)` → no match → engine called → entry saved with `profile_id = NULL`
2. User opens Game X (profile "GameX") → translates "attack" → `find_cached("attack", en, es, Some("GameX"))` → no match (NULL ≠ "GameX") → engine called → entry saved with `profile_id = "GameX"`
3. User translates "attack" again in Game X → `find_cached("attack", en, es, Some("GameX"))` → MATCH → returns GameX's translation (could be "ataque" noun or "atacar" verb, context-specific)
4. User closes Game X → no profile → translates "attack" → `find_cached("attack", en, es, None)` → MATCH on NULL → returns browser's translation
5. User opens Game X again → translates "attack" → MATCH on "GameX" → returns GameX's translation

### History window example

1. User opens Settings → clicks "OPEN HISTORY" button → `open_history_window` command → history window shows
2. User sees all entries with profile labels (some show "[GameX]", some show "[no profile]")
3. User searches "attack" → FTS5 search → filtered list shows
4. User clicks Export JSON → downloads `overlex-history.json`
5. User clicks Clear All → confirmation → all entries deleted
6. User closes history window → can reopen anytime

### Engine indicator example

1. First translate "sword" (fresco) → indicator: `> google_gtx · 14:30:45` (with engine, gray)
2. Second translate "sword" (cache hit) → indicator: `> cached · 14:30:45` (no engine, green) + ↻ button visible
3. User clicks ↻ → engine called → entry updated → indicator: `> groq · 14:31:00` (new engine, new timestamp, gray) + ↻ button HIDDEN (no longer cached)

## Edge Cases to Verify

### Per-profile cache
- **Old entries (no profile_id column)**: migration adds the column as NULL; existing entries work correctly
- **Profile name changes**: the cache is scoped by `profile.display_name` (current code), so if a user renames a profile, the old cache entries are orphaned (won't be found, won't be saved to the new name). Acceptable — old entries can be cleaned via Clear.
- **No game detected**: `matched_profile = None` → `profile_id = None` → cache scoped to "no profile"
- **Profile match error**: if `active_game_state` is locked or unavailable, `get_active_profile_id` returns `None` → safe fallback

### History window
- **Window not created yet**: Tauri creates windows from config at startup, so it's available. If somehow not, `open_history_window` returns error and shows a message.
- **Window already open**: `show()` brings it to front, doesn't create a duplicate
- **DB empty**: shows "No history yet" message
- **DB has thousands of entries**: pagination via Load More (20 at a time)
- **Search with special chars**: FTS5 sanitization already in place (`sanitize_fts5_query`)

### Engine indicator
- **Cache hit on first-ever translation**: impossible (no cache yet)
- **Cache hit after force re-translate**: the entry was updated, so it has a new `created_at` and a real engine — indicator should show engine, not "cached"
- **Engine label unknown**: existing `ENGINE_LABELS` map handles this; falls back to engine ID

## Test Plan

User must validate after building:

1. **Per-profile cache**:
   - Translate "attack" in browser (no game) → save
   - Open Game X (profile "GameX") → translate "attack" → new entry with GameX profile, NOT the browser's translation
   - Translate "attack" again in Game X → cache hit (GameX's translation)
   - Close Game X → translate "attack" → cache hit (browser's translation)
   - Open Game X → translate "attack" → cache hit (GameX's translation)
   - Check history window → see two entries: one with `[no profile]`, one with `[GameX]`

2. **History window**:
   - Open Settings → "OPEN HISTORY" button visible
   - Click button → history window opens
   - Window shows entries with profile labels
   - Search works
   - Load more works
   - Export JSON downloads file
   - Export CSV downloads file
   - Clear All works (with confirmation)
   - Delete by entry works (with confirmation)
   - Close window → can reopen from Settings

3. **Engine indicator**:
   - First translate "hello" → indicator shows `> google_gtx · 14:30` (engine, gray)
   - Second translate "hello" → indicator shows `> cached · 14:30` (NO engine, green) + ↻ button visible
   - Click ↻ → engine called → indicator shows `> google_gtx · 14:31` (new engine, new time) + ↻ button HIDDEN
   - Same tests in write window per message

## Version Bump

- `src-tauri/tauri.conf.json`: `0.9.11` → `0.9.12`
- `src-tauri/Cargo.toml`: `0.9.11` → `0.9.12`
- `package.json`: `0.9.11` → `0.9.12`

## Documentation

- Add to `CHANGELOG.md`:
  ```
  ## [0.9.12] - 2026-06-15
  - feat: per-profile history cache — translations are now scoped to the active game profile, so the same word can have different translations in different game contexts
  - feat: dedicated history window — moved the translation history from a panel inside Settings to its own window, decluttering Settings
  - fix: engine indicator no longer shows when the translation came from the cache (only "cached · timestamp" is shown); after force re-translate, the engine name reappears and the retranslate button is hidden
  ```
- Add to `docs/decisions.md`:
  ```
  ## ADR-030 — Per-profile cache, dedicated history window, engine indicator fix
  - **Context**: The v0.9.11 cache had three UX/data issues: (1) it ignored game profiles, causing the same word to return the same translation across different game contexts; (2) the history panel inside Settings took up significant space; (3) the engine name was shown even when the translation came from the cache (misleading).
  - **Decisions**:
    1. Scope the cache to the active game profile (profile_id column in translations table, NULL for no-profile)
    2. Move history to a dedicated Tauri window (label: "history")
    3. Hide engine name when from_cache; show "cached · timestamp" instead. After force re-translate, engine name reappears.
  - **Why**: Fixes real data bug (cache pollution), declutters Settings, makes the cache UX more honest.
  - **Tradeoffs**: Per-profile cache means the same word can be stored N times (one per profile). The DB is small so this is fine. History window requires a new Tauri window setup, more code than a modal would, but consistent with the app's pattern of separate windows per concern.
  ```

## Out of Scope (Noted, Not Fixed)

- Re-translate button in history window (user explicitly said no)
- Copy-to-clipboard button in history window (user explicitly said no)
- Per-engine cache (cache by engine preference, not just text+lang+profile)
- TTL on cache entries
- Cache hit/miss statistics
- History window's own "clear by profile" or "clear by date range" filters
