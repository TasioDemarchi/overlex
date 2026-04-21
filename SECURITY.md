# Security

## API Key Storage

Overlex uses the Windows Credential Manager (via DPAPI) through the `keyring` crate to securely store API keys. API keys are **never** written to disk in plaintext — no `settings.json`, no `.env`, no logs.

## No-Logging Policy

The following data is **never logged** under any circumstance:

- Text captured via OCR
- Translated text or original text
- API keys or tokens
- Window content or UI state

This policy is enforced in code — logging statements explicitly exclude these fields.

## No-Injection Policy

Overlex runs as a separate process and does **not** inject into other applications or games. It is safe for use with anti-cheat systems.

## Windows Permissions Required

Overlex requests the following OS-level permissions and explains why each is needed:

| Permission | Win32 API | Reason |
|---|---|---|
| **RegisterHotKey** | `RegisterHotKey` | Required to capture global hotkeys (Ctrl+Shift+T, etc.) even when the app is not focused. Used in a dedicated thread with its own message pump. Hotkeys are unregistered when the app exits or is paused. |
| **BitBlt / Screen Capture** | `BitBlt`, `GetDeviceCaps` | Required to capture a frozen screenshot of the entire desktop before showing the OCR selection overlay. Uses `PrintWindow` as fallback. |
| **OCR** | Windows.Media.Ocr | Built-in Windows 10+ OCR engine. Runs locally — no OCR data leaves the machine. |

## Settings File

User preferences are stored in `%APPDATA%/overlex/settings.json`. This file contains **no secrets** — only non-sensitive settings like language preferences, hotkey bindings, and overlay position.

## Network Security

All translation API calls are made over HTTPS using `reqwest` with TLS enabled by default.

## Recommended Practices

- Use API keys with minimal permissions (read-only translation access)
- Rotate API keys periodically
- Avoid sharing your machine with untrusted users while Overlex is configured

## Reporting a Security Vulnerability

If you discover a security issue in Overlex, please report it responsibly:

1. **Do not open a public GitHub issue** for security vulnerabilities.
2. Email the maintainer directly with:
   - A clear description of the issue and its impact
   - Steps to reproduce it
   - Any relevant logs or screenshots (exclude sensitive data)
3. Allow reasonable time for a fix before public disclosure (we'll coordinate with you on timeline).

We appreciate responsible disclosure and will credit reporters unless anonymity is requested.