//! Real audio analyzer implementation for Phase 2
//!
//! Uses actual audio analysis for BPM detection.
//! Other features (key, waveform) remain stubs for now.

use super::bpm::detect_bpm;
use super::traits::{AnalysisResult, AudioAnalyzer, WaveformData};
use anyhow::Result;
use std::path::Path;

/// Real audio analyzer with BPM detection
pub struct RealAnalyzer {
    /// Whether to skip BPM detection on errors
    skip_on_error: bool,
}

impl RealAnalyzer {
    pub fn new() -> Self {
        Self {
            skip_on_error: true,
        }
    }

    /// Create analyzer that fails on BPM detection errors
    pub fn strict() -> Self {
        Self {
            skip_on_error: false,
        }
    }
}

impl Default for RealAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioAnalyzer for RealAnalyzer {
    fn analyze(&self, audio_path: &Path) -> Result<AnalysisResult> {
        log::debug!("Analyzing: {:?}", audio_path);

        // Detect BPM
        let bpm = match detect_bpm(audio_path) {
            Ok(result) => {
                log::info!(
                    "BPM detected: {:.1} (confidence: {:.2})",
                    result.bpm,
                    result.confidence
                );
                Some(result.bpm)
            }
            Err(e) => {
                if self.skip_on_error {
                    log::warn!("BPM detection failed for {:?}: {}", audio_path, e);
                    None
                } else {
                    return Err(e);
                }
            }
        };

        // Phase 2 TODO: Key detection
        let key = None;

        // Phase 2 TODO: Beat grid
        let beatgrid = None;

        // Phase 2 TODO: Waveform generation
        let waveforms = WaveformData::minimal_stub();

        Ok(AnalysisResult {
            bpm,
            key,
            beatgrid,
            waveforms,
        })
    }
}
