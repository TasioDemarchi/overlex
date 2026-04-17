# Overlex

Lightweight Windows desktop overlay for instant text translation without Alt+Tabping.

## Features

- **OCR Capture**: Capture screen regions and extract text using Tesseract OCR
- **Write Mode**: Type text directly for instant translation
- **Overlay Results**: Display translated text as a non-intrusive overlay
- **Global Hotkeys**: Trigger capture and translation from anywhere
- **Settings**: Configure source/target languages, API keys, and hotkeys

## Prerequisites

- Rust 1.75+
- Node.js 18+
- Tauri CLI 2.x
- Windows 10/11

## Setup

```bash
# Clone the repository
git clone <repository-url>

# Navigate to the Tauri backend
cd overlex/src-tauri

# Check Rust dependencies compile
cargo check

# Return to project root
cd ..

# Start development server
npx tauri dev
```

## File Structure

```
overlex/
├── src/
│   ├── freeze/     # Freeze/unfreeze overlay
│   ├── result/    # Display translation results
│   ├── settings/  # Settings UI
│   └── write/     # Write mode for direct input
├── src-tauri/
│   └── src/
│       ├── capture.rs    # Screen capture logic
│       ├── commands.rs  # Tauri commands
│       ├── hotkeys.rs    # Global hotkey registration
│       ├── lib.rs       # Library exports
│       ├── main.rs      # Application entry point
│       ├── ocr.rs       # Tesseract OCR integration
│       ├── settings.rs  # Settings management
│       ├── tray.rs      # System tray
│       └── translation/ # Translation API integration
└── README.md
```

## How It Works

### OCR Flow
1. User triggers capture via global hotkey
2. Screen region is captured using Windows API
3. Captured image is processed with Tesseract OCR
4. Extracted text is sent to translation API
5. Result is displayed as an overlay

### Write Flow
1. User activates write mode
2. Text input is captured
3. Text is sent to translation API
4. Translated result is shown in overlay

## License

MIT