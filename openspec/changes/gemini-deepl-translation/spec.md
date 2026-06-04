# Delta Spec: Gemini + DeepL Translation with Game Context

## Domain: Translation Engine Core

### Requirement: TranslationContext Struct

The system MUST provide a `TranslationContext` struct containing `process_name: Option<String>` and `profile_name: Option<String>` to convey active game context to translation adapters.

#### Scenario: Context built from active game info

- GIVEN an active foreground process matches a game profile
- WHEN a translation is requested
- THEN `TranslationContext` SHALL contain the process executable name and matched profile display name

#### Scenario: Context is None when no game detected

- GIVEN no active game is detected
- WHEN a translation is requested
- THEN `TranslationContext` SHALL be `None`

### Requirement: TranslationEngine Trait Signature

The system MUST update the `TranslationEngine` trait so that `translate()` accepts an optional `&TranslationContext` as its fourth parameter.

#### Scenario: Existing adapters compile with new signature

- GIVEN the trait signature is updated
- WHEN existing adapters (Google GTX, MyMemory, LibreTranslate) are compiled
- THEN they SHALL accept the new parameter (even if ignored) without breaking changes

#### Scenario: Gemini and DeepL adapters consume context

- GIVEN Gemini or DeepL engine is active and game context is available
- WHEN `translate()` is invoked
- THEN the engine SHALL inject game context into its API request

## Domain: Gemini Translation

### Requirement: GeminiAdapter API Integration

The system MUST implement a `GeminiAdapter` that calls `POST https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={api_key}`.

#### Scenario: Successful translation with context

- GIVEN a valid Gemini API key and non-empty `TranslationContext`
- WHEN `translate()` is called
- THEN the request body SHALL include `systemInstruction` with a game-aware prompt, and the response SHALL return the translated text

#### Scenario: Missing API key

- GIVEN the engine is `gemini` and no API key is stored
- WHEN `translate()` is called
- THEN it SHALL return `TranslationError::InvalidApiKey`

#### Scenario: Rate limit or service error

- GIVEN the Gemini API returns HTTP 429 or 5xx
- WHEN `translate()` is called
- THEN it SHALL return `TranslationError::RateLimit` or `TranslationError::ServiceDown`

### Requirement: GeminiAdapter Context Handling

The system SHALL format the `systemInstruction` as: "You are a professional game translator. Translate the following text from {source} to {target}. Game context: {process_name} (profile: {profile_name}). Respond ONLY with the translated text, no explanations, no quotes, no markdown."

#### Scenario: Context fields present

- GIVEN `process_name` is `"poe2.exe"` and `profile_name` is `"Path of Exile 2"`
- WHEN `translate()` builds the request
- THEN the `systemInstruction` SHALL include both values

#### Scenario: Context fields absent

- GIVEN `TranslationContext` is `None` or fields are `None`
- WHEN `translate()` builds the request
- THEN the `systemInstruction` SHALL omit game-specific clauses gracefully

### Requirement: GeminiAdapter detected_source

The system SHALL set `detected_source` to `None` in `TranslationResult` because the Gemini API does not provide source language detection.

#### Scenario: Any Gemini translation response

- GIVEN a successful Gemini translation
- WHEN the result is returned
- THEN `detected_source` SHALL be `None`

## Domain: DeepL Translation

### Requirement: DeepLAdapter API Integration

The system MUST implement a `DeepLAdapter` that calls `POST https://api-free.deepl.com/v2/translate` with `Authorization: DeepL-Auth-Key {api_key}`.

#### Scenario: Successful translation with context

- GIVEN a valid DeepL API key and non-empty `TranslationContext`
- WHEN `translate()` is called
- THEN the request SHALL include `context: "Game: {process_name}"`, and the response SHALL return `translations[0].text`

#### Scenario: Source language auto-detected by DeepL

- GIVEN source language is `"auto"` and DeepL returns `detected_source_language`
- WHEN the translation succeeds
- THEN `TranslationResult.detected_source` SHALL be populated with the detected language code

#### Scenario: Missing API key

- GIVEN the engine is `deepl` and no API key is stored
- WHEN `translate()` is called
- THEN it SHALL return `TranslationError::InvalidApiKey`

#### Scenario: DeepL free tier rate limit

- GIVEN the DeepL API returns HTTP 429 or 456 (quota exceeded)
- WHEN `translate()` is called
- THEN it SHALL return `TranslationError::RateLimit`

### Requirement: DeepLAdapter Context Handling

The system SHALL pass the `context` parameter to DeepL as `"Game: {process_name}"` when `process_name` is present.

#### Scenario: Context present

- GIVEN `process_name` is `"eldenring.exe"`
- WHEN `translate()` is called
- THEN the request JSON SHALL include `"context": "Game: eldenring.exe"`

#### Scenario: Context absent

- GIVEN `TranslationContext` is `None`
- WHEN `translate()` is called
- THEN the request SHALL omit the `context` field entirely

## Domain: Engine Factory

### Requirement: create_engine supports Gemini and DeepL

The system MUST extend `create_engine()` to return `GeminiAdapter` when `settings.engine == "gemini"` and `DeepLAdapter` when `settings.engine == "deepl"`.

#### Scenario: Engine string "gemini"

- GIVEN `settings.engine` is `"gemini"` and a valid API key exists
- WHEN `create_engine()` is called
- THEN it SHALL return a boxed `GeminiAdapter`

#### Scenario: Engine string "deepl"

- GIVEN `settings.engine` is `"deepl"` and a valid API key exists
- WHEN `create_engine()` is called
- THEN it SHALL return a boxed `DeepLAdapter`

#### Scenario: Invalid engine falls back to default

- GIVEN `settings.engine` is an unrecognized value
- WHEN `create_engine()` is called
- THEN it SHALL fall back to `GoogleGtxAdapter`

## Domain: Command Integration

### Requirement: translate_text passes context

The system MUST update `translate_text` to build `TranslationContext` from `ActiveGameState` and pass it to `engine.translate()`.

#### Scenario: Write mode with active game

- GIVEN the user triggers write-mode translation while a game is active
- WHEN `translate_text` executes
- THEN it SHALL read `active_game.info`, build `TranslationContext`, and pass it to `translate()`

### Requirement: ocr_capture_region passes context

The system MUST update `ocr_capture_region` to build and pass `TranslationContext` before translating OCR output.

#### Scenario: OCR capture with active game

- GIVEN the user captures a region while a game is active
- WHEN `ocr_capture_region` reaches the translation step
- THEN it SHALL pass `TranslationContext` to `engine.translate()`

### Requirement: translate_chat passes context

The system MUST update `translate_chat` to build and pass `TranslationContext`.

#### Scenario: Chat translation with active game

- GIVEN a chat message is submitted while a game is active
- WHEN `translate_chat` executes
- THEN it SHALL pass `TranslationContext` to `engine.translate()`

## Domain: Settings UI

### Requirement: Engine dropdown includes Gemini and DeepL

The system MUST add `<option value="gemini">Gemini 2.0 Flash (Requires API Key)</option>` and `<option value="deepl">DeepL Free (Requires API Key)</option>` to both the global engine dropdown and the profile engine override dropdown.

#### Scenario: Global settings engine select

- GIVEN the settings page is loaded
- WHEN the user opens the "Translation Engine" dropdown
- THEN Gemini 2.0 Flash and DeepL Free SHALL be visible options

#### Scenario: Profile engine override select

- GIVEN the user is adding or editing a game profile
- WHEN the user opens the "Translation Engine" override dropdown
- THEN Gemini 2.0 Flash and DeepL Free SHALL be visible options

### Requirement: API key UI labels updated

The system SHALL ensure the API key input field label clearly indicates it is required for LibreTranslate, Gemini, and DeepL.

#### Scenario: User selects engine requiring API key

- GIVEN the user selects Gemini, DeepL, or LibreTranslate
- WHEN the settings UI updates
- THEN the API key field label SHALL read "API Key (Required for selected engine)"

## Domain: Error Handling

### Requirement: Missing API key shows clear error

The system SHALL emit an `overlex-error` event with code `"INVALID_API_KEY"` when a keyed engine is selected but no API key is configured.

#### Scenario: Gemini without key

- GIVEN engine is set to Gemini and no API key is stored
- WHEN translation is attempted
- THEN the user SHALL see an error overlay stating the API key is missing
