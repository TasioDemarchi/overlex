# Plan: Translation history cache with force re-translate button (v0.9.11)

## Goal

Reduce API token usage and translation latency for repeated game text (e.g. "Press E to interact", "Save your progress", common dialog patterns) by checking the history database **before** calling the translation engine. When a match is found, return the cached translation immediately. When the user wants a fresh translation (e.g. the engine has improved or context changed), a "↻" button forces the engine to run again, which updates the existing history entry in place.

## Behavior

### Cache lookup (before engine call)

When `use_history_cache = true` and the user triggers a translation:
1. Normalize the input text: `lowercase().trim()` and collapse internal whitespace
2. Query the history DB: `SELECT ... FROM translations WHERE LOWER(TRIM(original_text)) = ?1 AND source_lang = ?2 AND target_lang = ?3 ORDER BY id DESC LIMIT 1`
3. If a match is found:
   - Use the cached `translated_text` and `engine` directly
   - Mark the result as `from_cache: true`
   - Display the timestamp of the cache entry (when it was first cached)
   - Skip the engine call entirely
4. If no match: call the engine normally, mark as `from_cache: false`

### Force re-translate

When the user clicks the "↻" button on a cached result:
1. Call the engine ignoring the cache
2. Update the existing DB entry (same `id`): `UPDATE translations SET translated_text = ?, engine = ?, created_at = datetime('now') WHERE id = ?`
3. The timestamp updates to "now" (it's a fresh translation)
4. Frontend updates the displayed translation and timestamp

## Why normalized match (B)

User chose B (normalized) over A (exact). Rationale: game UI text often has minor whitespace variations (trailing spaces, double spaces, different capitalization). Exact match would miss these and force unnecessary engine calls. Normalized match (lowercase + trim + collapse whitespace) handles these cases without being so loose that it gives false positives (we still require same source/target lang).

## Why update in place (not INSERT new)

User chose to update the existing entry. Rationale: keeps DB size bounded — a frequently re-translated text stays as one row, not dozens. The `created_at` is reset to the latest re-translation time, so the "freshness" is reflected in the timestamp.

## Files to Touch

| File | Lines | Change |
|------|-------|--------|
| `src-tauri/src/history.rs` | after `get_all` (line ~137) | Add `find_cached(normalized_text, source, target) -> Result<Option<HistoryEntry>, String>` |
| `src-tauri/src/history.rs` | after `delete` (line ~213) | Add `update_translation(id, translated_text, engine) -> Result<(), String>` |
| `src-tauri/src/commands.rs` | `ResultPayload` struct (line ~125) | Add `from_cache: bool` field |
| `src-tauri/src/commands.rs` | `ocr_capture_region` function (~line 1000) | Add cache lookup before engine call |
| `src-tauri/src/commands.rs` | `translate_chat` / `translate_text` (~line 720) | Add cache lookup before engine call |
| `src-tauri/src/commands.rs` | after `delete_history_entry` | Add new Tauri command `force_retranslate(original, source, target) -> Result<HistoryEntry, String>` |
| `src-tauri/src/commands.rs` | `Settings` struct (line ~266) | Add `use_history_cache: bool` field with `#[serde(default = "default_true")]` |
| `src-tauri/src/commands.rs` | `SettingsRaw` struct (line ~302) | Same field, same annotation |
| `src-tauri/src/commands.rs` | `impl Default for Settings` (line ~380) | Add `use_history_cache: true` |
| `src-tauri/src/commands.rs` | `From` impl (line ~320) | Add `use_history_cache: s.use_history_cache` |
| `src-tauri/src/commands.rs` | `Deserialize` impl (line ~360) | Add `use_history_cache: raw.use_history_cache` |
| `src/result/result.js` | `updateEngineUsedIndicator` (line ~142) | Modify to show "cached · timestamp" when `from_cache` is true |
| `src/result/result.js` | render function (line ~87) | Add "↻" button (only visible when `from_cache == true`) that calls `force_retranslate` |
| `src/result/result.html` | dismiss button area | Add the "↻" button (or use JS-only injection since the result window HTML is small) |
| `src/write/write.js` | message render (~line 50-80) | Add "↻" button to each message; modify timestamp display to show "cached · ts" when applicable |
| `src/write/write.js` | translate flow (line ~169) | Add cache lookup before engine call in `translate_chat` |
| `src/settings/index.html` | System section | Add "Use history cache" checkbox, disabled when `history_enabled == false` |
| `src/settings/settings.js` | load/save | Load `use_history_cache`, save it; disable the checkbox when `history_enabled == false` |
| `docs/decisions.md` | end | Add ADR-029 |
| `CHANGELOG.md` | top | Add `## [0.9.11]` section |
| 3 version files | version field | Bump `0.9.10` → `0.9.11` |

## Implementation Detail

### `history.rs`: `find_cached` (new function, ~25 lines)

```rust
/// Normalize text for cache lookup: lowercase + trim + collapse internal whitespace
fn normalize_for_cache(text: &str) -> String {
    text.trim().to_lowercase()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Find a cached translation matching the given text and language pair.
/// Returns the most recent match (ORDER BY id DESC).
/// The text matching is normalized (lowercase + trim + whitespace collapse).
pub fn find_cached(
    original_text: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<Option<HistoryEntry>, String> {
    let conn = Self::get_conn()?.lock().unwrap();
    let normalized = normalize_for_cache(original_text);
    let mut stmt = conn.prepare(
        "SELECT id, original_text, translated_text, source_lang, target_lang, engine, created_at
         FROM translations
         WHERE LOWER(TRIM(original_text)) = ?1
           AND source_lang = ?2
           AND target_lang = ?3
         ORDER BY id DESC
         LIMIT 1"
    ).map_err(|e| format!("Failed to prepare cache query: {}", e))?;

    let mut rows = stmt.query(rusqlite::params![normalized, source_lang, target_lang])
        .map_err(|e| format!("Failed to query cache: {}", e))?;

    if let Some(row) = rows.next().map_err(|e| format!("Failed to read cache row: {}", e))? {
        Ok(Some(HistoryEntry {
            id: row.get(0)?,
            original_text: row.get(1)?,
            translated_text: row.get(2)?,
            source_lang: row.get(3)?,
            target_lang: row.get(4)?,
            engine: row.get(5)?,
            created_at: row.get(6)?,
        }))
    } else {
        Ok(None)
    }
}
```

### `history.rs`: `update_translation` (new function, ~10 lines)

```rust
/// Update the translated text and engine of an existing history entry.
/// Also resets the created_at timestamp to the current time.
pub fn update_translation(id: i64, translated_text: &str, engine: &str) -> Result<(), String> {
    let conn = Self::get_conn()?.lock().unwrap();
    conn.execute(
        "UPDATE translations
         SET translated_text = ?1, engine = ?2, created_at = datetime('now')
         WHERE id = ?3",
        rusqlite::params![translated_text, engine, id],
    ).map_err(|e| format!("Failed to update history entry: {}", e))?;
    Ok(())
}
```

### `commands.rs`: `ResultPayload` (add field)

```rust
pub struct ResultPayload {
    pub original: String,
    pub translated: String,
    pub error: Option<String>,
    pub timeout_ms: u32,
    pub source_lang: String,
    pub target_lang: String,
    pub engine_used: String,
    pub fallback: bool,
    /// True if this result was served from the history cache (no engine call)
    #[serde(default)]
    pub from_cache: bool,
}
```

### `commands.rs`: `ocr_capture_region` cache integration

In `ocr_capture_region` (around line 1000), **before** the engine call:

```rust
// NEW: Try cache first if enabled
let cached_entry = if settings.history_enabled && settings.use_history_cache {
    history::HistoryDb::find_cached(&original_text, &settings.source_lang, &settings.target_lang)
        .ok()
        .flatten()
} else {
    None
};

let (translation_result, from_cache) = if let Some(entry) = cached_entry {
    // Cache hit — use cached translation, skip engine
    app_log!("[CACHE] Hit for: {}", &original_text);
    (
        TranslationResult {
            original: original_text.clone(),
            translated: entry.translated_text.clone(),
            detected_source: None,
            engine_used: entry.engine.clone(),
            fallback: false,
        },
        true,
    )
} else {
    // Cache miss — call the engine
    let result = /* existing translation call */;
    (result, false)
};
```

And in the ResultPayload construction:
```rust
let payload = ResultPayload {
    // ... existing fields ...
    from_cache,
};
```

### `commands.rs`: `force_retranslate` (new Tauri command)

```rust
/// Force a re-translation by calling the engine, ignoring the cache.
/// If a matching cache entry exists, update it in place with the new translation.
#[tauri::command]
pub async fn force_retranslate(
    original_text: String,
    source_lang: String,
    target_lang: String,
    app_handle: tauri::AppHandle,
) -> Result<HistoryEntry, String> {
    // Find existing cache entry
    let existing = history::HistoryDb::find_cached(&original_text, &source_lang, &target_lang)?;

    // Call engine
    let settings = /* get current settings */;
    let translation_result = /* call engine directly */;

    // Update or insert
    if let Some(entry) = existing {
        history::HistoryDb::update_translation(entry.id, &translation_result.translated, &translation_result.engine_used)?;
        // Re-read the updated entry
        let updated = history::HistoryDb::find_cached(&original_text, &source_lang, &target_lang)?
            .ok_or_else(|| "Entry disappeared after update".to_string())?;
        Ok(updated)
    } else {
        // No existing entry — insert new
        let new_entry = history::HistoryEntry {
            id: 0,
            original_text: original_text.clone(),
            translated_text: translation_result.translated.clone(),
            source_lang: source_lang.clone(),
            target_lang: target_lang.clone(),
            engine: translation_result.engine_used.clone(),
            created_at: String::new(),
        };
        let _ = history::HistoryDb::insert(&new_entry)?;
        let created = history::HistoryDb::find_cached(&original_text, &source_lang, &target_lang)?
            .ok_or_else(|| "Entry disappeared after insert".to_string())?;
        Ok(created)
    }
}
```

### Frontend: result window "↻" button

In `src/result/result.js`, modify the render function to add the button conditionally:

```javascript
// Inside onTranslationResult or equivalent
if (payload.from_cache) {
    // Show "cached · timestamp" instead of engine name
    const engineEl = document.getElementById('engine-used');
    if (engineEl) {
        engineEl.textContent = `cached · ${formatTimestamp(payload.cached_at)}`;
        engineEl.style.color = '#69db7c'; // green for cached
    }
    // Show the re-translate button
    const retranslateBtn = document.getElementById('retranslate-btn');
    if (retranslateBtn) {
        retranslateBtn.style.display = 'inline-block';
        retranslateBtn.onclick = async () => {
            try {
                const updated = await invoke('force_retranslate', {
                    originalText: payload.original,
                    sourceLang: payload.source_lang,
                    targetLang: payload.target_lang,
                });
                // Update the displayed translation
                // ... (update DOM with new translated text and timestamp)
            } catch (err) {
                console.error('Re-translate failed:', err);
            }
        };
    }
} else {
    // Hide the re-translate button
    const retranslateBtn = document.getElementById('retranslate-btn');
    if (retranslateBtn) retranslateBtn.style.display = 'none';
    // Normal engine display
    updateEngineUsedIndicator(payload.engine_used, payload.fallback);
}
```

### Frontend: write window per-message "↻" button

In `src/write/write.js`, modify the message render to add the button:

```javascript
// After creating messageEntry div and appending translated text + meta
if (from_cache) {
    // Modify meta text to show "cached · ts"
    metaEl.textContent = `cached · ${formatTimestamp(cached_at)}`;
    metaEl.style.color = '#69db7c';
    // Add re-translate button
    const retranslateBtn = document.createElement('button');
    retranslateBtn.className = 'retranslate-msg-btn';
    retranslateBtn.textContent = '↻';
    retranslateBtn.title = 'Force re-translate';
    retranslateBtn.onclick = async (e) => {
        e.stopPropagation();
        try {
            const updated = await invoke('force_retranslate', {
                originalText: originalText,
                sourceLang: sourceLang,
                targetLang: targetLang,
            });
            // Update this message's translation in-place
            translatedEl.textContent = updated.translated_text;
            metaEl.textContent = `cached · ${formatTimestamp(updated.created_at)}`;
        } catch (err) {
            console.error('Re-translate failed:', err);
        }
    };
    messageEntry.appendChild(retranslateBtn);
}
```

### Frontend: settings checkbox

In `src/settings/index.html`, add to the System section (near `history-enabled`):

```html
<label class="terminal-checkbox">
    <input type="checkbox" id="use-history-cache" />
    <span class="cb-display"></span>
    <span class="cb-label">Use history cache</span>
</label>
```

In `src/settings/settings.js`:
- Load: `useHistoryCacheCheckbox.checked = settings.use_history_cache !== false;`
- Save: include `use_history_cache: useHistoryCacheCheckbox.checked` in payload
- Disable when `history_enabled == false`: `useHistoryCacheCheckbox.disabled = !historyEnabledCheckbox.checked;`

## How it Works (Flow)

### First translation of "Press E to interact"
1. User triggers OCR, captures "Press E to interact"
2. Cache lookup: no match
3. Engine called → translates → `from_cache: false`
4. Result shown: "Pulsa E para interactuar" with engine name
5. Saved to history

### Second translation of "press e to interact" (lowercase, same text)
1. User triggers OCR
2. Cache lookup: MATCH (normalized) → returns "Pulsa E para interactuar" with original engine
3. `from_cache: true` → result window shows "cached · 2024-12-15 14:30"
4. "↻" button visible
5. Engine NOT called (saves tokens + ~500-2000ms latency)
6. (Not re-saved to history; the existing entry is reused)

### User clicks "↻" (force re-translate)
1. `force_retranslate` command called
2. Engine called → gets fresh translation (maybe same, maybe different)
3. Existing history entry UPDATED in place (same `id`, new `translated_text`, reset `created_at`)
4. Result window updates to show new translation + new timestamp
5. (No new row inserted; DB doesn't grow)

## Edge Cases to Verify

- **Cache hit on first-ever use**: impossible, history is empty at startup
- **Settings changed mid-flight**: `use_history_cache` is read fresh on each translation call, so changes take effect immediately
- **`history_enabled` toggled off**: cache lookups skip entirely (treated as disabled)
- **Multiple cache hits for same text**: `ORDER BY id DESC LIMIT 1` returns the most recent
- **Translation fails with cache enabled**: cache lookup still runs, if hit returns cached; if miss, engine call fails → error displayed (same as before)
- **Force re-translate when no cache entry exists**: inserts a new entry (graceful fallback)
- **DB locked** (e.g. another process accessing it): `get_conn()` returns error, cache lookup fails gracefully, engine called as fallback (no panic)
- **Normalized text edge cases**: leading/trailing whitespace, double spaces, tabs, newlines all handled by `split_whitespace().join(" ")` (which collapses all Unicode whitespace)
- **Long texts**: normalized match still works, no length limit
- **Empty text after normalization**: `split_whitespace()` on empty string returns empty iterator, `join(" ")` returns `""`. Edge case but handled (no false matches because DB never has empty `original_text`)

## Test Plan

User must validate after building:

1. **Build and run** v0.9.11.
2. **First translation**: trigger OCR with "Press E to interact" → translation appears with engine name (not "cached"), no "↻" button.
3. **Second translation (identical text)**: trigger OCR with "Press E to interact" again → translation appears INSTANTLY (or much faster) with "cached · timestamp" and "↻" button visible.
4. **Normalized match**: trigger OCR with "  press E to interact  " (lowercase + extra spaces) → still hits cache.
5. **Different language pair**: change target language, translate same text → cache miss (different `target_lang`), engine called, new entry created.
6. **Force re-translate**: with cached result visible, click "↻" → engine called, translation may stay same or change, timestamp updates to current time, "↻" button stays visible (still cached after update).
7. **Write window cache**: in write window, translate "Hello world" → appears normally. Translate "Hello world" again → instant, with "cached" indicator and "↻" button on that message.
8. **Write window force re-translate**: click "↻" on the cached message → message updates with new translation and timestamp.
9. **History size**: after multiple translations and re-translations of the same text, check Settings → History → should have only ONE entry per unique text (not many duplicates).
10. **Disable cache**: Settings → uncheck "Use history cache" → save → trigger OCR with cached text → engine called, fresh translation, no "cached" indicator.
11. **Disable history entirely**: Settings → uncheck "history-enabled" → save → "Use history cache" checkbox becomes disabled (greyed out) → cache lookups skip → all translations fresh.
12. **DB growth**: after 100 translations of the same text, history DB should have ~1 entry for that text, not 100.

## Version Bump

- `src-tauri/tauri.conf.json`: `0.9.10` → `0.9.11`
- `src-tauri/Cargo.toml`: `0.9.10` → `0.9.11`
- `package.json`: `0.9.10` → `0.9.11`

## Documentation

- Add to `CHANGELOG.md`:
  ```
  ## [0.9.11] - 2026-06-15
  - feat: history cache — repeated translations skip the engine and return the cached result instantly, saving tokens and latency. Adds a "↻" button on cached results to force a fresh translation. Controlled by the new "Use history cache" setting (default true).
  ```
- Add to `docs/decisions.md`:
  ```
  ## ADR-029 — History as translation cache with normalized match
  - **Context**: User noticed that game UI text repeats often ("Press E to interact", etc.) and wanted to avoid paying for engine API calls on already-translated text. Speed is also a factor.
  - **Decision**: Before calling the engine, check the history DB with a normalized text match (lowercase + trim + whitespace collapse). If found, return the cached translation. The user can force a fresh translation via a "↻" button, which updates the existing history entry in place.
  - **Why**: Saves tokens (especially on paid engines like Groq/DeepL), reduces latency (~1ms cache vs 500-2000ms engine), DB doesn't grow unbounded (updates in place instead of inserting new rows). Normalized match handles whitespace/case variations common in OCR.
  - **Tradeoff**: User might miss engine improvements if they don't click "↻". Mitigated by the prominent button.
  - **Settings**: `use_history_cache` (default true), disabled when `history_enabled == false`.
  ```

## Out of Scope (Noted, Not Fixed)

- TTL-based cache invalidation (user chose not to add complexity for now; can be added later if needed)
- Per-engine cache (e.g. "use Groq cache, but always use Google for fresh") — current design is engine-agnostic
- Cache hit/miss statistics in Settings — could be a future addition
- `find_cached` is currently O(n) on the matching rows (SQLite index would help but not critical for typical history sizes)
