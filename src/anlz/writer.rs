//! ANLZ file writer implementation
//!
//! Writes ANLZ files with PPTH, PQTZ (beatgrid), and waveform sections.

use crate::analysis::{AnalysisResult, WaveformData};
use crate::model::Track;
use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// PMAI magic number (identifies ANLZ files)
const PMAI_MAGIC: &[u8; 4] = b"PMAI";
/// PPTH magic number (path section)
const PPTH_MAGIC: &[u8; 4] = b"PPTH";
/// PQTZ magic number (beatgrid section)
const PQTZ_MAGIC: &[u8; 4] = b"PQTZ";
/// PWAV magic number (waveform preview)
const PWAV_MAGIC: &[u8; 4] = b"PWAV";
/// PWV2 magic number (tiny waveform preview)
const PWV2_MAGIC: &[u8; 4] = b"PWV2";
/// PWV3 magic number (waveform detail)
const PWV3_MAGIC: &[u8; 4] = b"PWV3";
/// PWV5 magic number (color waveform detail)
const PWV5_MAGIC: &[u8; 4] = b"PWV5";
/// PWV4 magic number (color waveform preview)
const PWV4_MAGIC: &[u8; 4] = b"PWV4";
/// PWV4 header size (24 bytes)
const PWV4_HEADER_SIZE: u32 = 24;
/// PWV4 entry count (1200 columns)
const PWV4_ENTRY_COUNT: u32 = 1200;
/// PWV4 entry size (6 bytes per entry)
const PWV4_ENTRY_SIZE: u32 = 6;
/// PCOB magic number (cue points)
const PCOB_MAGIC: &[u8; 4] = b"PCOB";
/// PCO2 magic number (cue points v2)
const PCO2_MAGIC: &[u8; 4] = b"PCO2";
/// PMAI header size (28 bytes)
const PMAI_HEADER_SIZE: u32 = 28;
/// PPTH header size (16 bytes)
const PPTH_HEADER_SIZE: u32 = 16;
/// PQTZ header size (24 bytes)
const PQTZ_HEADER_SIZE: u32 = 24;
/// PQTZ beat entry size (8 bytes)
const PQTZ_BEAT_ENTRY_SIZE: u32 = 8;
/// PVBR magic number (VBR index section)
const PVBR_MAGIC: &[u8; 4] = b"PVBR";
/// PVBR header size (16 bytes)
const PVBR_HEADER_SIZE: u32 = 16;
/// PVBR total size (1620 bytes - header + VBR index data)
const PVBR_TOTAL_SIZE: u32 = 1620;
/// PWAV header size (20 bytes)
const PWAV_HEADER_SIZE: u32 = 20;
/// PWV2 header size (20 bytes)
const PWV2_HEADER_SIZE: u32 = 20;
/// PWV3 header size (24 bytes)
const PWV3_HEADER_SIZE: u32 = 24;
/// PWV5 header size (24 bytes)
const PWV5_HEADER_SIZE: u32 = 24;
/// PCOB header size (24 bytes)
const PCOB_HEADER_SIZE: u32 = 24;

/// Write a .DAT analysis file
///
/// Contains: PMAI header + PPTH + PVBR + PQTZ + PWAV + PWV2 + PCOB sections
pub fn write_dat_file(
    path: &Path,
    track: &Track,
    analysis: &AnalysisResult,
    audio_path: &str,
) -> Result<()> {
    log::debug!("Writing ANLZ .DAT file: {:?}", path);

    // Ensure parent directories exist (for hierarchical ANLZ structure)
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create ANLZ directory: {:?}", parent))?;
    }

    let mut file = File::create(path)
        .with_context(|| format!("Failed to create ANLZ .DAT file: {:?}", path))?;

    // Get BPM - prefer track metadata (ID3), fallback to analyzed BPM
    let bpm = track.bpm.or(analysis.bpm);

    write_dat_sections(&mut file, audio_path, bpm, track.duration_ms, &analysis.waveforms)?;

    log::debug!("ANLZ .DAT file written with waveform sections");
    Ok(())
}

/// Write a .EXT analysis file
///
/// Contains: PMAI header + PPTH + PWV3 + PCOB + PCO2 + PWV5 sections
pub fn write_ext_file(
    path: &Path,
    _track: &Track,
    analysis: &AnalysisResult,
    audio_path: &str,
) -> Result<()> {
    log::debug!("Writing ANLZ .EXT file: {:?}", path);

    // Ensure parent directories exist (for hierarchical ANLZ structure)
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create ANLZ directory: {:?}", parent))?;
    }

    let mut file = File::create(path)
        .with_context(|| format!("Failed to create ANLZ .EXT file: {:?}", path))?;

    write_ext_sections(&mut file, audio_path, &analysis.waveforms)?;

    log::debug!("ANLZ .EXT file written with waveform sections");
    Ok(())
}

/// Encode a path string as UTF-16 big-endian with NUL terminator
fn encode_path_utf16_be(path: &str) -> Vec<u8> {
    let mut result = Vec::new();
    for c in path.chars() {
        let code_point = c as u32;
        if code_point <= 0xFFFF {
            // BMP character - single UTF-16 code unit
            result.extend_from_slice(&(code_point as u16).to_be_bytes());
        } else {
            // Supplementary character - surrogate pair
            let adjusted = code_point - 0x10000;
            let high_surrogate = ((adjusted >> 10) + 0xD800) as u16;
            let low_surrogate = ((adjusted & 0x3FF) + 0xDC00) as u16;
            result.extend_from_slice(&high_surrogate.to_be_bytes());
            result.extend_from_slice(&low_surrogate.to_be_bytes());
        }
    }
    // Add NUL terminator (required by Pioneer devices)
    result.extend_from_slice(&0u16.to_be_bytes());
    result
}

/// Write .DAT file sections: PPTH → PVBR → PQTZ → PWAV → PWV2 → PCOB × 2
fn write_dat_sections(
    file: &mut File,
    audio_path: &str,
    bpm: Option<f32>,
    duration_ms: u32,
    waveforms: &WaveformData,
) -> Result<()> {
    // Encode the path as UTF-16 big-endian (includes NUL terminator)
    let path_utf16 = encode_path_utf16_be(audio_path);
    let path_len = path_utf16.len() as u32;

    // Calculate section sizes
    let ppth_section_len = PPTH_HEADER_SIZE + path_len;

    // PQTZ section - always present (even with 0 beats)
    let beat_entries = if let Some(bpm_value) = bpm {
        generate_beat_entries(bpm_value, duration_ms)
    } else {
        Vec::new() // Empty beatgrid
    };
    let pqtz_section_len = PQTZ_HEADER_SIZE + (beat_entries.len() as u32 * PQTZ_BEAT_ENTRY_SIZE);

    // PWAV section (400 bytes of waveform data)
    let pwav_entries = if waveforms.preview.len() == 400 {
        waveforms.preview.len() as u32
    } else {
        400 // Default to 400 if not exactly right
    };
    let pwav_section_len = PWAV_HEADER_SIZE + pwav_entries;

    // PWV2 section (100 bytes of tiny preview)
    let pwv2_entries = if waveforms.tiny_preview.len() == 100 {
        waveforms.tiny_preview.len() as u32
    } else {
        100
    };
    let pwv2_section_len = PWV2_HEADER_SIZE + pwv2_entries;

    // PCOB sections (2 of them, each 24 bytes)
    let pcob_section_len = PCOB_HEADER_SIZE;

    // Total file size
    let total_file_size = PMAI_HEADER_SIZE
        + ppth_section_len
        + PVBR_TOTAL_SIZE
        + pqtz_section_len
        + pwav_section_len
        + pwv2_section_len
        + pcob_section_len * 2;

    // --- PMAI Header (28 bytes) ---
    file.write_all(PMAI_MAGIC)?;
    file.write_all(&PMAI_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&total_file_size.to_be_bytes())?;
    file.write_all(&1u32.to_be_bytes())?; // Offset 12
    file.write_all(&0x00010000u32.to_be_bytes())?; // Offset 16
    file.write_all(&0x00010000u32.to_be_bytes())?; // Offset 20
    file.write_all(&0u32.to_be_bytes())?; // Offset 24

    // --- PPTH Section ---
    file.write_all(PPTH_MAGIC)?;
    file.write_all(&PPTH_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&ppth_section_len.to_be_bytes())?;
    file.write_all(&path_len.to_be_bytes())?;
    file.write_all(&path_utf16)?;

    // --- PVBR Section (VBR index - 1620 bytes total) ---
    // Header is 16 bytes: magic(4) + len_header(4) + len_tag(4) + unknown(4)
    file.write_all(PVBR_MAGIC)?;
    file.write_all(&PVBR_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&PVBR_TOTAL_SIZE.to_be_bytes())?;
    file.write_all(&0u32.to_be_bytes())?; // unknown field (part of 16-byte header)
    // Write VBR index data (1604 bytes of zeros)
    file.write_all(&vec![0u8; (PVBR_TOTAL_SIZE - PVBR_HEADER_SIZE) as usize])?;

    // --- PQTZ Section (beatgrid) - always present ---
    let num_beats = beat_entries.len() as u32;
    file.write_all(PQTZ_MAGIC)?;
    file.write_all(&PQTZ_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&pqtz_section_len.to_be_bytes())?;
    file.write_all(&0u32.to_be_bytes())?; // unknown1
    file.write_all(&0x00080000u32.to_be_bytes())?; // unknown2 (0x00080000 per reference)
    file.write_all(&num_beats.to_be_bytes())?;

    for entry in &beat_entries {
        file.write_all(&entry.beat_number.to_be_bytes())?;
        file.write_all(&entry.tempo.to_be_bytes())?;
        file.write_all(&entry.time.to_be_bytes())?;
    }

    log::debug!("PQTZ beatgrid written: {} beats", num_beats);

    // --- PWAV Section (waveform preview) ---
    file.write_all(PWAV_MAGIC)?;
    file.write_all(&PWAV_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&pwav_section_len.to_be_bytes())?;
    file.write_all(&pwav_entries.to_be_bytes())?; // len_entries
    file.write_all(&0x00010000u32.to_be_bytes())?; // unknown (always 0x00010000)
    // Write waveform data (400 bytes)
    if waveforms.preview.len() == 400 {
        file.write_all(&waveforms.preview)?;
    } else {
        // Fallback: generate flat waveform
        file.write_all(&vec![0xa2u8; 400])?;
    }

    // --- PWV2 Section (tiny waveform preview) ---
    file.write_all(PWV2_MAGIC)?;
    file.write_all(&PWV2_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&pwv2_section_len.to_be_bytes())?;
    file.write_all(&pwv2_entries.to_be_bytes())?; // len_entries
    file.write_all(&0x00010000u32.to_be_bytes())?; // unknown
    // Write tiny preview data (100 bytes)
    if waveforms.tiny_preview.len() == 100 {
        file.write_all(&waveforms.tiny_preview)?;
    } else {
        file.write_all(&vec![0x01u8; 100])?;
    }

    // --- PCOB Sections (cue points - 2 empty sections) ---
    // PCOB 1: hot cues
    file.write_all(PCOB_MAGIC)?;
    file.write_all(&PCOB_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&PCOB_HEADER_SIZE.to_be_bytes())?; // len_tag = header only
    file.write_all(&1u32.to_be_bytes())?; // entry_count = 1 (type)
    file.write_all(&0u32.to_be_bytes())?; // unknown
    file.write_all(&0xffffffffu32.to_be_bytes())?; // memory_count = -1

    // PCOB 2: memory cues
    file.write_all(PCOB_MAGIC)?;
    file.write_all(&PCOB_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&PCOB_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&0u32.to_be_bytes())?; // entry_count = 0
    file.write_all(&0u32.to_be_bytes())?;
    file.write_all(&0xffffffffu32.to_be_bytes())?;

    Ok(())
}

/// Write .EXT file sections: PPTH → PWV3 → PCOB × 2 → PCO2 × 2 → PWV5
fn write_ext_sections(
    file: &mut File,
    audio_path: &str,
    waveforms: &WaveformData,
) -> Result<()> {
    let path_utf16 = encode_path_utf16_be(audio_path);
    let path_len = path_utf16.len() as u32;
    let ppth_section_len = PPTH_HEADER_SIZE + path_len;

    // PWV3 section (detail waveform, 1 byte per entry)
    let pwv3_entries = waveforms.detail.len() as u32;
    let pwv3_section_len = PWV3_HEADER_SIZE + pwv3_entries;

    // PWV5 section (color detail, 2 bytes per entry)
    let pwv5_entries = (waveforms.color_detail.len() / 2) as u32;
    let pwv5_section_len = PWV5_HEADER_SIZE + (pwv5_entries * 2);

    // PWV4 section (color preview, 1200 entries × 6 bytes)
    let pwv4_section_len = PWV4_HEADER_SIZE + (PWV4_ENTRY_COUNT * PWV4_ENTRY_SIZE);

    // Total file size
    let total_file_size = PMAI_HEADER_SIZE
        + ppth_section_len
        + pwv3_section_len
        + PCOB_HEADER_SIZE * 2  // PCOB sections
        + 20 * 2                 // PCO2 sections (20 bytes each)
        + pwv5_section_len
        + pwv4_section_len;

    // --- PMAI Header ---
    file.write_all(PMAI_MAGIC)?;
    file.write_all(&PMAI_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&total_file_size.to_be_bytes())?;
    file.write_all(&1u32.to_be_bytes())?;
    file.write_all(&0x00010000u32.to_be_bytes())?;
    file.write_all(&0x00010000u32.to_be_bytes())?;
    file.write_all(&0u32.to_be_bytes())?;

    // --- PPTH Section ---
    file.write_all(PPTH_MAGIC)?;
    file.write_all(&PPTH_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&ppth_section_len.to_be_bytes())?;
    file.write_all(&path_len.to_be_bytes())?;
    file.write_all(&path_utf16)?;

    // --- PWV3 Section (detail waveform) ---
    file.write_all(PWV3_MAGIC)?;
    file.write_all(&PWV3_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&pwv3_section_len.to_be_bytes())?;
    file.write_all(&1u32.to_be_bytes())?; // unknown1 (always 1)
    file.write_all(&pwv3_entries.to_be_bytes())?; // len_entries
    file.write_all(&150u16.to_be_bytes())?; // entries_per_second (0x0096)
    file.write_all(&0u16.to_be_bytes())?; // unknown2
    file.write_all(&waveforms.detail)?;

    // --- PCOB Sections (cue points) ---
    // PCOB 1
    file.write_all(PCOB_MAGIC)?;
    file.write_all(&PCOB_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&PCOB_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&1u32.to_be_bytes())?;
    file.write_all(&0u32.to_be_bytes())?;
    file.write_all(&0xffffffffu32.to_be_bytes())?;

    // PCOB 2
    file.write_all(PCOB_MAGIC)?;
    file.write_all(&PCOB_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&PCOB_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&0u32.to_be_bytes())?;
    file.write_all(&0u32.to_be_bytes())?;
    file.write_all(&0xffffffffu32.to_be_bytes())?;

    // --- PCO2 Sections (extended cue points) ---
    // PCO2 1
    file.write_all(PCO2_MAGIC)?;
    file.write_all(&20u32.to_be_bytes())?; // len_header = 20
    file.write_all(&20u32.to_be_bytes())?; // len_tag = 20
    file.write_all(&1u32.to_be_bytes())?; // unknown
    file.write_all(&0u32.to_be_bytes())?;

    // PCO2 2
    file.write_all(PCO2_MAGIC)?;
    file.write_all(&20u32.to_be_bytes())?;
    file.write_all(&20u32.to_be_bytes())?;
    file.write_all(&0u32.to_be_bytes())?;
    file.write_all(&0u32.to_be_bytes())?;

    // --- PWV5 Section (color detail waveform) ---
    file.write_all(PWV5_MAGIC)?;
    file.write_all(&PWV5_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&pwv5_section_len.to_be_bytes())?;
    file.write_all(&2u32.to_be_bytes())?; // unknown1 (always 2 = bytes per entry)
    file.write_all(&pwv5_entries.to_be_bytes())?; // len_entries
    file.write_all(&150u16.to_be_bytes())?; // entries_per_second
    file.write_all(&0x0305u16.to_be_bytes())?; // unknown2 (observed value)
    file.write_all(&waveforms.color_detail)?;

    // --- PWV4 Section (color preview waveform) ---
    // 1200 columns × 6 bytes per entry = 7200 bytes data
    file.write_all(PWV4_MAGIC)?;
    file.write_all(&PWV4_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&pwv4_section_len.to_be_bytes())?;
    file.write_all(&PWV4_ENTRY_SIZE.to_be_bytes())?; // unknown1 (entry size = 6)
    file.write_all(&PWV4_ENTRY_COUNT.to_be_bytes())?; // len_entries (1200)
    file.write_all(&0u32.to_be_bytes())?; // unknown2 (0 in reference)
    // Write color preview data
    if waveforms.color_preview.len() == (PWV4_ENTRY_COUNT * PWV4_ENTRY_SIZE) as usize {
        file.write_all(&waveforms.color_preview)?;
    } else {
        // Fallback: write zeros (as in reference)
        file.write_all(&vec![0u8; (PWV4_ENTRY_COUNT * PWV4_ENTRY_SIZE) as usize])?;
    }

    Ok(())
}

/// Beat entry for PQTZ section
struct BeatEntry {
    beat_number: u16, // 1-4, position in measure
    tempo: u16,       // BPM * 100
    time: u32,        // milliseconds
}

/// Generate beat entries for a constant-tempo track
/// Only generates beats for tracks >= 4 seconds (enough for at least one bar at slow tempos)
fn generate_beat_entries(bpm: f32, duration_ms: u32) -> Vec<BeatEntry> {
    // Don't generate beatgrid for very short tracks (like sound effects)
    // Reference exports show 0 beats for 1-second samples
    if duration_ms < 4000 {
        log::debug!("Track too short for beatgrid: {}ms < 4000ms", duration_ms);
        return Vec::new();
    }

    let tempo = (bpm * 100.0) as u16;
    let beat_interval_ms = 60000.0 / bpm; // ms per beat

    let mut entries = Vec::new();
    let mut time_ms: f32 = 0.0;
    let mut beat_in_bar: u16 = 1;

    while (time_ms as u32) < duration_ms {
        entries.push(BeatEntry {
            beat_number: beat_in_bar,
            tempo,
            time: time_ms as u32,
        });

        time_ms += beat_interval_ms;
        beat_in_bar = if beat_in_bar >= 4 { 1 } else { beat_in_bar + 1 };
    }

    entries
}
