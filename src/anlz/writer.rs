//! ANLZ file writer implementation
//!
//! Writes ANLZ files with PPTH section for Phase 1.
//! Phase 2 will add real waveform and beatgrid data.

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
/// PMAI header size (28 bytes)
const PMAI_HEADER_SIZE: u32 = 28;
/// PPTH header size (16 bytes)
const PPTH_HEADER_SIZE: u32 = 16;

/// Write a .DAT analysis file
///
/// Phase 1: PMAI header + PPTH section (path to audio file)
/// Phase 2: Add beatgrid and waveform sections
pub fn write_dat_file(path: &Path, _track: &Track, _analysis: &AnalysisResult, audio_path: &str) -> Result<()> {
    log::debug!("Writing ANLZ .DAT file: {:?}", path);

    // Ensure parent directories exist (for hierarchical ANLZ structure)
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create ANLZ directory: {:?}", parent))?;
    }

    let mut file = File::create(path)
        .with_context(|| format!("Failed to create ANLZ .DAT file: {:?}", path))?;

    write_anlz_with_ppth(&mut file, audio_path)?;

    log::debug!("ANLZ .DAT file written with PPTH section");
    Ok(())
}

/// Write a .EXT analysis file
///
/// Phase 1: PMAI header + PPTH section
/// Phase 2: Add color waveform sections
pub fn write_ext_file(path: &Path, _track: &Track, _analysis: &AnalysisResult, audio_path: &str) -> Result<()> {
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

/// Encode a path string as UTF-16 big-endian
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
    result
}

/// Write an ANLZ file with PMAI header and PPTH section
fn write_anlz_with_ppth(file: &mut File, audio_path: &str) -> Result<()> {
    // Encode the path as UTF-16 big-endian
    let path_utf16 = encode_path_utf16_be(audio_path);
    let path_len = path_utf16.len() as u32;

    // Calculate section sizes
    let ppth_section_len = PPTH_HEADER_SIZE + path_len;
    let total_file_size = PMAI_HEADER_SIZE + ppth_section_len;

    // --- PMAI Header (28 bytes) ---
    file.write_all(PMAI_MAGIC)?;
    file.write_all(&PMAI_HEADER_SIZE.to_be_bytes())?;
    file.write_all(&total_file_size.to_be_bytes())?;
    // Remaining header bytes (16 bytes) - match reference export values
    // Offset 12: 0x00000001
    file.write_all(&1u32.to_be_bytes())?;
    // Offset 16: 0x00010000
    file.write_all(&0x00010000u32.to_be_bytes())?;
    // Offset 20: 0x00010000
    file.write_all(&0x00010000u32.to_be_bytes())?;
    // Offset 24: 0x00000000
    file.write_all(&0u32.to_be_bytes())?;

    // --- PPTH Section ---
    // PPTH magic (4 bytes)
    file.write_all(PPTH_MAGIC)?;
    // Header length (4 bytes, big-endian) - always 16
    file.write_all(&PPTH_HEADER_SIZE.to_be_bytes())?;
    // Section length (4 bytes, big-endian) - header + path data
    file.write_all(&ppth_section_len.to_be_bytes())?;
    // Unknown field (4 bytes) - seems to be path_len (string length in bytes)
    file.write_all(&path_len.to_be_bytes())?;
    // Path string (UTF-16 big-endian)
    file.write_all(&path_utf16)?;

    Ok(())
}

// Phase 2: Implement these functions
/*
fn write_beat_grid_tag(file: &mut File, beatgrid: &BeatGrid) -> Result<()> {
    // PQTZ tag
}

fn write_waveform_preview_tag(file: &mut File, waveform: &[u8]) -> Result<()> {
    // PWV2 tag (tiny preview) or PWAV tag
}

fn write_waveform_detail_tag(file: &mut File, waveform: &[u8]) -> Result<()> {
    // PWV3 tag
}

fn write_color_waveform_preview_tag(file: &mut File, waveform: &[u8]) -> Result<()> {
    // PWV4 tag
}

fn write_color_waveform_detail_tag(file: &mut File, waveform: &[u8]) -> Result<()> {
    // PWV5 tag
}
*/
