/// DNS TXT record response chunker
///
/// Handles chunking of text responses into DNS TXT record format.
/// DNS TXT records have a limit of 255 bytes per string, and DNS UDP packets
/// are limited to 4096 bytes total.
/// Response chunker that splits text into DNS TXT record compatible chunks
#[derive(Debug, Clone)]
pub struct Chunker {
    /// Maximum size per chunk in bytes (default: 250 to leave safety margin)
    max_chunk_size: usize,
    /// Maximum total response size in bytes (default: 4096 for DNS UDP)
    max_total_size: usize,
}

impl Default for Chunker {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker {
    /// Create a new chunker with default settings
    ///
    /// Defaults:
    /// - max_chunk_size: 250 bytes (DNS TXT limit is 255, we use 250 for safety)
    /// - max_total_size: 4096 bytes (DNS UDP packet limit)
    pub fn new() -> Self {
        Self {
            max_chunk_size: 250,
            max_total_size: 4096,
        }
    }

    /// Create a new chunker with custom settings
    pub fn with_sizes(max_chunk_size: usize, max_total_size: usize) -> Self {
        Self {
            max_chunk_size,
            max_total_size,
        }
    }

    /// Chunk text into DNS TXT record compatible strings
    ///
    /// # Arguments
    /// * `text` - The text to chunk
    ///
    /// # Returns
    /// A vector of strings, each no longer than max_chunk_size
    ///
    /// # Behavior
    /// - Empty input returns empty vector
    /// - Text <= max_chunk_size returns single-element vector
    /// - Text > max_chunk_size is split into multiple chunks
    /// - Text > max_total_size is truncated to max_total_size
    /// - UTF-8 character boundaries are respected (no mid-character splits)
    pub fn chunk_text(&self, text: &str) -> Vec<String> {
        // Handle empty string
        if text.is_empty() {
            return Vec::new();
        }

        // If text fits in total size limit, proceed with chunking
        let text_to_chunk = if text.len() > self.max_total_size {
            // Truncate to max_total_size while respecting UTF-8 boundaries
            Self::truncate_to_char_boundary(text, self.max_total_size)
        } else {
            text
        };

        // If text fits in single chunk, return it
        if text_to_chunk.len() <= self.max_chunk_size {
            return vec![text_to_chunk.to_string()];
        }

        // Split into multiple chunks
        let mut chunks = Vec::new();
        let mut remaining = text_to_chunk;

        while !remaining.is_empty() {
            let chunk_size = std::cmp::min(self.max_chunk_size, remaining.len());

            // Find the safe split point that doesn't break UTF-8 characters
            let split_point = Self::find_char_boundary(remaining, chunk_size);

            let (chunk, rest) = remaining.split_at(split_point);
            chunks.push(chunk.to_string());
            remaining = rest;
        }

        chunks
    }

    /// Find a valid UTF-8 character boundary at or before the given byte position
    ///
    /// This ensures we don't split multi-byte UTF-8 characters.
    fn find_char_boundary(text: &str, max_bytes: usize) -> usize {
        let bytes = text.as_bytes();

        // Start from max_bytes and work backwards to find a valid UTF-8 boundary
        for i in (0..=max_bytes.min(bytes.len())).rev() {
            if text.is_char_boundary(i) {
                return i;
            }
        }

        0 // Fallback (shouldn't reach here with valid UTF-8)
    }

    /// Truncate text to a maximum byte size while respecting UTF-8 boundaries
    fn truncate_to_char_boundary(text: &str, max_bytes: usize) -> &str {
        if text.len() <= max_bytes {
            return text;
        }

        // Find the character boundary at or before max_bytes
        let truncate_point = Self::find_char_boundary(text, max_bytes);
        &text[..truncate_point]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_short_text() {
        let chunker = Chunker::new();
        let text = "Hello, world!";
        let chunks = chunker.chunk_text(text);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Hello, world!");
    }

    #[test]
    fn test_chunk_exact_250_bytes() {
        let chunker = Chunker::new();
        // Create text exactly 250 bytes (ASCII characters are 1 byte each)
        let text = "a".repeat(250);
        let chunks = chunker.chunk_text(&text);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 250);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_chunk_long_text() {
        let chunker = Chunker::new();
        // Create text longer than 250 bytes
        let text = "a".repeat(500);
        let chunks = chunker.chunk_text(&text);

        assert!(chunks.len() > 1);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 250);
        assert_eq!(chunks[1].len(), 250);

        // Verify chunks concatenate back to original
        let reassembled = chunks.join("");
        assert_eq!(reassembled, text);
    }

    #[test]
    fn test_chunk_very_long_text() {
        let chunker = Chunker::new();
        // Create text longer than 4096 bytes (max_total_size)
        let text = "a".repeat(5000);
        let chunks = chunker.chunk_text(&text);

        // Verify total length doesn't exceed max_total_size
        let reassembled = chunks.join("");
        assert!(reassembled.len() <= 4096);
        assert_eq!(reassembled.len(), 4096); // Should be exactly truncated to max
    }

    #[test]
    fn test_chunk_unicode_boundaries() {
        let chunker = Chunker::with_sizes(10, 4096);

        // "こんにちは" (5 Japanese characters, each 3 bytes in UTF-8 = 15 bytes total)
        // With chunk size 10, we should get one "こ" (3 bytes) + one "ん" (3 bytes) +
        // and continue respecting boundaries
        let text = "こんにちは";
        let chunks = chunker.chunk_text(text);

        // Verify we have multiple chunks
        assert!(!chunks.is_empty());

        // Verify chunks are valid UTF-8
        for chunk in &chunks {
            // This will panic if not valid UTF-8, but shouldn't happen
            let _ = chunk.chars().count();
        }

        // Verify reassembled text matches (minus potential truncation)
        let reassembled = chunks.join("");
        assert_eq!(reassembled, text);
    }

    #[test]
    fn test_chunk_unicode_with_truncation() {
        let chunker = Chunker::with_sizes(250, 10);

        // Create text with multi-byte characters
        let text = "Hello こんにちは World أهلا وسهلا";
        let chunks = chunker.chunk_text(text);

        // Should have chunks
        assert!(!chunks.is_empty());

        // Verify all chunks are valid UTF-8
        for chunk in &chunks {
            let _ = chunk.chars().count();
        }

        // Verify reassembled text doesn't exceed max_total_size
        let reassembled = chunks.join("");
        assert!(reassembled.len() <= 10);
    }

    #[test]
    fn test_chunk_empty_string() {
        let chunker = Chunker::new();
        let chunks = chunker.chunk_text("");

        assert_eq!(chunks.len(), 0);
        assert_eq!(chunks, Vec::<String>::new());
    }

    #[test]
    fn test_chunk_newlines() {
        let chunker = Chunker::new();
        let text = "Line 1\nLine 2\nLine 3\nLine 4";
        let chunks = chunker.chunk_text(text);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
        assert!(chunks[0].contains('\n'));
    }

    #[test]
    fn test_chunk_multiple_chunks_with_newlines() {
        let chunker = Chunker::with_sizes(30, 4096);
        let text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6";
        let chunks = chunker.chunk_text(text);

        assert!(chunks.len() > 1);

        // Verify chunks are valid
        for chunk in &chunks {
            assert!(!chunk.is_empty());
        }

        // Verify reassembled matches original
        let reassembled = chunks.join("");
        assert_eq!(reassembled, text);
    }

    #[test]
    fn test_chunker_with_custom_sizes() {
        let chunker = Chunker::with_sizes(100, 300);
        let text = "a".repeat(250);
        let chunks = chunker.chunk_text(&text);

        // Should be chunked into 100-byte chunks (no truncation since 250 < 300)
        // 250 bytes / 100 bytes per chunk = 2 full chunks + 1 chunk of 50 bytes
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 100);
        assert_eq!(chunks[1].len(), 100);
        assert_eq!(chunks[2].len(), 50);
    }

    #[test]
    fn test_chunk_single_byte_over_limit() {
        let chunker = Chunker::with_sizes(5, 4096);
        let text = "Hello World"; // 11 bytes
        let chunks = chunker.chunk_text(text);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "Hello");
        assert_eq!(chunks[1], " Worl");
        assert_eq!(chunks[2], "d");
    }

    #[test]
    fn test_chunker_respects_utf8_in_truncation() {
        let chunker = Chunker::with_sizes(250, 5);
        // Each emoji is 4 bytes: "🎉" repeated
        let text = "🎉".repeat(10); // 40 bytes total
        let chunks = chunker.chunk_text(&text);

        // Verify the truncated text is valid UTF-8
        for chunk in &chunks {
            let _ = chunk.chars().count(); // Will panic if invalid UTF-8
        }

        let reassembled = chunks.join("");
        // With max_total_size of 5 bytes, we should get 1 emoji (4 bytes)
        assert_eq!(reassembled.len(), 4);
        assert_eq!(reassembled, "🎉");
    }

    #[test]
    fn test_chunker_default_trait() {
        let chunker1 = Chunker::new();
        let chunker2 = Chunker::default();

        let text = "test";
        assert_eq!(chunker1.chunk_text(text), chunker2.chunk_text(text));
    }

    #[test]
    fn test_truncate_short_text() {
        // Test line 112: when text.len() <= max_bytes in truncate_to_char_boundary
        let chunker = Chunker::with_sizes(250, 100);
        let text = "Short"; // 5 bytes, well under 100
        let chunks = chunker.chunk_text(text);

        // Should return single chunk without truncation
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_find_char_boundary_with_single_multibyte() {
        // Test edge case for find_char_boundary
        let chunker = Chunker::with_sizes(2, 4096);
        // "é" is 2 bytes in UTF-8 (0xC3 0xA9)
        let text = "é";
        let chunks = chunker.chunk_text(text);

        // With chunk size 2, the character fits exactly
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "é");
    }

    #[test]
    fn test_chunk_at_exact_boundaries() {
        // Test to ensure proper character boundary detection
        let chunker = Chunker::with_sizes(6, 4096);
        // "🎉" is 4 bytes, "a" is 1 byte
        // "🎉a" = 5 bytes total
        // With chunk size 6, should fit in one chunk
        let text = "🎉a";
        let chunks = chunker.chunk_text(text);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_chunk_split_between_multibyte() {
        // Test splitting exactly between multi-byte characters
        let chunker = Chunker::with_sizes(4, 4096);
        // Each emoji is 4 bytes
        let text = "🎉🌟"; // 8 bytes total
        let chunks = chunker.chunk_text(text);

        // Should split into 2 chunks of 4 bytes each
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "🎉");
        assert_eq!(chunks[1], "🌟");
    }
}
