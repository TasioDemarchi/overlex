// Result overlay - display translation result

// DOM elements
const loadingEl = document.getElementById('loading');
const originalEl = document.getElementById('original');
const translatedEl = document.getElementById('translated');
const errorEl = document.getElementById('error');
const dismissBtn = document.getElementById('dismiss-btn');
const originalToggle = document.getElementById('original-toggle');
const originalSection = document.getElementById('original-section');
const langDisplay = document.getElementById('lang-display');
const swapBtn = document.getElementById('swap-btn');

// Store current original text for re-translation
let __currentOriginal = '';

// Timer state
let __dismissTimerId = null;
let __progressIntervalId = null;

function startDismissTimer(timeoutMs) {
    // Clear any existing timer
    if (__dismissTimerId) {
        clearTimeout(__dismissTimerId);
        __dismissTimerId = null;
    }
    if (__progressIntervalId) {
        clearInterval(__progressIntervalId);
        __progressIntervalId = null;
    }

    const progressBar = document.getElementById('dismiss-progress');
    if (!progressBar) return;

    // Hide progress bar if no valid timeout
    if (timeoutMs <= 0) {
        progressBar.style.display = 'none';
        return;
    }

    // Show progress bar
    progressBar.style.display = 'block';
    progressBar.style.width = '100%';

    const startTime = Date.now();
    const totalMs = timeoutMs;

    // Update progress bar every 50ms for smooth animation
    __progressIntervalId = setInterval(() => {
        const elapsed = Date.now() - startTime;
        const remaining = Math.max(0, totalMs - elapsed);
        const percent = (remaining / totalMs) * 100;
        progressBar.style.width = percent + '%';

        if (remaining <= 0) {
            clearInterval(__progressIntervalId);
            __progressIntervalId = null;
        }
    }, 50);

    // Set dismiss timer
    __dismissTimerId = setTimeout(async () => {
        __dismissTimerId = null;
        __progressIntervalId = null;
        try {
            await window.__TAURI__?.core?.invoke('dismiss_result');
        } catch (e) {
            console.error('Auto-dismiss failed:', e);
        }
    }, timeoutMs);
}

// Toggle original text visibility
originalToggle.addEventListener('click', () => {
    const isOpen = originalEl.style.display !== 'none';
    originalEl.style.display = isOpen ? 'none' : 'block';
    originalToggle.textContent = isOpen ? '▶ Original' : '▼ Original';
});

// Reset toggle to collapsed state
function resetOriginalToggle() {
    originalEl.style.display = 'none';
    originalToggle.textContent = '▶ Original';
}

// === GLOBAL HANDLERS (called via eval from Rust — guaranteed delivery) ===
window.onTranslationResult = function(payload) {
    window.__overlexTimeoutMs = payload.timeout_ms ?? 5000;
    startDismissTimer(window.__overlexTimeoutMs);
    loadingEl.style.display = 'none';

    // Update language indicator from payload (fall back to displayed source)
    if (payload.source_lang && payload.target_lang) {
        const sourceUpper = (payload.source_lang === 'auto' ? 'AUTO' : payload.source_lang.toUpperCase());
        const targetUpper = payload.target_lang.toUpperCase();
        langDisplay.textContent = `${sourceUpper} → ${targetUpper}`;
    }

    // Store original text for re-translation
    if (payload.original) {
        window.__currentOriginal = payload.original;
    }

    if (payload.error) {
        errorEl.textContent = payload.error;
        errorEl.style.display = 'block';
        originalEl.style.display = 'none';
        translatedEl.style.display = 'none';
        originalSection.style.display = 'none';
    } else {
        translatedEl.textContent = payload.translated;
        originalEl.textContent = payload.original;
        resetOriginalToggle();
        errorEl.style.display = 'none';
        translatedEl.style.display = 'block';
        originalSection.style.display = 'block';
    }
};

window.onOverlexError = function(payload) {
    loadingEl.style.display = 'none';
    errorEl.textContent = payload.message;
    errorEl.style.display = 'block';
    originalEl.style.display = 'none';
    translatedEl.style.display = 'none';
    originalSection.style.display = 'none';
};

// Fallback: also try Tauri event listeners (wrapped safely)
try {
    const listen = window.__TAURI__?.event?.listen;
    if (typeof listen === 'function') {
        listen('translation-result', (event) => window.onTranslationResult(event.payload));
        listen('overlex-error', (event) => window.onOverlexError(event.payload));
        listen('languages-swapped', (event) => {
            const { source_lang, target_lang } = event.payload;
            const sourceUpper = (source_lang === 'auto' ? 'AUTO' : source_lang.toUpperCase());
            const targetUpper = target_lang.toUpperCase();
            langDisplay.textContent = `${sourceUpper} → ${targetUpper}`;
        });
    }
} catch (err) {
    console.warn('Tauri event listen not available:', err);
}

// === Debug line indicator ===
let __currentEngine = '—';

(async function initDebugLine() {
    try {
        const settings = await window.__TAURI__?.core?.invoke('get_settings');
        if (settings) {
            __currentEngine = settings.engine || '—';
            if (settings.show_debug) {
                const debugEl = document.getElementById('debug-line');
                if (debugEl) debugEl.classList.add('visible');
            }
        }
    } catch (e) {
        console.warn('Failed to load debug settings:', e);
    }

    // Listen for active game changes
    try {
        const listen = window.__TAURI__?.event?.listen;
        if (typeof listen === 'function') {
            // Active game changed
            listen('active-game-changed', (event) => {
                const info = event.payload;
                const debugEl = document.getElementById('debug-line');
                if (!debugEl) return;
                if (info.process_name) {
                    let text = info.process_name;
                    if (info.matched_profile) text += ` [${info.matched_profile}]`;
                    text += ` · ${__currentEngine}`;
                    if (info.fullscreen_exclusive) text += ' ⚠ Fullscreen';
                    debugEl.textContent = text;
                } else {
                    debugEl.textContent = `— · ${__currentEngine}`;
                }
            });

            // Settings changed (show_debug toggled)
            listen('settings-changed', (event) => {
                const payload = event.payload;
                if (payload && typeof payload.show_debug === 'boolean') {
                    const debugEl = document.getElementById('debug-line');
                    if (!debugEl) return;
                    if (payload.show_debug) {
                        debugEl.classList.add('visible');
                    } else {
                        debugEl.classList.remove('visible');
                    }
                }
            });
        }
    } catch (e) {
        console.warn('Failed to set up debug listeners:', e);
    }
})();

// Dismiss button
dismissBtn.addEventListener('click', async () => {
    try { await window.__TAURI__?.core?.invoke('dismiss_result'); } catch (e) { console.error('Failed to dismiss:', e); }
});

// Swap button - swap languages and re-translate
swapBtn.addEventListener('click', async () => {
    const originalText = window.__currentOriginal;
    if (!originalText) {
        console.warn('No original text to re-translate');
        return;
    }

    try {
        // Call swap_languages command
        const result = await window.__TAURI__?.core?.invoke('swap_languages');
        if (result) {
            // Update display immediately (will be confirmed by event)
            const sourceUpper = (result.source_lang === 'auto' ? 'AUTO' : result.source_lang.toUpperCase());
            const targetUpper = result.target_lang.toUpperCase();
            langDisplay.textContent = `${sourceUpper} → ${targetUpper}`;

            // Re-translate with the NEW direction
            const translateResult = await window.__TAURI__?.core?.invoke('translate_chat', {
                text: originalText
            });

            if (translateResult) {
                // Update the translated text
                translatedEl.textContent = translateResult.translated;
                originalEl.textContent = originalText;

                if (translateResult.detected_source) {
                    // Update indicator if source was auto-detected
                    const detected = translateResult.detected_source.toUpperCase();
                    langDisplay.textContent = `${detected} → ${result.target_lang.toUpperCase()}`;
                }
            }
        }
    } catch (e) {
        console.error('Failed to swap languages:', e);
    }
});

// ESC key
document.addEventListener('keydown', async (e) => {
    if (e.key === 'Escape') {
        try { await window.__TAURI__?.core?.invoke('dismiss_result'); } catch (e) { console.error('Failed to dismiss:', e); }
    }
});

// Manual drag for WS_EX_NOACTIVATE window
(function() {
    const dragBar = document.getElementById('drag-bar');
    if (!dragBar) return;

    let isDragging = false;
    let dragStartX = 0;
    let dragStartY = 0;
    let winStartX = 0;
    let winStartY = 0;

    dragBar.style.cursor = 'grab';

    dragBar.addEventListener('pointerdown', async (e) => {
        if (e.target.id === 'dismiss-btn' || e.target.id === 'swap-btn') return;

        isDragging = true;
        dragStartX = e.screenX;
        dragStartY = e.screenY;
        dragBar.setPointerCapture(e.pointerId);
        dragBar.style.cursor = 'grabbing';

        try {
            const win = window.__TAURI__?.window?.getCurrentWindow?.()
                     || window.__TAURI__?.webviewWindow?.getCurrentWindow?.();
            if (win) {
                const pos = await win.outerPosition();
                winStartX = pos.x;
                winStartY = pos.y;
            }
        } catch (err) {
            console.error('Failed to get window position:', err);
            isDragging = false;
        }
    });

    dragBar.addEventListener('pointermove', (e) => {
        if (!isDragging) return;

        const deltaX = e.screenX - dragStartX;
        const deltaY = e.screenY - dragStartY;
        const newX = winStartX + deltaX;
        const newY = winStartY + deltaY;

        // Fire and forget — do NOT await to prevent lag
        window.__TAURI__?.core?.invoke('drag_result_window_noactivate', {
            x: newX,
            y: newY
        });
    });

    dragBar.addEventListener('pointerup', (e) => {
        if (isDragging) {
            dragBar.releasePointerCapture(e.pointerId);
            isDragging = false;
            dragBar.style.cursor = 'grab';
        }
    });
})();

console.log('Result overlay initialized');