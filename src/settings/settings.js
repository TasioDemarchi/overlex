// Settings panel - load/save settings via Tauri invoke

const { invoke } = window.__TAURI__.core;

// DOM elements
const ocrHotkeyInput = document.getElementById('ocr-hotkey');
const writeHotkeyInput = document.getElementById('write-hotkey');
const sourceLangSelect = document.getElementById('source-lang');
const targetLangSelect = document.getElementById('target-lang');
const engineSelect = document.getElementById('engine');
const apiKeyInput = document.getElementById('api-key');
const overlayPositionSelect = document.getElementById('overlay-position');
const overlayTimeoutInput = document.getElementById('overlay-timeout');
const startWithWindowsCheckbox = document.getElementById('start-with-windows');
const saveBtn = document.getElementById('save-btn');
const messageEl = document.getElementById('message');

// Show message (success or error)
function showMessage(text, isError = false) {
    messageEl.textContent = text;
    messageEl.className = isError ? 'error' : 'success';
}

// Setup hotkey capture on an input element
function setupHotkeyCapture(inputElement) {
    inputElement.addEventListener('keydown', (e) => {
        e.preventDefault();
    });

    inputElement.addEventListener('focus', () => {
        inputElement.placeholder = 'Press key combination...';
        inputElement.value = '';
    });

    inputElement.addEventListener('blur', () => {
        if (!inputElement.value) {
            inputElement.placeholder = 'Click to set hotkey';
        }
    });

    inputElement.addEventListener('keyup', (e) => {
        const parts = [];

        if (e.ctrlKey) parts.push('CTRL');
        if (e.shiftKey) parts.push('SHIFT');
        if (e.altKey) parts.push('ALT');

        // Handle the key - either a single character or a function key
        if (e.key.length === 1) {
            // Single character key (A-Z, 0-9, etc.)
            parts.push(e.key.toUpperCase());
        } else if (e.key.startsWith('F') && e.key.length > 1 && !isNaN(e.key.slice(1))) {
            // Function keys (F1-F12)
            parts.push(e.key.toUpperCase());
        } else if (e.key === 'Escape') {
            parts.push('ESC');
        }

        // Only set if we have a modifier + key combo
        // (require at least CTRL/SHIFT/ALT + another key)
        if (parts.length > 1) {
            inputElement.value = parts.join('+');
        }
    });
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', async () => {
    // Setup hotkey capture
    setupHotkeyCapture(ocrHotkeyInput);
    setupHotkeyCapture(writeHotkeyInput);

    // Load current settings
    try {
        const settings = await invoke('get_settings');

        // Populate form fields
        ocrHotkeyInput.value = settings.ocr_hotkey || '';
        writeHotkeyInput.value = settings.write_hotkey || '';
        sourceLangSelect.value = settings.source_lang || 'auto';
        targetLangSelect.value = settings.target_lang || 'es';
        engineSelect.value = settings.engine || 'libretranslate';
        overlayPositionSelect.value = settings.overlay_position || 'near-selection';
        overlayTimeoutInput.value = settings.overlay_timeout_ms || 5000;
        startWithWindowsCheckbox.checked = settings.start_with_windows || false;

        // Load API key (if any)
        try {
            const key = await invoke('get_api_key', { engine: settings.engine });
            if (key) {
                apiKeyInput.value = key;
            }
        } catch {
            // No API key stored, leave empty
        }
    } catch (e) {
        console.error('Failed to load settings:', e);
        showMessage('Failed to load settings: ' + e, true);
    }
});

// Save button handler
saveBtn.addEventListener('click', async () => {
    // Gather form values
    const settings = {
        ocr_hotkey: ocrHotkeyInput.value || 'CTRL+SHIFT+T',
        write_hotkey: writeHotkeyInput.value || 'CTRL+SHIFT+W',
        source_lang: sourceLangSelect.value,
        target_lang: targetLangSelect.value,
        engine: engineSelect.value,
        overlay_position: overlayPositionSelect.value,
        overlay_timeout_ms: parseInt(overlayTimeoutInput.value) || 5000,
        start_with_windows: startWithWindowsCheckbox.checked,
        libre_translate_url: 'https://libretranslate.com', // Default URL
    };

    // Validate hotkeys are set
    if (!settings.ocr_hotkey || settings.ocr_hotkey === 'Click to set hotkey') {
        showMessage('OCR hotkey is required', true);
        return;
    }
    if (!settings.write_hotkey || settings.write_hotkey === 'Click to set hotkey') {
        showMessage('Write mode hotkey is required', true);
        return;
    }

    try {
        // Save settings (this validates hotkeys on the backend)
        await invoke('save_settings', { settings });

        // Also save API key separately if provided
        if (apiKeyInput.value) {
            await invoke('set_api_key', {
                engine: settings.engine,
                key: apiKeyInput.value
            });
        }

        showMessage('Settings saved successfully!');
    } catch (e) {
        console.error('Failed to save settings:', e);
        showMessage('Failed to save: ' + e, true);
    }
});

// ESC key to close window (optional, for consistency)
document.addEventListener('keydown', async (e) => {
    if (e.key === 'Escape') {
        // Could close window here if needed
        // window.close();
    }
});

console.log('Settings panel initialized');