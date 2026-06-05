# Delta Spec: Engine Settings Redesign

## ADDED Requirements

### REQ-01: Engine Enablement
The system MUST allow explicit enablement of paid engines via checkbox. Free engines (`google_gtx`, `mymemory`) MUST always be enabled and MUST NOT show a checkbox.

- **Scenario: Paid engine checked**
  - GIVEN the user checks "Enable Gemini"
  - THEN `enabled_engines` SHALL include `"gemini"`

- **Scenario: Free engine implicit**
  - GIVEN a fresh settings load
  - THEN `enabled_engines` SHALL always contain `["google_gtx", "mymemory"]`

### REQ-02: Primary Engine Selection
The primary engine dropdown MUST only include engines present in `enabled_engines`. The user MUST select exactly one primary engine.

- **Scenario: Filtered dropdown**
  - GIVEN only `google_gtx` and `gemini` are enabled
  - WHEN the user opens the primary engine dropdown
  - THEN only `google_gtx` and `gemini` SHALL be listed

- **Scenario: Disabled engine hidden**
  - GIVEN `deepl` is not enabled
  - WHEN the dropdown renders
  - THEN `deepl` SHALL NOT appear

### REQ-03: Per-Engine API Keys
Each paid engine MUST have its own isolated API key input and Test Key button. Keys MUST persist per-engine in Credential Manager and MUST NOT mix between engines.

- **Scenario: Gemini key stored**
  - GIVEN the user enters a Gemini key and clicks Test Key
  - WHEN the test succeeds
  - THEN the key SHALL be stored under `overlex-gemini` and SHALL NOT affect `overlex-deepl`

- **Scenario: DeepL key isolated**
  - GIVEN a Gemini key is already stored
  - WHEN the user saves a DeepL key
  - THEN `get_api_key("deepl")` SHALL return the DeepL key, not the Gemini key

### REQ-04: Adaptive Multi-Engine Fallback Chain
`translate_text`, `ocr_capture_region`, and `translate_chat` MUST attempt the primary engine first. On any `TranslationError`, they MUST try the other enabled paid engine, then fall back to `google_gtx` as the ultimate backup. The fallback order is ADAPTIVE based on the primary engine selection — NOT a fixed priority list. The chain MUST NOT loop infinitely.

- **Scenario: Primary Gemini fails, DeepSeek succeeds**
  - GIVEN primary is `gemini` and `deepseek` is enabled with a valid key
  - WHEN `gemini` returns `InvalidApiKey`
  - THEN the system SHALL try `deepseek` next, then `google_gtx` as ultimate fallback

- **Scenario: Primary DeepSeek fails, Gemini succeeds**
  - GIVEN primary is `deepseek` and `gemini` is enabled with a valid key
  - WHEN `deepseek` returns `RateLimit`
  - THEN the system SHALL try `gemini` next, then `google_gtx` as ultimate fallback

- **Scenario: Ultimate fallback — all paid engines fail**
  - GIVEN primary `gemini` fails and no other paid engine is available or succeeds
  - WHEN translation is requested
  - THEN the system SHALL fall back to `google_gtx`

- **Scenario: No paid engines enabled**
  - GIVEN only free engines are enabled
  - WHEN translation is requested
  - THEN the system SHALL use `google_gtx` directly

The adaptive fallback order: primary → other enabled paid engines (in order they appear in `enabled_engines`) → `google_gtx`.

### REQ-05: Engine Used Reporting and User Notification
`TranslationResult` MUST include the name of the engine that actually performed the translation. The `ResultPayload` emitted to overlays MUST include this field. Overlays MUST display the engine name and indicate if fallback occurred. When fallback occurs, the system MUST notify the user that the engine was changed — the overlay MUST update to show the actual engine used instead of the primary.

- **Scenario: Result payload includes engine**
  - GIVEN DeepL succeeded via fallback
  - WHEN `emit_result` fires
  - THEN `ResultPayload` SHALL contain `engine_used: "DeepL"` and `fallback: true`

- **Scenario: Debug line shows fallback with engine change**
  - GIVEN primary is `gemini` but `google_gtx` succeeded via fallback
  - WHEN `show_debug: true` and the overlay renders
  - THEN the debug line SHALL show the actual engine used (e.g., `Google Translate (fallback)`) and the user SHALL be aware that the primary engine failed

- **Scenario: Normal operation — no fallback**
  - GIVEN primary engine `deepseek` succeeds
  - WHEN the overlay renders
  - THEN the debug line SHALL show `DeepSeek` with no fallback indicator

## MODIFIED Requirements

### Settings Struct Extension
(Previously: Settings had `engine: String`.)

The `Settings` struct MUST replace `engine` with `primary_engine: String` and `enabled_engines: Vec<String>`. On deserialization, the old `engine` field MUST migrate into both fields.

- **Scenario: Migration from old settings**
  - GIVEN an old `settings.json` with `engine: "deepl"`
  - WHEN settings load
  - THEN `primary_engine` SHALL be `"deepl"` and `enabled_engines` SHALL include `"deepl"` plus all free engines

- **Scenario: Round-trip**
  - GIVEN new settings with `primary_engine: "gemini"` and `enabled_engines: ["google_gtx", "gemini"]`
  - WHEN saved and reloaded
  - THEN both fields SHALL restore correctly

### TranslationState Engine Map
(Previously: `TranslationState` held a single `Arc<RwLock<Arc<dyn TranslationEngine>>>`.

`TranslationState` MUST hold a `HashMap<String, Arc<dyn TranslationEngine>>` containing all enabled engines. It MUST be rebuilt on every settings save that changes engine configuration.

- **Scenario: Multiple engines initialized**
  - GIVEN `enabled_engines` is `["gemini", "deepl", "google_gtx"]`
  - WHEN settings are saved
  - THEN `TranslationState` SHALL contain initialized adapters for all three

- **Scenario: Engine disabled, removed from map**
  - GIVEN `deepl` was previously enabled and is now unchecked
  - WHEN settings are saved
  - THEN `TranslationState` SHALL no longer contain a `deepl` adapter

### save_settings API Key Handling
(Previously: `save_settings` received a single `api_key: Option<String>`.

`save_settings` MUST accept a per-engine API key map or equivalent structure. When an enabled engine requires a key, its corresponding key MUST be passed to `create_engine`.

- **Scenario: Per-engine keys passed**
  - GIVEN the user enables `gemini` and `deepl` with valid keys
  - WHEN `save_settings` is called
  - THEN both keys SHALL be passed to their respective engine creations

- **Scenario: Engine disabled, key ignored**
  - GIVEN `deepseek` is disabled but a key is present in the input
  - WHEN `save_settings` is called
  - THEN no `deepseek` adapter SHALL be created and the key SHALL NOT be stored

### Settings UI Engine Section
(Previously: A single `<select id="engine">` and one shared `<input id="api-key">`.

The settings UI MUST display checkboxes for paid engines, a primary engine dropdown filtered to enabled engines, and per-engine API key inputs with individual Test Key buttons. Free engines MUST always appear as selected and disabled checkboxes (or omitted).

- **Scenario: Paid engine checked reveals key input**
  - GIVEN the user checks "Enable Gemini"
  - THEN a Gemini-specific API key input and Test Key button SHALL appear

- **Scenario: Uncheck hides key input**
  - GIVEN Gemini is enabled and its key input is visible
  - WHEN the user unchecks Gemini
  - THEN the key input SHALL hide

### Overlay Engine Display
(Previously: Overlays showed `settings.engine` in the debug line.

Overlays MUST listen to `settings-changed` and update the displayed primary engine name. When a translation result arrives, the overlay MUST display the actual `engine_used` from `ResultPayload` and indicate if fallback occurred.

- **Scenario: Settings change updates primary**
  - GIVEN the user changes primary engine to `mymemory`
  - WHEN `settings-changed` emits
  - THEN the overlay debug line SHALL update to show `mymemory` as primary

- **Scenario: Fallback indicated in overlay**
  - GIVEN primary `gemini` failed and `google_gtx` succeeded
  - WHEN the result overlay appears
  - THEN the debug line SHALL show `Google Translate (fallback)`

## REMOVED Requirements

### Single Shared API Key Input
(Reason: Replaced by per-engine inputs. The shared field caused the bug where saving one engine's key overwrote another engine's key in Credential Manager.)

The single `<input id="api-key">`, its associated save logic in `save_settings`, and the `api_key: Option<String>` parameter MUST be removed.
