# Security

## API Key Storage

Overlex uses the Windows Credential Manager (via DPAPI) through the `keyring` crate to securely store API keys. API keys are **never** written to disk in plaintext.

## No-Injection Policy

Overlex runs as a separate process and does **not** inject into other applications or games. It is safe for use with anti-cheat systems.

## No-Logging Policy

API keys and translated text are **not** logged. No sensitive data leaves your machine except to the translation API.

## Network Security

All translation API calls are made over HTTPS using `reqwest` with TLS enabled.

## Settings File

User preferences are stored in `%APPDATA%/overlex/settings.json`. This file contains **no secrets** — only non-sensitive settings like language preferences and hotkey configurations.

## Recommended Practices

- Use API keys with minimal permissions (read-only translation access)
- Rotate API keys periodically
- Avoid sharing your machine with untrusted users while Overlex is configured