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
const autoDismissEnabledCheckbox = document.getElementById('auto-dismiss-enabled');
const timeoutGroup = document.getElementById('timeout-group');
const overlayTimeoutInput = document.getElementById('overlay-timeout');
const startWithWindowsCheckbox = document.getElementById('start-with-windows');
const saveBtn = document.getElementById('save-btn');
const messageEl = document.getElementById('message');

// OCR pre-processing elements
const ocrPreprocessingCheckbox = document.getElementById('ocr-preprocessing-enabled');
const ocrBinarizeCheckbox = document.getElementById('ocr-binarize-enabled');

// History elements
const historyEnabledCheckbox = document.getElementById('history-enabled');
const historyPanel = document.getElementById('history-panel');
const historySearchInput = document.getElementById('history-search');
const historyList = document.getElementById('history-list');
const historyLoadMoreBtn = document.getElementById('history-load-more');
const historyExportJsonBtn = document.getElementById('history-export-json');
const historyExportCsvBtn = document.getElementById('history-export-csv');
const historyClearBtn = document.getElementById('history-clear');

// History state
let historyOffset = 0;
let historyHasMore = true;
let currentSettings = null;

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

// Render a single history entry
function renderHistoryEntry(entry) {
    const div = document.createElement('div');
    div.className = 'history-entry';
    div.dataset.id = entry.id;

    const createdAt = entry.created_at ? new Date(entry.created_at).toLocaleString() : '';

    div.innerHTML = `
        <div class="original">${escapeHtml(entry.original_text)}</div>
        <div class="translated">${escapeHtml(entry.translated_text)}</div>
        <div class="meta">
            <span>${entry.source_lang} → ${entry.target_lang} • ${createdAt}</span>
            <button class="delete-btn" data-id="${entry.id}">Delete</button>
        </div>
    `;

    // Delete button handler
    div.querySelector('.delete-btn').addEventListener('click', async (e) => {
        e.stopPropagation();
        const id = parseInt(e.target.dataset.id);
        if (confirm('Delete this history entry?')) {
            try {
                await invoke('delete_history_entry', { id });
                renderHistory();
            } catch (err) {
                showMessage('Failed to delete: ' + err, true);
            }
        }
    });

    return div;
}

// Escape HTML to prevent XSS
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Render history list
async function renderHistory() {
    if (!currentSettings || !currentSettings.history_enabled) {
        historyList.innerHTML = '<div style="color: var(--text-secondary); padding: 8px;">History disabled</div>';
        return;
    }

    try {
        const limit = 20;
        const entries = await invoke('get_history', { limit, offset: historyOffset });

        if (historyOffset === 0) {
            historyList.innerHTML = '';
        }

        if (entries.length === 0 && historyOffset === 0) {
            historyList.innerHTML = '<div style="color: var(--text-secondary); padding: 8px;">No history yet</div>';
            historyHasMore = false;
        } else {
            entries.forEach(entry => {
                historyList.appendChild(renderHistoryEntry(entry));
            });
            historyHasMore = entries.length >= limit;
        }

        historyLoadMoreBtn.style.display = historyHasMore ? 'inline-block' : 'none';
    } catch (err) {
        historyList.innerHTML = '<div style="color: var(--error); padding: 8px;">Failed to load history: ' + escapeHtml(err) + '</div>';
        historyHasMore = false;
    }
}

// Search history
let searchTimeout = null;
async function searchHistory() {
    const query = historySearchInput.value.trim();

    if (!currentSettings || !currentSettings.history_enabled) {
        return;
    }

    try {
        if (query) {
            const entries = await invoke('search_history', { query });
            historyList.innerHTML = '';
            if (entries.length === 0) {
                historyList.innerHTML = '<div style="color: var(--text-secondary); padding: 8px;">No matches found</div>';
            } else {
                entries.forEach(entry => {
                    historyList.appendChild(renderHistoryEntry(entry));
                });
            }
            historyHasMore = false;
            historyLoadMoreBtn.style.display = 'none';
        } else {
            historyOffset = 0;
            await renderHistory();
        }
    } catch (err) {
        showMessage('Search failed: ' + err, true);
    }
}

// Export history
async function exportHistory(format) {
    if (!currentSettings || !currentSettings.history_enabled) {
        showMessage('History is disabled', true);
        return;
    }

    try {
        const data = await invoke('export_history', { format });

        // Create download
        const blob = new Blob([data], { type: format === 'json' ? 'application/json' : 'text/csv' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `overlex-history.${format}`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);

        showMessage('Export complete');
    } catch (err) {
        showMessage('Export failed: ' + err, true);
    }
}

// Clear all history
async function clearHistory() {
    if (!currentSettings || !currentSettings.history_enabled) {
        return;
    }

    if (!confirm('Are you sure you want to delete ALL translation history? This cannot be undone.')) {
        return;
    }

    try {
        await invoke('clear_history');
        historyOffset = 0;
        await renderHistory();
        showMessage('History cleared');
    } catch (err) {
        showMessage('Failed to clear history: ' + err, true);
    }
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', async () => {
    // Setup hotkey capture
    setupHotkeyCapture(ocrHotkeyInput);
    setupHotkeyCapture(writeHotkeyInput);

    // Setup auto-dismiss checkbox toggle
    autoDismissEnabledCheckbox.addEventListener('change', () => {
        if (autoDismissEnabledCheckbox.checked) {
            timeoutGroup.style.display = 'block';
            if (!overlayTimeoutInput.value) {
                overlayTimeoutInput.value = 5000;
            }
        } else {
            timeoutGroup.style.display = 'none';
        }
    });

    // Setup OCR pre-processing toggle
    ocrPreprocessingCheckbox.addEventListener('change', () => {
        // Binarize only makes sense if preprocessing is enabled
        if (!ocrPreprocessingCheckbox.checked) {
            ocrBinarizeCheckbox.checked = false;
        }
        ocrBinarizeCheckbox.disabled = !ocrPreprocessingCheckbox.checked;
    });

    // Setup history toggle
    historyEnabledCheckbox.addEventListener('change', () => {
        historyPanel.style.display = historyEnabledCheckbox.checked ? 'block' : 'none';
        if (historyEnabledCheckbox.checked) {
            renderHistory();
        }
    });

    // Setup history search
    historySearchInput.addEventListener('input', () => {
        clearTimeout(searchTimeout);
        searchTimeout = setTimeout(searchHistory, 300);
    });

    // Setup history buttons
    historyLoadMoreBtn.addEventListener('click', async () => {
        historyOffset += 20;
        await renderHistory();
    });

    historyExportJsonBtn.addEventListener('click', () => exportHistory('json'));
    historyExportCsvBtn.addEventListener('click', () => exportHistory('csv'));
    historyClearBtn.addEventListener('click', clearHistory);

    // Load current settings
    try {
        const settings = await invoke('get_settings');
        currentSettings = settings;

        // Populate form fields
        ocrHotkeyInput.value = settings.ocr_hotkey || '';
        writeHotkeyInput.value = settings.write_hotkey || '';
        sourceLangSelect.value = settings.source_lang || 'auto';
        targetLangSelect.value = settings.target_lang || 'es';
        engineSelect.value = settings.engine || 'libretranslate';
        overlayPositionSelect.value = settings.overlay_position || 'near-selection';

        // Handle auto-dismiss: if timeout > 0, check and show; if 0, uncheck and hide
        if (settings.overlay_timeout_ms > 0) {
            autoDismissEnabledCheckbox.checked = true;
            timeoutGroup.style.display = 'block';
            overlayTimeoutInput.value = settings.overlay_timeout_ms;
        } else {
            autoDismissEnabledCheckbox.checked = false;
            timeoutGroup.style.display = 'none';
            overlayTimeoutInput.value = 5000;
        }

        startWithWindowsCheckbox.checked = settings.start_with_windows || false;

        // OCR pre-processing settings
        ocrPreprocessingCheckbox.checked = settings.ocr_preprocessing !== false;
        ocrBinarizeCheckbox.checked = settings.ocr_binarize === true;
        ocrBinarizeCheckbox.disabled = !ocrPreprocessingCheckbox.checked;

        // History settings
        historyEnabledCheckbox.checked = settings.history_enabled !== false;
        historyPanel.style.display = historyEnabledCheckbox.checked ? 'block' : 'none';

        // Load history if enabled
        if (historyEnabledCheckbox.checked) {
            renderHistory();
        }

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
        // If checkbox unchecked, send 0 (never dismiss). If checked, send the value or default 5000
        overlay_timeout_ms: autoDismissEnabledCheckbox.checked
            ? (parseInt(overlayTimeoutInput.value) || 5000)
            : 0,
        start_with_windows: startWithWindowsCheckbox.checked,
        libre_translate_url: 'https://libretranslate.com', // Default URL
        // OCR pre-processing
        ocr_preprocessing: ocrPreprocessingCheckbox.checked,
        ocr_binarize: ocrBinarizeCheckbox.checked,
        // History
        history_enabled: historyEnabledCheckbox.checked,
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

        // Update current settings
        currentSettings = settings;

        // Refresh history view if enabled
        if (settings.history_enabled) {
            historyOffset = 0;
            await renderHistory();
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
