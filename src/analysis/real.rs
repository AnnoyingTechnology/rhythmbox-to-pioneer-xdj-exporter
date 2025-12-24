//! Real audio analyzer implementation for Phase 2
//!
//! Uses actual audio analysis for BPM detection.
//! Other features (key, waveform) remain stubs for now.

use super::bpm::detect_bpm_with_range;
use super::traits::{AnalysisResult, AudioAnalyzer, WaveformData};
use crate::model::Track;
use anyhow::Result;
use std::path::Path;

/// Real audio analyzer with BPM detection
pub struct RealAnalyzer {
    /// Whether to skip BPM detection on errors
    skip_on_error: bool,
    /// Minimum BPM for detection range
    min_bpm: f32,
    /// Maximum BPM for detection range
    max_bpm: f32,
    /// Whether to cache detected BPM to ID3 tags
    cache_to_id3: bool,
}

impl RealAnalyzer {
    pub fn new() -> Self {
        Self {
            skip_on_error: true,
            min_bpm: 70.0,
            max_bpm: 170.0,
            cache_to_id3: false,
        }
    }

    /// Create analyzer with custom BPM range
    pub fn with_bpm_range(mut self, min: f32, max: f32) -> Self {
        self.min_bpm = min;
        self.max_bpm = max;
        self
    }

    /// Enable caching detected BPM to ID3 tags
    pub fn with_id3_caching(mut self, enable: bool) -> Self {
        self.cache_to_id3 = enable;
        self
    }

    /// Create analyzer that fails on BPM detection errors
    pub fn strict() -> Self {
        Self {
            skip_on_error: false,
            min_bpm: 70.0,
            max_bpm: 170.0,
            cache_to_id3: false,
        }
    }
}

impl Default for RealAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioAnalyzer for RealAnalyzer {
    fn analyze(&self, audio_path: &Path, track: &Track) -> Result<AnalysisResult> {
        log::debug!("Analyzing: {:?}", audio_path);

        // Check if track already has BPM from ID3 tags
        let bpm = if let Some(existing_bpm) = track.bpm {
            log::info!(
                "Using existing BPM from metadata: {:.1} for {}",
                existing_bpm,
                track.title
            );
            Some(existing_bpm)
        } else {
            // Detect BPM
            match detect_bpm_with_range(audio_path, self.min_bpm, self.max_bpm) {
                Ok(result) => {
                    log::info!(
                        "BPM detected: {:.1} (confidence: {:.2}) for {}",
                        result.bpm,
                        result.confidence,
                        track.title
                    );

                    // Cache to ID3 if enabled
                    if self.cache_to_id3 {
                        if let Err(e) = cache_bpm_to_id3(audio_path, result.bpm) {
                            log::warn!("Failed to cache BPM to ID3: {}", e);
                        }
                    }

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

/// Cache detected BPM to audio file metadata (Vorbis for FLAC, etc.)
/// Note: MP3 files are skipped due to a lofty library issue with TBPM frames
fn cache_bpm_to_id3(audio_path: &Path, bpm: f32) -> Result<()> {
    use lofty::config::WriteOptions;
    use lofty::prelude::*;
    use lofty::probe::Probe;
    use lofty::tag::Tag;

    // Read existing tags
    let mut tagged_file = Probe::open(audio_path)
        .map_err(|e| anyhow::anyhow!("Failed to open file for tagging: {}", e))?
        .read()
        .map_err(|e| anyhow::anyhow!("Failed to read tags: {}", e))?;

    let file_type = tagged_file.file_type();

    // Skip MP3 files - lofty has issues writing TBPM frames to ID3v2
    // TODO: Use mutagen (Python) or id3 crate as fallback for MP3
    if matches!(file_type, lofty::file::FileType::Mpeg) {
        log::debug!("Skipping BPM cache for MP3 file (lofty TBPM issue): {:?}", audio_path);
        return Ok(());
    }

    let tag_type = file_type.primary_tag_type();

    // Get or create the appropriate tag
    let tag = match tagged_file.tag_mut(tag_type) {
        Some(t) => t,
        None => {
            tagged_file.insert_tag(Tag::new(tag_type));
            tagged_file.tag_mut(tag_type).unwrap()
        }
    };

    // Set BPM - round to integer
    let bpm_int = bpm.round() as u32;
    tag.insert_text(ItemKey::Bpm, bpm_int.to_string());

    // Save tags back to file (only modifies metadata, not audio data)
    tagged_file
        .save_to_path(audio_path, WriteOptions::default())
        .map_err(|e| anyhow::anyhow!("Failed to save tags: {}", e))?;

    log::info!("Cached BPM {} to metadata for {:?}", bpm_int, audio_path);
    Ok(())
}
