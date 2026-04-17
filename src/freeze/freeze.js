// Freeze overlay - canvas-based region selection
// Implements T-006: Freeze overlay plumbing

(function() {
    'use strict';

    const canvas = document.getElementById('freeze-canvas');
    const ctx = canvas.getContext('2d');

    // State
    let screenshotImage = null;
    let isDragging = false;
    let dragStart = { x: 0, y: 0 };
    let dragCurrent = { x: 0, y: 0 };

    // Constants
    const DIM_OVERLAY = 'rgba(0, 0, 0, 0.3)';
    const MIN_DRAG_DISTANCE = 5;

    // Initialize canvas to full window size
    function resizeCanvas() {
        canvas.width = window.innerWidth;
        canvas.height = window.innerHeight;
        // Redraw if we already have an image
        if (screenshotImage) {
            draw();
        }
    }

    // Main draw function: screenshot + dim overlay + selection
    function draw() {
        if (!screenshotImage) return;

        // 1. Draw the screenshot
        ctx.drawImage(screenshotImage, 0, 0, canvas.width, canvas.height);

        // 2. Apply dim overlay over entire screen
        ctx.fillStyle = DIM_OVERLAY;
        ctx.fillRect(0, 0, canvas.width, canvas.height);

        // 3. If dragging, clear the selection rectangle (show original brightness)
        if (isDragging) {
            const x = Math.min(dragStart.x, dragCurrent.x);
            const y = Math.min(dragStart.y, dragCurrent.y);
            const w = Math.abs(dragCurrent.x - dragStart.x);
            const h = Math.abs(dragCurrent.y - dragStart.y);

            if (w > 0 && h > 0) {
                // Clear the dimmed area to show original screenshot
                ctx.clearRect(x, y, w, h);
                // Redraw the screenshot in that area
                ctx.drawImage(screenshotImage, x, y, w, h, x, y, w, h);

                // Draw selection border
                ctx.strokeStyle = '#00ff00';
                ctx.lineWidth = 2;
                ctx.strokeRect(x, y, w, h);
            }
        }
    }

    // Handle mouse down - start drag
    function onMouseDown(e) {
        isDragging = true;
        dragStart.x = e.clientX;
        dragStart.y = e.clientY;
        dragCurrent.x = e.clientX;
        dragCurrent.y = e.clientY;
    }

    // Handle mouse move - update drag
    function onMouseMove(e) {
        if (!isDragging) return;
        dragCurrent.x = e.clientX;
        dragCurrent.y = e.clientY;
        draw();
    }

    // Handle mouse up - end drag, send coordinates to Rust
    function onMouseUp(e) {
        if (!isDragging) return;
        isDragging = false;

        // Calculate selection rectangle
        const x = Math.min(dragStart.x, dragCurrent.x);
        const y = Math.min(dragStart.y, dragCurrent.y);
        const width = Math.abs(dragCurrent.x - dragStart.x);
        const height = Math.abs(dragCurrent.y - dragStart.y);

        // Ignore small clicks (not a real drag)
        if (width < MIN_DRAG_DISTANCE || height < MIN_DRAG_DISTANCE) {
            console.log('Drag too small, ignoring');
            return;
        }

        // Convert to integers and call Rust
        const rect = {
            x: Math.round(x),
            y: Math.round(y),
            width: Math.round(width),
            height: Math.round(height)
        };

        console.log('Selection:', rect);

        // Invoke Rust to capture the region
        window.__TAURI__.core.invoke('ocr_capture_region', rect)
            .then(() => {
                console.log('OCR region sent to Rust');
                // Rust will close the window
            })
            .catch(err => {
                console.error('Failed to invoke ocr_capture_region:', err);
                // Fallback: close the window ourselves if invoke fails
                window.__TAURI__.window.getCurrentWindow().close();
            });
    }

    // Handle ESC key to dismiss without OCR
    function onKeyDown(e) {
        if (e.key === 'Escape') {
            console.log('ESC pressed, closing freeze overlay');
            window.__TAURI__.window.getCurrentWindow().close();
        }
    }

    // Listen for start-freeze event from Rust
    async function init() {
        console.log('Freeze overlay initializing...');

        // Set up canvas size
        resizeCanvas();
        window.addEventListener('resize', resizeCanvas);

        // Mouse events
        canvas.addEventListener('mousedown', onMouseDown);
        window.addEventListener('mousemove', onMouseMove);
        window.addEventListener('mouseup', onMouseUp);

        // Keyboard events
        window.addEventListener('keydown', onKeyDown);

        // Listen for the screenshot from Rust
        try {
            await window.__TAURI__.event.listen('start-freeze', (event) => {
                console.log('Received start-freeze event');
                const payload = event.payload;
                const screenshotB64 = payload.screenshot_b64;

                if (!screenshotB64) {
                    console.error('No screenshot data in event');
                    return;
                }

                // Decode base64 to image
                const img = new Image();
                img.onload = () => {
                    screenshotImage = img;
                    resizeCanvas();
                    draw();
                    console.log('Screenshot drawn on canvas');
                };
                img.onerror = () => {
                    console.error('Failed to load screenshot image');
                };
                img.src = 'data:image/png;base64,' + screenshotB64;
            });
            console.log('Listening for start-freeze events');
        } catch (err) {
            console.error('Failed to set up event listener:', err);
        }
    }

    // Start when DOM is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();