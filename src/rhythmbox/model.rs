//! Internal Rhythmbox data structures for XML parsing

use std::path::PathBuf;

/// Rhythmbox track entry (as stored in rhythmdb.xml)
#[derive(Debug, Clone, Default)]
pub struct RhythmboxEntry {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub track_number: Option<u32>,
    pub duration: Option<u32>, // seconds
    pub location: Option<String>, // file:// URI
    pub bpm: Option<f32>,
    pub year: Option<u32>,
    pub comment: Option<String>,
}

impl RhythmboxEntry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert file:// URI to PathBuf
    pub fn get_file_path(&self) -> Option<PathBuf> {
        self.location.as_ref().and_then(|loc| {
            // Remove file:// prefix and decode URL encoding
            if let Some(path) = loc.strip_prefix("file://") {
                // Basic URL decoding (handle %20 spaces, etc.)
                let decoded = urlencoding::decode(path).ok()?;
                Some(PathBuf::from(decoded.into_owned()))
            } else {
                None
            }
        })
    }
}
