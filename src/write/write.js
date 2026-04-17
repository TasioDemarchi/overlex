// Write mode - manual text input for translation
// TODO:
// 1. Show input field focused on window open
// 2. On Enter, invoke("translate_text", { text }) and close window
// 3. On Escape, close window without action

console.log("Write mode loaded");

// Placeholder: will be implemented in T-011
const input = document.getElementById('write-input');
input?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
        // TODO: invoke translate_text and close
    } else if (e.key === 'Escape') {
        // TODO: close window
    }
});