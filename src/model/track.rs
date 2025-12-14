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
    /// Based on the key table format in Pioneer PDB files
    pub fn to_rekordbox_id(&self) -> u32 {
        match self {
            // Major keys (Ionian mode)
            MusicalKey::CMajor => 1,
            MusicalKey::DbMajor => 2,
            MusicalKey::DMajor => 3,
            MusicalKey::EbMajor => 4,
            MusicalKey::EMajor => 5,
            MusicalKey::FMajor => 6,
            MusicalKey::GbMajor => 7,
            MusicalKey::GMajor => 8,
            MusicalKey::AbMajor => 9,
            MusicalKey::AMajor => 10,
            MusicalKey::BbMajor => 11,
            MusicalKey::BMajor => 12,

            // Minor keys (Aeolian mode)
            MusicalKey::CMinor => 13,
            MusicalKey::CsMinor => 14,
            MusicalKey::DMinor => 15,
            MusicalKey::EbMinor => 16,
            MusicalKey::EMinor => 17,
            MusicalKey::FMinor => 18,
            MusicalKey::FsMinor => 19,
            MusicalKey::GMinor => 20,
            MusicalKey::AbMinor => 21,
            MusicalKey::AMinor => 22,
            MusicalKey::BbMinor => 23,
            MusicalKey::BMinor => 24,
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
