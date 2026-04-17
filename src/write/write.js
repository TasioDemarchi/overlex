// Write mode - manual text input for translation

console.log("Write mode loaded");

const input = document.getElementById('write-input');
const status = document.getElementById('status');

// Auto-focus on load
input?.focus();

input?.addEventListener('keydown', async (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        const text = input.value.trim();
        if (!text) return;

        // Show translating status
        status.classList.add('visible');
        input.disabled = true;

        try {
            // Invoke translate_text command
            const result = await window.__TAURI__.core.invoke('translate_text', { text });
            console.log('Translation result:', result);
        } catch (err) {
            console.error('Translation error:', err);
            // Still close on error - let user retry
        }
        // Note: write window is closed by Rust after translation completes
    } else if (e.key === 'Escape') {
        // Close without translating
        window.__TAURI__.window.getCurrent().close();
    }
});

// Handle window close via escape key also on window level
window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
        window.__TAURI__.window.getCurrent().close();
    }
});