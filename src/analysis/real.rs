//! Real audio analyzer implementation for Phase 2
//!
//! Uses stratum-dsp for both BPM and key detection in a single pass.
//! Other features (waveform) remain stubs for now.

use super::stratum::analyze_audio_file;
use super::traits::{AnalysisResult, AudioAnalyzer, WaveformData};
use crate::model::{MusicalKey, Track};
use anyhow::Result;
use std::path::Path;

/// Real audio analyzer with BPM and key detection (powered by stratum-dsp)
pub struct RealAnalyzer {
    /// Whether to skip detection on errors
    skip_on_error: bool,
    /// Minimum BPM for detection range
    min_bpm: f32,
    /// Maximum BPM for detection range
    max_bpm: f32,
    /// Whether to cache detected values to metadata
    cache_to_id3: bool,
    /// Whether to detect key (can be disabled for speed)
    detect_key: bool,
    /// Whether to detect BPM (can be disabled)
    detect_bpm: bool,
}

impl RealAnalyzer {
    pub fn new() -> Self {
        Self {
            skip_on_error: true,
            min_bpm: 70.0,
            max_bpm: 170.0,
            cache_to_id3: false,
            detect_key: true,
            detect_bpm: true,
        }
    }

    /// Create analyzer with custom BPM range
    pub fn with_bpm_range(mut self, min: f32, max: f32) -> Self {
        self.min_bpm = min;
        self.max_bpm = max;
        self
    }

    /// Enable caching detected values to metadata
    pub fn with_id3_caching(mut self, enable: bool) -> Self {
        self.cache_to_id3 = enable;
        self
    }

    /// Enable/disable key detection
    pub fn with_key_detection(mut self, enable: bool) -> Self {
        self.detect_key = enable;
        self
    }

    /// Enable/disable BPM detection
    pub fn with_bpm_detection(mut self, enable: bool) -> Self {
        self.detect_bpm = enable;
        self
    }

    /// Create analyzer that fails on detection errors
    pub fn strict() -> Self {
        Self {
            skip_on_error: false,
            min_bpm: 70.0,
            max_bpm: 170.0,
            cache_to_id3: false,
            detect_key: true,
            detect_bpm: true,
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

        // Check what we already have from metadata
        let has_bpm = track.bpm.is_some();
        let has_key = track.key.is_some();

        // Determine if we need to run audio analysis
        let need_bpm = self.detect_bpm && !has_bpm;
        let need_key = self.detect_key && !has_key;
        let need_analysis = need_bpm || need_key;

        // Get BPM and key - either from metadata or from analysis
        let (bpm, key) = if need_analysis {
            // Run unified stratum-dsp analysis (decodes audio once)
            match analyze_audio_file(audio_path, self.min_bpm, self.max_bpm) {
                Ok(result) => {
                    // Use detected values where needed, metadata where available
                    let final_bpm = if has_bpm {
                        log::info!(
                            "Using existing BPM from metadata: {:.1} for {}",
                            track.bpm.unwrap(),
                            track.title
                        );
                        track.bpm
                    } else if self.detect_bpm {
                        log::info!(
                            "BPM detected: {:.1} for {}",
                            result.bpm,
                            track.title
                        );
                        if self.cache_to_id3 {
                            if let Err(e) = cache_bpm_to_id3(audio_path, result.bpm) {
                                log::warn!("Failed to cache BPM: {}", e);
                            }
                        }
                        Some(result.bpm)
                    } else {
                        None
                    };

                    let final_key = if has_key {
                        log::info!(
                            "Using existing key from metadata: {} for {}",
                            track.key.unwrap().name(),
                            track.title
                        );
                        track.key
                    } else if self.detect_key {
                        if let Some(k) = result.key {
                            log::info!("Key detected: {} for {}", k.name(), track.title);
                            if self.cache_to_id3 {
                                if let Err(e) = cache_key_to_id3(audio_path, k) {
                                    log::warn!("Failed to cache key: {}", e);
                                }
                            }
                        }
                        result.key
                    } else {
                        None
                    };

                    (final_bpm, final_key)
                }
                Err(e) => {
                    if self.skip_on_error {
                        log::warn!("Audio analysis failed for {:?}: {}", audio_path, e);
                        // Fall back to metadata values
                        (track.bpm, track.key)
                    } else {
                        return Err(e);
                    }
                }
            }
        } else {
            // No analysis needed, use metadata
            if has_bpm {
                log::info!(
                    "Using existing BPM from metadata: {:.1} for {}",
                    track.bpm.unwrap(),
                    track.title
                );
            }
            if has_key {
                log::info!(
                    "Using existing key from metadata: {} for {}",
                    track.key.unwrap().name(),
                    track.title
                );
            }
            (track.bpm, track.key)
        };

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

    let mut tagged_file = Probe::open(audio_path)
        .map_err(|e| anyhow::anyhow!("Failed to open file for tagging: {}", e))?
        .read()
        .map_err(|e| anyhow::anyhow!("Failed to read tags: {}", e))?;

    let file_type = tagged_file.file_type();

    // Skip MP3 files - lofty has issues writing TBPM frames to ID3v2
    if matches!(file_type, lofty::file::FileType::Mpeg) {
        log::debug!("Skipping BPM cache for MP3 (lofty issue): {:?}", audio_path);
        return Ok(());
    }

    let tag_type = file_type.primary_tag_type();
    let tag = match tagged_file.tag_mut(tag_type) {
        Some(t) => t,
        None => {
            tagged_file.insert_tag(Tag::new(tag_type));
            tagged_file.tag_mut(tag_type).unwrap()
        }
    };

    let bpm_int = bpm.round() as u32;
    tag.insert_text(ItemKey::Bpm, bpm_int.to_string());

    tagged_file
        .save_to_path(audio_path, WriteOptions::default())
        .map_err(|e| anyhow::anyhow!("Failed to save tags: {}", e))?;

    log::info!("Cached BPM {} to metadata for {:?}", bpm_int, audio_path);
    Ok(())
}

/// Cache detected key to audio file metadata
/// Note: MP3 files are skipped due to lofty library issues with TKEY frames
fn cache_key_to_id3(audio_path: &Path, key: MusicalKey) -> Result<()> {
    use lofty::config::WriteOptions;
    use lofty::prelude::*;
    use lofty::probe::Probe;
    use lofty::tag::Tag;

    let mut tagged_file = Probe::open(audio_path)
        .map_err(|e| anyhow::anyhow!("Failed to open file for tagging: {}", e))?
        .read()
        .map_err(|e| anyhow::anyhow!("Failed to read tags: {}", e))?;

    let file_type = tagged_file.file_type();

    // Skip MP3 files - lofty has issues writing ID3v2 frames
    if matches!(file_type, lofty::file::FileType::Mpeg) {
        log::debug!("Skipping key cache for MP3 (lofty issue): {:?}", audio_path);
        return Ok(());
    }

    let tag_type = file_type.primary_tag_type();
    let tag = match tagged_file.tag_mut(tag_type) {
        Some(t) => t,
        None => {
            tagged_file.insert_tag(Tag::new(tag_type));
            tagged_file.tag_mut(tag_type).unwrap()
        }
    };

    let key_str = key_to_metadata_string(key);
    tag.insert_text(ItemKey::InitialKey, key_str.clone());

    tagged_file
        .save_to_path(audio_path, WriteOptions::default())
        .map_err(|e| anyhow::anyhow!("Failed to save tags: {}", e))?;

    log::info!("Cached key {} to metadata for {:?}", key_str, audio_path);
    Ok(())
}

/// Convert MusicalKey to standard metadata string
fn key_to_metadata_string(key: MusicalKey) -> String {
    match key {
        MusicalKey::CMajor => "C".to_string(),
        MusicalKey::DbMajor => "Db".to_string(),
        MusicalKey::DMajor => "D".to_string(),
        MusicalKey::EbMajor => "Eb".to_string(),
        MusicalKey::EMajor => "E".to_string(),
        MusicalKey::FMajor => "F".to_string(),
        MusicalKey::GbMajor => "Gb".to_string(),
        MusicalKey::GMajor => "G".to_string(),
        MusicalKey::AbMajor => "Ab".to_string(),
        MusicalKey::AMajor => "A".to_string(),
        MusicalKey::BbMajor => "Bb".to_string(),
        MusicalKey::BMajor => "B".to_string(),
        MusicalKey::CMinor => "Cm".to_string(),
        MusicalKey::CsMinor => "C#m".to_string(),
        MusicalKey::DMinor => "Dm".to_string(),
        MusicalKey::EbMinor => "Ebm".to_string(),
        MusicalKey::EMinor => "Em".to_string(),
        MusicalKey::FMinor => "Fm".to_string(),
        MusicalKey::FsMinor => "F#m".to_string(),
        MusicalKey::GMinor => "Gm".to_string(),
        MusicalKey::AbMinor => "Abm".to_string(),
        MusicalKey::AMinor => "Am".to_string(),
        MusicalKey::BbMinor => "Bbm".to_string(),
        MusicalKey::BMinor => "Bm".to_string(),
    }
}
