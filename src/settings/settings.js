// App version is hardcoded in index.html (#app-version) — keep in sync with src-tauri/tauri.conf.json
// Settings panel - load/save settings via Tauri invoke

const { invoke } = window.__TAURI__.core;
const listen = window.__TAURI__?.event?.listen;

// Engines that require an API key
const ALL_ENGINES = ['google_gtx', 'mymemory', 'gemini', 'deepl', 'deepseek', 'groq'];
const PAID_ENGINES = ['gemini', 'deepl', 'deepseek', 'groq'];
const FREE_ENGINES = ['google_gtx', 'mymemory'];
const ENGINE_LABELS = {
    google_gtx: 'Google Translate',
    mymemory: 'MyMemory',
    gemini: 'Gemini',
    deepl: 'DeepL',
    deepseek: 'DeepSeek',
    groq: 'Groq',
};

// DOM elements
const ocrHotkeyInput = document.getElementById('ocr-hotkey');
const writeHotkeyInput = document.getElementById('write-hotkey');
const sourceLangSelect = document.getElementById('source-lang');
const targetLangSelect = document.getElementById('target-lang');
const primaryEngineSelect = document.getElementById('primary-engine');
const engineCheckboxesDiv = document.getElementById('engine-checkboxes');
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

// API Key help modal elements
const engineHelpBtn = document.getElementById('engine-help-btn');
const apiKeyModal = document.getElementById('api-key-modal');
const apiKeyModalClose = document.getElementById('api-key-modal-close');
const apiKeyModalTitle = document.getElementById('api-key-modal-title');
const apiKeyModalBody = document.getElementById('api-key-modal-body');
const historyExportJsonBtn = document.getElementById('history-export-json');
const historyExportCsvBtn = document.getElementById('history-export-csv');
const historyClearBtn = document.getElementById('history-clear');

// Profile elements
const activeProfileBanner = document.getElementById('active-profile-banner');
const activeProfileName = document.getElementById('active-profile-name');
const activeProfileDetails = document.getElementById('active-profile-details');
const showDebugCheckbox = document.getElementById('show-debug');
const profileList = document.getElementById('profile-list');
const addProfileBtn = document.getElementById('add-profile-btn');
const profileForm = document.getElementById('profile-form');
const profileFormTitle = document.getElementById('profile-form-title');
const profileDisplayName = document.getElementById('profile-display-name');
const profileProcessNames = document.getElementById('profile-process-names');
const profileSourceLang = document.getElementById('profile-source-lang');
const profileTargetLang = document.getElementById('profile-target-lang');
const profileEngine = document.getElementById('profile-engine');
const profileOcrPreprocessing = document.getElementById('profile-ocr-preprocessing');
const profileOcrBinarize = document.getElementById('profile-ocr-binarize');
const profileSaveBtn = document.getElementById('profile-save-btn');
const profileCancelBtn = document.getElementById('profile-cancel-btn');

// History state
let historyOffset = 0;
let historyHasMore = true;
let currentSettings = null;

// Profile state
let profiles = [];
let editingProfile = null;
let activeGameInfo = null;

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

// Render a single history entry (terminal style)
function renderHistoryEntry(entry) {
    const div = document.createElement('div');
    div.className = 'history-entry';
    div.dataset.id = entry.id;

    const engineLabel = ENGINE_LABELS[entry.engine] || entry.engine || 'unknown';

    const prefixSpan = document.createElement('span');
    prefixSpan.className = 'entry-prefix';
    prefixSpan.textContent = '> ';

    const originalSpan = document.createElement('span');
    originalSpan.className = 'entry-original';
    originalSpan.textContent = '"' + entry.original_text + '"';

    const arrowSpan = document.createElement('span');
    arrowSpan.className = 'entry-arrow';
    arrowSpan.textContent = ' → ';

    const translatedSpan = document.createElement('span');
    translatedSpan.className = 'entry-translated';
    translatedSpan.textContent = '"' + entry.translated_text + '"';

    const metaSpan = document.createElement('span');
    metaSpan.className = 'entry-meta';
    metaSpan.textContent = ' [' + engineLabel + '] ';

    const deleteBtn = document.createElement('button');
    deleteBtn.className = 'delete-entry-btn';
    deleteBtn.dataset.id = entry.id;
    deleteBtn.textContent = '[delete]';

    div.appendChild(prefixSpan);
    div.appendChild(originalSpan);
    div.appendChild(arrowSpan);
    div.appendChild(translatedSpan);
    div.appendChild(metaSpan);
    div.appendChild(deleteBtn);

    // Delete button handler
    deleteBtn.addEventListener('click', async (e) => {
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

// ============================================================
// Game Profiles CRUD
// ============================================================

// Render all profiles into #profile-list
function renderProfiles() {
    profileList.innerHTML = '';
    if (profiles.length === 0) {
        profileList.innerHTML = '<div style="color: var(--text-secondary); padding: 8px; font-size: 0.85rem;">No profiles configured</div>';
        return;
    }
    profiles.forEach(profile => {
        const card = document.createElement('div');
        card.className = 'profile-card';

        // Process names as monospace tags
        const processTags = profile.process_names.map(n =>
            `<code class="process-tag">${escapeHtml(n)}</code>`
        ).join(' ');

        // Override badges for non-null overrides
        const badges = [];
        if (profile.source_lang) badges.push(`<span class="badge badge-lang">src: ${escapeHtml(profile.source_lang)}</span>`);
        if (profile.target_lang) badges.push(`<span class="badge badge-lang">→ ${escapeHtml(profile.target_lang)}</span>`);
        if (profile.primary_engine) badges.push(`<span class="badge badge-engine">${escapeHtml(profile.primary_engine)}</span>`);
        if (profile.ocr_preprocessing) badges.push('<span class="badge badge-ocr">preprocess</span>');
        if (profile.ocr_binarize) badges.push('<span class="badge badge-ocr">binarize</span>');

        card.innerHTML = `
            <div class="profile-card-header">
                <strong>${escapeHtml(profile.display_name)}</strong>
                <div class="profile-card-actions">
                    <button class="small-btn edit-profile-btn">EDIT</button>
                    <button class="small-btn danger delete-profile-btn">DELETE</button>
                </div>
            </div>
            <div class="profile-card-processes">${processTags}</div>
            ${badges.length > 0 ? `<div class="profile-card-badges">${badges.join(' ')}</div>` : ''}
        `;

        // Event listeners for card buttons
        card.querySelector('.edit-profile-btn').addEventListener('click', () => openProfileForm(profile));
        card.querySelector('.delete-profile-btn').addEventListener('click', () => deleteProfile(profile.display_name));

        profileList.appendChild(card);
    });
}

// Open the profile form for add (no arg) or edit (with profile)
function openProfileForm(profile) {
    editingProfile = profile || null;

    if (profile) {
        profileFormTitle.textContent = 'Edit Profile';
        profileDisplayName.value = profile.display_name || '';
        profileProcessNames.value = (profile.process_names || []).join(', ');
        profileSourceLang.value = profile.source_lang || '';
        profileTargetLang.value = profile.target_lang || '';
        profileEngine.value = profile.primary_engine || profile.engine || '';
        profileOcrPreprocessing.checked = profile.ocr_preprocessing === true;
        profileOcrBinarize.checked = profile.ocr_binarize === true;
    } else {
        profileFormTitle.textContent = 'Add Profile';
        profileDisplayName.value = '';
        profileProcessNames.value = '';
        profileSourceLang.value = '';
        profileTargetLang.value = '';
        profileEngine.value = '';
        profileOcrPreprocessing.checked = false;
        profileOcrBinarize.checked = false;
    }

    // Show form, hide list and add button
    profileForm.style.display = 'block';
    profileList.style.display = 'none';
    addProfileBtn.style.display = 'none';
}

// Close the profile form and return to list view
function closeProfileForm() {
    profileForm.style.display = 'none';
    profileList.style.display = 'block';
    addProfileBtn.style.display = 'inline-block';
    editingProfile = null;
}

// Save a profile (add or update)
async function saveProfile() {
    const displayName = profileDisplayName.value.trim();
    const processNamesRaw = profileProcessNames.value.trim();

    if (!displayName) {
        showMessage('Display name is required', true);
        return;
    }
    if (!processNamesRaw) {
        showMessage('At least one process name is required', true);
        return;
    }

    const processNames = processNamesRaw.split(',').map(n => n.trim()).filter(n => n.length > 0);
    if (processNames.length === 0) {
        showMessage('At least one process name is required', true);
        return;
    }

    const profile = {
        display_name: displayName,
        process_names: processNames,
        source_lang: profileSourceLang.value || null,
        target_lang: profileTargetLang.value || null,
        primary_engine: profileEngine.value || null,
        ocr_preprocessing: profileOcrPreprocessing.checked ? true : null,
        ocr_binarize: profileOcrBinarize.checked ? true : null,
    };

    try {
        if (editingProfile) {
            await invoke('update_profile', { profile });
            // Update local array optimistically (match by original display_name)
            const idx = profiles.findIndex(p => p.display_name === editingProfile.display_name);
            if (idx >= 0) profiles[idx] = profile;
        } else {
            await invoke('add_profile', { profile });
            // Push optimistically — renderProfiles() works even without a re-fetch
            profiles.push(profile);
        }
        showMessage(editingProfile ? 'Profile updated' : 'Profile added');
    } catch (e) {
        showMessage('Failed to save profile: ' + e, true);
    } finally {
        renderProfiles();
        closeProfileForm();
    }
}

// Delete a profile by display name
async function deleteProfile(displayName) {
    if (!confirm(`Delete profile "${displayName}"?`)) return;

    try {
        await invoke('remove_profile', { displayName });
        profiles = await invoke('list_profiles');
        renderProfiles();
        showMessage('Profile deleted');
    } catch (e) {
        showMessage('Failed to delete profile: ' + e, true);
    }
}

// Update the active profile banner based on activeGameInfo
function updateBanner() {
    if (!activeGameInfo || !activeGameInfo.matched_profile) {
        activeProfileBanner.style.display = 'none';
        return;
    }

    const profile = profiles.find(p => p.display_name === activeGameInfo.matched_profile);

    activeProfileName.textContent = activeGameInfo.matched_profile;

    const details = [];
    if (activeGameInfo.process_name) {
        details.push(activeGameInfo.process_name);
    }
    if (activeGameInfo.fullscreen_exclusive) {
        details.push('Exclusive Fullscreen');
    }
    if (profile) {
        if (profile.target_lang) details.push(`Target: ${profile.target_lang}`);
        if (profile.primary_engine) details.push(`Engine: ${profile.primary_engine}`);
        if (profile.source_lang) details.push(`Source: ${profile.source_lang}`);
    }

    activeProfileDetails.textContent = details.join(' · ');
    activeProfileBanner.style.display = 'block';
}

// Retry helper for invoke calls that may fail due to state timing
async function invokeWithRetry(cmd, args, maxRetries = 3, delayMs = 500) {
    for (let attempt = 0; attempt < maxRetries; attempt++) {
        try {
            return await invoke(cmd, args);
        } catch (e) {
            const isStateError = typeof e === 'string' && e.includes('state not managed');
            if (isStateError && attempt < maxRetries - 1) {
                console.warn(`[retry] ${cmd} attempt ${attempt + 1} failed: ${e}. Retrying in ${delayMs}ms...`);
                await new Promise(resolve => setTimeout(resolve, delayMs));
                continue;
            }
            throw e;
        }
    }
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', async () => {
    // --- Window controls (custom title bar) ---
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

    // --- Custom terminal selects ---
    document.querySelectorAll('select[data-terminal-select]').forEach(sel => {
        createTerminalSelect(sel);
    });

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

    // Load current settings (with retry for Tauri v2 state timing)
    try {
        const settings = await invokeWithRetry('get_settings');
        currentSettings = settings;

        // Populate form fields
        ocrHotkeyInput.value = settings.ocr_hotkey || '';
        writeHotkeyInput.value = settings.write_hotkey || '';
        sourceLangSelect.value = settings.source_lang || 'auto';
        targetLangSelect.value = settings.target_lang || 'es';

        // Render engine UI with new multi-engine design
        const enabledEngines = settings.enabled_engines || ['google_gtx', 'mymemory'];
        const primaryEngine = settings.primary_engine || 'google_gtx';
        renderEnginesWithKeys(enabledEngines, primaryEngine);

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
    } catch (e) {
        console.error('Failed to load settings:', e);
        showMessage('Failed to load settings: ' + e, true);
        // Still populate form with defaults so the page isn't blank
        ocrHotkeyInput.value = 'CTRL+SHIFT+T';
        writeHotkeyInput.value = 'CTRL+SHIFT+W';
        sourceLangSelect.value = 'auto';
        targetLangSelect.value = 'es';
        renderEnginesWithKeys(['google_gtx', 'mymemory'], 'google_gtx');
        overlayPositionSelect.value = 'near-selection';
        autoDismissEnabledCheckbox.checked = true;
        timeoutGroup.style.display = 'block';
        overlayTimeoutInput.value = 5000;
        startWithWindowsCheckbox.checked = false;
        ocrPreprocessingCheckbox.checked = true;
        ocrBinarizeCheckbox.checked = false;
        ocrBinarizeCheckbox.disabled = false;
        historyEnabledCheckbox.checked = true;
        historyPanel.style.display = 'block';
    }
});

// Save button handler
saveBtn.addEventListener('click', async () => {
    // Read enabled paid engines from checkboxes
    const checkedPaidEngines = getCheckedPaidEngines();

    // Build enabled_engines: free engines always included + checked paid engines
    const enabledEngines = [...FREE_ENGINES, ...checkedPaidEngines];

    // Gather form values
    const settings = {
        ocr_hotkey: ocrHotkeyInput.value || 'CTRL+SHIFT+T',
        write_hotkey: writeHotkeyInput.value || 'CTRL+SHIFT+W',
        source_lang: sourceLangSelect.value,
        target_lang: targetLangSelect.value,
        primary_engine: primaryEngineSelect.value,
        enabled_engines: enabledEngines,
        overlay_position: overlayPositionSelect.value,
        // If checkbox unchecked, send 0 (never dismiss). If checked, send the value or default 5000
        overlay_timeout_ms: autoDismissEnabledCheckbox.checked
            ? (parseInt(overlayTimeoutInput.value) || 5000)
            : 0,
        start_with_windows: startWithWindowsCheckbox.checked,
        // OCR pre-processing
        ocr_preprocessing: ocrPreprocessingCheckbox.checked,
        ocr_binarize: ocrBinarizeCheckbox.checked,
        // History
        history_enabled: historyEnabledCheckbox.checked,
        // Debug indicator
        show_debug: showDebugCheckbox.checked,
        // Game profiles (must be included to avoid deleting them on save)
        profiles: profiles,
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
        // Collect API keys per engine
        const apiKeys = {};
        PAID_ENGINES.forEach(engine => {
            const input = document.getElementById(`api-key-${engine}`);
            if (input && input.value.trim()) {
                apiKeys[engine] = input.value.trim();
            }
        });

        // Save settings (validates hotkeys + recreates engines with the api_keys map)
        const response = await invoke('save_settings', { settings, apiKeys });

        // Handle partial keyring failures (best-effort)
        if (response && response.key_errors && Object.keys(response.key_errors).length > 0) {
            const engines = Object.keys(response.key_errors);
            console.warn('[settings] API key persistence errors:', response.key_errors);
            const errorMsg = engines
                .map(e => `${ENGINE_LABELS[e] || e}: ${response.key_errors[e]}`)
                .join('; ');
            showMessage(`Settings saved, but API keys for ${engines.join(', ')} could not be stored. Check console for details.`, true);
        } else {
            showMessage('Settings saved successfully!');
        }

        // Update current settings
        currentSettings = settings;

        // Refresh history view if enabled
        if (settings.history_enabled) {
            historyOffset = 0;
            await renderHistory();
        }
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

// ============================================================
// Game Profiles initialization
// ============================================================
document.addEventListener('DOMContentLoaded', async () => {
    // --- Profile form event listeners ---
    addProfileBtn.addEventListener('click', () => openProfileForm());
    profileCancelBtn.addEventListener('click', closeProfileForm);
    profileSaveBtn.addEventListener('click', saveProfile);

    // --- Show debug checkbox ---
    showDebugCheckbox.addEventListener('change', async () => {
        try {
            await invoke('toggle_debug', { show: showDebugCheckbox.checked });
        } catch (e) {
            console.error('Failed to toggle debug:', e);
            showMessage('Failed to toggle debug: ' + e, true);
        }
    });

    // --- Load profiles ---
    closeProfileForm();
    try {
        profiles = await invokeWithRetry('list_profiles');
        renderProfiles();
    } catch (e) {
        console.error('Failed to load profiles:', e);
    }

    // --- Load active game info ---
    try {
        activeGameInfo = await invokeWithRetry('get_active_game');
        updateBanner();
    } catch (e) {
        console.error('Failed to get active game:', e);
    }

    // --- Load show_debug state ---
    try {
        const settings = await invoke('get_settings');
        showDebugCheckbox.checked = settings.show_debug === true;
    } catch (e) {
        console.error('Failed to load settings for show_debug:', e);
    }

    // --- Listen to events ---
    if (listen) {
        listen('active-game-changed', (event) => {
            activeGameInfo = event.payload;
            updateBanner();
        }).catch(e => console.error('Failed to listen active-game-changed:', e));

        listen('settings-changed', (event) => {
            const s = event.payload;
            if (typeof s.show_debug === 'boolean') {
                showDebugCheckbox.checked = s.show_debug;
            }
            // Update engine UI if engine config changed
            if (s.primary_engine || s.enabled_engines) {
                const enabledEngines = s.enabled_engines || __currentEnabledEngines;
                const primaryEngine = s.primary_engine || __currentPrimaryEngine;
                renderEnginesWithKeys(enabledEngines, primaryEngine);
            }
        }).catch(e => console.error('Failed to listen settings-changed:', e));
    }
});

// ============================================================
// Engine UI — Multi-Engine Checkboxes, Primary Dropdown, Per-Engine Keys
// ============================================================

// Track current engine state
let __currentEnabledEngines = [...FREE_ENGINES];
let __currentPrimaryEngine = 'google_gtx';

// ============================================================
// Custom terminal-style select component
// ============================================================

function createTerminalSelect(nativeSelect) {
    // If already wrapped, tear down old wrapper
    const existingWrap = nativeSelect.parentElement;
    if (existingWrap && existingWrap.classList.contains('terminal-select-wrap')) {
        const oldTs = existingWrap.querySelector('.terminal-select');
        if (oldTs) oldTs.remove();
        // Move native select back to original parent before re-wrapping
        existingWrap.parentNode.insertBefore(nativeSelect, existingWrap);
        existingWrap.remove();
    }

    // Wrap native select in hidden container
    const wrap = document.createElement('div');
    wrap.className = 'terminal-select-wrap';
    nativeSelect.parentNode.insertBefore(wrap, nativeSelect);
    wrap.appendChild(nativeSelect);

    // Create terminal select UI
    const ts = document.createElement('div');
    ts.className = 'terminal-select';
    wrap.appendChild(ts);

    // Current value display
    const current = document.createElement('div');
    current.className = 'ts-current';
    current.textContent = nativeSelect.options[nativeSelect.selectedIndex]?.textContent || nativeSelect.value || '';
    ts.appendChild(current);

    // Options list
    const options = document.createElement('div');
    options.className = 'ts-options';

    Array.from(nativeSelect.options).forEach(opt => {
        const optDiv = document.createElement('div');
        optDiv.className = 'ts-option';
        if (opt.selected) optDiv.classList.add('selected');
        optDiv.textContent = opt.textContent;
        optDiv.dataset.value = opt.value;
        optDiv.addEventListener('click', () => {
            nativeSelect.value = opt.value;
            nativeSelect.dispatchEvent(new Event('change', { bubbles: true }));
            current.textContent = opt.textContent;
            ts.classList.remove('open');
            // Update selected class
            options.querySelectorAll('.ts-option').forEach(o => o.classList.remove('selected'));
            optDiv.classList.add('selected');
        });
        options.appendChild(optDiv);
    });

    ts.appendChild(options);

    // Toggle open/close on current display click
    current.addEventListener('click', (e) => {
        e.stopPropagation();
        ts.classList.toggle('open');
    });

    // Close when clicking outside
    const closeHandler = (e) => {
        if (!ts.contains(e.target)) {
            ts.classList.remove('open');
        }
    };
    document.addEventListener('click', closeHandler);

    return ts;
}

function getCheckedPaidEngines() {
    const checked = [];
    PAID_ENGINES.forEach(engine => {
        const cb = document.getElementById(`engine-cb-${engine}`);
        if (cb && cb.checked) {
            checked.push(engine);
        }
    });
    return checked;
}

function renderEnginesWithKeys(enabledEngines, primaryEngine) {
    __currentEnabledEngines = enabledEngines;
    __currentPrimaryEngine = primaryEngine;

    const enginesList = document.getElementById('engines-list');
    if (!enginesList) return;
    enginesList.innerHTML = '';

    PAID_ENGINES.forEach(engine => {
        const block = document.createElement('div');
        block.className = 'engine-block';

        // Checkbox row
        const cbLabel = document.createElement('label');
        cbLabel.className = 'terminal-checkbox';

        const cb = document.createElement('input');
        cb.type = 'checkbox';
        cb.id = `engine-cb-${engine}`;
        cb.value = engine;
        cb.checked = enabledEngines.includes(engine);

        const cbDisplay = document.createElement('span');
        cbDisplay.className = 'cb-display';

        const cbLabelText = document.createElement('span');
        cbLabelText.className = 'cb-label';
        cbLabelText.textContent = ` Enable ${ENGINE_LABELS[engine]}`;

        cbLabel.appendChild(cb);
        cbLabel.appendChild(cbDisplay);
        cbLabel.appendChild(cbLabelText);
        block.appendChild(cbLabel);

        // API key row (hidden if unchecked)
        const keyRow = document.createElement('div');
        keyRow.className = 'engine-key-row';
        if (enabledEngines.includes(engine)) {
            keyRow.classList.add('visible');
        }

        const keyInputWrapper = document.createElement('div');
        keyInputWrapper.className = 'terminal-input';

        const keyInput = document.createElement('input');
        keyInput.type = 'password';
        keyInput.id = `api-key-${engine}`;
        keyInput.placeholder = `Enter ${ENGINE_LABELS[engine]} API key`;

        keyInputWrapper.appendChild(keyInput);
        keyRow.appendChild(keyInputWrapper);
        block.appendChild(keyRow);

        // Status element
        const statusEl = document.createElement('small');
        statusEl.id = `api-key-status-${engine}`;
        statusEl.className = 'engine-status';
        statusEl.style.display = 'none';
        block.appendChild(statusEl);

        // Toggle key row on checkbox change
        cb.addEventListener('change', () => {
            if (cb.checked) {
                keyRow.classList.add('visible');
                statusEl.style.display = '';
                checkEngineKeyStatus(engine).catch(e => console.warn(`checkEngineKeyStatus ${engine} failed:`, e));
            } else {
                keyRow.classList.remove('visible');
                statusEl.style.display = 'none';
                statusEl.textContent = '';
            }
            renderPrimaryDropdown(primaryEngineSelect.value);
        });

        enginesList.appendChild(block);
    });

    // Render primary engine dropdown
    renderPrimaryDropdown(primaryEngine);

    // Check stored keys for enabled paid engines only
    PAID_ENGINES.forEach(engine => {
        const cb = document.getElementById(`engine-cb-${engine}`);
        if (cb && cb.checked) {
            checkEngineKeyStatus(engine).catch(e => console.warn(`checkEngineKeyStatus ${engine} failed:`, e));
        }
    });
}

function renderPrimaryDropdown(selectedEngine) {
    const checkedPaid = getCheckedPaidEngines();
    const allEnabled = [...FREE_ENGINES, ...checkedPaid];

    primaryEngineSelect.innerHTML = '';
    allEnabled.forEach(engine => {
        const option = document.createElement('option');
        option.value = engine;
        option.textContent = ENGINE_LABELS[engine] || engine;
        if (engine === selectedEngine) {
            option.selected = true;
        }
        primaryEngineSelect.appendChild(option);
    });

    // Ensure selection is valid
    if (!allEnabled.includes(selectedEngine) && allEnabled.length > 0) {
        primaryEngineSelect.value = allEnabled[0];
    }

    __currentPrimaryEngine = primaryEngineSelect.value;

    // Refresh the terminal select wrapper for primary-engine (options changed)
    createTerminalSelect(primaryEngineSelect);
}

// When primary engine changes, update local state
primaryEngineSelect.addEventListener('change', () => {
    __currentPrimaryEngine = primaryEngineSelect.value;
});

// Check and display API key status for a specific engine
async function checkEngineKeyStatus(engine) {
    const statusEl = document.getElementById(`api-key-status-${engine}`);
    const inputEl = document.getElementById(`api-key-${engine}`);

    if (!PAID_ENGINES.includes(engine)) return;

    try {
        const key = await invoke('get_api_key', { engine });
        if (key && inputEl) {
            inputEl.value = key;
        }
        if (statusEl) {
            if (key) {
                statusEl.textContent = `✓ ${ENGINE_LABELS[engine]} API key stored`;
                statusEl.style.color = 'var(--success, #51cf66)';
            } else {
                statusEl.textContent = `✗ No ${ENGINE_LABELS[engine]} API key stored`;
                statusEl.style.color = 'var(--error, #ff6b6b)';
            }
        }
    } catch (e) {
        if (statusEl) {
            statusEl.textContent = `✗ Error checking key: ${e}`;
            statusEl.style.color = 'var(--error, #ff6b6b)';
        }
    }
}

// Test All Keys button
document.addEventListener('DOMContentLoaded', () => {
    const testAllBtn = document.getElementById('test-all-keys-btn');
    if (testAllBtn) {
        testAllBtn.addEventListener('click', testAllEnabledKeys);
    }
});

async function testAllEnabledKeys() {
    const testAllBtn = document.getElementById('test-all-keys-btn');
    const statusEl = document.getElementById('test-all-status');
    const checkedPaid = getCheckedPaidEngines();

    if (checkedPaid.length === 0) {
        if (statusEl) {
            statusEl.textContent = 'No paid engines enabled.';
            statusEl.className = 'test-status';
            statusEl.style.color = 'var(--text-secondary)';
        }
        return;
    }

    // Disable button during test
    if (testAllBtn) {
        testAllBtn.disabled = true;
        testAllBtn.textContent = 'TESTING...';
    }
    if (statusEl) {
        statusEl.textContent = 'Testing...';
        statusEl.className = 'test-status';
        statusEl.style.color = 'var(--text-secondary)';
    }

    const valid = [];
    const failed = [];
    const empty = [];

    for (const engine of checkedPaid) {
        const inputEl = document.getElementById(`api-key-${engine}`);
        const key = inputEl ? inputEl.value.trim() : '';

        if (!key) {
            empty.push(engine);
            continue;
        }

        try {
            const result = await invoke('test_api_key', { engine, key });
            if (result.success) {
                // Auto-save on success
                await invoke('set_api_key', { engine, key });
                valid.push(engine);
                // Update per-engine status (only if still checked)
                const cb = document.getElementById(`engine-cb-${engine}`);
                if (cb && cb.checked) {
                    const engStatus = document.getElementById(`api-key-status-${engine}`);
                    if (engStatus) {
                        engStatus.textContent = `✓ ${result.message}`;
                        engStatus.style.color = 'var(--terminal)';
                        engStatus.style.display = '';
                    }
                }
            } else {
                failed.push(engine);
                const cb = document.getElementById(`engine-cb-${engine}`);
                if (cb && cb.checked) {
                    const engStatus = document.getElementById(`api-key-status-${engine}`);
                    if (engStatus) {
                        engStatus.textContent = `✗ ${result.message}`;
                        engStatus.style.color = 'var(--error)';
                        engStatus.style.display = '';
                    }
                }
            }
        } catch (e) {
            failed.push(engine);
            const cb = document.getElementById(`engine-cb-${engine}`);
            if (cb && cb.checked) {
                const engStatus = document.getElementById(`api-key-status-${engine}`);
                if (engStatus) {
                    engStatus.textContent = `✗ Test failed: ${e}`;
                    engStatus.style.color = 'var(--error)';
                    engStatus.style.display = '';
                }
            }
        }
    }

    // Build status message
    let msg = '';
    if (empty.length === checkedPaid.length) {
        msg = 'All enabled engines have empty API keys.';
    } else if (failed.length === 0 && empty.length === 0) {
        msg = '✓ All keys valid and saved';
    } else if (valid.length === 0 && empty.length === 0) {
        msg = `✗ All failed: ${failed.map(e => ENGINE_LABELS[e] || e).join(', ')}`;
    } else {
        const parts = [];
        if (valid.length > 0) parts.push(`✓ ${valid.length} valid`);
        if (failed.length > 0) parts.push(`✗ ${failed.length} failed`);
        if (empty.length > 0) parts.push(`${empty.length} empty`);
        msg = parts.join(', ');
    }

    if (statusEl) {
        statusEl.textContent = msg;
        statusEl.className = 'test-status';
        statusEl.style.color = failed.length > 0 ? 'var(--error)' : (valid.length > 0 ? 'var(--terminal)' : 'var(--text-secondary)');
    }

    // Re-enable button
    if (testAllBtn) {
        testAllBtn.disabled = false;
        testAllBtn.textContent = 'TEST ALL KEYS';
    }
}

// ============================================================
// API Key Help Modal
// ============================================================

const API_KEY_HELP = {
    gemini: {
        title: 'Get Gemini API Key (Free)',
        content: `
            <p>Gemini 2.0 Flash offers a generous free tier with 15 requests/minute and 1M tokens/minute.</p>
            <ol>
                <li>Go to <a href="https://aistudio.google.com/apikey" target="_blank">Google AI Studio</a></li>
                <li>Sign in with your Google account</li>
                <li>Click <strong>"Create API Key"</strong></li>
                <li>Copy the generated key</li>
                <li>Paste it here and click Save</li>
            </ol>
            <div class="api-key-note">
                <strong>Free tier includes:</strong> 15 requests/min, 1M tokens/min. No credit card required.
            </div>
        `
    },
    deepl: {
        title: 'Get DeepL API Key (Free)',
        content: `
            <p>DeepL offers 500,000 characters/month free with their API.</p>
            <ol>
                <li>Go to <a href="https://www.deepl.com/pro-api" target="_blank">DeepL API</a></li>
                <li>Click "Get started for free"</li>
                <li>Create an account or sign in</li>
                <li>Navigate to your account's <strong>"API Keys"</strong> section</li>
                <li>Copy your authentication key</li>
                <li>Paste it here and click Save</li>
            </ol>
            <div class="api-key-note">
                <strong>Free tier includes:</strong> 500K characters/month. Translations are cached for consecutive identical texts.
            </div>
        `
    },
    google_gtx: {
        title: 'Google Translate (No API Key Required)',
        content: `
            <p>Google Translate is a free, unofficial Google Translate endpoint. No API key is needed.</p>
            <div class="api-key-note">
                <strong>How it works:</strong> Uses Google's unofficial translation API directly. Best-effort translation with no rate limits or quotas enforced.
            </div>
        `
    },
    mymemory: {
        title: 'MyMemory (No API Key Required)',
        content: `
            <p>MyMemory offers 5,000 characters/day free without registration (or 50,000/day with email).</p>
            <div class="api-key-note">
                <strong>How it works:</strong> Free machine translation service. No API key needed. Daily limit resets at midnight (UTC).
            </div>
        `
    },
    deepseek: {
        title: 'Get DeepSeek API Key',
        content: `
            <p>DeepSeek uses a prepaid balance system with very affordable pricing.</p>
            <ol>
                <li>Go to <a href="https://platform.deepseek.com" target="_blank">platform.deepseek.com</a></li>
                <li>Sign up or log in</li>
                <li>Navigate to API Keys section</li>
                <li>Create a new API key</li>
                <li>Copy and paste the key here</li>
            </ol>
            <div class="api-key-note">
                <strong>Note:</strong> DeepSeek uses a prepaid balance system. You need to add credits to your account (minimum ~$2 USD). The API is very affordable — $2 can handle thousands of translations.
            </div>
        `
    },
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
};

// Open help modal for current primary engine
engineHelpBtn.addEventListener('click', () => {
    const engine = primaryEngineSelect.value;
    const help = API_KEY_HELP[engine];

    if (help) {
        apiKeyModalTitle.textContent = help.title;
        apiKeyModalBody.innerHTML = help.content;
    } else {
        apiKeyModalTitle.textContent = 'API Key Help';
        apiKeyModalBody.innerHTML = '<p>Select an engine to see instructions.</p>';
    }

    apiKeyModal.classList.add('visible');
});

// Close modal
apiKeyModalClose.addEventListener('click', () => {
    apiKeyModal.classList.remove('visible');
});

// Close modal when clicking outside
apiKeyModal.addEventListener('click', (e) => {
    if (e.target === apiKeyModal) {
        apiKeyModal.classList.remove('visible');
    }
});

// Close modal on Escape key
document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && apiKeyModal.classList.contains('visible')) {
        apiKeyModal.classList.remove('visible');
    }
});

// ============================================================
// Logs Modal
// ============================================================

const viewLogsBtn = document.getElementById('view-logs-btn');

function formatLogLine(text) {
    const safe = escapeHtml(text);
    const upper = text.toUpperCase();
    let cssClass = 'log-line default';
    if (upper.includes('ERROR') || upper.includes('PANIC')) {
        cssClass = 'log-line error';
    } else if (upper.includes('WARN') || upper.includes('FAILED')) {
        cssClass = 'log-line warn';
    } else if (upper.includes('[OK]') || upper.includes('SUCCESS')) {
        cssClass = 'log-line ok';
    }
    return `<div class="${cssClass}">&gt; ${safe}</div>`;
}

async function openLogsModal() {
    const body = document.getElementById('logs-modal-body');
    const modal = document.getElementById('logs-modal');
    if (!modal || !body) return;

    try {
        const logs = await invoke('get_recent_logs');
        if (!logs || logs.length === 0) {
            body.innerHTML = '<div class="log-line default">&gt; No logs yet</div>';
        } else {
            body.innerHTML = logs.map(entry => {
                const level = entry.level || 'INFO';
                const timestamp = entry.timestamp || '';
                const message = entry.message || '';
                const text = `[${timestamp}] [${level}] ${message}`;
                return formatLogLine(text);
            }).join('');
        }
        body.scrollTop = body.scrollHeight;
    } catch (e) {
        body.innerHTML = `<div class="log-line error">&gt; Error loading logs: ${escapeHtml(String(e))}</div>`;
    }

    modal.classList.add('visible');
}

function closeLogsModal() {
    const modal = document.getElementById('logs-modal');
    if (modal) {
        modal.classList.remove('visible');
    }
}

viewLogsBtn.addEventListener('click', () => {
    openLogsModal();
});

// Logs modal close and clear buttons
document.addEventListener('DOMContentLoaded', () => {
    const closeBtn = document.getElementById('logs-modal-close');
    if (closeBtn) {
        closeBtn.addEventListener('click', closeLogsModal);
    }

    const clearBtn = document.getElementById('logs-modal-clear');
    if (clearBtn) {
        clearBtn.addEventListener('click', async () => {
            const body = document.getElementById('logs-modal-body');
            try {
                await invoke('clear_logs');
                if (body) body.innerHTML = '<div class="log-line default">&gt; Logs cleared</div>';
            } catch (e) {
                if (body) body.innerHTML = `<div class="log-line error">&gt; Error clearing logs: ${escapeHtml(String(e))}</div>`;
            }
        });
    }

    // Close logs modal when clicking outside
    const logsModal = document.getElementById('logs-modal');
    if (logsModal) {
        logsModal.addEventListener('click', (e) => {
            if (e.target === logsModal) {
                closeLogsModal();
            }
        });
    }
});

// Close logs modal on Escape
document.addEventListener('keydown', (e) => {
    const logsModal = document.getElementById('logs-modal');
    if (e.key === 'Escape' && logsModal && logsModal.classList.contains('visible')) {
        closeLogsModal();
    }
});

// Auto-refresh logs every 3 seconds when modal is visible
setInterval(() => {
    const modal = document.getElementById('logs-modal');
    if (modal && modal.classList.contains('visible')) {
        const body = document.getElementById('logs-modal-body');
        if (body) {
            invoke('get_recent_logs').then(logs => {
                if (logs && logs.length > 0) {
                    body.innerHTML = logs.map(entry => {
                        const level = entry.level || 'INFO';
                        const timestamp = entry.timestamp || '';
                        const message = entry.message || '';
                        const text = `[${timestamp}] [${level}] ${message}`;
                        return formatLogLine(text);
                    }).join('');
                    // Keep scroll at bottom if user hasn't scrolled up
                    const atBottom = body.scrollHeight - body.scrollTop - body.clientHeight < 40;
                    if (atBottom) {
                        body.scrollTop = body.scrollHeight;
                    }
                }
            }).catch(() => {});
        }
    }
}, 3000);
