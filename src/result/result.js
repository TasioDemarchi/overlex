// Result overlay - display translation result
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// DOM elements
const loadingEl = document.getElementById('loading');
const originalEl = document.getElementById('original');
const translatedEl = document.getElementById('translated');
const errorEl = document.getElementById('error');
const dismissBtn = document.getElementById('dismiss-btn');

// Auto-dismiss timer state
let dismissTimer = null;
let isHovering = false;

// Start auto-dismiss timer
function startDismissTimer(timeoutMs) {
    // Clear any existing timer
    if (dismissTimer) {
        clearTimeout(dismissTimer);
    }

    // Don't start timer if timeout is 0 (disabled)
    if (!timeoutMs || timeoutMs === 0) {
        return;
    }

    dismissTimer = setTimeout(async () => {
        try {
            await invoke('dismiss_result');
        } catch (e) {
            console.error('Failed to auto-dismiss:', e);
        }
    }, timeoutMs);
}

// Pause timer when hovering
function pauseDismissTimer() {
    isHovering = true;
    if (dismissTimer) {
        clearTimeout(dismissTimer);
        dismissTimer = null;
    }
}

// Resume timer on mouse leave
function resumeDismissTimer(timeoutMs) {
    isHovering = false;
    startDismissTimer(timeoutMs);
}

// Add hover listeners to the result overlay container
const overlayContainer = document.querySelector('.overlay');
if (overlayContainer) {
    overlayContainer.addEventListener('mouseenter', pauseDismissTimer);
    overlayContainer.addEventListener('mouseleave', () => resumeDismissTimer(window.__overlexTimeoutMs || 5000));
}

// Listen for translation-result event from Rust
listen('translation-result', (event) => {
    const payload = event.payload;

    // Store timeout and make available for hover resume
    window.__overlexTimeoutMs = payload.timeout_ms || 5000;

    // Start auto-dismiss timer
    startDismissTimer(window.__overlexTimeoutMs);

    // Hide loading
    loadingEl.style.display = 'none';

    if (payload.error) {
        // Show error
        errorEl.textContent = payload.error;
        errorEl.style.display = 'block';
        originalEl.style.display = 'none';
        translatedEl.style.display = 'none';
    } else {
        // Show translation result
        originalEl.textContent = payload.original;
        translatedEl.textContent = payload.translated;
        originalEl.style.display = 'block';
        translatedEl.style.display = 'block';
        errorEl.style.display = 'none';
    }
});

// Listen for overlex-error events from Rust commands
listen('overlex-error', (event) => {
    const payload = event.payload;

    // Hide loading, show error
    loadingEl.style.display = 'none';
    errorEl.textContent = payload.message;
    errorEl.style.display = 'block';
    originalEl.style.display = 'none';
    translatedEl.style.display = 'none';
});

// Dismiss button handler
dismissBtn.addEventListener('click', async () => {
    try {
        await invoke('dismiss_result');
    } catch (e) {
        console.error('Failed to dismiss:', e);
    }
});

// ESC key handler
document.addEventListener('keydown', async (e) => {
    if (e.key === 'Escape') {
        try {
            await invoke('dismiss_result');
        } catch (e) {
            console.error('Failed to dismiss:', e);
        }
    }
});

console.log('Result overlay initialized');