use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a single music track with all its metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// Unique identifier for this track
    pub id: String,

    /// Track title
    pub title: String,

    /// Artist name
    pub artist: String,

    /// Album name
    pub album: String,

    /// Genre (optional)
    pub genre: Option<String>,

    /// Track duration in milliseconds
    pub duration_ms: u32,

    /// BPM (beats per minute) - Phase 1: from Rhythmbox if available, Phase 2: detected
    pub bpm: Option<f32>,

    /// Musical key - Phase 1: None, Phase 2: detected
    pub key: Option<MusicalKey>,

    /// File path to the audio file
    pub file_path: PathBuf,

    /// File size in bytes (for copying validation)
    pub file_size: u64,

    /// Track number in album (optional)
    pub track_number: Option<u32>,

    /// Year/date (optional)
    pub year: Option<u32>,

    /// Comment/description (optional)
    pub comment: Option<String>,
}

/// Musical key representation (Camelot/Open Key notation)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MusicalKey {
    // Major keys
    CMajor,
    DbMajor,
    DMajor,
    EbMajor,
    EMajor,
    FMajor,
    GbMajor,
    GMajor,
    AbMajor,
    AMajor,
    BbMajor,
    BMajor,

    // Minor keys
    CMinor,
    CsMinor,
    DMinor,
    EbMinor,
    EMinor,
    FMinor,
    FsMinor,
    GMinor,
    AbMinor,
    AMinor,
    BbMinor,
    BMinor,
}

impl MusicalKey {
    /// Convert to Rekordbox key encoding
    /// Must match the Keys table in PDB (chromatic order starting from A)
    /// Minor keys: 1-12, Major keys: 13-24
    pub fn to_rekordbox_id(&self) -> u32 {
        match self {
            // Minor keys (chromatic from A) - IDs 1-12
            MusicalKey::AMinor => 1,
            MusicalKey::BbMinor => 2,
            MusicalKey::BMinor => 3,
            MusicalKey::CMinor => 4,
            MusicalKey::CsMinor => 5,
            MusicalKey::DMinor => 6,
            MusicalKey::EbMinor => 7,
            MusicalKey::EMinor => 8,
            MusicalKey::FMinor => 9,
            MusicalKey::FsMinor => 10,
            MusicalKey::GMinor => 11,
            MusicalKey::AbMinor => 12,

            // Major keys (chromatic from A) - IDs 13-24
            MusicalKey::AMajor => 13,
            MusicalKey::BbMajor => 14,
            MusicalKey::BMajor => 15,
            MusicalKey::CMajor => 16,
            MusicalKey::DbMajor => 17,
            MusicalKey::DMajor => 18,
            MusicalKey::EbMajor => 19,
            MusicalKey::EMajor => 20,
            MusicalKey::FMajor => 21,
            MusicalKey::GbMajor => 22,
            MusicalKey::GMajor => 23,
            MusicalKey::AbMajor => 24,
        }
    }

    /// Get human-readable key name
    pub fn name(&self) -> &'static str {
        match self {
            MusicalKey::CMajor => "C Major",
            MusicalKey::DbMajor => "Db Major",
            MusicalKey::DMajor => "D Major",
            MusicalKey::EbMajor => "Eb Major",
            MusicalKey::EMajor => "E Major",
            MusicalKey::FMajor => "F Major",
            MusicalKey::GbMajor => "Gb Major",
            MusicalKey::GMajor => "G Major",
            MusicalKey::AbMajor => "Ab Major",
            MusicalKey::AMajor => "A Major",
            MusicalKey::BbMajor => "Bb Major",
            MusicalKey::BMajor => "B Major",

            MusicalKey::CMinor => "C Minor",
            MusicalKey::CsMinor => "C# Minor",
            MusicalKey::DMinor => "D Minor",
            MusicalKey::EbMinor => "Eb Minor",
            MusicalKey::EMinor => "E Minor",
            MusicalKey::FMinor => "F Minor",
            MusicalKey::FsMinor => "F# Minor",
            MusicalKey::GMinor => "G Minor",
            MusicalKey::AbMinor => "Ab Minor",
            MusicalKey::AMinor => "A Minor",
            MusicalKey::BbMinor => "Bb Minor",
            MusicalKey::BMinor => "B Minor",
        }
    }
}
