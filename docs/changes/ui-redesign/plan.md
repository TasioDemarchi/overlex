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

---

## Refinements — v0.9.2 (appended to v0.9.0 plan)

### Intent

After shipping v0.9.0, user reviewed the redesign and identified 5 issues that need correction. The hybrid aesthetic is kept (gray base + green terminal accents), but:

1. **Color predominance is wrong** — current implementation is blue/violet predominant (sections are `#16213e` blue, titles use `--accent` blue, etc.). The user wants the **gray palette from "Console Settings" image (Image 2 in original discussion)** to be the predominant color, with terminal green and blue reserved for accents only.

2. **Checkbox state visualization is wrong** — currently shows `[ ]` regardless of state. Should show `[ ]` unchecked, `[x]` checked (lowercase x, per user preference).

3. **Bug: "Error checking key" appears for disabled engines** — the `checkEngineKeyStatus()` function is called for all paid engines regardless of whether their checkbox is checked. The `<small class="engine-status">` element is always visible. Result: every disabled engine shows `X Error checking key: No API key stored for [engine]`.

4. **API key help modal is in the wrong place** — currently a `?` button next to the Primary Engine dropdown. The modal content explains how to get API keys per engine, which is unrelated to the primary engine selection. The help affordance should be next to the engine list (where API keys are configured), not next to the primary engine.

5. **Selects are native browser dropdowns, not terminal style** — the user wants selects to look like `AUTO-DETECT >` / `SPANISH >` (plain text + `>` arrow, no native browser chrome). This applies to: Source Language, Target Language, Primary Engine, Overlay Position. (Profile form selects: Source Lang, Target Lang, Engine — out of scope for now, too much surface area.)

### Scope

#### Fix 1 — Gray predominant palette (color correction)

**Goal**: change the visual hierarchy so gray is the dominant color, with blue and green only as accents on interactive elements.

**Changes to `src/settings/index.html` CSS:**

**A. Update `--bg-*` tokens** in the `:root` block to gray-neutral values:
- `--bg-primary: #1a1a2e` → `#1f2937` (pure gray, no violet tint)
- `--bg-secondary: #16213e` → `#111827` (slightly darker gray, like Image 2)
- `--bg-tertiary: #0f0f1a` → `#0b1220` (darker gray for input backgrounds)

**B. Reduce blue accent usage** — change `color: var(--accent)` and `border-color: var(--accent)` in:
- `h2` selector (line ~54) — section titles become `var(--text-primary)` (gray-white) instead of blue
- All `.small-btn:hover` — change to `border-color: var(--text-secondary)` (gray)
- All `:focus` outlines — keep `var(--accent)` blue (needed for accessibility/focus indication)
- Section dividers/borders — change from `var(--accent)` to `var(--terminal-dim)` (green) or `var(--border-strong)` (gray)

**C. Keep `--accent` blue ONLY for:**
- `:focus` and `:focus-visible` outlines (accessibility)
- `<a>` link colors
- Active profile banner background (it was blue, keep as accent)

**D. Update `border-left` for h2** — keep `var(--terminal-dim)` green (already correct).

#### Fix 2 — Checkbox `[ ]` / `[x]` (state visualization)

**Goal**: make checkboxes show `[x]` when checked, `[ ]` when unchecked.

**Approach**: use CSS `::before` pseudo-element to render the bracket content, so the actual `<span class="cb-display">` text is empty. This avoids touching the JS that sets `cbDisplay.textContent = '[ ]';`.

**Changes to `src/settings/index.html` CSS:**

Replace the existing `.cb-display` rules with:
```css
.terminal-checkbox .cb-display {
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 0.85rem;
    user-select: none;
    min-width: 2.2em;
    flex-shrink: 0;
}
.terminal-checkbox .cb-display::before {
    content: '[ ]';
}
.terminal-checkbox input:checked + .cb-display::before {
    content: '[x]';
}
.terminal-checkbox input:checked + .cb-display {
    color: var(--terminal);
}
.terminal-checkbox:hover .cb-display::before {
    color: var(--accent);
}
.terminal-checkbox input:checked:hover + .cb-display::before {
    color: var(--terminal-dim);
}
.terminal-checkbox input:focus-visible + .cb-display {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 2px;
}
```

**No JS changes needed** — the `cbDisplay.textContent = '[ ]';` in `renderEnginesWithKeys` becomes a no-op since CSS overrides it with `::before`. (Or remove the line for cleanliness.)

**Also update the static HTML checkboxes** (those with hardcoded `<span class="cb-display">[ ]</span>` in `index.html`):
- The hardcoded text `[ ]` is now cosmetic only (CSS overrides). Leave as-is, no edit needed.

#### Fix 3 — Hide "Error checking key" for disabled engines

**Goal**: status messages (`X Error checking key: ...`, `✓ [Engine] API key stored`) should only appear for engines that the user has enabled (checkbox checked). For disabled engines, the entire status line should be hidden.

**Changes to `src/settings/settings.js`:**

**A. In `renderEnginesWithKeys()`** (around line 909):
- Change the loop that calls `checkEngineKeyStatus()` to only run for enabled engines:
  ```js
  PAID_ENGINES.forEach(engine => {
      const cb = document.getElementById(`engine-cb-${engine}`);
      if (cb && cb.checked) {
          checkEngineKeyStatus(engine).catch(e => console.warn(`checkEngineKeyStatus ${engine} failed:`, e));
      }
  });
  ```
- The `checkEngineKeyStatus()` function itself doesn't need changes — it correctly checks the storage and sets status. The issue is only that it was being called for disabled engines.

**B. In `renderEnginesWithKeys()`** (around line 887-890), when creating the status element:
- Initially set `statusEl.style.display = 'none'` so disabled engines don't show a status at all
- Add a `change` listener on the checkbox that shows/hides the status:
  ```js
  cb.addEventListener('change', () => {
      // ... existing code ...
      if (cb.checked) {
          keyRow.classList.add('visible');
          statusEl.style.display = '';
          checkEngineKeyStatus(engine).catch(e => console.warn(...));
      } else {
          keyRow.classList.remove('visible');
          statusEl.style.display = 'none';
          statusEl.textContent = '';
      }
  });
  ```

**C. In `testAllEnabledKeys()`** (around line 1015-1050):
- After testing, the per-engine status is updated (existing code, lines 1024-1042). These lines should also respect the checkbox state — if the engine is unchecked when the test runs, don't update its status. Add `if (cb && cb.checked)` guard before the status update blocks.

#### Fix 4 — Move API key help modal trigger

**Goal**: the help modal should be triggered by a link/button in the engines list area, not next to the Primary Engine dropdown.

**Changes to `src/settings/index.html`:**

**A. Remove the `?` button** from the Primary Engine row (lines 650-655):
```html
<!-- REMOVE this block: -->
<div style="display: flex; gap: 8px; align-items: center;">
    <select id="primary-engine" style="flex: 1;">...</select>
    <button id="engine-help-btn" type="button" ...>?</button>
</div>
```
Replace with just the select:
```html
<select id="primary-engine">
    <!-- Populated dynamically by JS -->
</select>
```

**B. Add a help link above the engines list** (insert before `<div id="engines-list" class="engines-list">`, around line 659):
```html
<div class="engines-help-row">
    <button id="engine-help-btn" class="small-btn">[ HOW TO GET API KEYS ]</button>
</div>
```

**No JS changes needed** for the click handler — it already exists and listens for `engineHelpBtn` clicks. Only the button's location and label change.

#### Fix 5 — Custom terminal-style selects

**Goal**: replace native browser `<select>` dropdowns with custom HTML/CSS/JS components that look like `AUTO-DETECT >` / `SPANISH >` (plain text + `>` arrow, no native chrome).

**Affected selects** (4 total):
- `#source-lang`
- `#target-lang`
- `#primary-engine`
- `#overlay-position`

**Out of scope for v0.9.2** (for size limit, can be a future change):
- Profile form selects (`#profile-source-lang`, `#profile-target-lang`, `#profile-engine`)
- All `<select>` elements in HTML — these keep native chrome

**Approach**: hide the native `<select>`, build a custom dropdown component on top of it that syncs values to the hidden native select. Native select keeps the form values, the custom UI is the visual.

**A. New CSS class `.terminal-select`** in `index.html`:
```css
.terminal-select {
    position: relative;
    display: block;
    width: 100%;
    font-family: var(--font-mono);
    font-size: 0.9rem;
    cursor: pointer;
    user-select: none;
}
.terminal-select .ts-current {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 12px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border-strong);
    border-radius: var(--terminal-radius);
    color: var(--text-primary);
    transition: border-color 0.2s;
}
.terminal-select .ts-current:hover {
    border-color: var(--text-secondary);
}
.terminal-select.open .ts-current {
    border-color: var(--terminal);
}
.terminal-select .ts-current::after {
    content: '>';  /* terminal-style arrow */
    color: var(--terminal);
    font-weight: bold;
    margin-left: 8px;
    transition: transform 0.2s;
}
.terminal-select.open .ts-current::after {
    transform: rotate(90deg);
}
.terminal-select .ts-options {
    display: none;
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    background: var(--bg-secondary);
    border: 1px solid var(--terminal-dim);
    border-radius: var(--terminal-radius);
    margin-top: 2px;
    max-height: 240px;
    overflow-y: auto;
    z-index: 100;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
}
.terminal-select.open .ts-options {
    display: block;
}
.terminal-select .ts-option {
    padding: 8px 12px;
    color: var(--text-primary);
    cursor: pointer;
    transition: background 0.1s;
}
.terminal-select .ts-option:hover {
    background: var(--bg-tertiary);
    color: var(--terminal);
}
.terminal-select .ts-option.selected {
    color: var(--terminal);
    background: var(--bg-tertiary);
}
```

**B. Hide the native `<select>` elements** that get wrapped:
```css
.terminal-select-wrap select {
    position: absolute;
    opacity: 0;
    pointer-events: none;
    width: 0;
    height: 0;
}
```

**C. New JS function `createTerminalSelect(nativeSelect)`** in `settings.js`:
- Wraps a native `<select>` in a `<div class="terminal-select">` with the current value display and options list
- Reads the native select's options to populate the custom list
- On click of an option, updates the native select's value, dispatches a `change` event (so existing handlers like `primaryEngineSelect.addEventListener('change', ...)` still fire)
- Click outside closes the dropdown
- Keyboard support: Enter/Space toggles, Arrow keys navigate, Esc closes (optional for v0.9.2, can be a future enhancement)

**D. Apply to the 4 selects** in `DOMContentLoaded`:
```js
document.querySelectorAll('select[data-terminal-select]').forEach(sel => {
    createTerminalSelect(sel);
});
```

**E. Mark the 4 selects** with `data-terminal-select` attribute in `index.html`:
- `<select id="source-lang" data-terminal-select>`
- `<select id="target-lang" data-terminal-select>`
- `<select id="primary-engine" data-terminal-select>`
- `<select id="overlay-position" data-terminal-select>`

**F. Handle dynamic options**: `#primary-engine` is populated dynamically by `renderPrimaryDropdown()`. The custom select wrapper must re-read the options after this function runs. Add a hook: after `renderPrimaryDropdown()` updates the native select, also re-render the custom dropdown. Simplest: call `createTerminalSelect(primaryEngineSelect)` again (it tears down the old wrapper and creates a new one) or call a new `refreshTerminalSelect(nativeSelect)` function.

### Affected Files (v0.9.2)

| File | Action | Notes |
|------|--------|-------|
| `src/settings/index.html` | Modify | CSS tokens (Fix 1), checkbox CSS (Fix 2), remove ? button + add help link (Fix 4), custom select CSS (Fix 5), add `data-terminal-select` attrs (Fix 5) |
| `src/settings/settings.js` | Modify | Wrap cbDisplay empty (Fix 2), conditional checkEngineKeyStatus + show/hide status (Fix 3), add `createTerminalSelect` function (Fix 5), call it in DOMContentLoaded (Fix 5), refresh after `renderPrimaryDropdown` (Fix 5), guard test-all status updates with `cb.checked` (Fix 3) |
| `CHANGELOG.md` | Modify | Add `[0.9.2]` entry (no breaking changes, refinements over v0.9.0/v0.9.1) |
| `docs/decisions.md` | Modify | Add ADR-021 (refinements: gray predominance, terminal select component) — verify the last ADR number first |
| `src-tauri/tauri.conf.json` | Modify | Version bump `0.9.1` → `0.9.2` |
| `src/settings/index.html` (footer) | Modify | `OverLex v0.9.1` → `OverLex v0.9.2` |
| `docs/changes/ui-redesign/plan.md` | Already appended (this section) | No edit needed, this is the plan |

**Total**: 0 new files, 4 modified files (the same files touched in v0.9.0 + version bump). No backend changes, no new Tauri commands.

### Impact Checklist (v0.9.2)

- [ ] **Color**: section backgrounds are gray (not blue), section titles are gray-white (not blue), accents remain blue (focus) and green (terminal cues)
- [ ] **Color**: profile cards and history entries use gray base with green terminal accents
- [ ] **Checkbox**: shows `[ ]` unchecked, `[x]` checked (lowercase x), green color when checked
- [ ] **Checkbox**: works in all locations (engines list, OCR preproc, history enabled, start with windows, show debug, profile form preproc checkboxes, auto-dismiss)
- [ ] **No error messages for disabled engines**: with all 4 paid engines unchecked, the engines list shows only the checkboxes — no `X Error checking key...` lines
- [ ] **Status appears only when enabled**: check `[x] Enable Gemini` → status line appears with current key state (or "no key stored")
- [ ] **Status hidden when disabled**: uncheck `[ ] Enable Gemini` → status line disappears
- [ ] **Status reappears on re-enable**: check again → status reappears (and re-checks storage)
- [ ] **Help modal trigger moved**: no `?` button next to Primary Engine dropdown; instead `[ HOW TO GET API KEYS ]` button above the engines list
- [ ] **Help modal still works**: click the new button → modal opens with instructions for all 4 engines (gemini, deepl, deepseek, groq)
- [ ] **Source Language select**: custom terminal style, click opens dropdown with all 11 options, click selects, value persists
- [ ] **Target Language select**: same custom style
- [ ] **Primary Engine select**: same custom style; updates dynamically when engines are enabled/disabled; selection persists in `currentSettings.primary_engine`
- [ ] **Overlay Position select**: same custom style
- [ ] **All selects save correctly**: change source-lang, target-lang, primary-engine, overlay-position, save, restart, verify persistence
- [ ] **Primary engine change event still fires**: changing primary engine updates `__currentPrimaryEngine` (used by translation chain)
- [ ] **No regression in v0.9.1** (Groq integration, all 4 paid engines)
- [ ] **No regression in v0.9.0** (UI redesign, terminal aesthetic, logs modal, history/profile restyle)
- [ ] **No regression in v0.8.6** (instant freeze flow)
- [ ] **No regression in v0.8.5/4/3** (settings persistence, profile hydration, context_prompt)

### Decisions (v0.9.2)

- **D20 (approach)**: refinements to v0.9.0 plan, appended here per user request (no new plan file).
- **D21 (color)**: gray predominant, blue only for `:focus` and `<a>` (accessibility + links), green for terminal cues. Per user correction.
- **D22 (checkbox)**: lowercase `[x]` per user preference. CSS `::before` approach, no JS state management.
- **D23 (status visibility)**: only show for enabled engines. Hide entire `<small class="engine-status">` when checkbox is unchecked. Re-check storage on re-enable.
- **D24 (help modal location)**: above the engines list as `[ HOW TO GET API KEYS ]` button. Removed from Primary Engine row.
- **D25 (custom selects)**: build custom dropdown component wrapping native `<select>`. Native select keeps form values, custom UI is visual. 4 selects (source-lang, target-lang, primary-engine, overlay-position) — profile form selects are out of scope.
- **D26 (versioning)**: 0.9.2. Second iteration of UI redesign, no breaking changes, no data migration. Refinements over v0.9.0.
- **D27 (no backend changes)**: all changes are CSS/HTML/JS in the frontend. No Rust changes, no new Tauri commands.
- **D28 (no settings migration)**: all data formats unchanged. Users keep all their settings, engines, API keys, profiles, history.
- **D29 (test scope)**: manual testing per Impact Checklist. No new automated tests.

### Out of Scope (v0.9.2)

- Profile form selects (`#profile-source-lang`, `#profile-target-lang`, `#profile-engine`) — too much surface area for one change. Can be a follow-up.
- Keyboard navigation in custom selects (Arrow keys, Enter, Esc) — mouse-only is acceptable for v0.9.2, can be added later.
- Animation polish (slide-in dropdown, fade-in options) — KISS, instant show/hide is fine.
- Multi-select support (not needed for current selects).
- Search/filter inside dropdowns (not needed — max 11 options).
- Redesign of `#api-key-missing-banner` or `#active-profile-banner` — separate concern, not blocking v0.9.2.
- Light theme / theme switching — not requested.

### Observations (v0.9.2, not implemented now)

- **O7**: The custom select component adds ~80 lines of JS. If we ever add a framework (React, Vue, etc.), this would be replaced by a library component. For now, vanilla JS is fine.
- **O8**: The `:focus` blue accent is the only place where blue is still prominent in the UI. If the user wants even less blue, we could replace with green (`var(--terminal)`), but that would reduce contrast on focus indicators. Current decision: keep blue for accessibility.
- **O9**: The Profile form's `Add Profile` modal has several native selects (Source Lang, Target Lang, Engine). For consistency, these should also become custom selects in a future change. But for v0.9.2, only the 4 main settings selects are converted to avoid scope creep.
- **O10**: The custom select component does not support optgroups. None of the 4 affected selects use optgroups, so this is not a problem. If a future select needs grouping, the component will need extension.

### Migration Notes (v0.9.2)

None. Pure visual/UX refinements. Users on v0.9.1 keep all their current settings, engines, API keys, profiles, history. The UI just looks different (more gray, custom selects, fixed checkbox visualization, fixed status visibility, help modal in new location).

---

## Refinements — v0.9.2 (appended to v0.9.0 plan, continued)

### Custom window controls for main settings window (additional refinement)

#### Intent

The `main` (Settings) window currently uses native Windows decorations: title bar with OverLex title, system buttons (minimize, maximize, close). The user wants to remove the native title bar and replace it with a custom title bar that matches the hybrid terminal aesthetic, with two custom buttons:

- `[ — ]` minimize → real minimize to taskbar
- `[ X ]` close → hide (not exit, not destroy — the window stays in memory, re-opened from system tray)

No maximize button (the window is not resizable anymore per user decision). The other 3 windows (`freeze`, `result`, `write`) are unaffected — they already have `decorations: false` and `freeze` is fullscreen; `result` and `write` already have custom drag bars.

#### Scope

##### Change 1 — `tauri.conf.json`: disable decorations and resizing on `main`

**File**: `src-tauri/tauri.conf.json`, line 14-23 (main window block).

**Changes**:
- Add `"decorations": false` to disable the native title bar
- Change `"resizable": true` to `"resizable": false` to prevent maximize (per user decision D-A)

```json
{
    "label": "main",
    "title": "OverLex Settings",
    "width": 600,
    "height": 500,
    "resizable": false,
    "visible": false,
    "center": true,
    "decorations": false,
    "url": "settings/index.html"
}
```

##### Change 2 — `index.html`: add custom title bar

**File**: `src/settings/index.html`.

**A. New HTML element** at the top of `<body>`, before any other content:
```html
<div class="window-titlebar">
    <span class="window-title">OverLex Settings</span>
    <div class="window-controls">
        <button class="window-btn minimize-btn" title="Minimize" aria-label="Minimize">[ — ]</button>
        <button class="window-btn close-btn" title="Close" aria-label="Close">[ X ]</button>
    </div>
</div>
```

**B. New CSS** for the title bar (added to the `<style>` block):
```css
/* Window title bar — custom, replaces native decorations */
.window-titlebar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 32px;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-strong);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    z-index: 1000;
    user-select: none;
    -webkit-user-select: none;
    font-family: var(--font-mono);
}
.window-title {
    color: var(--text-secondary);
    font-size: 0.85rem;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
}
.window-controls {
    display: flex;
    gap: 4px;
}
.window-btn {
    background: transparent;
    border: 1px solid var(--border-strong);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 0.75rem;
    padding: 2px 8px;
    cursor: pointer;
    border-radius: var(--terminal-radius);
    transition: background 0.1s, color 0.1s, border-color 0.1s;
    line-height: 1;
}
.window-btn:hover {
    background: var(--bg-secondary);
    color: var(--text-primary);
    border-color: var(--text-secondary);
}
.window-btn.close-btn:hover {
    background: rgba(255, 107, 107, 0.15);
    color: var(--error);
    border-color: var(--error);
}
```

**C. Adjust existing `body` padding** to account for the new fixed title bar (32px tall + some breathing room):
```css
body {
    /* existing rules */
    padding: 52px 20px 20px;  /* was: padding: 20px; */
}
```

**D. Drag the window by the title bar**: add `app-region: drag` to the title bar and `app-region: no-drag` to the control buttons so dragging works on the title bar but not on the buttons. (Tauri uses CSS `app-region` for this in v2, OR we use the existing `drag_result_window_noactivate` pattern from `result.js`.)

Best approach: use the existing `start_dragging` Tauri API. Reuse the same pattern as `result.js`:
```javascript
// In settings.js
const titleBar = document.querySelector('.window-titlebar');
titleBar.addEventListener('mousedown', async (e) => {
    if (e.target.closest('.window-btn')) return; // don't drag when clicking buttons
    try {
        const win = window.__TAURI__.window.getCurrentWindow();
        await win.startDragging();
    } catch (err) {
        console.error('Failed to start dragging:', err);
    }
});
```

**E. Exclude buttons from drag**: the mousedown check `e.target.closest('.window-btn')` prevents the drag from starting when clicking a control button.

##### Change 3 — `settings.js`: wire up button click handlers

**File**: `src/settings/settings.js`.

**In the `DOMContentLoaded` handler** (around line 691), add:

```javascript
// Window controls
const minimizeBtn = document.querySelector('.window-titlebar .minimize-btn');
const closeBtn = document.querySelector('.window-titlebar .close-btn');
if (minimizeBtn) {
    minimizeBtn.addEventListener('click', async () => {
        try {
            const win = window.__TAURI__.window.getCurrentWindow();
            await win.minimize();
        } catch (err) {
            console.error('Failed to minimize window:', err);
        }
    });
}
if (closeBtn) {
    closeBtn.addEventListener('click', async () => {
        try {
            // Hide the main window (not exit). The system tray can re-open it.
            // This matches the behavior of the native close button + prevent_close()
            // that the lib.rs on_window_event handler implements.
            await window.__TAURI__.core.invoke('hide_window', { label: 'main' });
        } catch (err) {
            console.error('Failed to hide window:', err);
        }
    });
}

// Window dragging via title bar
const titleBar = document.querySelector('.window-titlebar');
if (titleBar) {
    titleBar.addEventListener('mousedown', async (e) => {
        if (e.target.closest('.window-btn')) return;
        try {
            const win = window.__TAURI__.window.getCurrentWindow();
            await win.startDragging();
        } catch (err) {
            console.error('Failed to start dragging:', err);
        }
    });
}
```

**No new Tauri commands needed** — Tauri 2's `window.minimize()` and `window.startDragging()` are part of the standard `__TAURI__.window` API. The existing `hide_window` Tauri command is reused for the close action.

**Verify permissions in `capabilities/default.json`**: the `core:window:default` permission should already include `minimize` and `start-dragging`. If not, add:
```json
"core:window:allow-minimize",
"core:window:allow-start-dragging",
```

(These are already implied by `core:window:default` in Tauri 2, but explicit listing is safer.)

##### Change 4 — `lib.rs`: no changes needed

The existing `on_window_event` handler (lines 662-668) already prevents the close and hides the window:
```rust
.on_window_event(|window, event| {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == "main" {
            api.prevent_close();
            let _ = window.hide();
        }
    }
})
```

This handler is for the **native close button** (which no longer exists once `decorations: false`). However, it remains as a safety net for any case where the window receives a close request via other means (e.g., programmatic close, OS shutdown signal). **Leave as-is, no change.**

##### Change 5 — System tray: no changes needed

The system tray menu (in `tray.rs`) already has options to show the main window. When the user clicks the tray icon and selects "Show Settings", the `main` window is unhidden. This works regardless of whether the window has decorations or not. **No change needed.**

#### Affected Files (v0.9.2 cumulative, including this change)

| File | Action | Notes |
|------|--------|-------|
| `src-tauri/tauri.conf.json` | Modify | Add `decorations: false`, change `resizable: true` → `false` on main window |
| `src-tauri/capabilities/default.json` | Verify (add if missing) | Verify `core:window:allow-minimize` and `core:window:allow-start-dragging` are in the default permissions |
| `src/settings/index.html` | Modify | Add `.window-titlebar` HTML at top of `<body>`, add CSS for title bar, adjust body padding |
| `src/settings/settings.js` | Modify | Add click handlers for minimize and close buttons, add mousedown handler for title bar drag |
| `CHANGELOG.md` | Modify | Add `[0.9.2]` entry (includes both the 5 UI fixes from previous section AND this title bar change) |
| `docs/decisions.md` | Modify | Add ADR-021 documenting the custom title bar decision (verify the last ADR number first) |
| `src-tauri/tauri.conf.json` | Modify | Version bump `0.9.1` → `0.9.2` |
| `src/settings/index.html` (footer) | Modify | `OverLex v0.9.1` → `OverLex v0.9.2` |

**Total**: 0 new files, 5 modified files (1 backend config, 1 capabilities, 1 HTML, 1 JS, 1 changelog, 1 decisions, 2 in tauri.conf.json for both version and decorations). No new Rust code, no new Tauri commands.

#### Impact Checklist (cumulative for v0.9.2)

- [ ] **Color predominance**: settings panel uses gray base, blue only on focus rings, green for terminal cues
- [ ] **Checkbox state**: `[ ]` unchecked, `[x]` checked (green when checked)
- [ ] **No error for disabled engines**: engines list shows no error messages when checkboxes are unchecked
- [ ] **Status appears only for enabled engines**: status line hidden when checkbox is unchecked, appears on check
- [ ] **Help modal moved**: no `?` next to primary engine, `[ HOW TO GET API KEYS ]` button above engines list
- [ ] **Custom selects**: source-lang, target-lang, primary-engine, overlay-position use custom terminal style with `>` arrow
- [ ] **Title bar removed**: main settings window no longer has native Windows title bar
- [ ] **Custom title bar visible**: at top of settings window with "OverLex Settings" text + 2 buttons
- [ ] **Custom close button**: clicking `[ X ]` hides the window (not exits the app)
- [ ] **Custom minimize button**: clicking `[ — ]` minimizes the window to the taskbar
- [ ] **No maximize button**: cannot resize or maximize the settings window (resizable: false)
- [ ] **Window drag**: clicking and dragging the title bar moves the window
- [ ] **Buttons don't trigger drag**: clicking a button on the title bar does NOT start a drag
- [ ] **System tray still works**: right-clicking the tray icon shows the menu with "Show Settings" option, clicking it shows the hidden window
- [ ] **Minimize to taskbar**: after minimizing, the OverLex icon appears in the taskbar; clicking it restores the window
- [ ] **No regression in v0.9.1** (Groq integration, all 4 paid engines)
- [ ] **No regression in v0.9.0** (UI redesign, terminal aesthetic, logs modal, history/profile restyle)
- [ ] **No regression in v0.8.6** (instant freeze flow)
- [ ] **No regression in v0.8.5/4/3** (settings persistence, profile hydration, context_prompt)
- [ ] **No regression in freeze/result/write windows**: these windows are unchanged (still have `decorations: false` already, no custom title bar added to them)

#### Decisions (v0.9.2 cumulative, this section)

- **D30 (approach)**: refinements to v0.9.0 plan, second batch. Appended here per user request (no new plan file).
- **D31 (title bar location)**: only the `main` (Settings) window gets a custom title bar. The other 3 windows (`freeze`, `result`, `write`) are unchanged — they already have `decorations: false` and either are fullscreen (freeze) or have their own custom drag bars (result, write).
- **D32 (decorations)**: set `decorations: false` on the `main` window in `tauri.conf.json`. The native Windows title bar is removed.
- **D33 (resizable)**: set `resizable: false` on the `main` window. Maximize is not possible by any means (no button, no drag-to-edge). Per user decision A.
- **D34 (close behavior)**: clicking the custom close button calls `hide_window` Tauri command (already exists). Window is hidden, not destroyed. User can re-open via system tray. This matches the existing behavior of the native close button + `prevent_close()`. Per user decision B.
- **D35 (minimize behavior)**: clicking the custom minimize button calls `window.minimize()` from the Tauri 2 frontend API. Window minimizes to the Windows taskbar. User can restore by clicking the taskbar icon. Per user decision C.
- **D36 (drag)**: use Tauri's `window.startDragging()` API. Triggered by `mousedown` on the title bar, but excluded when the mousedown is on a control button (`.window-btn`). Reuses the same pattern as `result.js` for the result window's drag bar.
- **D37 (button labels)**: `[ — ]` for minimize (em-dash for "minimize line") and `[ X ]` for close. Matches the terminal aesthetic of other buttons (`[ SAVE SETTINGS ]`, `[ TEST ALL KEYS ]`).
- **D38 (button styling)**: subtle by default (gray border, gray text), red on hover for close button (signals destructive action), neutral hover for minimize. Matches the danger button pattern used for "Clear All" in the History section.
- **D39 (capabilities)**: verify `core:window:allow-minimize` and `core:window:allow-start-dragging` are in `capabilities/default.json`. If not, add them. These are typically included in `core:window:default` but explicit listing is safer.
- **D40 (no backend changes)**: zero Rust changes. The `on_window_event` handler in `lib.rs` is left as a safety net for programmatic close requests. No new Tauri commands.

#### Out of Scope (v0.9.2 cumulative, this section)

- Adding a custom title bar to `freeze`, `result`, or `write` windows — they have different needs (fullscreen, overlay with custom drag, etc.)
- Adding minimize/maximize to non-main windows — out of scope, those windows use `hide_window` instead
- Replacing the system tray with a custom UI — separate concern
- Adding app icon to the title bar (left of the title text) — KISS, can be added later
- Double-click on title bar to maximize — disabled because `resizable: false`
- Right-click context menu on title bar (move, size, etc.) — disabled because `decorations: false` removes the native context menu too
- Keyboard shortcuts (Alt+F4, etc.) for close — already handled by `on_window_event` (prevent_close)

#### Observations (v0.9.2 cumulative, this section)

- **O11**: With `decorations: false` and `resizable: false`, the main window cannot be resized by the user. This is fine for a settings panel (the content is designed for a fixed width of 600px), but if the user ever needs to fit a wider screen, the window size can be changed in `tauri.conf.json` and the user would need to restart the app. Alternative: implement custom resize handles on the edges — but that's premature for v0.9.2.
- **O12**: The `[ — ]` em-dash character is a stylistic choice for the minimize button. Some users might find it unclear. An alternative would be `[ _ ]` (underscore) which is the classic Windows convention, but em-dash matches the heavier terminal aesthetic better.
- **O13**: When the window is minimized, the system tray icon is still active. If the user wants the app to fully exit on minimize, that's a different behavior (some apps do this). Per user decision, minimize is real minimize (to taskbar), not exit.
- **O14**: The `drag_result_window_noactivate` Tauri command in `commands.rs` is a custom Win32 implementation specifically for the result window (which has `WS_EX_NOACTIVATE` to avoid stealing focus from the game). The main settings window does NOT have this constraint, so we use Tauri's standard `startDragging()` API instead. Different APIs for different windows, both valid for their use case.

#### Migration Notes (v0.9.2 cumulative, this section)

None. Pure visual/UX refinements. Users on v0.9.1 keep all their current settings, engines, API keys, profiles, history. The UI just looks different:
- More gray, less blue
- Custom checkbox visualization (`[x]` when checked)
- No error messages for disabled engines
- Help modal in new location
- Custom terminal-style selects
- Custom title bar with minimize and close buttons

The app still behaves the same way: system tray to show/hide the main window, hotkeys for OCR and write modes, same settings persistence, same translation engines, same data flow.
