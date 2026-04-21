// OCR module - Windows OCR API integration
use windows::{
    Foundation::{Collections::IVectorView, IAsyncOperation},
    Globalization::Language,
    Graphics::Imaging::{
        BitmapDecoder, BitmapPixelFormat, SoftwareBitmap,
    },
    Media::Ocr::{OcrEngine, OcrLine},
    Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
};

/// OCR result structure
#[derive(Debug, Clone)]
pub struct OcrResult {
    pub text: String,
    pub confidence: f32,
}

/// Heuristic: decides whether consecutive OCR lines belong to the same
/// paragraph (join with space) or are separate blocks (join with \n).
///
/// Rules:
/// - A line that looks like a bullet/list item  (starts with -, •, *, #, or a digit+dot)
///   is ALWAYS its own line.
/// - A line whose previous line ended with sentence-terminal punctuation
///   (. ! ? : — or similar) starts a new block.
/// - Otherwise: join with a space (it's a wrapped paragraph).
fn smart_join_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    // Characters that signal the END of a logical block
    fn is_block_end(s: &str) -> bool {
        let s = s.trim_end();
        if s.is_empty() { return true; }
        matches!(s.chars().last().unwrap(),
            '.' | '!' | '?' | ':' | ';' | '…' | '—' | '-')
    }

    // A line that is clearly a list item or heading — always its own line
    fn is_structural(s: &str) -> bool {
        let t = s.trim_start();
        if t.starts_with('-')
            || t.starts_with('•')
            || t.starts_with('*')
            || t.starts_with('#')
        {
            return true;
        }
        // "1. foo" / "2) foo"
        let mut chars = t.chars();
        if let Some(c) = chars.next() {
            if c.is_ascii_digit() {
                if let Some(next) = chars.next() {
                    if next == '.' || next == ')' {
                        return true;
                    }
                }
            }
        }
        false
    }

    let mut result = String::new();
    let mut iter = lines.iter().peekable();

    while let Some(current) = iter.next() {
        result.push_str(current);

        match iter.peek() {
            None => {} // last line — nothing to append
            Some(next) => {
                // Decide separator: \n or space
                let needs_newline = is_structural(current)
                    || is_structural(next)
                    || is_block_end(current);

                if needs_newline {
                    result.push('\n');
                } else {
                    result.push(' ');
                }
            }
        }
    }

    result
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

    // Extract text from lines - use Lines() then smart-join into paragraphs
    let lines: IVectorView<OcrLine> = ocr_result.Lines().map_err(|e| format!("Failed to get lines: {}", e))?;
    let count = lines.Size().map_err(|e| format!("Failed to get line count: {}", e))?;
    let mut raw_lines: Vec<String> = Vec::new();
    for i in 0..count {
        let line = lines.GetAt(i).map_err(|e| format!("Failed to get line {}: {}", i, e))?;
        let line_text = line.Text().map_err(|e| format!("Failed to get line text: {}", e))?.to_string();
        let trimmed = line_text.trim().to_string();
        if !trimmed.is_empty() {
            raw_lines.push(trimmed);
        }
    }
    let text = smart_join_lines(&raw_lines);

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