// Settings panel - load/save settings via Tauri invoke

const { invoke } = window.__TAURI__.core;
const listen = window.__TAURI__?.event?.listen;

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

// API Key help modal elements
const engineHelpBtn = document.getElementById('engine-help-btn');
const apiKeyModal = document.getElementById('api-key-modal');
const apiKeyModalClose = document.getElementById('api-key-modal-close');
const apiKeyModalTitle = document.getElementById('api-key-modal-title');
const apiKeyModalBody = document.getElementById('api-key-modal-body');
const engineKeyStatus = document.getElementById('engine-key-status');
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
        if (profile.engine) badges.push(`<span class="badge badge-engine">${escapeHtml(profile.engine)}</span>`);
        if (profile.ocr_preprocessing) badges.push('<span class="badge badge-ocr">preprocess</span>');
        if (profile.ocr_binarize) badges.push('<span class="badge badge-ocr">binarize</span>');

        card.innerHTML = `
            <div class="profile-card-header">
                <strong>${escapeHtml(profile.display_name)}</strong>
                <div class="profile-card-actions">
                    <button class="small-btn edit-profile-btn">Edit</button>
                    <button class="small-btn danger delete-profile-btn">Delete</button>
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
        profileEngine.value = profile.engine || '';
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
        engine: profileEngine.value || null,
        ocr_preprocessing: profileOcrPreprocessing.checked ? true : null,
        ocr_binarize: profileOcrBinarize.checked ? true : null,
    };

    try {
        if (editingProfile) {
            await invoke('update_profile', { profile });
        } else {
            await invoke('add_profile', { profile });
        }

        // Refresh list from backend
        profiles = await invoke('list_profiles');
        renderProfiles();
        closeProfileForm();
        showMessage(editingProfile ? 'Profile updated' : 'Profile added');
    } catch (e) {
        showMessage('Failed to save profile: ' + e, true);
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
        if (profile.engine) details.push(`Engine: ${profile.engine}`);
        if (profile.source_lang) details.push(`Source: ${profile.source_lang}`);
    }

    activeProfileDetails.textContent = details.join(' · ');
    activeProfileBanner.style.display = 'block';
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
        engineSelect.value = settings.engine || 'google_gtx';
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
        // Save settings (this validates hotkeys on the backend)
        await invoke('save_settings', { settings });

        // Also save API key separately if provided
        // Use current dropdown value, not the loaded settings object
        if (apiKeyInput.value) {
            await invoke('set_api_key', {
                engine: engineSelect.value,
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
    try {
        profiles = await invoke('list_profiles');
        renderProfiles();
    } catch (e) {
        console.error('Failed to load profiles:', e);
    }

    // --- Load active game info ---
    try {
        activeGameInfo = await invoke('get_active_game');
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
        }).catch(e => console.error('Failed to listen settings-changed:', e));
    }
});

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
    libretranslate: {
        title: 'Get LibreTranslate API Key',
        content: `
            <p>LibreTranslate requires running your own instance or using a public API with authentication.</p>
            <ol>
                <li>Option A: Use a public LibreTranslate server (may require API key)</li>
                <li>Option B: Run your own <a href="https://github.com/LibreTranslate/LibreTranslate" target="_blank">LibreTranslate server</a></li>
                <li>If using a public server, obtain the API key from that provider</li>
                <li>Paste it here and click Save</li>
            </ol>
            <div class="api-key-note">
                <strong>Note:</strong> LibreTranslate is open source. Running your own server gives you full control with no API costs.
            </div>
        `
    },
    google_gtx: {
        title: 'Google GTX (No API Key Required)',
        content: `
            <p>Google GTX is a free, unofficial Google Translate endpoint. No API key is needed.</p>
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
    }
};

// Open help modal
engineHelpBtn.addEventListener('click', () => {
    const engine = engineSelect.value;
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
// API Key Status Checker
// ============================================================

// Check and display API key status for current engine
async function checkEngineKeyStatus() {
    const engine = engineSelect.value;
    const enginesNeedingKey = ['gemini', 'deepl', 'libretranslate'];

    if (!enginesNeedingKey.includes(engine)) {
        engineKeyStatus.textContent = '';
        engineKeyStatus.style.color = '';
        return;
    }

    try {
        const hasKey = await invoke('check_api_key', { engine });
        if (hasKey) {
            engineKeyStatus.textContent = `✓ ${engine.toUpperCase()} API key stored`;
            engineKeyStatus.style.color = 'var(--success, #51cf66)';
        } else {
            engineKeyStatus.textContent = `✗ No ${engine.toUpperCase()} API key stored`;
            engineKeyStatus.style.color = 'var(--error, #ff6b6b)';
        }
    } catch (e) {
        engineKeyStatus.textContent = `✗ Error checking key: ${e}`;
        engineKeyStatus.style.color = 'var(--error, #ff6b6b)';
    }
}

// Check status when engine changes
engineSelect.addEventListener('change', checkEngineKeyStatus);
