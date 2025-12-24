//! Stub analyzer implementation for Phase 1
//!
//! Returns empty/minimal analysis data without actually processing audio.
//! This allows the export pipeline to work while we develop real analysis.

use super::traits::{AnalysisResult, AudioAnalyzer, WaveformData};
use crate::model::Track;
use anyhow::Result;
use std::path::Path;

/// Stub analyzer that returns minimal/empty analysis data
pub struct StubAnalyzer;

impl StubAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StubAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioAnalyzer for StubAnalyzer {
    fn analyze(&self, audio_path: &Path, track: &Track) -> Result<AnalysisResult> {
        log::debug!("Stub analysis for: {:?}", audio_path);

        // Use existing BPM from track metadata if available
        Ok(AnalysisResult {
            bpm: track.bpm,
            key: None,
            beatgrid: None,
            waveforms: WaveformData::minimal_stub(),
        })
    }
}
