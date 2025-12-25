//! Stub analyzer implementation for Phase 1
//!
//! Skips BPM/key detection but still generates waveforms from audio.
//! This allows faster exports when BPM/key are already in metadata.

use super::traits::{AnalysisResult, AudioAnalyzer, WaveformData};
use super::waveform::generate_waveforms;
use crate::model::Track;
use anyhow::Result;
use std::path::Path;

/// Stub analyzer that skips BPM/key detection but generates waveforms
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
        log::debug!("Stub analysis (waveforms only) for: {:?}", audio_path);

        // Generate waveforms from audio
        let waveforms = match generate_waveforms(audio_path, track.duration_ms) {
            Ok(w) => {
                log::debug!("Waveforms generated: PWV3={} bytes", w.detail.len());
                w
            }
            Err(e) => {
                log::warn!("Waveform generation failed for {:?}: {}", audio_path, e);
                WaveformData::minimal_stub()
            }
        };

        // Use existing BPM/key from track metadata if available
        Ok(AnalysisResult {
            bpm: track.bpm,
            key: track.key,
            beatgrid: None,
            waveforms,
        })
    }
}
