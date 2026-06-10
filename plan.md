# Plan: instant-flow (v0.8.6)

## Intent

Fix the freeze overlay flow so the user gets back to the game IMMEDIATELY after selecting a region, instead of waiting 2-5 seconds staring at the selection rectangle while the translation model responds. Two issues, both addressed in this change:

1. **Freeze overlay doesn't hide until translation completes** — the user sees the selection rectangle during the entire OCR + translation roundtrip.
2. **No version indicator in Settings** — the user can't tell at a glance which version is running.

This is a UX fix, not a feature. Surgical, low-risk, follows A1 approach (fire-and-forget + hide immediately).

## Scope

### Fix 1 — Hide freeze window immediately after OCR capture, before translation

**Root cause:** In `src-tauri/src/commands.rs` `ocr_capture_region()` (lines 725-986), the freeze window is hidden at the very end of the function (line 975-977), AFTER the translation completes. The translation can take 2-5s with Gemini/DeepSeek. The result window IS shown before the freeze is hidden (line 972 emits the result, line 975 hides the freeze), so the user sees: selection → result window appears → freeze still visible for a frame → freeze hides. On slower systems, "frame" becomes "seconds".

**Fix:** Move the `freeze_win.hide()` call to right after `emit_result()` for the empty-text error case (line 848-850 — already done), and add a similar hide call **right before the translation step** (line 892) for the success path. This way:
- OCR runs and detects text → freeze hides immediately
- Result window shows when translation completes
- User returns to game the moment text is detected, not when translation is done

**Why this is safe:**
- The `translation-result` event is already emitted asynchronously (the result window listens for it).
- The result window positioning (line 970) happens AFTER the freeze is hidden, so no z-order issues.
- The error paths (no text detected, OCR failed, translation failed) all already hide the freeze themselves.

**Changes to `src-tauri/src/commands.rs`:**
- Line 891 (before `// 7. Translate via chain`): insert `freeze_win.hide()` block
- Keep existing hide calls for error paths untouched

### Fix 2 — Show app version in Settings footer

**Approach: V1 (static from tauri.conf.json).** Tauri exposes the version via `app.config().package.version` on the backend, but for the frontend the cleanest path is to bake the version into the HTML at build time. Since we don't have a build step for the HTML, we use a simple approach: hardcode the version in `index.html` and `settings.js`, sourced from `tauri.conf.json` (manually kept in sync via a comment).

**Why V1 over V2:** KISS, zero new invoke, no runtime overhead. The version is in `tauri.conf.json` already; the developer updates it in 2 places when bumping (1 Rust file via `tauri.conf.json` + 1 frontend file). We add a comment cross-referencing the source of truth.

**Alternative considered:** Add a `get_app_version` Tauri command. Rejected — adds 1 new command, 1 new invoke, for displaying a string that changes maybe 10 times a year.

**Changes to `src/settings/index.html`:**
- Add a `<footer>` element at the end of `<body>` with `id="app-version"` containing `OverLex v0.8.6`
- Add minimal CSS: position bottom-right, subtle gray text (`var(--text-secondary)`), small font (0.75rem), margin-top 24px

**Changes to `src/settings/settings.js`:**
- No code changes needed — the version is static HTML. But add a comment at the top: `// App version is hardcoded in index.html — keep in sync with tauri.conf.json`

**Changes to `src-tauri/tauri.conf.json`:**
- Bump version from `0.8.5` to `0.8.6`

**Changes to `src/settings/index.html` (version text):**
- Update `OverLex v0.8.6` (manually)

## Affected Files

- `src-tauri/src/commands.rs` — Fix 1: add early `freeze_win.hide()` before translation step (1 insertion, ~5 lines)
- `src/settings/index.html` — Fix 2: add `<footer id="app-version">` element + CSS (~15 lines total)
- `src/settings/settings.js` — Fix 2: add top-of-file comment about version source of truth (1 line)
- `src-tauri/tauri.conf.json` — version bump `0.8.5` → `0.8.6`
- `CHANGELOG.md` — add `[0.8.6]` entry
- `docs/decisions.md` — add ADR-016 documenting the fire-and-forget freeze pattern

## Impact Checklist

- [ ] **Primary test**: select a region with text in a game → freeze hides within ~100ms of mouseup (OCR time only, no translation wait) → result overlay appears ~2-5s later with translation
- [ ] **Error path: no text in selection**: freeze hides when "No text detected" error fires (existing behavior preserved)
- [ ] **Error path: OCR fails**: freeze hides when OCR error fires (existing behavior preserved)
- [ ] **Error path: translation fails**: result window shows error message, freeze was already hidden (no regression)
- [ ] **Version visible**: open Settings → see "OverLex v0.8.6" at the bottom-right corner
- [ ] **Version matches `tauri.conf.json`**: both show 0.8.6
- [ ] No regression in v0.8.5 fixes (game profile UI hydration)
- [ ] No regression in v0.8.4 fixes (API key persistence)
- [ ] No regression in v0.8.3 fixes (CSP, profile hydration, context_prompt)

## Decisions

- **D1 (approach)**: A1 (fire-and-forget + immediate hide). The result window is already event-driven via `translation-result` events; we just need to decouple the freeze hide from the await chain.
- **D2 (where to hide)**: Right after OCR succeeds, before the translation call. This is the right cut point because: (a) text detection is the prerequisite for everything else, (b) it's already the slowest step before the network call, (c) the user has confirmed intent by releasing the mouse, no need to keep the freeze visible as a "wait indicator".
- **D3 (version source)**: V1 (static HTML, hardcoded). KISS, no new commands, no invoke. 2-file sync (tauri.conf.json + index.html) with a comment noting the dependency.
- **D4 (version location)**: Footer bottom-right, subtle gray, small font. Not in header (would compete with the title), not in a modal (too invasive for a static value), not behind a click (defeats the purpose of "see at a glance").
- **D5 (versioning)**: Bump to v0.8.6. UX fix, not a feature. 0.9.0 reserved for background capture or other architectural changes.
- **D6 (no ADR for version)**: Adding a static label doesn't warrant an ADR. Only Fix 1 gets ADR-016.
- **D7 (no new tests)**: Same as v0.8.5 — vanilla JS frontend, no test infrastructure, manual testing per Impact Checklist.

## Out of Scope

- Background screen capture (was option B from the previous discussion) — defer to a future change
- Hide freeze during the `start-ocr-flow` capture phase (the screenshot still needs ~200ms) — out of scope, low impact
- Animated transition for freeze hiding (e.g., fade out) — KISS, just hide
- Showing the version in the tray menu or in other windows — Settings only, per the request
- Auto-update checks / "check for new version" — out of scope

## Observations (not implemented now)

- **O1**: The result window is positioned via `position_result_window` using the selection's `x, y, width, height` parameters (line 970). With the new flow, the freeze will be gone when the result window positions itself, which is actually BETTER UX (no visual confusion). Noted for future, no action needed.
- **O2**: The PNG encode of the fullscreen (in `lib.rs:412-439`) still happens synchronously after capture. If we want even faster "hotkey to freeze shown" time, we could move the encode to on-demand (only encode when region is selected). But that's a different optimization (B from the original discussion). Out of scope for A1.
- **O3**: The version sync between `tauri.conf.json` and `index.html` is manual. A future improvement could be a build script that injects the version. Overkill for a small project — KISS wins.

## Migration Notes

None. Both changes are non-breaking UX improvements. Users on v0.8.5 will just get a snappier freeze flow + a version label in Settings.
