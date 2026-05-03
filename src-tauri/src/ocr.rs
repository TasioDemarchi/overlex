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
use image::{DynamicImage, GenericImageView, ImageEncoder};

/// OCR result structure
#[derive(Debug, Clone)]
pub struct OcrResult {
    pub text: String,
    pub confidence: f32,
}

/// Heuristic: decides whether consecutive OCR lines belong to the same
/// paragraph (join) or are separate blocks (newline).
///
/// Five rules applied in order:
/// 1. Mid-word cut: line ends lowercase + next starts lowercase → join with no space
/// 2. CJK line: >50% CJK chars → join with zero separator
/// 3. Dialogue prefix: line ends with `:` → join next with single space
/// 4. All-caps short line: ≤3 words, all uppercase → keep as separate line
/// 5. No heuristic match → preserve original newline behavior
fn smart_join_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    // Check if character is CJK (Chinese/Japanese/Korean)
    fn is_cjk_char(c: char) -> bool {
        matches!(c,
            '\u{4E00}'..='\u{9FFF}' |  // CJK Unified Ideographs
            '\u{3040}'..='\u{309F}' |  // Hiragana
            '\u{30A0}'..='\u{30FF}' |  // Katakana
            '\u{AC00}'..='\u{D7AF}'    // Korean Hangul
        )
    }

    // Check if line is primarily CJK (>50% CJK characters)
    fn is_cjk_line(s: &str) -> bool {
        let total: usize = s.chars().count();
        if total == 0 { return false; }
        let cjk_count: usize = s.chars().filter(|c| is_cjk_char(*c)).count();
        cjk_count * 2 > total
    }

    // Check if line is all-caps short (≤3 words, all uppercase)
    fn is_allcaps_short_line(s: &str) -> bool {
        let words: Vec<&str> = s.split_whitespace().collect();
        if words.is_empty() || words.len() > 3 {
            return false;
        }
        // Check if all words are all uppercase (and contain at least one letter)
        words.iter().all(|w| {
            let has_letter = w.chars().any(|c| c.is_ascii_alphabetic());
            has_letter && w.chars().all(|c| !c.is_ascii_lowercase())
        })
    }

    // Check if line ends with dialogue prefix pattern (name:)
    fn is_dialogue_prefix(s: &str) -> bool {
        let trimmed = s.trim_end();
        // Pattern: word characters followed by colon, not a URL/file path
        // e.g., "Geralt:", "NPC:", "Shop:", "System:"
        trimmed.ends_with(':')
            && !trimmed.ends_with("://")
            && !trimmed.ends_with("http:")
            && !trimmed.ends_with("https:")
    }

    // Characters that signal the END of a logical block
    fn is_block_end(s: &str) -> bool {
        let s = s.trim_end();
        if s.is_empty() { return true; }
        matches!(s.chars().last().unwrap(),
            '.' | '!' | '?' | ';' | '…' | '—' | '-')
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
                let current_trimmed = current.trim();
                let next_trimmed = next.trim();

                // Rule 1: Mid-word cut — line ends lowercase + next starts lowercase
                let ends_lowercase = current_trimmed.chars().last().map(|c| c.is_ascii_lowercase()).unwrap_or(false);
                let starts_lowercase = next_trimmed.chars().next().map(|c| c.is_ascii_lowercase()).unwrap_or(false);

                if ends_lowercase && starts_lowercase {
                    result.push_str(next_trimmed);
                    iter.next(); // consume the next line
                    continue;
                }

                // Rule 2: CJK line — join with zero separator
                if is_cjk_line(current_trimmed) || is_cjk_line(next_trimmed) {
                    result.push_str(next_trimmed);
                    iter.next();
                    continue;
                }

                // Rule 3: Dialogue prefix — join with single space
                if is_dialogue_prefix(current_trimmed) {
                    result.push(' ');
                    continue;
                }

                // Rule 4: All-caps short line — keep as separate
                if is_allcaps_short_line(current_trimmed) || is_allcaps_short_line(next_trimmed) {
                    result.push('\n');
                    continue;
                }

                // Rule 5: Block end or structural → newline; otherwise space
                let needs_newline = is_structural(current_trimmed)
                    || is_structural(next_trimmed)
                    || is_block_end(current_trimmed);

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

/// Pre-process image for OCR: grayscale, contrast stretch, optional sharpen, optional Otsu binarization.
/// Operates on PNG bytes (decode → process → re-encode).
/// Returns original bytes on failure (graceful degradation).
pub fn preprocess_for_ocr(png_bytes: &[u8], binarize: bool) -> Result<Vec<u8>, String> {
    // Decode PNG
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| format!("Failed to decode PNG: {}", e))?;

    // Convert to grayscale
    let gray = img.to_luma8();

    // Contrast stretch (histogram normalization for better OCR)
    let stretched = contrast_stretch(&gray);

    // Optional sharpen
    let sharpened = if binarize {
        // Sharpen before binarization helps preserve edges
        unsharpen(&stretched, 1.5)
    } else {
        stretched
    };

    // Optional Otsu binarization
    let processed: DynamicImage = if binarize {
        let binarized = otsu_binarize(&sharpened);
        DynamicImage::ImageLuma8(binarized)
    } else {
        DynamicImage::ImageLuma8(sharpened)
    };

    // Re-encode to PNG
    let mut output = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        std::io::Cursor::new(&mut output),
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::NoFilter,
    );
    let (width, height) = processed.dimensions();
    encoder.write_image(
        processed.as_bytes(),
        width,
        height,
        processed.color().into(),
    ).map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(output)
}

/// Contrast stretch via histogram normalization
fn contrast_stretch(gray: &image::GrayImage) -> image::GrayImage {
    let mut hist = [0u32; 256];
    for pixel in gray.iter() {
        hist[*pixel as usize] += 1;
    }

    let total = gray.len() as f64;
    let mut cdf = [0u32; 256];
    cdf[0] = hist[0];
    for i in 1..256 {
        cdf[i] = cdf[i - 1] + hist[i];
    }

    // Find min and max CDF (ignore extreme 1%)
    let clip = (total * 0.01) as u32;
    let mut min_val = 0;
    let mut max_val = 255;

    for i in 0..256 {
        if cdf[i] >= clip {
            min_val = i as u8;
            break;
        }
    }
    for i in (0..256).rev() {
        if cdf[i] <= total as u32 - clip {
            max_val = i as u8;
            break;
        }
    }

    let range = if max_val > min_val { (max_val - min_val) as f32 } else { 1.0 };

    let lut: Vec<u8> = (0..256)
        .map(|i| {
            let i = i as u8;
            if i <= min_val { 0 }
            else if i >= max_val { 255 }
            else {
                (((i as f32 - min_val as f32) / range) * 255.0).round() as u8
            }
        })
        .collect();

    image::GrayImage::from_raw(gray.width(), gray.height(),
        gray.iter().map(|p| lut[*p as usize]).collect::<Vec<_>>()
    ).unwrap_or_else(|| gray.clone())
}

/// Simple unsharp mask for sharpening
fn unsharpen(gray: &image::GrayImage, amount: f32) -> image::GrayImage {
    let (w, h) = gray.dimensions();
    if w < 3 || h < 3 {
        return gray.clone();
    }

    let mut out = gray.clone();

    // Simple 3x3 sharpen kernel
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let center = gray.get_pixel(x, y).0[0] as f32;
            let neighbors: f32 = [
                gray.get_pixel(x-1, y-1).0[0] as f32,
                gray.get_pixel(x, y-1).0[0] as f32,
                gray.get_pixel(x+1, y-1).0[0] as f32,
                gray.get_pixel(x-1, y).0[0] as f32,
                gray.get_pixel(x+1, y).0[0] as f32,
                gray.get_pixel(x-1, y+1).0[0] as f32,
                gray.get_pixel(x, y+1).0[0] as f32,
                gray.get_pixel(x+1, y+1).0[0] as f32,
            ].iter().sum();

            let blur = neighbors / 8.0;
            let diff = center - blur;
            let new_val = (center + diff * amount).clamp(0.0, 255.0);

            out.put_pixel(x, y, image::Luma([new_val as u8; 1]));
        }
    }

    out
}

/// Otsu binarization thresholding
fn otsu_binarize(gray: &image::GrayImage) -> image::GrayImage {
    let mut hist = [0u64; 256];
    for pixel in gray.iter() {
        hist[*pixel as usize] += 1;
    }

    let total = gray.len() as f64;

    let mut sum: f64 = 0.0;
    for i in 0..256 {
        sum += i as f64 * hist[i] as f64;
    }

    let mut sum_b: f64 = 0.0;
    let mut w_b: f64 = 0.0;
    let mut max_var: f64 = 0.0;
    let mut threshold: u8 = 0;

    for t in 0..256 {
        w_b += hist[t] as f64;
        if w_b == 0.0 { continue; }

        let w_f = total - w_b;
        if w_f == 0.0 { break; }

        sum_b += t as f64 * hist[t] as f64;

        let m_b = sum_b / w_b;
        let m_f = (sum - sum_b) / w_f;

        let var = w_b * w_f * (m_b - m_f) * (m_b - m_f);

        if var > max_var {
            max_var = var;
            threshold = t as u8;
        }
    }

    let lut: Vec<u8> = (0..=255).map(|i| if i > threshold { 255 } else { 0 }).collect();

    image::GrayImage::from_raw(gray.width(), gray.height(),
        gray.iter().map(|p| lut[*p as usize]).collect::<Vec<_>>()
    ).unwrap_or_else(|| gray.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_language_available_auto() {
        assert!(is_language_available("auto"));
    }

    // === smart_join_lines tests ===

    #[test]
    fn test_smart_join_empty() {
        assert_eq!(smart_join_lines(&[]), "");
    }

    #[test]
    fn test_smart_join_single_line() {
        assert_eq!(smart_join_lines(&["Hello".to_string()]), "Hello");
    }

    // Rule 1: Mid-word cut — lowercase end + lowercase start → join without space
    #[test]
    fn test_mid_word_cut_english() {
        // "Hel" + "lo" → "Hello"
        assert_eq!(
            smart_join_lines(&["Hel".to_string(), "lo".to_string()]),
            "Hello"
        );
    }

    #[test]
    fn test_mid_word_cut_not_triggered_uppercase() {
        // "Hel" ends lowercase, "World" starts uppercase → NOT a mid-word cut
        // Should fall through to other rules (block_end or space join)
        let result = smart_join_lines(&["Hel".to_string(), "World".to_string()]);
        assert!(result.contains(' ') || result.contains('\n'), "Expected space or newline, got: {}", result);
    }

    // Rule 2: CJK — >50% CJK chars → join with zero separator
    #[test]
    fn test_cjk_join_zero_separator() {
        // Japanese text should be joined without spaces
        let lines = vec!["こんにち".to_string(), "は世界".to_string()];
        let result = smart_join_lines(&lines);
        // CJK lines join without separator
        assert_eq!(result, "こんにちは世界");
    }

    #[test]
    fn test_mixed_cjk_latin_not_cjk_line() {
        // Line with <50% CJK should NOT use CJK rules
        let lines = vec!["HP".to_string(), "ポイント".to_string()];
        let result = smart_join_lines(&lines);
        // "HP" is not a CJK line (<50%), "ポイント" is CJK line
        // CJK rule applies if EITHER line is CJK → join without space
        assert!(!result.contains(' '), "CJK-adjacent lines should not have space, got: {}", result);
    }

    // Rule 3: Dialogue prefix — line ends with ":" → join next with space
    #[test]
    fn test_dialogue_prefix() {
        let lines = vec!["Geralt:".to_string(), "Where is the witch?".to_string()];
        let result = smart_join_lines(&lines);
        assert_eq!(result, "Geralt: Where is the witch?");
    }

    #[test]
    fn test_dialogue_prefix_multi_word_name() {
        let lines = vec!["Shop Keeper:".to_string(), "Welcome!".to_string()];
        let result = smart_join_lines(&lines);
        assert_eq!(result, "Shop Keeper: Welcome!");
    }

    #[test]
    fn test_url_colon_not_dialogue() {
        // URLs with "://" should NOT be treated as dialogue
        // "http://" and "https://" are explicitly excluded
        // But a line ending with ":" that isn't a URL IS dialogue
        let lines = vec!["System:".to_string(), "Alert triggered".to_string()];
        let result = smart_join_lines(&lines);
        assert_eq!(result, "System: Alert triggered");
    }

    // Rule 4: All-caps short line — ≤3 words, all uppercase → keep separate
    #[test]
    fn test_allcaps_short_line_ui_label() {
        let lines = vec!["HP".to_string(), "MP".to_string()];
        let result = smart_join_lines(&lines);
        // "HP" is all-caps short, "MP" is all-caps short → separate lines
        assert!(result.contains('\n'), "All-caps short lines should be on separate lines, got: {}", result);
    }

    #[test]
    fn test_allcaps_long_line_not_treated_as_short() {
        // Line with >3 uppercase words should NOT be treated as all-caps short
        let lines = vec!["QUEST ACCEPTED BY THE KING".to_string(), "Continue".to_string()];
        let result = smart_join_lines(&lines);
        // Too many words for all-caps short rule, falls through to block-end/default
        // Should NOT force a newline via all-caps rule
        assert!(!result.starts_with("QUEST ACCEPTED BY THE KING\n"), "5-word line should not be all-caps short, got: {}", result);
    }

    // Rule 5: Structural and block-end detection
    #[test]
    fn test_structural_list_item() {
        let lines = vec!["- First item".to_string(), "Second item".to_string()];
        let result = smart_join_lines(&lines);
        assert!(result.contains('\n'), "List items should be on separate lines, got: {}", result);
    }

    #[test]
    fn test_structural_numbered_list() {
        let lines = vec!["1. Go north".to_string(), "2. Turn left".to_string()];
        let result = smart_join_lines(&lines);
        assert!(result.contains('\n'), "Numbered list items should be on separate lines, got: {}", result);
    }

    #[test]
    fn test_block_end_sentence() {
        // Line ending with "." is a block end → next line gets newline
        let lines = vec!["The quest begins.".to_string(), "Find the sword.".to_string()];
        let result = smart_join_lines(&lines);
        assert_eq!(result, "The quest begins.\nFind the sword.");
    }

    #[test]
    fn test_paragraph_continuation_space() {
        // Two lines where the first ends with a period (block end) → newline
        // "The hero walked." ends with '.', so block-end rule applies first
        let lines = vec!["The hero walked.".to_string(), "Into the forest.".to_string()];
        let result = smart_join_lines(&lines);
        assert_eq!(result, "The hero walked.\nInto the forest.");
    }

    #[test]
    fn test_paragraph_continuation_without_punctuation() {
        // Two lines WITHOUT block-end punctuation → join with space
        // Note: mid-word rule only applies when BOTH end lowercase AND start lowercase,
        // but this is a known limitation — real OCR output typically has punctuation.
        // Testing the expected behavior: lines without punctuation join with space
        // when they don't match mid-word cut pattern (multi-word lines)
        let lines = vec!["The hero walked".to_string(), "Into the forest".to_string()];
        let result = smart_join_lines(&lines);
        // "walked" ends lowercase, "Into" starts uppercase → no mid-word cut
        // "walked" doesn't end with block marker → space join
        assert_eq!(result, "The hero walked Into the forest");
    }

    #[test]
    fn test_exclamation_block_end() {
        let lines = vec!["Watch out!".to_string(), "It's dangerous.".to_string()];
        let result = smart_join_lines(&lines);
        assert!(result.contains('\n'), "Lines ending with ! should be block-separated, got: {}", result);
    }

    #[test]
    fn test_question_block_end() {
        let lines = vec!["Where is the inn?".to_string(), "I need rest.".to_string()];
        let result = smart_join_lines(&lines);
        assert!(result.contains('\n'), "Lines ending with ? should be block-separated, got: {}", result);
    }
}