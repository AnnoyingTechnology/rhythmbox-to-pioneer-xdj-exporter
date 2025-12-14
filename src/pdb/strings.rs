//! DeviceSQL string encoding

use super::types::string_flags;

/// Encode a string in DeviceSQL format (short ASCII for Phase 1)
///
/// DeviceSQL string format (from rekordcrate source):
/// - Short ASCII: header = ((len + 1) << 1) | 1, then content bytes
/// - Long ASCII: flags (0x40), length u16 (content_len + 4), padding (0x00), then content bytes
///
/// Phase 1: Only ASCII strings
/// Phase 2: Add UTF-16 support if needed
pub fn encode_device_sql(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let len = bytes.len();

    if len <= 126 {
        // Short ASCII encoding
        // header = ((content.len() + 1) << 1) | 1
        let mut result = Vec::with_capacity(1 + len);
        let header = ((((len + 1) << 1) as u8) | string_flags::SHORT_ASCII);
        result.push(header);
        result.extend_from_slice(bytes);
        result
    } else {
        // Long ASCII encoding (for strings > 126 chars)
        // Format: flags (1 byte), length (2 bytes), padding (1 byte), content
        // length = content.len() + 4 (includes 4-byte header: flags + length + padding)
        let mut result = Vec::with_capacity(4 + len);
        result.push(string_flags::LONG_ASCII); // flags
        let total_length = (len + 4) as u16; // content + 4-byte header
        result.extend_from_slice(&total_length.to_le_bytes()); // length (little-endian)
        result.push(0u8); // padding
        result.extend_from_slice(bytes); // content
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_string() {
        let encoded = encode_device_sql("Hello");
        // header = ((5 + 1) << 1) | 1 = (6 << 1) | 1 = 12 | 1 = 13 (0x0D)
        assert_eq!(encoded[0], 0x0D);
        assert_eq!(&encoded[1..], b"Hello");
    }

    #[test]
    fn test_empty_string() {
        let encoded = encode_device_sql("");
        assert_eq!(encoded.len(), 1);
        // header = ((0 + 1) << 1) | 1 = (1 << 1) | 1 = 2 | 1 = 3 (0x03)
        assert_eq!(encoded[0], 0x03);
    }
}
