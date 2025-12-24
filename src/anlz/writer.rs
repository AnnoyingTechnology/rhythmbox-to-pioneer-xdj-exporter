//! ANLZ file writer implementation
//!
//! Writes ANLZ files with PPTH and PQTZ (beatgrid) sections.

use crate::analysis::AnalysisResult;
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
/// PVBR total size (20 bytes - header + 4 bytes of data)
const PVBR_TOTAL_SIZE: u32 = 20;

/// Write a .DAT analysis file
///
/// Contains PMAI header + PPTH section + PQTZ beatgrid (if BPM available)
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

    // Get BPM from track metadata only (not from analysis)
    // Analysis BPM causes issues - need to investigate
    let bpm = track.bpm;

    write_anlz_with_ppth_and_pqtz(&mut file, audio_path, bpm, track.duration_ms)?;

    log::debug!("ANLZ .DAT file written with PPTH and PQTZ sections");
    Ok(())
}

/// Write a .EXT analysis file
///
/// Phase 1: PMAI header + PPTH section
/// Phase 2: Add color waveform sections
pub fn write_ext_file(
    path: &Path,
    _track: &Track,
    _analysis: &AnalysisResult,
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

    write_anlz_with_ppth(&mut file, audio_path)?;

    log::debug!("ANLZ .EXT file written with PPTH section");
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

/// Write an ANLZ file with PMAI header, PPTH section, PVBR, and optional PQTZ beatgrid
/// Section order must be: PPTH → PVBR → PQTZ (matching Rekordbox reference)
fn write_anlz_with_ppth_and_pqtz(
    file: &mut File,
    audio_path: &str,
    bpm: Option<f32>,
    duration_ms: u32,
) -> Result<()> {
    // Encode the path as UTF-16 big-endian (includes NUL terminator)
    let path_utf16 = encode_path_utf16_be(audio_path);
    let path_len = path_utf16.len() as u32;

    // Calculate PPTH section size
    // Reference shows: len_tag = header (16) + path data (no separate path_len field in len_tag)
    // But we need to write path_len as a 4-byte field before the path
    // Total section: header(12) + len_path(4) + path_data = header_part(16) + path_data
    let ppth_section_len = PPTH_HEADER_SIZE + path_len;

    // Calculate PQTZ section if BPM is available
    let (pqtz_section_len, beat_entries) = if let Some(bpm_value) = bpm {
        let entries = generate_beat_entries(bpm_value, duration_ms);
        let len = PQTZ_HEADER_SIZE + (entries.len() as u32 * PQTZ_BEAT_ENTRY_SIZE);
        (len, Some(entries))
    } else {
        (0, None)
    };

    // Total file size: PMAI + PPTH + PVBR + PQTZ
    // Section order MUST be: PPTH first, then PVBR, then PQTZ
    let total_file_size = PMAI_HEADER_SIZE + ppth_section_len + PVBR_TOTAL_SIZE + pqtz_section_len;

    // --- PMAI Header (28 bytes) ---
    file.write_all(PMAI_MAGIC)?;
    file.write_all(&PMAI_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&total_file_size.to_be_bytes())?;
    // Remaining header bytes (16 bytes) - match reference export values
    file.write_all(&1u32.to_be_bytes())?; // Offset 12: 0x00000001
    file.write_all(&0x00010000u32.to_be_bytes())?; // Offset 16: 0x00010000
    file.write_all(&0x00010000u32.to_be_bytes())?; // Offset 20: 0x00010000
    file.write_all(&0u32.to_be_bytes())?; // Offset 24: 0x00000000

    // --- PPTH Section (MUST come first after header per reference) ---
    file.write_all(PPTH_MAGIC)?;
    file.write_all(&PPTH_HEADER_SIZE.to_be_bytes())?; // len_header = 16
    file.write_all(&ppth_section_len.to_be_bytes())?; // len_tag (total section size)
    file.write_all(&path_len.to_be_bytes())?; // len_path (just the path bytes)
    file.write_all(&path_utf16)?;

    // --- PVBR Section (VBR index - comes after PPTH) ---
    file.write_all(PVBR_MAGIC)?;
    file.write_all(&PVBR_HEADER_SIZE.to_be_bytes())?; // len_header
    file.write_all(&PVBR_TOTAL_SIZE.to_be_bytes())?; // len_tag
    file.write_all(&0u32.to_be_bytes())?; // unknown data (4 bytes)

    // --- PQTZ Section (beatgrid - comes last) ---
    if let Some(entries) = beat_entries {
        let num_beats = entries.len() as u32;
        let pqtz_total_len = PQTZ_HEADER_SIZE + (num_beats * PQTZ_BEAT_ENTRY_SIZE);

        file.write_all(PQTZ_MAGIC)?;
        file.write_all(&PQTZ_HEADER_SIZE.to_be_bytes())?; // len_header
        file.write_all(&pqtz_total_len.to_be_bytes())?; // len_tag
        file.write_all(&0u32.to_be_bytes())?; // unknown1
        file.write_all(&0x00800000u32.to_be_bytes())?; // unknown2 (always 0x00800000)
        file.write_all(&num_beats.to_be_bytes())?; // len_beats

        // Write beat entries
        for entry in entries {
            file.write_all(&entry.beat_number.to_be_bytes())?;
            file.write_all(&entry.tempo.to_be_bytes())?;
            file.write_all(&entry.time.to_be_bytes())?;
        }

        log::debug!("PQTZ beatgrid written: {} beats", num_beats);
    }

    Ok(())
}

/// Write an ANLZ file with PMAI header and PPTH section only (for .EXT files)
fn write_anlz_with_ppth(file: &mut File, audio_path: &str) -> Result<()> {
    write_anlz_with_ppth_and_pqtz(file, audio_path, None, 0)
}

/// Beat entry for PQTZ section
struct BeatEntry {
    beat_number: u16, // 1-4, position in measure
    tempo: u16,       // BPM * 100
    time: u32,        // milliseconds
}

/// Generate beat entries for a constant-tempo track
fn generate_beat_entries(bpm: f32, duration_ms: u32) -> Vec<BeatEntry> {
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

// Phase 2 TODO: Waveform sections (PWAV, PWV2, PWV3, PWV4, PWV5)
