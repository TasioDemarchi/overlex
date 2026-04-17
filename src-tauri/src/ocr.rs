// OCR module - Windows OCR API integration
// TODO: Implement Windows.Media.Ocr via windows crate

/// OCR result structure
#[derive(Debug, Clone)]
pub struct OcrResult {
    pub text: String,
    pub confidence: f32,
}

/// Perform OCR on image region - stub
pub async fn ocr_region(_image_data: &[u8]) -> Result<OcrResult, String> {
    // TODO: Use Windows.Media.Ocr via windows crate
    // Convert image to Windows.Graphics.Bitmap, create OcrEngine, recognize
    Err("Not implemented".to_string())
}

/// Check if OCR language is available - stub
pub fn is_language_available(_lang: &str) -> bool {
    // TODO: Check Windows language packs
    false
}