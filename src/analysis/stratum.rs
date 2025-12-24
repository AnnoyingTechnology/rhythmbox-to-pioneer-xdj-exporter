//! Unified audio analysis using stratum-dsp
//!
//! This module provides BPM and key detection using stratum-dsp's
//! chroma-based analysis. Audio is decoded once and analyzed together.

use crate::model::MusicalKey;
use anyhow::{Context, Result};
use std::path::Path;
use stratum_dsp::{analyze_audio, AnalysisConfig};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Combined analysis result from stratum-dsp
#[derive(Debug, Clone)]
pub struct StratumResult {
    /// Detected BPM
    pub bpm: f32,
    /// Detected musical key (if successfully mapped)
    pub key: Option<MusicalKey>,
}

/// Analyze audio file for BPM and key using stratum-dsp
///
/// This decodes the audio once and extracts both BPM and key.
/// Memory is released as soon as analysis completes.
pub fn analyze_audio_file(path: &Path, min_bpm: f32, max_bpm: f32) -> Result<StratumResult> {
    log::debug!("Analyzing audio with stratum-dsp: {:?}", path);

    // Decode audio to mono f32 samples
    let (samples, sample_rate) = decode_to_mono(path)?;

    let num_samples = samples.len();
    log::debug!(
        "Decoded {} samples ({:.1}s) at {}Hz",
        num_samples,
        num_samples as f32 / sample_rate as f32,
        sample_rate
    );

    if num_samples < 44100 {
        anyhow::bail!("Audio too short for analysis");
    }

    // Run stratum-dsp analysis
    let config = AnalysisConfig::default();
    let result = analyze_audio(&samples, sample_rate, config)
        .map_err(|e| anyhow::anyhow!("Audio analysis failed: {:?}", e))?;

    // Explicitly drop samples to free memory immediately after analysis
    drop(samples);

    // Get BPM with range normalization
    let mut bpm = result.bpm;
    if min_bpm > 0.0 && max_bpm > 0.0 && bpm > 0.0 {
        // Double BPM if below minimum
        while bpm < min_bpm && bpm * 2.0 <= max_bpm {
            bpm *= 2.0;
            log::debug!("BPM doubled to {:.1} (was below minimum {})", bpm, min_bpm);
        }
        // Halve BPM if above maximum
        while bpm > max_bpm && bpm / 2.0 >= min_bpm {
            bpm /= 2.0;
            log::debug!("BPM halved to {:.1} (was above maximum {})", bpm, max_bpm);
        }
    }

    // Map key (may fail for unknown formats)
    let key = match map_stratum_key(&result.key) {
        Ok(k) => Some(k),
        Err(e) => {
            log::warn!("Could not map key: {}", e);
            None
        }
    };

    log::info!(
        "Analysis complete: BPM={:.1}, Key={}",
        bpm,
        key.map(|k| k.name()).unwrap_or("unknown")
    );

    Ok(StratumResult { bpm, key })
}

/// Decode audio file to mono f32 samples
fn decode_to_mono(path: &Path) -> Result<(Vec<f32>, u32)> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open audio file: {:?}", path))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension() {
        hint.with_extension(ext.to_str().unwrap_or(""));
    }

    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .with_context(|| format!("Failed to probe audio format: {:?}", path))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .context("No audio track found")?;

    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .context("No sample rate in audio track")?;

    let dec_opts = DecoderOptions::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .context("Failed to create audio decoder")?;

    let mut all_samples: Vec<f32> = Vec::new();
    let max_samples = sample_rate as usize * 120; // 120 seconds max

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                log::warn!("Error reading packet: {:?}", e);
                break;
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Error decoding packet: {:?}", e);
                continue;
            }
        };

        let spec = *decoded.spec();
        let duration = decoded.capacity() as u64;

        let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
        sample_buf.copy_interleaved_ref(decoded);

        let samples = sample_buf.samples();

        // Convert to mono if stereo
        let channels = spec.channels.count();
        if channels > 1 {
            for chunk in samples.chunks(channels) {
                let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                all_samples.push(mono);
            }
        } else {
            all_samples.extend_from_slice(samples);
        }

        if all_samples.len() >= max_samples {
            break;
        }
    }

    Ok((all_samples, sample_rate))
}

/// Map stratum-dsp Key to our MusicalKey enum
fn map_stratum_key(key: &stratum_dsp::Key) -> Result<MusicalKey> {
    let name = key.name();
    let lower = name.to_lowercase();

    let musical_key = match lower.as_str() {
        // Major keys (with or without "major" suffix)
        "c major" | "cmaj" | "c" => MusicalKey::CMajor,
        "c# major" | "db major" | "c#maj" | "dbmaj" | "c#" | "db" => MusicalKey::DbMajor,
        "d major" | "dmaj" | "d" => MusicalKey::DMajor,
        "d# major" | "eb major" | "d#maj" | "ebmaj" | "d#" | "eb" => MusicalKey::EbMajor,
        "e major" | "emaj" | "e" => MusicalKey::EMajor,
        "f major" | "fmaj" | "f" => MusicalKey::FMajor,
        "f# major" | "gb major" | "f#maj" | "gbmaj" | "f#" | "gb" => MusicalKey::GbMajor,
        "g major" | "gmaj" | "g" => MusicalKey::GMajor,
        "g# major" | "ab major" | "g#maj" | "abmaj" | "g#" | "ab" => MusicalKey::AbMajor,
        "a major" | "amaj" | "a" => MusicalKey::AMajor,
        "a# major" | "bb major" | "a#maj" | "bbmaj" | "a#" | "bb" => MusicalKey::BbMajor,
        "b major" | "bmaj" | "b" => MusicalKey::BMajor,

        // Minor keys
        "c minor" | "cm" | "cmin" => MusicalKey::CMinor,
        "c# minor" | "db minor" | "c#m" | "c#min" | "dbm" | "dbmin" => MusicalKey::CsMinor,
        "d minor" | "dm" | "dmin" => MusicalKey::DMinor,
        "d# minor" | "eb minor" | "d#m" | "d#min" | "ebm" | "ebmin" => MusicalKey::EbMinor,
        "e minor" | "em" | "emin" => MusicalKey::EMinor,
        "f minor" | "fm" | "fmin" => MusicalKey::FMinor,
        "f# minor" | "gb minor" | "f#m" | "f#min" | "gbm" | "gbmin" => MusicalKey::FsMinor,
        "g minor" | "gm" | "gmin" => MusicalKey::GMinor,
        "g# minor" | "ab minor" | "g#m" | "g#min" | "abm" | "abmin" => MusicalKey::AbMinor,
        "a minor" | "am" | "amin" => MusicalKey::AMinor,
        "a# minor" | "bb minor" | "a#m" | "a#min" | "bbm" | "bbmin" => MusicalKey::BbMinor,
        "b minor" | "bm" | "bmin" => MusicalKey::BMinor,

        _ => anyhow::bail!("Unknown key: {}", name),
    };

    Ok(musical_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_requires_file() {
        let result = analyze_audio_file(Path::new("/nonexistent/file.mp3"), 70.0, 170.0);
        assert!(result.is_err());
    }
}
