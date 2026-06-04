# Proposal: gemini-deepl-translation

## Intent

Add two AI-powered translation engines — Gemini 2.0 Flash and DeepL Free — with game-context-aware prompts. Phase 1 gave us game detection and profile auto-switching; Phase 2 leverages that context to dramatically improve translation quality for gaming text (items, skills, dialog, UI) by injecting the game name and profile into the translation request.

## Scope

### In Scope
- Gemini 2.0 Flash adapter with `systemInstruction` game context
- DeepL Free adapter with native `context` parameter
- Update `TranslationEngine` trait signature to accept `Option<&TranslationContext>`
- Update all call sites in `commands.rs` (translate_text, translate_chat, ocr_capture_region)
- Add engine dropdown options: Gemini, DeepL
- Profile engine override support for Gemini/DeepL
- API key storage via existing Windows Credential Manager

### Out of Scope
- Streaming responses (synchronous `generateContent` only)
- Glossary / term injection (Phase 3)
- Smart lookup / auto-enrich (Phase 4)
- Adaptive OCR preprocessing (Phase 5)

## Capabilities

### New Capabilities
- `gemini-translation`: Context-aware AI translation via Gemini 2.0 Flash
- `deepl-translation`: High-quality fallback translation via DeepL Free

### Modified Capabilities
- `translation-engine`: Trait signature adds `context: Option<&TranslationContext>`
- `settings`: Engine dropdown includes Gemini and DeepL options

## Approach

1. **Trait change**: Add `TranslationContext { process_name, profile_name, source_lang, target_lang }` and update `translate()` signature in `translation/mod.rs`.
2. **Gemini adapter**: POST to `generativelanguage.googleapis.com` with `systemInstruction` containing game context (e.g., "You are translating text from Path of Exile 2..."). Handle missing `detected_source` by returning `None`.
3. **DeepL adapter**: POST to `api-free.deepl.com` with `context` parameter. Parse `detected_source_language` from response.
4. **Call site updates**: All three command functions (`translate_text`, `ocr_capture_region`, `translate_chat`) build `TranslationContext` from active game info + settings and pass it to `translate()`.
5. **UI**: Add `<option>` entries for Gemini and DeepL in both global engine and profile engine dropdowns.
6. **API keys**: Reuse existing `get_api_key`/`set_api_key` commands and Windows Credential Manager storage. Adapters fetch keys on creation.

## Affected Areas

| File | Impact | Description |
|------|--------|-------------|
| `src-tauri/src/translation/gemini.rs` | New | Gemini 2.0 Flash adapter |
| `src-tauri/src/translation/deepl.rs` | New | DeepL Free adapter |
| `src-tauri/src/translation/mod.rs` | Modified | Add `TranslationContext`, update trait, add `create_engine` cases |
| `src-tauri/src/commands.rs` | Modified | Update trait call sites, build context |
| `src/settings/index.html` | Modified | Add engine options to dropdowns |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Gemini API key exposed in query param (not header) | Low | Accept query-param auth as documented by Google; key is user-managed |
| Free tier rate limits hit during gameplay | Med | Show clear error message; user can switch to DeepL or free engines |
| Context prompt bloats token usage | Low | Keep systemInstruction concise (~100 tokens max) |
| Trait signature change breaks existing adapters | Low | Update all adapters in same PR; compiler catches mismatches |

## Rollback Plan

Remove `gemini.rs` and `deepl.rs`. Revert trait signature to 3-arg version. Remove context-building code from call sites. Revert dropdown additions. Old settings with `engine: "google_gtx"` load fine.

## Dependencies

- Phase 1 game detection and profile system (already implemented)
- Existing `get_api_key`/`set_api_key` Windows Credential Manager commands
- `reqwest` already in Cargo.toml

## Success Criteria

- [ ] Gemini translates text with game name injected in system prompt
- [ ] DeepL translates text with context parameter
- [ ] All three translation paths (OCR, write, chat) pass context correctly
- [ ] Profile engine override can select Gemini or DeepL
- [ ] Missing API key shows clear error; free engines still work
- [ ] Settings UI shows new engine options and API key fields
