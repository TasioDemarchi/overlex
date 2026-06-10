# Plan: ui-redesign (v0.9.0)

## Intent

Full visual redesign of the Settings panel. The current UI uses a generic dark theme with blue accents. This change moves to a hybrid aesthetic that combines:

- **Image 1 baseline** (Console Settings): gray dark theme, sans-serif headings, two-column language distribution, console-app feel
- **Image 2 accent** (Terminal Settings): monospace body text, ASCII-style checkboxes `[x] [ ]`, prompt-style `>` prefix on user-input fields, green accent for terminal cues

The result is a "developer console" aesthetic — a tool that feels like it was made for people who use terminals, not for casual users. All existing settings, options, and data flows remain **100% identical**. This is a pure visual/UX refresh.

**This is a UI-1 change (full panel redesign)**, not UI-2 (focalized). Per user decision, the entire Settings panel gets the new aesthetic in one go.

## Scope

### Section 1 — Global CSS variables and design tokens

**Replace** the `:root` block (lines 7-19 of `index.html`) with an expanded token set that supports the hybrid aesthetic:

```css
:root {
    /* Base palette (Image 1: gray dark) */
    --bg-primary: #1a1a2e;
    --bg-secondary: #16213e;
    --bg-tertiary: #0f0f1a;
    --text-primary: #e4e4e7;
    --text-secondary: #9ca3af;
    --text-muted: #6b7280;
    --border: #2a2a3e;
    --border-strong: #3a3a4e;

    /* Accent: blue base (Image 1) + green terminal (Image 2) */
    --accent: #4e9af1;
    --accent-hover: #5fa8f9;
    --terminal: #51cf66;       /* green for [x], prompts, success */
    --terminal-dim: #2d8a3d;   /* darker green for borders, separators */
    --error: #ff6b6b;
    --warning: #f59f00;
    --success: var(--terminal);

    /* Typography */
    --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    --font-mono: 'Cascadia Code', 'Consolas', 'Monaco', 'Menlo', monospace;

    /* Sizing */
    --terminal-radius: 4px;
    --panel-radius: 8px;
}
```

### Section 2 — Typography and base layout

- `body`: `font-family: var(--font-sans)` (default for headings, paragraphs, sections)
- `h1`: keep sans-serif bold, larger (1.75rem)
- `h2`: keep sans-serif, uppercase, letter-spacing 0.05em (terminal section header feel)
- All `label`, `input`, `select`, `button`, `.small-btn`, `.profile-card`, etc.: `font-family: var(--font-mono)`
- Section titles (`h2`): add left-border accent (`border-left: 3px solid var(--terminal-dim)`) instead of bottom-border

### Section 3 — Custom terminal checkboxes `[x] [ ]`

**Replace** all `<input type="checkbox">` rendering with custom HTML+CSS checkboxes. This requires:

**A. New CSS for `.terminal-checkbox`:**
- Visually hides the native `<input type="checkbox">` (keep it in the DOM for accessibility and form values)
- Renders a styled span that shows `[ ]` when unchecked and `[x]` when checked
- Checked state: green text (`var(--terminal)`)
- Hover state: subtle background highlight
- Focus state: blue ring (for keyboard navigation)

**B. Wrapper class for the visual span.** Pattern:

```html
<label class="terminal-checkbox">
    <input type="checkbox" id="..." />
    <span class="cb-display">[ ]</span>
    <span class="cb-label">Enable Gemini</span>
</label>
```

CSS uses `:checked + .cb-display` to swap `[ ]` → `[x]`.

**C. Affects ALL checkboxes in settings.html and dynamically-rendered ones in settings.js:**

- HTML static checkboxes: `auto-dismiss-enabled`, `ocr-preprocessing-enabled`, `ocr-binarize-enabled`, `show-debug`, `history-enabled`, `start-with-windows`, `profile-ocr-preprocessing`, `profile-ocr-binarize`
- Dynamic checkboxes created by `renderEngineUI()` in `settings.js:808-855` (engine enable checkboxes)

**D. JS update required:** `renderEngineUI()` must use the new wrapper pattern when creating checkboxes programmatically. The native `<input type="checkbox">` stays in the DOM, so `getCheckedPaidEngines()` and save logic continue to work unchanged.

### Section 4 — Inputs and selects with terminal styling

**A. Inputs and selects (general):**
- `font-family: var(--font-mono)`
- Background: `var(--bg-tertiary)`
- Border: `1px solid var(--border-strong)`
- Border-radius: `var(--terminal-radius)` (smaller, sharper than current 6px)
- Focus state: border-color `var(--accent)`, subtle glow

**B. Special inputs with `>` prompt prefix:**

The `>` prefix renders as a `::before` pseudo-element on a wrapper class `.terminal-input`. Pattern:

```html
<div class="terminal-input">
    <input type="text" id="ocr-hotkey" ... />
</div>
```

CSS:
```css
.terminal-input {
    position: relative;
}
.terminal-input::before {
    content: '>';
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--terminal);
    font-family: var(--font-mono);
    font-weight: bold;
    pointer-events: none;
}
.terminal-input input {
    padding-left: 26px;  /* room for the > */
}
```

**Applies to:**
- `#ocr-hotkey`, `#write-hotkey` (hotkey inputs)
- `#api-key-gemini`, `#api-key-deepl`, `#api-key-deepseek` (API key inputs)
- `#history-search` (search input)

**C. Selects (dropdowns):** styled with monospace font and terminal feel, but NO `>` prefix (selects already have their own visual indicator). Custom dropdown arrow using a `▾` character.

### Section 5 — Engines + API Keys consolidation (the main restructure)

**Current structure** (lines 404-456 of `index.html`):

```html
<label>Translation Engines</label>
<div id="engine-checkboxes">[checkboxes rendered here]</div>
<label for="primary-engine">Primary Engine</label>
<select id="primary-engine">...</select>

<div class="engine-api-key" data-engine="gemini">...API key input...</div>
<div class="engine-api-key" data-engine="deepl">...API key input...</div>
<div class="engine-api-key" data-engine="deepseek">...API key input...</div>
```

**New structure** (consolidated, in this exact order):

```html
<h2>Translation Engines</h2>
<div class="section terminal-section">
    <!-- Languages row (Image 1 distribution) -->
    <div class="form-row">
        <div class="form-group half">
            <label for="source-lang">SOURCE LANG</label>
            <select id="source-lang">...</select>
        </div>
        <div class="form-group half">
            <label for="target-lang">TARGET LANG</label>
            <select id="target-lang">...</select>
        </div>
    </div>

    <!-- Primary engine -->
    <div class="form-group">
        <label for="primary-engine">PRIMARY ENGINE</label>
        <div class="terminal-input">
            <select id="primary-engine">...</select>
        </div>
    </div>

    <!-- Engines + API keys consolidated (rendered by JS) -->
    <div id="engines-list" class="engines-list">
        <!-- JS renders: for each paid engine:
             [x] Enable Gemini
             > [API key input for Gemini]
             ---
             [x] Enable DeepL
             > [API key input for DeepL]
             ---
             [x] Enable DeepSeek
             > [API key input for DeepSeek]
        -->
    </div>

    <!-- Single Test button at the end -->
    <div class="form-group">
        <button id="test-all-keys-btn" class="terminal-btn">[ TEST ALL KEYS ]</button>
        <div id="test-all-status" class="test-status"></div>
    </div>
</div>
```

**Notes on the new structure:**
- The `engines-list` div is populated by a NEW JS function `renderEnginesWithKeys()` that replaces the current separation of "engine checkboxes" + "API key sections"
- Each engine block: checkbox on top, API key input below it (only shown when checkbox is checked)
- API key input uses the `.terminal-input` wrapper for the `>` prefix
- Engines are separated by a thin horizontal line (`border-top: 1px solid var(--border)`)
- The "Test Key" button is **removed** from each engine — replaced by the single `[ TEST ALL KEYS ]` button at the bottom
- The `engine-help-btn` (?) is kept, repositioned next to the test button

**A. JS changes:**
- **NEW function** `renderEnginesWithKeys(enabledEngines, primaryEngine)`: replaces `renderEngineUI()`
- Generates DOM dynamically for each paid engine: checkbox + label + API key input + status
- Toggling a checkbox shows/hides the API key input for that engine
- Removes the existing `.engine-api-key` static divs from the HTML
- The `test-key-btn` event listeners are **removed** (no more per-engine test)
- The single `test-all-keys-btn` calls a new function `testAllEnabledKeys()`:
  - For each engine in `__currentEnabledEngines` (only paid enabled ones):
    - Read its current API key from its input
    - Skip if empty (track as "empty")
    - Otherwise call `invoke('test_api_key', { engine, key })`
  - For each successful test, **automatically save the key** via `invoke('set_api_key', { engine, key })` — this preserves the existing behavior of the per-engine Test buttons (which auto-save on success). This is consistent with current UX and avoids surprising the user.
  - Display results in `#test-all-status`:
    - All empty: "All enabled engines have empty API keys."
    - All valid: "✓ All keys valid and saved"
    - Some failed: "✗ X failed: Gemini, DeepL"
    - Mixed: "✓ 2 valid, ✗ 1 failed"

**B. IDs preserved:** All existing IDs (`#source-lang`, `#target-lang`, `#primary-engine`, `#api-key-gemini`, `#api-key-deepl`, `#api-key-deepseek`, `#api-key-status-gemini`, etc.) are preserved so the save logic (lines 639-650) works without changes. The inputs are just inside new wrapper divs.

**C. Save logic unchanged:** `collectApiKeys()` in saveSettings handler still reads `#api-key-{engine}` inputs by ID. The wrapper doesn't affect this.

### Section 6 — Save button and other buttons

- `#save-btn` (the big bottom button) gets the new terminal look: `[ SAVE SETTINGS ]` style
- `.small-btn` (used throughout for secondary actions like `+ Add Profile`, `Test Key`, `Load More`, etc.) gets the terminal look: `[ ACTION ]` style with monospace font
- `.small-btn.danger` (Clear All, etc.) stays red but in terminal style

### Section 7 — Game Profiles (full redesign)

**Current:** flat form with native inputs, profile cards with badges and tags.

**New:**
- Profile cards: monospace, terminal style with `[EDIT] [DELETE]` action buttons
- Add Profile button: `[ + ADD PROFILE ]` terminal style
- Form inputs: monospace, terminal styling (no `>` prefix on form inputs — only on user-input-style fields, but the form is a multi-field form, so we use terminal-input style on all of them for consistency)
- Override labels: styled as terminal section subheaders (`> OPTIONAL OVERRIDES`)
- Save/Cancel buttons: `[ SAVE ]` and `[ CANCEL ]` terminal style

**Affected elements:** `profileList`, `addProfileBtn`, `profileForm`, all `#profile-*` inputs, `profileSaveBtn`, `profileCancelBtn`, profile cards (rendered by `renderProfiles()` in JS)

### Section 8 — History section (full redesign)

**Current:** flat list with cards, plain search, plain buttons.

**New:**
- Search input: `.terminal-input` style (with `>` prefix)
- Each history entry: monospace, terminal-style line:
  ```
  > "original text here" → "translated text here" [engine] [delete]
  ```
- Load More / Export JSON / Export CSV / Clear All: `.small-btn` terminal style
- Empty state: `> No history entries.` (terminal feel)

**Affects:** `historyList`, `historySearchInput`, `historyLoadMoreBtn`, `historyExportJsonBtn`, `historyExportCsvBtn`, `historyClearBtn`, and the `renderHistoryEntry()` function in JS.

### Section 9 — API Key Help modal (redesign)

**Current:** generic modal with blue accent, rounded corners.

**New:**
- Border: `2px solid var(--terminal)` (green, sharp corners)
- Background: `var(--bg-secondary)` with subtle terminal-style header
- Title: monospace, green, with `> HOW TO GET API KEYS` prefix
- Body: monospace, formatted with terminal-style line breaks
- Close button: `[ X ]` terminal style

**Affects:** `#api-key-modal`, `#api-key-modal-content`, `#api-key-modal-title`, `#api-key-modal-body`, `#api-key-modal-close`.

### Section 10 — Logs panel (redesign as modal with terminal formatting)

**Current:** inline expandable panel, raw text with `white-space: pre-wrap`, hard to read.

**New:**
- Inline "View Logs" button (kept) opens a **modal** (full-screen overlay) when clicked
- Modal styled like the API Key Help modal: green border, monospace, terminal feel
- Log lines are **parsed and color-coded**:
  - Lines containing `ERROR` or `panic` → red text
  - Lines containing `WARN` or `Failed` → yellow text
  - Lines containing `[OK]` or `success` → green text
  - Default → gray text
- Each line prefixed with `>` for terminal feel
- Timestamps in bold/monospace at the start
- Modal has `[ CLOSE ]` and `[ CLEAR ]` buttons at the bottom

**A. JS changes:**
- **NEW function** `openLogsModal()`: hides inline `#log-panel`, shows new `#logs-modal`
- **NEW function** `closeLogsModal()`: reverse
- **NEW function** `formatLogLine(text)`: parses a log string, returns sanitized HTML with appropriate color classes
- `viewLogsBtn` event listener updated to call `openLogsModal()` instead of toggling inline
- The inline `#log-panel` and the inner `#clear-logs-btn` are **REMOVED from the HTML** (no dead code, no fallback). The new `#logs-modal` has its own clear button that calls the same backend function.

**B. Modal HTML:** new `#logs-modal` element added, styled identically to `#api-key-modal`

### Section 11 — Footer (unchanged)

`#app-version` stays as-is. Subtle gray, bottom-right, no terminal styling needed for a secondary label.

### Section 12 — Section dividers and visual rhythm

- Between major sections, add a terminal-style separator:
  ```
  ────────────────────────
  ```
  (a thin line with terminal-dim color)
- Section background: remove the `var(--bg-secondary)` panel feel, use a more open terminal feel (transparent or very subtle bg)
- Padding/spacing adjusted to feel more like a terminal: slightly more line-height, generous margins between sections

## Affected Files

- `src/settings/index.html` — full CSS rewrite (lines 7-336), structural HTML changes (Engines + API Keys consolidation, new logs modal, profile/history/inputs restyle)
- `src/settings/settings.js` — multiple function changes:
  - `renderEngineUI()` → renamed/replaced by `renderEnginesWithKeys()` (line 808)
  - NEW: `renderEnginesWithKeys(enabledEngines, primaryEngine)`
  - NEW: `testAllEnabledKeys()` (replaces per-engine test handlers)
  - `checkEngineKeyStatus()` updated to work with new structure
  - `renderProfiles()` updated for new card style
  - `renderHistoryEntry()` updated for new entry style
  - NEW: `openLogsModal()`, `closeLogsModal()`, `formatLogLine()`
  - Event listener updates: `viewLogsBtn`, `test-all-keys-btn`, engine checkbox changes
- `CHANGELOG.md` — add `[0.9.0]` entry
- `docs/decisions.md` — add ADR-018 documenting the hybrid aesthetic decision
- `src-tauri/tauri.conf.json` — version bump `0.8.6` → `0.9.0`
- This plan file lives at `docs/changes/ui-redesign/plan.md` (per new `docs/changes/<name>/` convention)

## Impact Checklist

- [ ] **Visual: full panel** opens, all sections render with hybrid aesthetic
- [ ] **Visual: checkboxes** show `[ ]` unchecked, `[x]` checked (green) across all checkboxes including dynamically-rendered engine ones
- [ ] **Visual: hotkey inputs** show `>` prefix in green
- [ ] **Visual: API key inputs** show `>` prefix in green
- [ ] **Visual: history search** shows `>` prefix in green
- [ ] **Visual: Save button** shows `[ SAVE SETTINGS ]` terminal style
- [ ] **Visual: small buttons** show `[ ACTION ]` terminal style
- [ ] **Visual: Game Profile cards** show terminal style
- [ ] **Visual: Game Profile form** inputs are monospace + terminal style
- [ ] **Visual: History entries** show as `> original → translated [engine] [delete]`
- [ ] **Visual: API Key Help modal** has green border, monospace content
- [ ] **Visual: Logs modal** opens when clicking "View Logs", shows color-coded log lines
- [ ] **Visual: Section headers** have left-border accent, uppercase
- [ ] **Functional: save settings** works — all values persist (verify by editing source-lang, target-lang, primary engine, all API keys, hotkeys, OCR preproc, history, etc., save, restart, reload)
- [ ] **Functional: single Test button** tests all enabled engines' API keys, shows consolidated status
- [ ] **Functional: per-engine API key inputs** still work (typing, clearing, showing stored value on load)
- [ ] **Functional: profile CRUD** works (add, edit, delete, save, cancel)
- [ ] **Functional: history** works (load, search, load more, export JSON, export CSV, clear all)
- [ ] **Functional: log display** works (open modal, see color-coded logs, close)
- [ ] **Functional: missing API key banner** still appears when paid engines are enabled without keys
- [ ] **Functional: hotkey capture** still works (click input, press keys, value updates)
- [ ] **Functional: settings-changed event** still propagates to other windows
- [ ] **No regression in v0.8.6** (instant freeze flow)
- [ ] **No regression in v0.8.5** (game profile UI hydration)
- [ ] **No regression in v0.8.4** (API key persistence)
- [ ] **No regression in v0.8.3** (CSP, profile hydration, context_prompt)

## Decisions

- **D1 (approach)**: UI-1, full panel redesign in one change. Per user decision.
- **D2 (color)**: Blue base (`#4e9af1`) for general accents, green (`#51cf66`) for terminal-specific cues (checkboxes, `>` prefix, section dividers, success). User decision.
- **D3 (typography)**: Hybrid. h1/h2 in sans-serif (current behavior), all body content (labels, inputs, buttons, profile cards, history entries) in monospace. User decision.
- **D4 (checkboxes)**: Custom HTML+CSS `[x] [ ]` for ALL checkboxes. Native `<input type="checkbox">` stays in DOM for accessibility and form values. User decision.
- **D5 (>` prefix)**: Applied to hotkey inputs, API key inputs, and history search only. NOT to selects, numbers, or form fields. Per architect's call (semantic alignment — `>` = "awaiting user input").
- **D6 (test feedback)**: Single status line below the test button. "All valid", "All empty", or "X failed: engines". User decision (option A).
- **D7 (test scope)**: Tests only ENABLED paid engines. Disabled engines are skipped. Per user decision.
- **D8 (save button)**: Redefined to terminal style `[ SAVE SETTINGS ]`. User decision.
- **D9 (game profiles)**: Full redesign in same change. User decision.
- **D10 (history)**: Full redesign in same change. User decision. History IS implemented (not placeholder), confirmed via grep on renderHistory / renderHistoryEntry.
- **D11 (logs)**: Converted from inline panel to modal. Color-coded lines (red/yellow/green/gray). User decision.
- **D12 (modal)**: Logs modal styled like API Key Help modal (green border, monospace, terminal feel). User decision.
- **D13 (footer)**: Unchanged. Subtle gray, bottom-right. Per user decision.
- **D14 (versioning)**: 0.9.0. Major UX overhaul, justifies minor version bump. User decision.
- **D15 (data flow)**: 100% preserved. No IDs change. No event signatures change for existing events. CSS/HTML/JS only. **EXCEPTION**: The new `[ CLEAR ]` button in the logs modal requires a new `clear_logs` Tauri command (the in-memory log buffer previously had no clear path). This is a 5-line backend addition (`pub fn clear_logs` in `commands.rs` + 1-line registration in `lib.rs`). The plan was originally "no backend changes" but the logs modal's clear feature cannot work without it. This is the minimum viable backend addition to make the feature complete.
- **D16 (no new tests)**: Same as v0.8.5/v0.8.6 — vanilla JS frontend, no test runner, manual testing per Impact Checklist.
- **D17 (no new dependencies)**: Pure CSS, no new npm or crates.
- **D18 (no new Tauri commands)**: The `test_api_key` command is reused per-engine from JS in a loop. No batch command needed.
- **D19 (structure)**: Plan lives at `docs/changes/ui-redesign/plan.md`. New convention for change plans. Future changes will follow `docs/changes/<change-name>/plan.md`.

## Out of Scope

- Touching freeze overlay UI (separate concern)
- Touching result overlay UI (separate concern)
- Touching write overlay UI (separate concern)
- Adding new settings options
- Changing settings data model
- Adding new Tauri commands
- Touching Rust backend
- Touching installer / release workflow
- New keyboard shortcuts
- Light theme / theme switching
- Internationalization (UI is English-only for now)

## Observations (not implemented now)

- **O1**: The `api-key-missing-banner` (lines 342-345) is a notification banner that doesn't fit the terminal aesthetic perfectly. Could be redesigned as a terminal-style alert (`> ⚠ API KEY REQUIRED: ...`) in a future change. Out of scope for v0.9.0 to avoid scope creep.
- **O2**: The active profile banner (lines 347-352) is similar — could be terminal-styled in a future change.
- **O3**: There's no scroll-spy or section navigation. Long settings panels benefit from a sticky nav. Future enhancement, not v0.9.0.
- **O4**: The logs modal is a nice upgrade, but real power users want copy-to-clipboard and filter-by-level. Future enhancement.
- **O5**: The test-all-keys button could show a spinner during testing. Current implementation just shows the result. Acceptable for v0.9.0, can be polished later.
- **O6**: The hybrid aesthetic (sans headings + mono body) is opinionated. Some users might prefer all-sans or all-mono. We can iterate based on feedback.

## Migration Notes

None. Pure visual/UX refresh. Users on v0.8.6 will just see a redesigned Settings panel with the same functionality. All settings, keys, profiles, history data persist unchanged.
