// Freeze overlay - canvas-based region selection
// TODO:
// 1. Listen for 'start-freeze' event with base64 screenshot
// 2. Draw image on canvas, apply dim overlay
// 3. Handle mousedown, mousemove, mouseup for drag selection
// 4. On mouseup, invoke("ocr_capture_region", {x, y, w, h}) and close window

console.log("Freeze overlay loaded");

// Placeholder: will be implemented in T-006/T-007
const canvas = document.getElementById('freeze-canvas');
const ctx = canvas?.getContext('2d');

// Listen for screenshot from Rust
// window.__TAURI__.event.listen('start-freeze', (event) => { ... })