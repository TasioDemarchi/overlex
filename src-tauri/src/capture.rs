// Capture module - screen capture via DXGI Desktop Duplication
// Captures the primary monitor as a PNG, and crops regions from screenshots.

use image::{ImageBuffer, Rgba, ImageEncoder};
use image::codecs::png::{PngEncoder, CompressionType, FilterType};
use std::io::Cursor;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
    GetDIBits, GetDeviceCaps, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HORZRES, LOGPIXELSX, SRCCOPY, VERTRES,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11_CPU_ACCESS_READ,
    D3D11_MAP_READ, ID3D11Texture2D, D3D11_CREATE_DEVICE_FLAG,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIOutputDuplication, IDXGIOutput1,
    DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::core::Interface;

/// Capture the primary monitor and return PNG bytes.
pub fn capture_fullscreen() -> Result<Vec<u8>, String> {
    let (pixels, width, height) = capture_fullscreen_raw()?;

    // Encode as PNG using the image crate
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixels)
            .ok_or_else(|| "Failed to create image buffer from pixels".to_string())?;

    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = PngEncoder::new_with_quality(Cursor::new(&mut png_bytes), CompressionType::Fast, FilterType::NoFilter);
    encoder
        .write_image(img.as_raw(), width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("PNG encoding failed: {e}"))?;

    Ok(png_bytes)
}

/// Capture the primary monitor and return raw RGBA bytes (NO PNG encoding).
/// Returns (rgba_bytes, width, height).
/// Uses DXGI Desktop Duplication (~50ms) with fallback to GDI (~6s).
pub fn capture_fullscreen_raw() -> Result<(Vec<u8>, u32, u32), String> {
    // Try DXGI first, fallback to GDI if it fails
    match capture_fullscreen_raw_dxgi() {
        Ok(result) => {
            eprintln!("[CAPTURE] DXGI succeeded {}x{}", result.1, result.2);
            Ok(result)
        },
        Err(e) => {
            eprintln!("[CAPTURE] DXGI failed: {}, falling back to GDI", e);
            let r = capture_fullscreen_raw_gdi();
            if let Ok(ref res) = r {
                eprintln!("[CAPTURE] GDI succeeded {}x{}", res.1, res.2);
            }
            r
        }
    }
}

/// DXGI Desktop Duplication capture (fast, ~50ms)
fn capture_fullscreen_raw_dxgi() -> Result<(Vec<u8>, u32, u32), String> {
    unsafe {
        // Step 1: Create D3D11 device
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;

        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            D3D11_CREATE_DEVICE_FLAG(0),
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(|e| format!("D3D11CreateDevice failed: {}", e))?;

        let device = device.ok_or_else(|| "D3D11 device is None".to_string())?;
        let context = context.ok_or_else(|| "D3D11 context is None".to_string())?;

        // Step 2: Get DXGI device and adapter
        let dxgi_device: IDXGIDevice = device
            .cast()
            .map_err(|e| format!("Failed to cast to IDXGIDevice: {}", e))?;

        let adapter: windows::Win32::Graphics::Dxgi::IDXGIAdapter = dxgi_device
            .GetAdapter()
            .map_err(|e| format!("GetAdapter failed: {}", e))?;

        // Step 3: Get primary output
        let output: windows::Win32::Graphics::Dxgi::IDXGIOutput = adapter
            .EnumOutputs(0)
            .map_err(|e| format!("EnumOutputs(0) failed: {}", e))?;

        let output1: IDXGIOutput1 = output
            .cast()
            .map_err(|e| format!("Failed to cast to IDXGIOutput1: {}", e))?;

        // Get output description for dimensions
        let output_desc = output1
            .GetDesc()
            .map_err(|e| format!("GetDesc failed: {}", e))?;

        let width = output_desc.DesktopCoordinates.right - output_desc.DesktopCoordinates.left;
        let height = output_desc.DesktopCoordinates.bottom - output_desc.DesktopCoordinates.top;

        if width <= 0 || height <= 0 {
            return Err("Invalid output dimensions".to_string());
        }

        // Step 4: Create desktop duplication
        let duplication: IDXGIOutputDuplication = output1
            .DuplicateOutput(&dxgi_device)
            .map_err(|e| format!("DuplicateOutput failed: {}", e))?;

        // Step 5: Acquire next frame — retry up to 5 times skipping empty frames
        let timeout_ms = 200u32;
        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut desktop_resource: Option<windows::Win32::Graphics::Dxgi::IDXGIResource> = None;

        for attempt in 0..5 {
            frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            desktop_resource = None;
            duplication
                .AcquireNextFrame(timeout_ms, &mut frame_info, &mut desktop_resource)
                .map_err(|e| format!("AcquireNextFrame failed: {}", e))?;
            if frame_info.AccumulatedFrames > 0 {
                eprintln!("[DXGI] got frame on attempt {}, AccumulatedFrames={}", attempt, frame_info.AccumulatedFrames);
                break;
            }
            eprintln!("[DXGI] empty frame on attempt {}, retrying...", attempt);
            let _ = duplication.ReleaseFrame();
        }

        let desktop_resource = desktop_resource.ok_or_else(|| "Desktop resource is None".to_string())?;

        // Step 6: Cast to ID3D11Texture2D
        let captured_texture: ID3D11Texture2D = desktop_resource
            .cast()
            .map_err(|e| format!("Failed to cast to ID3D11Texture2D: {}", e))?;

        // Get texture description
        let mut tex_desc = D3D11_TEXTURE2D_DESC::default();
        captured_texture.GetDesc(&mut tex_desc);

        // Step 7: Create staging texture for CPU readback
        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: tex_desc.Width,
            Height: tex_desc.Height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: tex_desc.SampleDesc,
            Usage: D3D11_USAGE_STAGING,
            BindFlags: windows::Win32::Graphics::Direct3D11::D3D11_BIND_FLAG(0).0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: windows::Win32::Graphics::Direct3D11::D3D11_RESOURCE_MISC_FLAG(0).0 as u32,
        };

        let mut staging: Option<ID3D11Texture2D> = None;
        device
            .CreateTexture2D(&staging_desc, None, Some(&mut staging))
            .map_err(|e| format!("Create staging texture failed: {}", e))?;

        let staging = staging.ok_or_else(|| "Staging texture is None".to_string())?;

        // Step 8: Copy the captured texture to staging
        context.CopyResource(&staging, &captured_texture);

        // Step 9: Map the staging texture to read pixels
        let mut mapped = windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
        context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .map_err(|e| format!("Map failed: {}", e))?;

        let src_slice: &[u8] = std::slice::from_raw_parts(
            mapped.pData as *const u8,
            mapped.DepthPitch as usize,
        );

        // Step 10: Handle RowPitch (DXGI may have padding)
        let width_usize = width as usize;
        let height_usize = height as usize;
        let row_size = width_usize * 4;
        let row_pitch = mapped.RowPitch as usize;

        let mut pixels = vec![0u8; row_size * height_usize];

        for y in 0..height_usize {
            let src_offset = y * row_pitch;
            let dst_offset = y * row_size;
            pixels[dst_offset..dst_offset + row_size]
                .copy_from_slice(&src_slice[src_offset..src_offset + row_size]);
        }

        // Step 11: Unmap and release frame
        context.Unmap(&staging, 0);
        let _ = duplication.ReleaseFrame();

        // Step 12: Convert BGRA → RGBA (swap bytes 0 and 2)
        let pixel_count = width_usize * height_usize;
        for i in 0..pixel_count {
            let offset = i * 4;
            pixels.swap(offset, offset + 2); // swap B and R
            pixels[offset + 3] = 255; // ensure alpha is opaque
        }

        // Cleanup is automatic via Drop
        Ok((pixels, width as u32, height as u32))
    }
}

/// GDI fallback capture (slow, ~6s)
fn capture_fullscreen_raw_gdi() -> Result<(Vec<u8>, u32, u32), String> {
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

        Ok((pixels, width as u32, height as u32))
    }
}

/// Get the DPI scale factor for the primary monitor.
/// Returns the scale factor (e.g., 1.0 for 100%/96 DPI, 1.5 for 150%/144 DPI, 2.0 for 200%/192 DPI).
pub fn get_dpi_scale() -> Result<f64, String> {
    unsafe {
        let screen_dc = GetDC(HWND::default());
        if screen_dc.is_invalid() {
            return Err("Failed to get screen DC".to_string());
        }

        let log_pixels_x = GetDeviceCaps(screen_dc, LOGPIXELSX);
        let _ = ReleaseDC(HWND::default(), screen_dc);

        if log_pixels_x <= 0 {
            return Err("Failed to get LOGPIXELSX".to_string());
        }

        Ok(log_pixels_x as f64 / 96.0)
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
        .map_err(|e| format!("Failed to decode screenshot PNG: {}", e))?;

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
    let encoder = PngEncoder::new_with_quality(Cursor::new(&mut png_bytes), CompressionType::Fast, FilterType::NoFilter);
    let rgba = cropped.to_rgba8();
    encoder
        .write_image(rgba.as_raw(), width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("PNG encoding of cropped region failed: {}", e))?;

    Ok(png_bytes)
}