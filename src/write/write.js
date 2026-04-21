// Write mode - terminal-style chat for translation

console.log("Write mode loaded");

const chatHistory = document.getElementById('chat-history');
const input = document.getElementById('write-input');
const closeBtn = document.getElementById('close-btn');

let hasMessages = false;

function removeEmptyState() {
    if (!hasMessages) {
        const emptyState = chatHistory.querySelector('.empty-state');
        if (emptyState) {
            emptyState.remove();
            hasMessages = true;
        }
    }
}

function scrollToBottom() {
    chatHistory.scrollTop = chatHistory.scrollHeight;
}

function closeWindow() {
    chatHistory.innerHTML = '<div class="empty-state">Type text and press Enter to translate...</div>';
    hasMessages = false;
    window.__TAURI__.core.invoke('hide_window', { label: 'write' });
}

// Auto-focus on load
input?.focus();

// Close button
closeBtn?.addEventListener('click', closeWindow);

input?.addEventListener('keydown', async (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        const text = input.value.trim();
        if (!text) return;

        removeEmptyState();

        // Create message entry container
        const entry = document.createElement('div');
        entry.className = 'message-entry';

        // Original text (small, gray)
        const originalEl = document.createElement('div');
        originalEl.className = 'original-text';
        originalEl.textContent = text;
        entry.appendChild(originalEl);

        // Loading placeholder
        const loadingEl = document.createElement('div');
        loadingEl.className = 'loading-text';
        loadingEl.textContent = 'Translating';
        entry.appendChild(loadingEl);

        chatHistory.appendChild(entry);
        input.value = '';
        input.disabled = true;
        scrollToBottom();

        try {
            const result = await window.__TAURI__.core.invoke('translate_chat', { text });
            console.log('Translation result:', result);

            // Replace loading with translated text
            loadingEl.remove();

            const translatedEl = document.createElement('div');
            translatedEl.className = 'translated-text';
            translatedEl.textContent = result.translated;
            entry.appendChild(translatedEl);

            if (result.detected_source) {
                const langEl = document.createElement('div');
                langEl.className = 'detected-lang';
                langEl.textContent = `Detected: ${result.detected_source}`;
                entry.appendChild(langEl);
            }

            scrollToBottom();

        } catch (err) {
            console.error('Translation error:', err);
            loadingEl.remove();

            const errorEl = document.createElement('div');
            errorEl.className = 'error-text';
            errorEl.textContent = `Error: ${err}`;
            entry.appendChild(errorEl);
            scrollToBottom();
        }

        input.disabled = false;
        input.focus();

    } else if (e.key === 'Escape') {
        closeWindow();
    }
});

// ESC at window level
window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closeWindow();
});

// Focus input on window focus
window.addEventListener('focus', () => {
    input?.focus();
});

// Enable dragging on header
const header = document.getElementById('header');
header?.addEventListener('pointerdown', (e) => {
    if (e.target.id === 'close-btn') return;
    try {
        const win = window.__TAURI__?.window?.getCurrentWindow?.()
                 || window.__TAURI__?.webviewWindow?.getCurrentWindow?.();
        if (win && typeof win.startDragging === 'function') {
            win.startDragging();
        }
    } catch (err) {
        console.error('Drag failed:', err);
    }
});
