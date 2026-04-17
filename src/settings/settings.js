// Settings panel - load/save settings via Tauri invoke
// TODO:
// 1. On load, invoke("get_settings") and populate form
// 2. On save, validate hotkeys, invoke("save_settings")
// 3. Handle hotkey input capture for custom key combos

console.log("Settings loaded");

// Placeholder: will be implemented in T-012/T-015
// document.addEventListener('DOMContentLoaded', async () => {
//     const settings = await window.__TAURI__.core.invoke('get_settings');
//     // populate form
// });

// document.getElementById('save-btn')?.addEventListener('click', async () => {
//     // gather form data, invoke save_settings
// });