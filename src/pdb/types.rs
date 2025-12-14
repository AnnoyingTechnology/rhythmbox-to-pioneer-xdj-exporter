//! PDB type definitions and constants

/// Table type identifiers
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableType {
    Tracks = 0x00,
    Genres = 0x01,
    Artists = 0x02,
    Albums = 0x03,
    Labels = 0x04,
    Keys = 0x05,
    Colors = 0x06,
    PlaylistTree = 0x07,
    PlaylistEntries = 0x08,
    Artwork = 0x0d,
    Columns = 0x10,
    HistoryPlaylists = 0x11,
    HistoryEntries = 0x12,
    History = 0x13,
}

/// File type encodings
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum FileType {
    Unknown = 0x00,
    Mp3 = 0x01,
    M4a = 0x04,
    Flac = 0x05,
    Wav = 0x0b,
    Aiff = 0x0c,
}

impl FileType {
    /// Detect file type from extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "mp3" => FileType::Mp3,
            "m4a" | "mp4" | "aac" => FileType::M4a,
            "flac" => FileType::Flac,
            "wav" => FileType::Wav,
            "aiff" | "aif" => FileType::Aiff,
            _ => FileType::Unknown,
        }
    }
}

/// DeviceSQL string encoding flags
pub mod string_flags {
    /// Short ASCII string (length in lower 7 bits, bit 0 set)
    pub const SHORT_ASCII: u8 = 0x01;

    /// Long ASCII string (bit 6 set)
    pub const LONG_ASCII: u8 = 0x40;

    /// Long UTF-16 LE string (bits 4,7 set)
    pub const LONG_UTF16LE: u8 = 0x90;
}
