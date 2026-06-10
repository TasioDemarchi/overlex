# Plan: groq-integration (v0.9.1)

## Intent

Add Groq as a new paid translation engine alongside Gemini, DeepL, and DeepSeek. Groq provides a generous free tier (6K TPM, 500K TPD on `llama-3.1-8b-instant`) and extremely fast inference (840 TPS), making it a low-cost option for users who don't want to pay for DeepSeek tokens but still want an AI-powered translation with context support.

**This is an additive change (G3 from the analysis)** — no existing engines are removed, no defaults are changed, no settings migration is required. Users opt-in by adding Groq to their `enabled_engines` and providing an API key. The new engine is fully integrated into the translation chain, the API key test flow, and the Settings UI.

**Model**: `llama-3.1-8b-instant` — 8B parameters, fast (840 TPS), generous free tier. If quality is insufficient after real-world testing, can be swapped to `llama-3.3-70b-versatile` in a future change.

**Groq API is OpenAI-compatible** (`https://api.groq.com/openai/v1/chat/completions`) with the same `messages` array format and `Authorization: Bearer` header as DeepSeek. This means the adapter is ~95% identical to `deepseek.rs` — only the URL, model name, and engine name differ.

## Scope

### Change 1 — New `GroqAdapter` translation engine

**Create** `src-tauri/src/translation/groq.rs` (new file, ~280 lines).

**Structure**: copy `deepseek.rs` as a template, adapt the following:

- **Module doc comment** (top of file):
  ```rust
  // Groq adapter — OpenAI-compatible chat completions via Groq LPU
  // POST https://api.groq.com/openai/v1/chat/completions
  // Auth: Authorization: Bearer {api_key}
  // Model: llama-3.1-8b-instant (8B, fast, free tier)
  ```

- **Struct name**: `GroqAdapter` (instead of `DeepSeekAdapter`)
- **Field**: `api_key: Option<String>`, `client: Client` (same as DeepSeek)
- **`new()` constructor**: identical to DeepSeek (15s timeout client)
- **`language_name()` method**: identical to DeepSeek — the function is a copy of the same language code → human name table
- **`build_system_instruction()` method**: identical to DeepSeek — game context prompt construction is the same, since both APIs accept the same `messages` format

- **`translate()` method** (the core):
  - URL: `https://api.groq.com/openai/v1/chat/completions`
  - Model: `llama-3.1-8b-instant`
  - Headers: `Authorization: Bearer {key}`, `Content-Type: application/json`
  - Body: identical structure to DeepSeek:
    ```json
    {
      "model": "llama-3.1-8b-instant",
      "messages": [
        {"role": "system", "content": system_instruction},
        {"role": "user", "content": text}
      ],
      "temperature": 0.1,
      "max_tokens": 4096
    }
    ```
  - HTTP status handling: identical to DeepSeek (429 → RateLimit, 401/403 → InvalidApiKey, 5xx → ServiceDown, others → Network)
  - Response parsing: identical to DeepSeek (`json["choices"][0]["message"]["content"]`)

- **`name()` method**: returns `"Groq"`
- **`requires_api_key()` method**: returns `true` (Groq requires an API key, even on free tier)

- **Unit tests** (bottom of file, in `mod tests`):
  - `test_adapter_creation()` — with key
  - `test_adapter_creation_no_key()` — without key
  - `test_system_instruction_with_full_context()` — game + profile
  - `test_system_instruction_with_process_only()` — game only
  - `test_system_instruction_without_context()` — no game
  - `test_system_instruction_empty_context_fields()` — empty context

  All 6 tests are direct copies of DeepSeek's tests with `GroqAdapter` substituted. The system instruction logic is identical between the two engines.

### Change 2 — Register Groq in the translation module

**File**: `src-tauri/src/translation/mod.rs`

**A. Module declarations** (lines 1-9):
- Add `mod groq;` alongside `mod deepseek;`
- Add `pub use groq::GroqAdapter;` alongside `pub use deepseek::DeepSeekAdapter;`

**B. Engine classification constants** (lines 26-28):
- Add `"groq"` to `PAID_ENGINES`: `pub const PAID_ENGINES: &[&str] = &["gemini", "deepl", "deepseek", "groq"];`
- Add `"groq"` to `ALL_ENGINES`: `pub const ALL_ENGINES: &[&str] = &["google_gtx", "mymemory", "gemini", "deepl", "deepseek", "groq"];`
- `FREE_ENGINES` is unchanged (Groq requires a key)

**C. `create_engine_internal()` function** (lines 94-146):
- Add a new `match` arm for `"groq"`, copy the DeepSeek arm as a template:
  ```rust
  "groq" => {
      let api_key = api_key_override
          .map(|s| s.to_string())
          .or_else(|| crate::settings::get_api_key("groq").ok());
      app_log!(
          "[ENGINE] Creating Groq Llama 3.1 8B Instant engine (API key: {})",
          match &api_key {
              Some(k) => format!("present ({} chars, starts with {}...)", k.len(), &k[..k.len().min(8)]),
              None => "NOT FOUND — save the API key in Settings first".to_string(),
          }
      );
      Box::new(GroqAdapter::new(api_key))
  }
  ```

**D. `create_engine()` doc comment** (line 149-150):
- Update to mention Groq: "Supports: google_gtx (default, free), mymemory (free), gemini (requires API key), deepl (requires API key), deepseek (requires API key), groq (requires API key)."

**E. Unit tests** (lines 183-261):
- Add a `test_create_engine_groq` test, copy of `test_create_engine_deepseek` (line 254-260), with `"groq"` substituted:
  ```rust
  #[test]
  fn test_create_engine_groq() {
      let mut settings = Settings::default();
      settings.primary_engine = "groq".to_string();
      let engine = create_engine(&settings, None);
      assert_eq!(engine.name(), "Groq");
      assert!(engine.requires_api_key());
  }
  ```

### Change 3 — Add Groq to `test_api_key` command

**File**: `src-tauri/src/commands.rs`

**Location**: inside `test_api_key()` function (lines 1175-1334), add a new `match` arm for `"groq"` between the `"deepseek"` arm (line 1287) and the catch-all `"_"` (line 1330).

**Implementation**: copy the `"deepseek"` arm as a template and adapt:

```rust
"groq" => {
    // Test Groq API with a minimal request (OpenAI-compatible chat completions)
    let url = "https://api.groq.com/openai/v1/chat/completions";

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let body = serde_json::json!({
        "model": "llama-3.1-8b-instant",
        "messages": [{"role": "user", "content": "Hi"}],
        "max_tokens": 10
    });

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if status.is_success() {
        Ok(TestApiKeyResult {
            success: true,
            message: "Groq API key is valid and working".to_string(),
        })
    } else {
        let body = response.text().await.unwrap_or_default();
        match status.as_u16() {
            401 | 403 => Ok(TestApiKeyResult {
                success: false,
                message: "Invalid API key. Get one at https://console.groq.com/keys".to_string(),
            }),
            429 => Ok(TestApiKeyResult {
                success: false,
                message: "Rate limit hit during test. The key is valid but you're exceeding Groq's free tier limits (6K TPM). Wait and retry.".to_string(),
            }),
            _ => Ok(TestApiKeyResult {
                success: false,
                message: format!("Error: {} - {}", status, body),
            }),
        }
    }
}
```

**Note on 429 handling**: unlike DeepSeek which returns a generic `RateLimit` error from `test_api_key`, Groq on free tier can hit 429 even with a valid key (rate limit per organization, not per key). The test should NOT mark the key as invalid in this case — it should report "rate limited, key is valid" so users know the key works but they're hitting free tier limits. This is more user-friendly than the DeepSeek implementation.

### Change 4 — Update CSP to allow Groq API calls

**File**: `src-tauri/tauri.conf.json`

**Location**: line 71, in the `csp` field of the `security` block.

**Change**: add `https://api.groq.com` to the `connect-src` directive.

**Current**:
```
connect-src 'self' https://*.googleapis.com https://api.mymemory.translated.net https://generativelanguage.googleapis.com https://api.deepl.com https://api.deepseek.com
```

**New**:
```
connect-src 'self' https://*.googleapis.com https://api.mymemory.translated.net https://generativelanguage.googleapis.com https://api.deepl.com https://api.deepseek.com https://api.groq.com
```

This is the same fix that was applied in v0.8.3 to allow paid engines through the CSP — same pattern, just for a new host.

### Change 5 — Update Settings UI to include Groq

**File**: `src/settings/settings.js`

**A. Engine constants** (lines 7-17):
- Add `'groq'` to `ALL_ENGINES`: `const ALL_ENGINES = ['google_gtx', 'mymemory', 'gemini', 'deepl', 'deepseek', 'groq'];`
- Add `'groq'` to `PAID_ENGINES`: `const PAID_ENGINES = ['gemini', 'deepl', 'deepseek', 'groq'];`
- Add label to `ENGINE_LABELS`: `groq: 'Groq',`

**B. `renderEnginesWithKeys()` function** (the function that was added in v0.9.0):
- This function iterates `PAID_ENGINES` to render the engine UI. Since we added `'groq'` to `PAID_ENGINES`, it will automatically render a Groq block with checkbox + API key input. **No change needed here** — the function is already dynamic.

**C. Profile form engine dropdown** (HTML, around line 574-578 in `index.html`):
- Add `<option value="groq">Groq (Requires API Key)</option>` to the `#profile-engine` select element

**D. `collectApiKeys()` in save handler** (around line 666):
- Already iterates `PAID_ENGINES` to collect API keys from `#api-key-{engine}` inputs. Since we added `'groq'` to `PAID_ENGINES`, the save logic will automatically pick up the Groq key. **No change needed.**

**E. API Key Help modal** (HTML, around line 651-657):
- The modal is populated dynamically by JS based on which engine's help button was clicked. Add a new entry to the `API_KEY_HELP` object (in `settings.js`, around line 971+):
  ```javascript
  groq: {
      title: 'Get Groq API Key (Free)',
      content: `
          <p>Groq offers a generous free tier with high-speed inference on Llama models.</p>
          <ol>
              <li>Go to <a href="https://console.groq.com/keys" target="_blank">Groq Console</a></li>
              <li>Sign in with your Google or GitHub account</li>
              <li>Click <strong>"Create API Key"</strong></li>
              <li>Copy the generated key (starts with <code>gsk_</code>)</li>
              <li>Paste it here and click Save</li>
          </ol>
          <div class="api-key-note">
              <strong>Free tier includes:</strong> 6K tokens/min, 500K tokens/day on llama-3.1-8b-instant. No credit card required.
          </div>
      `
  }
  ```

### Change 6 — Documentation

**A. `CHANGELOG.md`**: add new entry at the top (before `[0.9.0]`):
```markdown
## [0.9.1] - 2026-06-10

### Added
- Groq translation engine (model: `llama-3.1-8b-instant`). OpenAI-compatible API, free tier with generous rate limits (6K TPM, 500K TPD). Add your Groq API key in Settings > Translation Engines to enable. Groq appears as a new paid engine alongside Gemini, DeepL, and DeepSeek — opt-in only, not enabled by default.
- API key help modal now includes Groq setup instructions.

### Notes
- Additive change. No existing engines removed, no defaults changed, no settings migration required. Users on v0.9.0 keep all their current settings; Groq is just an additional option in the engine list.
- DeepSeek remains fully supported. Groq is added as an alternative for users who want a free tier or faster inference.
```

**B. `docs/decisions.md`**: add ADR-020 at the end (verify the last ADR number in the file before adding; ADR-019 was the docs/changes/ convention from v0.9.0):
- **Title**: "Add Groq as alternative paid translation engine"
- **Context**: DeepSeek is paid (token-based) and can be cost-prohibitive for some users. Groq offers a generous free tier with comparable quality on 8B models. User wants to try Groq without removing DeepSeek. AHA principle: don't remove until 3+ use cases justify it. Additive change (G3 from analysis) is lowest-risk.
- **Decision**: Add Groq as a new paid engine with `llama-3.1-8b-instant` model. OpenAI-compatible API means adapter is ~95% identical to DeepSeek. Users opt-in by adding Groq to `enabled_engines` and providing an API key.
- **Consequences**:
  - One more option in the engine dropdown (4 paid engines total: Gemini, DeepL, DeepSeek, Groq)
  - CSP updated to allow `https://api.groq.com`
  - `test_api_key` command now supports 4 engines (was 3)
  - 429 handling on test differs from DeepSeek (reports "rate limited but key valid" instead of generic failure) — better UX for free tier users
  - 6 new unit tests in `groq.rs` (5 adapter tests + 1 engine factory test)
  - Zero settings migration (additive)
  - If Groq quality is insufficient after real-world testing, model can be swapped to `llama-3.3-70b-versatile` in a future change (different `model` field, same API)
- **Alternatives considered**:
  - G1: Replace DeepSeek with Groq — rejected because user has not yet validated Groq quality; AHA principle says don't remove without evidence
  - G2: Add Groq as free engine (no key required) — rejected because it would silently fail for users without keys, creating a poor UX footgun
  - G4: Remove DeepSeek, don't replace — rejected because it reduces user choice without adding value

**C. `docs/changes/groq-integration/plan.md`**: this file (the plan artifact, per the `docs/changes/<name>/` convention from v0.9.0)

### Change 7 — Version bump

**File**: `src-tauri/tauri.conf.json`

- Change `"version": "0.9.0"` to `"version": "0.9.1"` (line 4)

**File**: `src/settings/index.html`

- Update footer: `OverLex v0.9.0` → `OverLex v0.9.1` (line 890)

## Affected Files

| File | Action | Lines |
|------|--------|-------|
| `src-tauri/src/translation/groq.rs` | **CREATE** | ~280 (new file) |
| `src-tauri/src/translation/mod.rs` | Modify | +15 lines (module decl, constants, factory case, test) |
| `src-tauri/src/commands.rs` | Modify | +50 lines (new test_api_key match arm) |
| `src-tauri/tauri.conf.json` | Modify | 1 line (CSP) + 1 line (version) |
| `src/settings/settings.js` | Modify | +20 lines (engine constants, ENGINE_LABELS, API_KEY_HELP entry) |
| `src/settings/index.html` | Modify | +1 line (profile engine option) + 1 line (footer version) |
| `CHANGELOG.md` | Modify | +12 lines (0.9.1 entry) |
| `docs/decisions.md` | Modify | +25 lines (ADR-020) |
| `docs/changes/groq-integration/plan.md` | **CREATE** | this file (plan artifact) |

**Total**: 2 new files, 7 modified files, ~400 lines added (including tests and docs).

## Impact Checklist

- [ ] **Backend: `GroqAdapter` compiles** with no warnings (all fields used, all match arms covered)
- [ ] **Backend: 6 unit tests pass** in `groq.rs` (5 adapter tests + 1 engine factory test)
- [ ] **Backend: existing unit tests still pass** (no regressions in `mod.rs` tests or other adapters)
- [ ] **Backend: `create_all_engines` works** when `groq` is in `enabled_engines` (with and without API key)
- [ ] **Backend: `test_api_key("groq", key)` works** end-to-end:
  - Empty key → returns `success: false, "API key is empty"`
  - Valid key → returns `success: true`
  - Invalid key (401/403) → returns `success: false, "Invalid API key..."`
  - Rate limited (429) → returns `success: false, "Rate limit hit... key is valid..."` (different from DeepSeek)
  - Server error (5xx) → returns `success: false, "Error: ..."`
- [ ] **CSP: Groq API calls not blocked** — open Settings with DevTools, check console for CSP violations
- [ ] **Settings UI: Groq appears in the engine list** (checkbox + API key input) when Settings is opened
- [ ] **Settings UI: `[ TEST ALL KEYS ]` tests Groq** if enabled with a key
- [ ] **Settings UI: API key help modal shows Groq instructions** when `?` is clicked
- [ ] **Settings UI: Groq can be selected as primary engine** in the Primary Engine dropdown
- [ ] **Settings UI: Groq can be selected per profile** in the Game Profile form engine dropdown
- [ ] **Functional: translation works end-to-end with Groq as primary** — translate a Japanese game text to Spanish, verify the result is coherent
- [ ] **Functional: translation works with Groq as fallback** — disable primary engine, set Groq as fallback, verify the chain reaches it when others fail
- [ ] **Functional: settings persist across restart** — enable Groq, save, restart app, verify it's still enabled with the same key
- [ ] **No regression in v0.9.0** (UI redesign, all sections still work, all engines still testable)
- [ ] **No regression in v0.8.6** (instant freeze flow)
- [ ] **No regression in v0.8.5** (game profile UI hydration)
- [ ] **No regression in v0.8.4** (API key persistence)
- [ ] **No regression in v0.8.3** (CSP for other engines, profile hydration, context_prompt)

## Decisions

- **D1 (approach)**: G3 — additive change. No engine removal, no defaults change, no migration. Lowest risk, allows empirical comparison with DeepSeek.
- **D2 (model)**: `llama-3.1-8b-instant` — 8B parameters, 840 TPS, generous free tier (6K TPM, 500K TPD). Can be swapped to 70B if quality is insufficient (single field change in `groq.rs`).
- **D3 (engine classification)**: Groq is PAID — requires an API key, even on free tier. NOT auto-enabled. NOT in `FREE_ENGINES`. Users opt-in by adding to `enabled_engines` and providing a key. This avoids the footgun of silent 401s for users without keys.
- **D4 (CSP)**: Add `https://api.groq.com` to `connect-src` in `tauri.conf.json`. Same pattern as v0.8.3 fix for other paid engines.
- **D5 (test_api_key 429 handling)**: Different from DeepSeek. Groq on free tier can hit 429 even with a valid key. Test reports "key valid but rate limited" instead of generic failure. Better UX for free tier users who might be testing during peak hours.
- **D6 (API key help modal)**: Add Groq entry to the `API_KEY_HELP` object. Same pattern as Gemini, DeepL, DeepSeek. Includes the `https://console.groq.com/keys` URL and notes about the free tier.
- **D7 (versioning)**: 0.9.1 — patch on 0.9.0. Additive change, no breaking changes, no migration. Minor version bump appropriate per semver (new feature, backward compatible).
- **D8 (data flow)**: Zero changes to data flow. Existing settings with `enabled_engines = ["deepseek"]` continue to work. Adding Groq is opt-in via Settings.
- **D9 (no new Tauri commands)**: `test_api_key` is reused, just with a new match arm. No new commands, no new events.
- **D10 (no new tests in settings.js)**: Same as v0.8.5/v0.8.6/v0.9.0 — vanilla JS frontend, no test runner, manual testing per Impact Checklist.
- **D11 (no settings migration)**: Confirmed not needed. Additive change means old settings files deserialize unchanged.
- **D12 (file structure)**: Plan lives at `docs/changes/groq-integration/plan.md` per the convention established in v0.9.0 (ADR-019).
- **D13 (no Rust dependencies added)**: Groq uses the same `reqwest`, `serde_json`, `async_trait` as DeepSeek. No new crates.

## Out of Scope

- Removing DeepSeek (deliberately deferred until Groq quality is validated)
- Swapping to `llama-3.3-70b-versatile` (current 8B is the starting point; 70B is a future change if needed)
- Adding Groq-specific features (prompt caching, vision models, etc.) — out of scope for v0.9.1
- Multi-region Groq deployment (Groq has US/EU regions; default to US)
- Rate limit handling in the translation chain (i.e., automatic retry on 429) — currently the chain just moves to the next engine on error
- Telemetry or usage tracking
- Streaming responses (current implementation is request-response)

## Observations (not implemented now)

- **O1**: Groq's free tier rate limits (6K TPM, 500K TPD) are per-organization, not per-key. If a user has multiple OverLex installations on different machines using the same Groq key, they share the rate limit. Not a problem for a single user with one machine, but worth noting for power users.
- **O2**: The `llama-3.1-8b-instant` model is fast (840 TPS) but smaller than DeepSeek V4 Flash. For most game text (short sentences, dialog), 8B is sufficient. For complex paragraphs with technical jargon, DeepSeek's larger model may produce better results. Real-world testing will reveal the quality gap (if any).
- **O3**: The `test_api_key` 429 handling is intentionally different from DeepSeek. This is the only place where Groq and DeepSeek diverge in the integration. If we ever consolidate engines into a generic OpenAI-compatible adapter, this divergence would need a design decision.
- **O4**: Groq supports several other models (Llama 3.3 70B, Llama 4 Scout, Qwen 3 32B, GPT OSS). The current implementation is hardcoded to 8B. A future "advanced settings" UI could let users pick the model, but that's premature for v0.9.1.
- **O5**: Groq has prompt caching (50% discount on cached input tokens). Not currently used by the adapter. Could be added in a future optimization pass.

## Migration Notes

None. This is an additive change. Users on v0.9.0 keep all their current settings. Groq is just an additional option in the engine list that they can opt-in to.

To enable Groq after upgrading to v0.9.1:
1. Get a free API key at https://console.groq.com/keys
2. Open OverLex Settings
3. Scroll to "Translation Engines"
4. Check `[x] Enable Groq`
5. Enter the API key in the input that appears
6. Optionally click `[ TEST ALL KEYS ]` to verify
7. Click `[ SAVE SETTINGS ]`
