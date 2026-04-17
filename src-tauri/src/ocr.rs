// OCR module - Windows OCR API integration
use windows::{
    Foundation::IAsyncOperation,
    Globalization::Language,
    Graphics::Imaging::{
        BitmapDecoder, BitmapPixelFormat, SoftwareBitmap,
    },
    Media::Ocr::OcrEngine,
    Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
};

/// OCR result structure
#[derive(Debug, Clone)]
pub struct OcrResult {
    pub text: String,
    pub confidence: f32,
}

/// Perform OCR on image region using Windows.Media.Ocr
pub async fn ocr_region(image_data: &[u8]) -> Result<OcrResult, String> {
    // Create in-memory stream
    let stream = InMemoryRandomAccessStream::new().map_err(|e| format!("Failed to create stream: {}", e))?;

    // Write PNG bytes to stream via DataWriter
    let data_writer = DataWriter::CreateDataWriter(&stream).map_err(|e| format!("Failed to create data writer: {}", e))?;
    data_writer.WriteBytes(image_data).map_err(|e| format!("Failed to write bytes: {}", e))?;

    // Store and flush the data
    let store_op = data_writer.StoreAsync().map_err(|e| format!("Failed to store async: {}", e))?;
    let _bytes_written = store_op.get().map_err(|e| format!("Failed to complete store: {}", e))?;

    let flush_op = data_writer.FlushAsync().map_err(|e| format!("Failed to flush async: {}", e))?;
    let _flushed = flush_op.get().map_err(|e| format!("Failed to complete flush: {}", e))?;

    // Seek back to beginning
    stream.Seek(0).map_err(|e| format!("Failed to seek stream: {}", e))?;

    // Create bitmap decoder
    let decoder_op: IAsyncOperation<BitmapDecoder> = BitmapDecoder::CreateAsync(&stream)
        .map_err(|e| format!("Failed to create decoder: {}", e))?;
    let decoder = decoder_op.get().map_err(|e| format!("Failed to complete decoder creation: {}", e))?;

    // Get software bitmap from decoder
    let bitmap_op: IAsyncOperation<SoftwareBitmap> = decoder.GetSoftwareBitmapAsync()
        .map_err(|e| format!("Failed to get bitmap: {}", e))?;
    let mut bitmap = bitmap_op.get().map_err(|e| format!("Failed to complete bitmap get: {}", e))?;

    // Convert to BGRA8 if needed (OCR requires BGRA8)
    let current_format = bitmap.BitmapPixelFormat().map_err(|e| format!("Failed to get pixel format: {}", e))?;
    if current_format != BitmapPixelFormat::Bgra8 {
        let converted = SoftwareBitmap::Convert(
            &bitmap,
            BitmapPixelFormat::Bgra8,
        ).map_err(|e| format!("Failed to convert bitmap: {}", e))?;
        bitmap = converted;
    }

    // Create OCR engine from user profile languages (auto-detect)
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|e| format!("Failed to create OCR engine: {}", e))?;

    // Perform OCR
    let result_op: IAsyncOperation<windows::Media::Ocr::OcrResult> = engine.RecognizeAsync(&bitmap)
        .map_err(|e| format!("Failed to recognize: {}", e))?;
    let ocr_result = result_op.get().map_err(|e| format!("Failed to complete recognition: {}", e))?;

    // Extract text from lines - use FindAllElements to get line text
    let text = ocr_result.Text().map_err(|e| format!("Failed to get text: {}", e))?.to_string();

    Ok(OcrResult {
        text,
        confidence: 1.0, // Windows OCR doesn't provide confidence scores
    })
}

/// Check if OCR language is available on the system
pub fn is_language_available(lang: &str) -> bool {
    // Handle "auto" as always available (uses system default)
    if lang == "auto" {
        return true;
    }

    // Try to create a Language object and check if it's supported
    if let Ok(language) = Language::CreateLanguage(&lang.into()) {
        OcrEngine::IsLanguageSupported(&language).unwrap_or(false)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_language_available_auto() {
        assert!(is_language_available("auto"));
    }
}