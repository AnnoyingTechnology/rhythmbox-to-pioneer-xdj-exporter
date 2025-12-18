//! BPM detection using aubio-rs and symphonia
//!
//! This module provides tempo detection by:
//! 1. Decoding audio to PCM using symphonia
//! 2. Detecting BPM using aubio's tempo tracker

use anyhow::{Context, Result};
use aubio_rs::{OnsetMode, Tempo};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// BPM detection result
#[derive(Debug, Clone)]
pub struct BpmResult {
    /// Detected BPM
    pub bpm: f32,
    /// Confidence (0.0-1.0) - higher is more confident
    pub confidence: f32,
}

/// Detect BPM from an audio file
pub fn detect_bpm(path: &Path) -> Result<BpmResult> {
    log::debug!("Detecting BPM for: {:?}", path);

    // Open the audio file
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open audio file: {:?}", path))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Probe the file format
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

    // Find the first audio track
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

    // Create decoder
    let dec_opts = DecoderOptions::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .context("Failed to create audio decoder")?;

    // Create aubio tempo detector
    // Buffer size and hop size affect detection quality
    // Typical values: buf_size=1024, hop_size=512 for 44100Hz
    // SpecFlux is a good general-purpose onset detection method for tempo
    let buf_size = 1024;
    let hop_size = 512;

    let mut tempo = Tempo::new(OnsetMode::SpecFlux, buf_size, hop_size, sample_rate)
        .map_err(|e| anyhow::anyhow!("Failed to create tempo detector: {:?}", e))?;

    // Collect samples for analysis
    let mut all_samples: Vec<f32> = Vec::new();
    let max_samples = sample_rate as usize * 120; // Analyze up to 120 seconds

    // Decode audio packets
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break; // End of file
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

        // Convert to f32 samples
        let spec = *decoded.spec();
        let duration = decoded.capacity() as u64;

        let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
        sample_buf.copy_interleaved_ref(decoded);

        let samples = sample_buf.samples();

        // Convert to mono if stereo (average channels)
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

    log::debug!(
        "Decoded {} samples ({:.1}s) at {}Hz",
        all_samples.len(),
        all_samples.len() as f32 / sample_rate as f32,
        sample_rate
    );

    if all_samples.len() < hop_size * 10 {
        anyhow::bail!("Audio too short for BPM detection");
    }

    // Process samples through tempo detector
    let mut beat_count = 0;
    let mut confidence_sum = 0.0;
    let mut last_bpm = 0.0;

    for chunk in all_samples.chunks(hop_size) {
        if chunk.len() < hop_size {
            break;
        }

        // aubio expects exactly hop_size samples
        let input: Vec<f32> = chunk.to_vec();

        // Process the samples
        let beat = tempo
            .do_result(&input)
            .map_err(|e| anyhow::anyhow!("Tempo processing failed: {:?}", e))?;

        if beat > 0.0 {
            beat_count += 1;
        }

        // Get current BPM estimate
        let current_bpm = tempo.get_bpm();
        if current_bpm > 0.0 {
            last_bpm = current_bpm;
            confidence_sum += tempo.get_confidence();
        }
    }

    let detected_bpm = tempo.get_bpm();
    let confidence = if beat_count > 0 {
        confidence_sum / beat_count as f32
    } else {
        tempo.get_confidence()
    };

    // Use last known good BPM if final is 0
    let final_bpm = if detected_bpm > 0.0 {
        detected_bpm
    } else {
        last_bpm
    };

    if final_bpm <= 0.0 {
        anyhow::bail!("Could not detect BPM");
    }

    log::info!(
        "Detected BPM: {:.1} (confidence: {:.2})",
        final_bpm,
        confidence
    );

    Ok(BpmResult {
        bpm: final_bpm,
        confidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bpm_detection_requires_file() {
        let result = detect_bpm(Path::new("/nonexistent/file.mp3"));
        assert!(result.is_err());
    }
}
