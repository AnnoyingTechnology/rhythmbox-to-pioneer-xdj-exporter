//! Analysis trait definitions and data structures

use crate::model::{MusicalKey, Track};
use anyhow::Result;
use std::path::Path;

/// Audio analyzer trait - allows swapping between stub and real implementations
pub trait AudioAnalyzer {
    /// Analyze an audio file and return all analysis data
    /// Takes the track context to check for existing metadata (e.g., BPM from ID3)
    fn analyze(&self, audio_path: &Path, track: &Track) -> Result<AnalysisResult>;
}

/// Complete analysis result for a track
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// Detected BPM (None in Phase 1, detected in Phase 2)
    pub bpm: Option<f32>,

    /// Detected musical key (None in Phase 1, detected in Phase 2)
    pub key: Option<MusicalKey>,

    /// Beat grid data (None in Phase 1, detected in Phase 2)
    pub beatgrid: Option<BeatGrid>,

    /// Waveform data (stub in Phase 1, full in Phase 2)
    pub waveforms: WaveformData,
}

/// Beat grid information
#[derive(Debug, Clone)]
pub struct BeatGrid {
    /// BPM (beats per minute)
    pub bpm: f32,

    /// Beat positions in milliseconds
    pub beats: Vec<f32>,

    /// Downbeat positions (first beat of each bar)
    pub downbeats: Vec<f32>,
}

/// Waveform data for all required formats
#[derive(Debug, Clone)]
pub struct WaveformData {
    /// Waveform preview (low-res overview, monochrome)
    pub preview: Vec<u8>,

    /// Tiny waveform preview (very compressed)
    pub tiny_preview: Vec<u8>,

    /// Waveform detail (high-res, monochrome)
    pub detail: Vec<u8>,

    /// Waveform color preview (frequency-band colored)
    pub color_preview: Vec<u8>,

    /// Waveform color detail (frequency-band colored, high-res)
    pub color_detail: Vec<u8>,
}

impl WaveformData {
    /// Create empty stub waveforms (Phase 1)
    pub fn empty_stub() -> Self {
        Self {
            preview: Vec::new(),
            tiny_preview: Vec::new(),
            detail: Vec::new(),
            color_preview: Vec::new(),
            color_detail: Vec::new(),
        }
    }

    /// Create minimal valid waveforms (Phase 1)
    /// These are the smallest possible valid waveforms that Pioneer devices will accept
    pub fn minimal_stub() -> Self {
        // TODO: Research minimum valid waveform sizes from rekordcrate/Deep Symmetry docs
        // For now, return empty - we'll populate with minimal valid data when implementing ANLZ writer
        Self::empty_stub()
    }
}
