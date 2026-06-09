# Plan: api-keys-json-storage

## Intent

Replace the Windows Credential Manager (keyring crate) backend with a plain JSON file at `%APPDATA%/overlex/api_keys.json` for storing translation engine API keys. The v0.8.3 fix attempted to make keyring-based persistence reliable, but root cause is that keyring on Windows fails when process elevation context changes between sessions (admin install → normal launch), and the fix only added logging without changing the failure path. The chain silently falls back to google_gtx when a paid engine has no key. For a personal single-user project, Credential Manager is overkill and creates reliability issues that the current architecture cannot solve without elevating privileges or re-prompting. A plain JSON file in `%APPDATA%` is persisted across NSIS upgrades by convention, is debuggable with Notepad, and removes the entire failure mode. User accepts the one-time friction of re-entering keys after upgrading to v0.8.4 (D2=B).

## Scope

### Fix 1 — Replace keyring with JSON file storage

- Delete dependency on `keyring` crate in `src-tauri/Cargo.toml`
- Implement new module-level functions in `src-tauri/src/settings.rs`:
  - `get_api_key(engine)` — reads `%APPDATA%/overlex/api_keys.json`, returns the key for the given engine or Err if not present
  - `set_api_key(engine, key)` — writes/updates the key in the JSON file
  - `delete_api_key(engine)` — removes the key from the JSON file
- JSON file schema (D1=B):
  ```json
  {
    "version": 1,
    "keys": {
      "deepseek": "sk-...",
      "gemini": "AIza...",
      "deepl": "fx-..."
    }
  }
  ```
- File is created with `version: 1, keys: {}` on first read if it doesn't exist
- Writes are atomic via `tempfile + rename` pattern (write to `api_keys.json.tmp`, then rename) to avoid corruption on crash mid-write
- `serde_json` is already a dependency — no new crates needed
- Returns the same `Result<String, String>` signature as the keyring version, so all callers (lib.rs, commands.rs, mod.rs) work unchanged

### Fix 2 — Improve startup error visibility

The current bug is silent — the user sees "translated by Google" instead of an error. Add a user-visible warning when a paid engine in `enabled_engines` has no key at startup:

- After loading keys at startup (`lib.rs:140-156`), if any paid engine in `enabled_engines` has no key, log a warning AND emit a new Tauri event `api-key-missing` to the frontend with the list of missing engines
- Frontend (`src/settings/index.html` + `settings.js`) shows a one-time banner/toast when this event is received, prompting the user to open Settings
- The warning is non-fatal — translation still works via the fallback chain, but the user knows why their paid engine is not being used

This is not scope creep — it's the same fix, because the original bug was "user couldn't tell why DeepSeek stopped working". Without this, they'll re-enter the key in v0.8.4 and STILL wonder why they had to do that.

### Fix 3 — Remove dead `keyring` references

- Remove `use keyring::Entry;` from `src-tauri/src/settings.rs`
- Remove `keyring = "3"` from `src-tauri/Cargo.toml`
- Grep for any other references to `keyring` or `Entry::new` and clean them up
- Keep the public function signatures identical so no other files need changes

## Affected Files

- `src-tauri/Cargo.toml` — remove keyring dependency
- `src-tauri/src/settings.rs` — replace `get_api_key`/`set_api_key`/`delete_api_key` with JSON file implementations
- `src-tauri/src/lib.rs` — add `api-key-missing` event emission after startup key load
- `src-tauri/src/commands.rs` — no signature changes (calls `settings::get_api_key` etc, signature preserved)
- `src-tauri/src/translation/mod.rs` — no signature changes (calls `settings::get_api_key` in fallback)
- `src/settings/index.html` — add banner element for missing key warning
- `src/settings/settings.js` — listen for `api-key-missing` event, show banner
- `CHANGELOG.md` — add `[Unreleased]` entry
- `docs/decisions.md` — close ADR-013 (API key persistence) and add ADR-016 (JSON storage decision)
- `README.md` — update "Data storage" section to mention api_keys.json

## Impact Checklist

- [ ] **Primary acceptance test**: configure DeepSeek API key → close OverLex → reopen → translate → translation works via DeepSeek (NOT fallback to google_gtx)
- [ ] Same test for Gemini and DeepL
- [ ] Configure all 3 paid engines → restart → all 3 work
- [ ] Configure only google_gtx + mymemory (no paid) → restart → no error shown, no `api-key-missing` event
- [ ] **Migration friction (D2=B)**: after installing v0.8.4 over v0.8.3, the api_keys.json is empty. User must re-enter keys in Settings. The `api-key-missing` event fires on first startup to make this discoverable.
- [ ] Delete the `keyring` crate from Cargo.toml — `cargo build` succeeds with no warnings about unused dependencies
- [ ] `cargo test` passes (existing unit tests in settings.rs, translation/mod.rs, etc.)
- [ ] JSON file is human-readable: open `%APPDATA%/overlex/api_keys.json` in Notepad, see the keys
- [ ] JSON file persists across app restart (manual test)
- [ ] JSON file persists across NSIS installer upgrade (simulate by running installer over current install)
- [ ] Atomic write: if app crashes mid-write, the JSON file is not corrupted (test by killing process during save)
- [ ] If api_keys.json is corrupted, app backs it up to `api_keys.json.bak` and starts with empty keys (same pattern as settings.json corruption handling in `load_settings`)
- [ ] No regression in v0.8.3 fixes (CSP allowlist, profile hydration, context_prompt)

## Decisions

- **D1 (JSON format)**: Use `{"version": 1, "keys": {...}}` structure. Allows future evolution (per-key metadata, rotation timestamps) without breaking changes. Minimal complexity (2 fields).
- **D2 (migration strategy)**: NO automatic migration from Credential Manager. User must re-enter keys in Settings after installing v0.8.4. The `api-key-missing` event on first startup makes this discoverable. Trade-off: 20 fewer lines of migration code, but the user pays a one-time re-entry cost. Acceptable for a personal project.
- **D3 (storage location)**: `%APPDATA%/overlex/api_keys.json` — same directory as `settings.json` and `history.db`. NSIS upgrades do not touch `%APPDATA%` by default, so persistence across upgrades is guaranteed by the installer convention.
- **D4 (atomic writes)**: Write to `api_keys.json.tmp` first, then `std::fs::rename` to final path. Avoids corruption if the process crashes mid-write. Standard pattern, no extra crates.
- **D5 (corruption handling)**: If `serde_json::from_str` fails on read, rename the file to `api_keys.json.bak` and start fresh with `{"version": 1, "keys": {}}`. Same pattern as `load_settings` (settings.rs:36-44). User loses their keys but the app doesn't crash.
- **D6 (frontend notification)**: `api-key-missing` event emitted on startup if any paid enabled engine has no key. Frontend shows a dismissible banner in the Settings window prompting the user to configure the key. Banner is in Settings only — not in the OCR overlay (avoid noise during gameplay).
- **D7 (no design.md)**: Small-to-medium refactor (~5 files touched), no architectural change. The data model is the same (string key per engine), only the storage backend changes. Following SDD Lite principle "most changes stop at level 2".

## Out of Scope

- **Encryption of api_keys.json**: It's plaintext on disk. For a personal project on a single-user PC, this is acceptable. If the user wants encryption later, it's a separate change.
- **Per-key metadata** (created_at, last_validated, etc.): The `version: 1` field allows this to be added later without breaking existing files.
- **Migration FROM api_keys.json back to Credential Manager**: One-way migration only. Once we're on JSON, we stay on JSON.
- **UI changes to Settings beyond the missing-key banner**: The settings panel works as-is.
- **Telemetry on which engine succeeds**: Observes the fallback chain behavior. Useful but separate concern.
- **Multiple API keys per engine**: Single key per engine is sufficient.

## Observations (not implemented now)

- **O1**: The `settings.rs` file mixes storage logic (keyring/JSON) with settings normalization logic (`normalize_settings`). Could be split into `settings_persistence.rs` and `settings_schema.rs` in a future refactor. Not for this change.
- **O2**: All API key access goes through `settings::get_api_key` which does I/O on every call. For 3 engines at startup, this is fine. If we add more engines or call this in hot paths later, an in-memory cache could help. Not now.
- **O3**: The `cargo test` tests in `settings.rs` test deserialization of `Settings` but not the new `ApiKeysStore` (JSON file storage). Adding unit tests for the new code is in scope (see Impact Checklist), but the existing tests should continue to pass unchanged.
- **O4**: The `api-keys-json-storage` change could be backported as a "settings migration" helper in a future version (e.g., to move data from one storage backend to another). Not now — premature abstraction (AHA principle).

## Migration Notes

When a user installs v0.8.4 over v0.8.3:

1. The installer preserves `%APPDATA%/overlex/` (NSIS convention), so `settings.json`, `history.db`, etc. survive.
2. `api_keys.json` does NOT exist yet — it will be created on first read with empty `keys`.
3. On first startup, the app detects that paid engines (configured in `settings.json`) have no keys in `api_keys.json`, and emits `api-key-missing` to the Settings window.
4. The user opens Settings, sees the banner, and re-enters the API keys for each paid engine.
5. The keys are now in `api_keys.json` and persist across all future restarts and upgrades.

This is a one-time cost. After the first re-entry, no further action is needed.
