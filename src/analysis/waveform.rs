//! Waveform generation for Pioneer ANLZ files
//!
//! Generates waveform data for Pioneer DJ equipment:
//! - PWAV: 400-byte monochrome preview (Nexus players)
//! - PWV2: 100-byte tiny preview (CDJ-900)
//! - PWV3: Variable-length monochrome detail (150 entries/sec)
//! - PWV5: Variable-length color detail (150 entries/sec, 2 bytes each)

use super::traits::WaveformData;
use anyhow::{Context, Result};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Number of columns in PWAV preview
const PWAV_COLUMNS: usize = 400;
/// Number of columns in PWV2 tiny preview
const PWV2_COLUMNS: usize = 100;
/// Entries per second for detail waveforms (PWV3, PWV5)
const DETAIL_ENTRIES_PER_SEC: f32 = 150.0;
/// Maximum height value for PWAV/PWV3 (5 bits = 0-31)
const MAX_HEIGHT_5BIT: u8 = 31;
/// Maximum height value for PWV2 (4 bits = 0-15)
const MAX_HEIGHT_4BIT: u8 = 15;
/// Maximum height value for PWV5 (5 bits in packed format)
const MAX_HEIGHT_PWV5: u8 = 31;

/// Generate all waveform data for a track
pub fn generate_waveforms(audio_path: &Path, _duration_ms: u32) -> Result<WaveformData> {
    log::debug!("Generating waveforms for {:?}", audio_path);

    // Decode audio to mono samples
    let (samples, sample_rate) = decode_to_mono_for_waveform(audio_path)?;

    let duration_secs = samples.len() as f32 / sample_rate as f32;

    // Find overall peak for debugging
    let overall_peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    log::debug!(
        "Decoded {} samples ({:.1}s) at {}Hz, peak amplitude: {:.4}",
        samples.len(),
        duration_secs,
        sample_rate,
        overall_peak
    );

    // Generate each waveform type
    let preview = generate_pwav(&samples, sample_rate);
    let tiny_preview = generate_pwv2(&samples, sample_rate);
    let detail = generate_pwv3(&samples, sample_rate);
    let color_detail = generate_pwv5(&samples, sample_rate);

    log::info!(
        "Waveforms generated: PWAV={}, PWV2={}, PWV3={}, PWV5={} bytes",
        preview.len(),
        tiny_preview.len(),
        detail.len(),
        color_detail.len()
    );

    Ok(WaveformData {
        preview,
        tiny_preview,
        detail,
        color_preview: Vec::new(), // PWV4 not implemented yet
        color_detail,
    })
}

/// Generate PWAV monochrome preview (400 bytes)
///
/// Each byte encodes: height (5 low bits, 0-31) + whiteness (3 high bits, 0-7)
fn generate_pwav(samples: &[f32], _sample_rate: u32) -> Vec<u8> {
    let samples_per_column = samples.len() / PWAV_COLUMNS;
    if samples_per_column == 0 {
        // Audio too short, return flat waveform
        return vec![encode_pwav_byte(16, 5); PWAV_COLUMNS];
    }

    let mut result = Vec::with_capacity(PWAV_COLUMNS);
    let mut max_height = 0u8;
    let mut max_peak = 0.0f32;

    for col in 0..PWAV_COLUMNS {
        let start = col * samples_per_column;
        let end = ((col + 1) * samples_per_column).min(samples.len());
        let chunk = &samples[start..end];

        // Calculate RMS and peak for this window
        let (rms, peak) = calculate_rms_and_peak(chunk);

        // Height based on peak (0-31)
        let height = (peak * MAX_HEIGHT_5BIT as f32).min(MAX_HEIGHT_5BIT as f32) as u8;

        if peak > max_peak {
            max_peak = peak;
        }
        if height > max_height {
            max_height = height;
        }

        // Whiteness based on crest factor (peak/rms ratio)
        // Higher crest = more transient = whiter
        // Use whiteness=5 like reference (not maximum 7)
        let crest_factor = if rms > 0.001 { peak / rms } else { 1.0 };
        let whiteness = ((crest_factor - 1.0) * 2.0).clamp(0.0, 7.0) as u8;
        let whiteness = whiteness.max(5); // Changed from 7 to 5 to match reference

        result.push(encode_pwav_byte(height, whiteness));
    }

    log::debug!("PWAV: samples_per_column={}, max_peak={:.4}, max_height={}",
        samples_per_column, max_peak, max_height);

    result
}

/// Generate PWV2 tiny preview (100 bytes)
///
/// Each byte uses 4 low bits for height (0-15)
fn generate_pwv2(samples: &[f32], _sample_rate: u32) -> Vec<u8> {
    let samples_per_column = samples.len() / PWV2_COLUMNS;
    if samples_per_column == 0 {
        return vec![8; PWV2_COLUMNS]; // Flat waveform at half height
    }

    let mut result = Vec::with_capacity(PWV2_COLUMNS);

    for col in 0..PWV2_COLUMNS {
        let start = col * samples_per_column;
        let end = ((col + 1) * samples_per_column).min(samples.len());
        let chunk = &samples[start..end];

        let (_, peak) = calculate_rms_and_peak(chunk);

        // Height based on peak (0-15)
        let height = (peak * MAX_HEIGHT_4BIT as f32).min(MAX_HEIGHT_4BIT as f32) as u8;
        result.push(height);
    }

    result
}

/// Generate PWV3 monochrome detail waveform (150 entries/sec)
///
/// Same encoding as PWAV: height (5 bits) + whiteness (3 bits)
fn generate_pwv3(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let duration_secs = samples.len() as f32 / sample_rate as f32;
    let num_entries = (duration_secs * DETAIL_ENTRIES_PER_SEC).ceil() as usize;
    let samples_per_entry = samples.len() / num_entries.max(1);

    if samples_per_entry == 0 {
        return vec![encode_pwav_byte(16, 5); num_entries.max(1)];
    }

    let mut result = Vec::with_capacity(num_entries);

    for i in 0..num_entries {
        let start = i * samples_per_entry;
        let end = ((i + 1) * samples_per_entry).min(samples.len());
        let chunk = &samples[start..end];

        let (rms, peak) = calculate_rms_and_peak(chunk);

        // Height based on peak (0-31)
        let height = (peak * MAX_HEIGHT_5BIT as f32).min(MAX_HEIGHT_5BIT as f32) as u8;

        // Whiteness=7 for PWV3 (unlike PWAV which uses 5)
        // Reference files consistently use whiteness=7 for detail waveforms
        let whiteness = 7u8;

        result.push(encode_pwav_byte(height, whiteness));
    }

    result
}

/// Generate PWV5 color detail waveform (150 entries/sec, 2 bytes each)
///
/// Bit packing (big-endian): 3-bit red | 3-bit green | 3-bit blue | 5-bit height | 2 unused
fn generate_pwv5(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let duration_secs = samples.len() as f32 / sample_rate as f32;
    let num_entries = (duration_secs * DETAIL_ENTRIES_PER_SEC).ceil() as usize;
    let samples_per_entry = samples.len() / num_entries.max(1);

    if samples_per_entry == 0 {
        // Default: white color, half height
        return vec![0xff, 0x80].repeat(num_entries.max(1));
    }

    let mut result = Vec::with_capacity(num_entries * 2);

    for i in 0..num_entries {
        let start = i * samples_per_entry;
        let end = ((i + 1) * samples_per_entry).min(samples.len());
        let chunk = &samples[start..end];

        let (_rms, peak) = calculate_rms_and_peak(chunk);

        // Height based on peak (0-31)
        let height = (peak * MAX_HEIGHT_PWV5 as f32).min(MAX_HEIGHT_PWV5 as f32) as u8;

        // For simple color: always use white (7,7,7) for maximum visibility
        // Reference files use ff80 (white at height 0) for silence
        // TODO: Use FFT for frequency band coloring (low=blue, mid=green, high=red)
        let red = 7u8;
        let green = 7u8;
        let blue = 7u8;

        // Pack into 2 bytes: RRRG GGBB BHHH HH00
        let packed = encode_pwv5_entry(red, green, blue, height);
        result.extend_from_slice(&packed);
    }

    result
}

/// Encode PWAV/PWV3 byte: height (5 bits) | whiteness (3 bits)
#[inline]
fn encode_pwav_byte(height: u8, whiteness: u8) -> u8 {
    (whiteness << 5) | (height & 0x1f)
}

/// Encode PWV5 entry (2 bytes): RRR GGG BB | B HHH HH 00
#[inline]
fn encode_pwv5_entry(red: u8, green: u8, blue: u8, height: u8) -> [u8; 2] {
    // Byte 0: RRR GGG BB (red 3, green 3, blue high 2)
    // Byte 1: B HHH HH 00 (blue low 1, height 5, unused 2)
    let byte0 = ((red & 0x07) << 5) | ((green & 0x07) << 2) | ((blue >> 1) & 0x03);
    let byte1 = ((blue & 0x01) << 7) | ((height & 0x1f) << 2);
    [byte0, byte1]
}

/// Calculate RMS and peak values for a sample chunk
#[inline]
fn calculate_rms_and_peak(samples: &[f32]) -> (f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }

    let mut sum_sq = 0.0f64;
    let mut peak = 0.0f32;

    for &sample in samples {
        let abs = sample.abs();
        sum_sq += (abs as f64) * (abs as f64);
        if abs > peak {
            peak = abs;
        }
    }

    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
    (rms, peak)
}

/// Decode audio to mono f32 samples (simplified version for waveform)
fn decode_to_mono_for_waveform(path: &Path) -> Result<(Vec<f32>, u32)> {
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
    }

    Ok((all_samples, sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_pwav_byte() {
        // Height 16, whiteness 5 = 0b101_10000 = 0xa0 + 0x10 = 0xb0
        assert_eq!(encode_pwav_byte(16, 5), 0xb0);
        // Height 31, whiteness 7 = 0b111_11111 = 0xff
        assert_eq!(encode_pwav_byte(31, 7), 0xff);
        // Height 0, whiteness 0 = 0x00
        assert_eq!(encode_pwav_byte(0, 0), 0x00);
    }

    #[test]
    fn test_encode_pwv5_entry() {
        // White (7,7,7) at height 31
        let [b0, b1] = encode_pwv5_entry(7, 7, 7, 31);
        // Byte 0: 111 111 11 = 0xff
        // Byte 1: 1 11111 00 = 0xfc
        assert_eq!(b0, 0xff);
        assert_eq!(b1, 0xfc);
    }

    #[test]
    fn test_calculate_rms_and_peak() {
        let samples = vec![0.0, 0.5, -0.5, 0.25, -0.25];
        let (rms, peak) = calculate_rms_and_peak(&samples);
        assert!((peak - 0.5).abs() < 0.001);
        assert!(rms > 0.0 && rms < peak);
    }
}
