# Plan: game-profile-ui-on-restart

## Intent

Fix the Game Profile UI bug in the Settings panel where the empty "create profile" form is shown after app restart, instead of the saved profile cards + the "Add Profile" button. The bug is caused by `invoke('list_profiles')` failing on startup due to Tauri state timing (the same issue `invokeWithRetry` was created to address, but `list_profiles` was missed when the retry pattern was introduced). Combined with a missing form state reset on initialization and a partial-failure path in `saveProfile` that leaves the form visible, the bug compounds across app restarts because the Tauri window is hidden (not destroyed) on close — DOM state is preserved.

## Scope

### Fix 1 — Use `invokeWithRetry` for `list_profiles` and `get_active_game`

In `src/settings/settings.js`, the second `DOMContentLoaded` handler (lines 687-782) currently does:
```javascript
try {
    profiles = await invoke('list_profiles');
    renderProfiles();
} catch (e) {
    console.error('Failed to load profiles:', e);
}
```

The first handler (line 528) already uses `invokeWithRetry('get_settings')` with 3 retries and 500ms delay. Apply the same pattern:
- `profiles = await invokeWithRetry('list_profiles')`
- `activeGameInfo = await invokeWithRetry('get_active_game')` (also currently has no retry)
- Keep the surrounding `try/catch` for genuine errors (e.g., malformed data, not just timing)

This fixes the root cause: when Tauri state isn't ready yet on startup, the retry handles the transient failure and eventually succeeds.

### Fix 2 — Guarantee form closure on `saveProfile` partial failure

In `src/settings/settings.js` `saveProfile()` (lines 399-413), move `closeProfileForm()` and `renderProfiles()` to a `finally` block so they always run, even if `list_profiles` fails after `add_profile` succeeds. Specifically:
- `renderProfiles()` must run on the local `profiles` array (already updated with the new profile) even if the network call to re-fetch fails
- `closeProfileForm()` must run regardless of success/failure
- The success message should still only show in the success path

### Fix 3 — Explicit form state reset on DOMContentLoaded

In the second `DOMContentLoaded` handler (lines 687-782), call `closeProfileForm()` at the start of the profile section initialization. This guarantees that even if some future code path leaves the form visible, the initial state is always: list visible, add button visible, form hidden. Cheap defensive code.

## Affected Files

- `src/settings/settings.js` — only file that needs changes
  - Fix 1: lines 704-709 (list_profiles) and 711-715 (get_active_game)
  - Fix 2: lines 399-413 (saveProfile function)
  - Fix 3: add `closeProfileForm()` call in the second DOMContentLoaded handler near line 702

## Impact Checklist

- [ ] **Primary acceptance test**: create 1 game profile → close OverLex → reopen → see the profile CARD (not the empty form) under "Game Profile" + the "+ Add Profile" button
- [ ] **Test with multiple profiles**: create 2-3 profiles → close → reopen → all cards visible
- [ ] **Test "Add Profile" still works**: after seeing cards, click "+ Add Profile" → form opens → fill and save → form closes, all cards re-render including the new one
- [ ] **Test edit still works**: click edit on a card → form opens with prefilled values → save → form closes, updated card visible
- [ ] **Test delete still works**: click delete on a card → confirmation → card disappears
- [ ] **Test partial save failure doesn't leave form open**: simulate a failure between `add_profile` and `list_profiles` (hard to test without code changes, but visual inspection of code is enough)
- [ ] No regression in v0.8.4 fixes (API key persistence still works)
- [ ] No regression in v0.8.3 fixes (CSP, profile hydration, context_prompt)

## Decisions

- **D1 (retry pattern)**: Use the existing `invokeWithRetry` (3 retries, 500ms delay) — same as `get_settings` uses. Don't create a new helper. Consistency wins.
- **D2 (scope)**: Surgical fix — 3 changes in 1 file. Do NOT touch the Tauri window lifecycle (Option B was considered and rejected by user). The window hide/destroy trade-off is a separate concern.
- **D3 (form state reset)**: Defensive `closeProfileForm()` call in DOMContentLoaded. Cost: 1 line. Benefit: prevents future similar bugs.
- **D4 (no new tests)**: This is a UI behavior fix. The codebase has no JS test infrastructure (it's vanilla JS, no test runner). Manual testing per Impact Checklist is sufficient.
- **D5 (versioning)**: Bump to v0.8.5. The v0.8.4 release workflow is currently queued/building for the API key fix; v0.8.5 will be released after this UI fix is verified.

## Out of Scope

- Changing the Tauri window lifecycle (hide vs destroy) — that's a larger architectural change
- Adding JS test infrastructure (Jest, Vitest, etc.) — overkill for a vanilla JS panel
- Refactoring the profile form HTML/JS structure
- Adding automated UI tests (Playwright, etc.)
- Fixing any other potential UI state bugs in settings.js (other panels may have similar issues, but they're out of scope for this change)

## Observations (not implemented now)

- **O1**: The first `DOMContentLoaded` handler (line 476-589) and the second one (line 687-782) are separate handlers that both run async. This is unusual and could lead to race conditions. A future refactor could consolidate them into a single `async function init()` called from one handler. Not for this change.
- **O2**: `get_active_game` (line 711-715) also lacks retry, and may have the same bug for the "active profile" banner at the top of Settings. Fix 1 covers this since I'm applying retry to both calls. The banner bug is the same root cause; if user reports it, it's already fixed by this change.
- **O3**: The Tauri window is configured with `visible: false` and uses `prevent_close + hide` to keep it alive. This is a performance choice (recreating a webview is slow), but it does mean any UI state bug persists across hide/show cycles. A future change could add a "reload webview" mechanism for debugging, but that's separate.

## Migration Notes

None. This is a pure UI fix. No data migration, no version conflicts with v0.8.4. Users on v0.8.4 will just get a Settings panel that works correctly after upgrading to v0.8.5.
