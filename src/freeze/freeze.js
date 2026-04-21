// Freeze overlay - canvas-based region selection
// Implements T-006: Freeze overlay plumbing

(function() {
    'use strict';

    const canvas = document.getElementById('freeze-canvas');
    const ctx = canvas.getContext('2d', { alpha: false });

    // Patch canvas.width and canvas.height setters to detect who clears the canvas
    const _origWidth = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, 'width');
    const _origHeight = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, 'height');
    Object.defineProperty(canvas, 'width', {
        set(v) {
            window.__TAURI__?.core?.invoke('js_log', {msg: '[CANVAS] width set to ' + v + ' stack: ' + new Error().stack.split('\n').slice(1,3).join(' | ')});
            _origWidth.set.call(this, v);
        },
        get() { return _origWidth.get.call(this); }
    });
    Object.defineProperty(canvas, 'height', {
        set(v) {
            window.__TAURI__?.core?.invoke('js_log', {msg: '[CANVAS] height set to ' + v + ' stack: ' + new Error().stack.split('\n').slice(1,3).join(' | ')});
            _origHeight.set.call(this, v);
        },
        get() { return _origHeight.get.call(this); }
    });

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
        // Only resize if we have no screenshot yet — resizing clears the canvas
        if (screenshotImage || window._screenshotImg) return;
        canvas.width = window.innerWidth;
        canvas.height = window.innerHeight;
    }

    function draw() {
        // Use module-level screenshotImage OR the one injected by Rust eval
        const img = screenshotImage || window._screenshotImg;
        if (!img) return;

        ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
        ctx.fillStyle = DIM_OVERLAY;
        ctx.fillRect(0, 0, canvas.width, canvas.height);

        if (isDragging) {
            const x = Math.min(dragStart.x, dragCurrent.x);
            const y = Math.min(dragStart.y, dragCurrent.y);
            const w = Math.abs(dragCurrent.x - dragStart.x);
            const h = Math.abs(dragCurrent.y - dragStart.y);
            if (w > 0 && h > 0) {
                ctx.clearRect(x, y, w, h);
                ctx.drawImage(img, x, y, w, h, x, y, w, h);
                ctx.strokeStyle = '#00ff00';
                ctx.lineWidth = 2;
                ctx.strokeRect(x, y, w, h);
            }
        }
    }

    function onMouseDown(e) {
        const img = screenshotImage || window._screenshotImg;
        if (!img) {
            loadFromStore();
            return;
        }
        if (!screenshotImage) screenshotImage = img; // sync up
        isDragging = true;
        dragStart.x = e.clientX;
        dragStart.y = e.clientY;
        dragCurrent.x = e.clientX;
        dragCurrent.y = e.clientY;
    }

    function onMouseMove(e) {
        if (!isDragging) return;
        dragCurrent.x = e.clientX;
        dragCurrent.y = e.clientY;
        draw();
    }

    function onMouseUp(e) {
        if (!isDragging) return;
        isDragging = false;

        const x = Math.min(dragStart.x, dragCurrent.x);
        const y = Math.min(dragStart.y, dragCurrent.y);
        const width = Math.abs(dragCurrent.x - dragStart.x);
        const height = Math.abs(dragCurrent.y - dragStart.y);

        if (width < MIN_DRAG_DISTANCE || height < MIN_DRAG_DISTANCE) {
            console.log('Drag too small, ignoring');
            return;
        }

        // Calculate DPI scale factor: screenshot pixels vs CSS pixels
        const img = screenshotImage || window._screenshotImg;
        const imgW = img.naturalWidth ?? img.width;
        const imgH = img.naturalHeight ?? img.height;
        const scaleX = imgW / canvas.width;
        const scaleY = imgH / canvas.height;

        const rect = {
            x: Math.round(x * scaleX),
            y: Math.round(y * scaleY),
            width: Math.round(width * scaleX),
            height: Math.round(height * scaleY)
        };

        console.log('Selection:', rect);

        window.__TAURI__.core.invoke('ocr_capture_region', rect)
            .then(() => {
                console.log('OCR region sent to Rust');
                resetState();
            })
            .catch(err => {
                console.error('Failed to invoke ocr_capture_region:', err);
                resetState();
                window.__TAURI__.core.invoke('hide_window', { label: 'freeze' });
            });
    }

    // ESC to dismiss
    function onKeyDown(e) {
        if (e.key === 'Escape') {
            console.log('ESC pressed, hiding freeze overlay');
            resetState();
            window.__TAURI__.core.invoke('hide_window', { label: 'freeze' });
        }
    }

    function resetState() {
        screenshotImage = null;
        isDragging = false;
        ctx.clearRect(0, 0, canvas.width, canvas.height);
    }

    // Load PNG from Rust store via invoke (reliable fallback)
    async function loadFromStore() {
        try {
            console.log('loadFromStore: invoking get_stored_screenshot...');
            const pngBytes = await window.__TAURI__.core.invoke('get_stored_screenshot');
            console.log('loadFromStore: got bytes, length:', pngBytes?.length);
            if (pngBytes && pngBytes.length > 0) {
                await loadPngScreenshot(pngBytes);
            }
        } catch (err) {
            console.error('loadFromStore error:', err);
        }
    }

    // Load PNG bytes as blob URL for display
    async function loadPngScreenshot(pngBytes) {
        try {
            const bytes = Array.isArray(pngBytes) ? new Uint8Array(pngBytes) : pngBytes;
            console.log('loadPngScreenshot: bytes length:', bytes.length, 'first4:', bytes[0], bytes[1], bytes[2], bytes[3]);
            const blob = new Blob([bytes], { type: 'image/png' });
            const url = URL.createObjectURL(blob);
            const img = new Image();
            img.onload = () => {
                screenshotImage = img;
                URL.revokeObjectURL(url);
                resizeCanvas();
                draw();
                console.log('PNG screenshot drawn on canvas (' + img.naturalWidth + 'x' + img.naturalHeight + ')');
            };
            img.onerror = (e) => {
                console.error('Failed to decode PNG image', e);
                URL.revokeObjectURL(url);
            };
            img.src = url;
        } catch (err) {
            console.error('loadPngScreenshot error:', err);
        }
    }

    async function init() {
        console.log('Freeze overlay initializing...');

        resizeCanvas();
        // Don't add resize listener — it clears canvas. Handle manually if needed.

        canvas.addEventListener('mousedown', onMouseDown);
        window.addEventListener('mousemove', onMouseMove);
        window.addEventListener('mouseup', onMouseUp);

        window.addEventListener('keydown', onKeyDown);
        document.addEventListener('keydown', onKeyDown);

        // screenshot-png event (kept for compatibility)
        if (window.__TAURI__?.event) {
            window.__TAURI__.event.listen('screenshot-png', async (event) => {
                const pngBytes = event.payload;
                if (pngBytes && pngBytes.length > 0) await loadPngScreenshot(pngBytes);
            });
        }
        // NOTE: no focus listener, no setTimeout — Rust injects via eval directly
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
