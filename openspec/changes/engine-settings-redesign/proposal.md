# Proposal: Engine Settings Redesign

## Intent
Fix the API key overwrite bug, add a multi-engine fallback chain, and redesign the settings UI to support per-engine API keys and explicit engine enablement.

## Scope

### In Scope
- Settings struct migration (`engine` → `primary_engine` + `enabled_engines`)
- Per-engine API key inputs with individual test buttons
- Engine enablement checkboxes + primary engine dropdown
- Multi-engine chain with fallback logic on translation errors
- Translation overlay engine status updates
- Backward compatibility for old settings files

### Out of Scope
- New translation engine adapters
- Engine-specific configuration beyond API keys
- UI theme or layout redesign beyond incremental changes

## Capabilities

### New Capabilities
- `multi-engine-fallback`: Fallback chain across enabled engines with Google GTX as ultimate fallback
- `per-engine-api-keys`: Isolated API key management per engine with individual test buttons

### Modified Capabilities
None — no existing `openspec/specs/` directory.

## Approach

1. **Settings Struct**: Replace `engine: String` with `primary_engine: String` and `enabled_engines: Vec<String>`. On load, migrate old `engine` value into both fields.
2. **Backend Chain**: `TranslationState` stores a `HashMap<String, Arc<dyn TranslationEngine>>` of all enabled engines. `translate_text()` tries primary first, then next enabled engine, then Google GTX. Returns the engine used in `TranslationResult`.
3. **Frontend UI**: Paid engines get checkboxes; free engines (Google GTX, MyMemory) are always enabled. Primary engine dropdown is filtered to enabled engines only. When a paid engine is checked or selected, reveal its own API key input + test button below.
4. **Overlay**: Listen to `settings-changed` events to update the displayed engine name. Show primary engine and backup status.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src-tauri/src/settings.rs` | Modified | Add migration logic for old `engine` field |
| `src-tauri/src/commands.rs` | Modified | `save_settings`, `test_api_key`, `translate_text` signatures |
| `src-tauri/src/translation/mod.rs` | Modified | `TranslationState` holds engine map; fallback chain |
| `src/settings/settings.js` | Modified | Per-engine checkboxes, dynamic API key inputs |
| `src/settings/index.html` | Modified | Incremental UI changes for new controls |
| `src/result/result.js` | Modified | Show engine used + fallback status |
| `src/write/write.js` | Modified | Show engine used + fallback status |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Breaking translation flow | High | Keep `TranslationEngine` trait unchanged; test full translation path |
| Settings migration errors | Med | Default to `google_gtx` if migration fails; log old value |
| UI complexity increase | Low | Incremental HTML changes; validate with manual test |

## Rollback Plan
Revert to the commit before this change. The old `engine` field in the settings file is preserved during migration and can be read directly if migration logic is removed.

## Dependencies
None.

## Success Criteria
- [ ] API keys persist per-engine and never mix between engines
- [ ] Selecting/checking an engine that needs a key shows its own API key input
- [ ] Fallback chain works: primary → next enabled → Google GTX
- [ ] Overlay updates engine name when the user changes the primary engine
- [ ] Old settings with `engine` field migrate correctly to new fields
- [ ] Translation functionality remains intact after all changes
