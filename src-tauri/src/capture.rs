// Capture module - screen capture via Win32 GDI BitBlt
// Captures the primary monitor as a PNG, and crops regions from screenshots.

use image::{ImageBuffer, Rgba, ImageEncoder};
use image::codecs::png::PngEncoder;
use std::io::Cursor;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
    GetDIBits, GetDeviceCaps, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HORZRES, SRCCOPY, VERTRES,
};

/// Capture the primary monitor and return PNG bytes.
pub fn capture_fullscreen() -> Result<Vec<u8>, String> {
    unsafe {
        // Get device context for the entire screen
        let screen_dc = GetDC(HWND::default());
        if screen_dc.is_invalid() {
            return Err("Failed to get screen DC".to_string());
        }

        // Get actual screen dimensions in pixels (DPI-aware)
        let width = GetDeviceCaps(screen_dc, HORZRES);
        let height = GetDeviceCaps(screen_dc, VERTRES);

        if width <= 0 || height <= 0 {
            ReleaseDC(HWND::default(), screen_dc);
            return Err("Failed to get screen dimensions".to_string());
        }

        // Create compatible DC and bitmap
        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc.is_invalid() {
            ReleaseDC(HWND::default(), screen_dc);
            return Err("Failed to create compatible DC".to_string());
        }

        let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
        if bitmap.is_invalid() {
            let _ = DeleteDC(mem_dc);
            ReleaseDC(HWND::default(), screen_dc);
            return Err("Failed to create compatible bitmap".to_string());
        }

        // Select bitmap into memory DC and BitBlt from screen
        let old_bitmap = SelectObject(mem_dc, bitmap);
        let blt_result = BitBlt(mem_dc, 0, 0, width, height, screen_dc, 0, 0, SRCCOPY);
        if blt_result.is_err() {
            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteObject(bitmap);
            let _ = DeleteDC(mem_dc);
            ReleaseDC(HWND::default(), screen_dc);
            return Err("BitBlt failed".to_string());
        }

        // Prepare BITMAPINFO to read pixel data as 32-bit BGRA
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // negative = top-down (origin at top-left)
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default()],
        };

        // Allocate buffer for pixel data (BGRA, 4 bytes per pixel)
        let pixel_count = (width * height) as usize;
        let mut pixels: Vec<u8> = vec![0u8; pixel_count * 4];

        let lines = GetDIBits(
            mem_dc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        // Cleanup GDI resources
        SelectObject(mem_dc, old_bitmap);
        let _ = DeleteObject(bitmap);
        let _ = DeleteDC(mem_dc);
        ReleaseDC(HWND::default(), screen_dc);

        if lines == 0 {
            return Err("GetDIBits failed to read pixel data".to_string());
        }

        // Convert BGRA → RGBA (swap B and R channels)
        for i in 0..pixel_count {
            let offset = i * 4;
            pixels.swap(offset, offset + 2); // swap B and R
            pixels[offset + 3] = 255; // ensure alpha is opaque
        }

        // Encode as PNG using the image crate
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(width as u32, height as u32, pixels)
                .ok_or_else(|| "Failed to create image buffer from pixels".to_string())?;

        let mut png_bytes: Vec<u8> = Vec::new();
        let encoder = PngEncoder::new(Cursor::new(&mut png_bytes));
        encoder
            .write_image(img.as_raw(), width as u32, height as u32, image::ExtendedColorType::Rgba8)
            .map_err(|e| format!("PNG encoding failed: {e}"))?;

        Ok(png_bytes)
    }
}

/// Get the actual screen size in pixels (DPI-aware).
/// Returns (width, height) of the primary monitor.
pub fn get_screen_size() -> Result<(i32, i32), String> {
    unsafe {
        let screen_dc = GetDC(HWND::default());
        if screen_dc.is_invalid() {
            return Err("Failed to get screen DC".to_string());
        }

        let width = GetDeviceCaps(screen_dc, HORZRES);
        let height = GetDeviceCaps(screen_dc, VERTRES);

        ReleaseDC(HWND::default(), screen_dc);

        if width <= 0 || height <= 0 {
            return Err("Failed to get screen dimensions".to_string());
        }

        Ok((width, height))
    }
}

/// Crop a region from a PNG screenshot and return the cropped PNG bytes.
pub fn capture_region(
    screenshot_png: &[u8],
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    // Decode the full screenshot PNG
    let full_img = image::load_from_memory(screenshot_png)
        .map_err(|e| format!("Failed to decode screenshot PNG: {e}"))?;

    // Validate bounds
    let img_width = full_img.width();
    let img_height = full_img.height();
    let x = x.max(0) as u32;
    let y = y.max(0) as u32;
    let width = width.min(img_width.saturating_sub(x));
    let height = height.min(img_height.saturating_sub(y));

    if width == 0 || height == 0 {
        return Err("Region is empty or outside screenshot bounds".to_string());
    }

    // Crop the region
    let cropped = full_img.crop_imm(x, y, width, height);

    // Re-encode to PNG
    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = PngEncoder::new(Cursor::new(&mut png_bytes));
    let rgba = cropped.to_rgba8();
    encoder
        .write_image(rgba.as_raw(), width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("PNG encoding of cropped region failed: {e}"))?;

    Ok(png_bytes)
}
