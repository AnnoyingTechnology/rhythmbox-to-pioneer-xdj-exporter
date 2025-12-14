//! PDB file writer implementation
//!
//! Phase 1: Minimal implementation with simplified single-page tables
//! Phase 2: Full multi-page support, all metadata fields, proper indexing

use crate::analysis::AnalysisResult;
use crate::model::{Playlist, Track};
use super::types::{TableType, FileType};
use super::strings::encode_device_sql;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Track metadata for PDB export
pub struct TrackMetadata {
    /// The track data
    pub track: Track,

    /// Relative path to music file on USB
    pub file_path: PathBuf,

    /// Relative path to ANLZ file on USB
    pub anlz_path: PathBuf,

    /// Analysis results for this track
    pub analysis: AnalysisResult,
}

// PDB constants
const PAGE_SIZE: u32 = 4096; // Standard 4KB pages
const HEAP_START: usize = 0x28; // Data starts at byte 40

/// Write a complete PDB file
///
/// Phase 1: Simplified implementation with minimal metadata
pub fn write_pdb(
    path: &Path,
    tracks: &[TrackMetadata],
    playlists: &[Playlist],
) -> Result<()> {
    log::info!("Writing PDB file: {:?}", path);
    log::info!("  Tracks: {}", tracks.len());
    log::info!("  Playlists: {}", playlists.len());

    let mut file = File::create(path)
        .with_context(|| format!("Failed to create PDB file: {:?}", path))?;

    // Build entity tables (deduplicate artists, albums, etc.)
    let entities = build_entity_tables(tracks)?;

    // Calculate table count
    let num_tables = 5; // tracks, artists, albums, playlist_tree, playlist_entries

    // Write file header
    write_file_header(&mut file, num_tables)?;

    // Write tables (each gets one page for Phase 1)
    // Page 0 is the header, data pages start at page 1
    let mut current_page = 1;

    // Table 1: Artists
    let artists_page = current_page;
    write_artists_table(&mut file, &entities.artists)?;
    current_page += 1;

    // Table 2: Albums
    let albums_page = current_page;
    write_albums_table(&mut file, &entities.albums)?;
    current_page += 1;

    // Table 3: Tracks
    let tracks_page = current_page;
    write_tracks_table(&mut file, tracks, &entities)?;
    current_page += 1;

    // Table 4: Playlist tree
    let playlist_tree_page = current_page;
    write_playlist_tree_table(&mut file, playlists)?;
    current_page += 1;

    // Table 5: Playlist entries
    let _playlist_entries_page = current_page;
    write_playlist_entries_table(&mut file, playlists, &entities.track_ids)?;

    // Go back and write table pointers in header
    file.seek(SeekFrom::Start(0x1c))?;
    write_table_pointer(&mut file, TableType::Artists as u32, artists_page, artists_page)?;
    write_table_pointer(&mut file, TableType::Albums as u32, albums_page, albums_page)?;
    write_table_pointer(&mut file, TableType::Tracks as u32, tracks_page, tracks_page)?;
    write_table_pointer(&mut file, TableType::PlaylistTree as u32, playlist_tree_page, playlist_tree_page)?;
    write_table_pointer(&mut file, TableType::PlaylistEntries as u32, current_page, current_page)?;

    log::info!("PDB file written successfully");
    Ok(())
}

/// Entity tables (deduplicated)
struct EntityTables {
    artists: Vec<String>,
    albums: Vec<String>,
    artist_map: HashMap<String, u32>,
    album_map: HashMap<String, u32>,
    track_ids: HashMap<String, u32>, // Maps Track.id to PDB row ID
}

/// Build deduplicated entity tables from tracks
fn build_entity_tables(tracks: &[TrackMetadata]) -> Result<EntityTables> {
    let mut artists = Vec::new();
    let mut albums = Vec::new();
    let mut artist_map = HashMap::new();
    let mut album_map = HashMap::new();
    let mut track_ids = HashMap::new();

    for (track_idx, track_meta) in tracks.iter().enumerate() {
        let track = &track_meta.track;

        // Track ID (1-based)
        track_ids.insert(track.id.clone(), (track_idx + 1) as u32);

        // Artist (deduplicate)
        if !artist_map.contains_key(&track.artist) {
            let artist_id = (artists.len() + 1) as u32;
            artist_map.insert(track.artist.clone(), artist_id);
            artists.push(track.artist.clone());
        }

        // Album (deduplicate)
        if !album_map.contains_key(&track.album) {
            let album_id = (albums.len() + 1) as u32;
            album_map.insert(track.album.clone(), album_id);
            albums.push(track.album.clone());
        }
    }

    Ok(EntityTables {
        artists,
        albums,
        artist_map,
        album_map,
        track_ids,
    })
}

/// Write PDB file header
fn write_file_header(file: &mut File, num_tables: u32) -> Result<()> {
    // Magic (4 bytes of zeros)
    file.write_all(&[0u8; 4])?;

    // len_page (4 bytes)
    file.write_all(&PAGE_SIZE.to_le_bytes())?;

    // num_tables (4 bytes)
    file.write_all(&num_tables.to_le_bytes())?;

    // nextu (4 bytes) - unclear purpose, use 0
    file.write_all(&[0u8; 4])?;

    // unknown (4 bytes at offset 0x10)
    file.write_all(&[0u8; 4])?;

    // sequence (4 bytes at offset 0x14)
    file.write_all(&1u32.to_le_bytes())?; // Version 1

    // unknown (4 bytes at offset 0x18)
    file.write_all(&[0u8; 4])?;

    // Table pointers start at 0x1c - we'll write them later
    // Each pointer is 16 bytes (4 x u32), reserve space for num_tables pointers
    let pointer_space = num_tables * 16;
    file.write_all(&vec![0u8; pointer_space as usize])?;

    // CRITICAL: Pad header to full page size (4096 bytes)
    // XDJ expects page 0 to start at byte 4096!
    let header_size = 0x1c + pointer_space;
    let padding_needed = PAGE_SIZE - header_size;
    file.write_all(&vec![0u8; padding_needed as usize])?;

    Ok(())
}

/// Write a table pointer in the header
fn write_table_pointer(file: &mut File, table_type: u32, first_page: u32, last_page: u32) -> Result<()> {
    file.write_all(&table_type.to_le_bytes())?; // type
    file.write_all(&0u32.to_le_bytes())?; // empty_candidate
    file.write_all(&first_page.to_le_bytes())?; // first_page
    file.write_all(&last_page.to_le_bytes())?; // last_page
    Ok(())
}

/// Write page header
fn write_page_header(file: &mut File, page_index: u32, table_type: u32, num_rows: u32) -> Result<()> {
    // Bytes 0x00-0x03: padding
    file.write_all(&[0u8; 4])?;

    // Bytes 0x04-0x07: page_index
    file.write_all(&page_index.to_le_bytes())?;

    // Bytes 0x08-0x0b: type
    file.write_all(&table_type.to_le_bytes())?;

    // Bytes 0x0c-0x0f: next_page (0 = last page)
    file.write_all(&0u32.to_le_bytes())?;

    // Bytes 0x10-0x13: version
    file.write_all(&1u32.to_le_bytes())?;

    // Bytes 0x14-0x17: unknown2
    file.write_all(&[0u8; 4])?;

    // Bytes 0x18-0x1b: num_rows_small, num_rows_valid (2 bytes!), page_flags
    if num_rows < 256 {
        file.write_all(&[num_rows as u8])?; // 0x18: num_rows_small
        let num_rows_valid = (num_rows * 32) as u16; // num_rows * 0x20
        file.write_all(&num_rows_valid.to_le_bytes())?; // 0x19-0x1a: num_rows_valid (2 bytes)
        file.write_all(&[0x24u8])?; // 0x1b: page_flags (0x24 = data page)
    } else {
        file.write_all(&[0u8])?; // 0x18: num_rows_small = 0
        let num_rows_valid = (num_rows * 32) as u16;
        file.write_all(&num_rows_valid.to_le_bytes())?; // 0x19-0x1a: num_rows_valid (2 bytes)
        file.write_all(&[0x24u8])?; // 0x1b: page_flags
    }

    // Bytes 0x1c-0x1d: free_size
    file.write_all(&[0u8; 2])?;

    // Bytes 0x1e-0x1f: used_size
    file.write_all(&[0u8; 2])?;

    // Bytes 0x20-0x21: u5
    file.write_all(&[0u8; 2])?;

    // Bytes 0x22-0x23: num_rows_large
    if num_rows >= 256 {
        file.write_all(&(num_rows as u16).to_le_bytes())?;
    } else {
        file.write_all(&[0u8; 2])?;
    }

    // Bytes 0x24-0x25: u6
    file.write_all(&[0u8; 2])?;

    // Bytes 0x26-0x27: u7
    file.write_all(&[0u8; 2])?;

    // Total: 40 bytes (0x28), heap starts here
    Ok(())
}

/// Write artists table
fn write_artists_table(file: &mut File, artists: &[String]) -> Result<()> {
    log::debug!("Writing artists table: {} artists", artists.len());

    let page_start = file.stream_position()?;
    write_page_header(file, 1, TableType::Artists as u32, artists.len() as u32)?;

    // Artist row structure (rekordcrate version with OffsetArrayContainer):
    // Fixed header (8 bytes):
    // - u16: subtype (0x60 for nearby name using u8 offsets)
    // - u16: index_shift (0)
    // - u32: artist_id
    // Offset array (2 x u8):
    // - u8: offset[0] (unknown purpose, use 0)
    // - u8: offset[1] (offset to name string from row start)
    // Then: DeviceSQL string data at the calculated offset

    let mut heap = Vec::new();
    let mut row_offsets = Vec::new();

    for (idx, artist) in artists.iter().enumerate() {
        let row_start = heap.len();

        // Fixed header (8 bytes)
        heap.extend_from_slice(&0x60u16.to_le_bytes()); // subtype (0x60 = nearby name, u8 offsets)
        heap.extend_from_slice(&0u16.to_le_bytes()); // index_shift
        heap.extend_from_slice(&((idx + 1) as u32).to_le_bytes()); // ID (1-based)

        // Offset array (2 bytes)
        heap.push(0x03u8); // offset[0] (constant value 0x03 for u8 offset arrays)

        // Calculate offset to name string from row start
        let name_offset = 10u8; // String starts at byte 10 (after 8-byte header + 2-byte offset array)
        heap.push(name_offset); // offset[1] (name offset from row start)

        // Encode and append string data at the offset
        let encoded_name = encode_device_sql(artist);
        heap.extend_from_slice(&encoded_name);

        row_offsets.push((HEAP_START + row_start) as u16);
    }

    // Write heap
    file.write_all(&heap)?;

    // Pad to near end of page for row index
    let current_pos = file.stream_position()? - page_start;
    let index_space_needed = row_offsets.len() * 2 + (row_offsets.len() + 15) / 16 * 2;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    // Write row index (offsets from end)
    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    // Write row presence flags
    let num_groups = (artists.len() + 15) / 16;
    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(artists.len());
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
    }

    // Ensure we're at exactly page boundary
    let final_pos = file.stream_position()?;
    let expected_pos = page_start + PAGE_SIZE as u64;
    if final_pos < expected_pos {
        file.write_all(&vec![0u8; (expected_pos - final_pos) as usize])?;
    }

    Ok(())
}

/// Write albums table (different structure from artists!)
fn write_albums_table(file: &mut File, albums: &[String]) -> Result<()> {
    log::debug!("Writing albums table: {} albums", albums.len());

    let page_start = file.stream_position()?;
    write_page_header(file, 2, TableType::Albums as u32, albums.len() as u32)?;

    // Album row structure (rekordcrate version with OffsetArrayContainer):
    // Fixed header (20 bytes):
    // - u16: subtype (0x80 for nearby name using u8 offsets)
    // - u16: index_shift (0)
    // - u32: unknown2 (0)
    // - u32: artist_id (0 for now, no artist linkage)
    // - u32: album_id
    // - u32: unknown3 (0)
    // Offset array (2 x u8):
    // - u8: offset[0] (unknown purpose, use 0)
    // - u8: offset[1] (offset to name string from row start)
    // Then: DeviceSQL string data at the calculated offset

    let mut heap = Vec::new();
    let mut row_offsets = Vec::new();

    for (idx, album) in albums.iter().enumerate() {
        let row_start = heap.len();

        // Fixed header (20 bytes)
        heap.extend_from_slice(&0x80u16.to_le_bytes()); // subtype (0x80 = nearby name, u8 offsets)
        heap.extend_from_slice(&0u16.to_le_bytes()); // index_shift
        heap.extend_from_slice(&0u32.to_le_bytes()); // unknown2
        heap.extend_from_slice(&0u32.to_le_bytes()); // artist_id (0 = no artist link)
        heap.extend_from_slice(&((idx + 1) as u32).to_le_bytes()); // album_id (1-based)
        heap.extend_from_slice(&0u32.to_le_bytes()); // unknown3

        // Offset array (2 bytes)
        heap.push(0x03u8); // offset[0] (constant value 0x03 for u8 offset arrays)

        // Calculate offset to name string from row start
        let name_offset = 22u8; // String starts at byte 22 (after 20-byte header + 2-byte offset array)
        heap.push(name_offset); // offset[1] (name offset from row start)

        // Encode and append string data at the offset
        let encoded_name = encode_device_sql(album);
        heap.extend_from_slice(&encoded_name);

        row_offsets.push((HEAP_START + row_start) as u16);
    }

    file.write_all(&heap)?;

    let current_pos = file.stream_position()? - page_start;
    let index_space_needed = row_offsets.len() * 2 + (row_offsets.len() + 15) / 16 * 2;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    let num_groups = (albums.len() + 15) / 16;
    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(albums.len());
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
    }

    let final_pos = file.stream_position()?;
    let expected_pos = page_start + PAGE_SIZE as u64;
    if final_pos < expected_pos {
        file.write_all(&vec![0u8; (expected_pos - final_pos) as usize])?;
    }

    Ok(())
}

/// Write tracks table
///
/// Track row structure per Deep Symmetry documentation:
/// Header (0x00-0x5D = 94 bytes):
///   0x00-0x01: subtype (0x0024)
///   0x02-0x03: index_shift
///   0x04-0x07: bitmask
///   0x08-0x0B: sample_rate
///   0x0C-0x0F: composer_id
///   0x10-0x13: file_size
///   0x14-0x17: u2
///   0x18-0x19: u3
///   0x1A-0x1B: u4
///   0x1C-0x1F: artwork_id
///   0x20-0x23: key_id
///   0x24-0x27: original_artist_id
///   0x28-0x2B: label_id
///   0x2C-0x2F: remixer_id
///   0x30-0x33: bitrate
///   0x34-0x37: track_number (u32!)
///   0x38-0x3B: tempo (BPM * 100)
///   0x3C-0x3F: genre_id
///   0x40-0x43: album_id
///   0x44-0x47: artist_id
///   0x48-0x4B: id (track ID)
///   0x4C-0x4D: disc_number
///   0x4E-0x4F: play_count
///   0x50-0x51: year
///   0x52-0x53: sample_depth
///   0x54-0x55: duration (seconds)
///   0x56-0x57: u5 (always 0x0029)
///   0x58: color_id
///   0x59: rating
///   0x5A-0x5B: file_type
///   0x5C-0x5D: u7 (always 0x0003, precedes string offsets)
/// String offsets (0x5E onwards): 21 x u16 offsets
/// String data follows
fn write_tracks_table(file: &mut File, tracks: &[TrackMetadata], entities: &EntityTables) -> Result<()> {
    log::debug!("Writing tracks table: {} tracks", tracks.len());

    let page_start = file.stream_position()?;
    write_page_header(file, 3, TableType::Tracks as u32, tracks.len() as u32)?;

    let mut heap = Vec::new();
    let mut row_offsets = Vec::new();

    for (idx, track_meta) in tracks.iter().enumerate() {
        let row_start = heap.len();
        let track = &track_meta.track;

        // Get IDs
        let artist_id = *entities.artist_map.get(&track.artist).unwrap_or(&0);
        let album_id = *entities.album_map.get(&track.album).unwrap_or(&0);
        let track_id = (idx + 1) as u32;

        // File type
        let file_type = track.file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| FileType::from_extension(e) as u16)
            .unwrap_or(FileType::Unknown as u16);

        // --- Header (94 bytes: 0x00-0x5D) ---

        // 0x00-0x01: subtype (always 0x0024 for tracks)
        heap.extend_from_slice(&0x0024u16.to_le_bytes());

        // 0x02-0x03: index_shift
        heap.extend_from_slice(&0u16.to_le_bytes());

        // 0x04-0x07: bitmask (unknown)
        heap.extend_from_slice(&0u32.to_le_bytes());

        // 0x08-0x0B: sample_rate
        heap.extend_from_slice(&44100u32.to_le_bytes());

        // 0x0C-0x0F: composer_id
        heap.extend_from_slice(&0u32.to_le_bytes());

        // 0x10-0x13: file_size
        heap.extend_from_slice(&(track.file_size as u32).to_le_bytes());

        // 0x14-0x17: u2 (unknown)
        heap.extend_from_slice(&0u32.to_le_bytes());

        // 0x18-0x19: u3 (unknown, typically 19048)
        heap.extend_from_slice(&0u16.to_le_bytes());

        // 0x1A-0x1B: u4 (unknown, typically 30967)
        heap.extend_from_slice(&0u16.to_le_bytes());

        // 0x1C-0x1F: artwork_id
        heap.extend_from_slice(&0u32.to_le_bytes());

        // 0x20-0x23: key_id
        heap.extend_from_slice(&0u32.to_le_bytes());

        // 0x24-0x27: original_artist_id
        heap.extend_from_slice(&0u32.to_le_bytes());

        // 0x28-0x2B: label_id
        heap.extend_from_slice(&0u32.to_le_bytes());

        // 0x2C-0x2F: remixer_id
        heap.extend_from_slice(&0u32.to_le_bytes());

        // 0x30-0x33: bitrate
        heap.extend_from_slice(&320u32.to_le_bytes());

        // 0x34-0x37: track_number (u32!)
        let track_number = track.track_number.unwrap_or(0) as u32;
        heap.extend_from_slice(&track_number.to_le_bytes());

        // 0x38-0x3B: tempo (BPM * 100)
        let tempo = track.bpm.map(|bpm| (bpm * 100.0) as u32).unwrap_or(0);
        heap.extend_from_slice(&tempo.to_le_bytes());

        // 0x3C-0x3F: genre_id
        heap.extend_from_slice(&0u32.to_le_bytes());

        // 0x40-0x43: album_id
        heap.extend_from_slice(&album_id.to_le_bytes());

        // 0x44-0x47: artist_id
        heap.extend_from_slice(&artist_id.to_le_bytes());

        // 0x48-0x4B: id (track ID)
        heap.extend_from_slice(&track_id.to_le_bytes());

        // 0x4C-0x4D: disc_number
        heap.extend_from_slice(&1u16.to_le_bytes());

        // 0x4E-0x4F: play_count
        heap.extend_from_slice(&0u16.to_le_bytes());

        // 0x50-0x51: year
        let year = track.year.unwrap_or(0) as u16;
        heap.extend_from_slice(&year.to_le_bytes());

        // 0x52-0x53: sample_depth
        heap.extend_from_slice(&16u16.to_le_bytes());

        // 0x54-0x55: duration (seconds)
        let duration_secs = (track.duration_ms / 1000) as u16;
        heap.extend_from_slice(&duration_secs.to_le_bytes());

        // 0x56-0x57: u5 (always 0x0029)
        heap.extend_from_slice(&0x0029u16.to_le_bytes());

        // 0x58: color_id
        heap.push(0u8);

        // 0x59: rating
        heap.push(0u8);

        // 0x5A-0x5B: file_type
        heap.extend_from_slice(&file_type.to_le_bytes());

        // 0x5C-0x5D: u7 (always 0x0003, precedes string offsets)
        heap.extend_from_slice(&0x0003u16.to_le_bytes());

        // Verify header size is 94 bytes (0x5E)
        assert_eq!(heap.len() - row_start, 0x5E, "Track header should be 94 bytes");

        // --- String offset array (21 x u16 = 42 bytes) ---
        // Offsets are relative to row start
        let string_data_start = 0x5E + (21 * 2); // After header + offset array = 136 bytes (0x88)

        // Build strings and calculate offsets
        // String indices per Deep Symmetry documentation:
        // 0: isrc, 1: lyricist, 2-4: unknown, 5: message, 6: publish_track_info,
        // 7: autoload_hotcues, 8-9: unknown, 10: date_added, 11: release_date,
        // 12: mix_name, 13: unknown, 14: analyze_path, 15: analyze_date,
        // 16: comment, 17: title, 18: unknown, 19: filename, 20: file_path
        let mut string_data = Vec::new();

        // CRITICAL: Start with an empty DeviceSQL string (0x03 = ShortASCII, length 0)
        // All unused string offsets will point to this empty string at the start
        let empty_string_offset = string_data_start as u16;
        string_data.push(0x03); // Empty DeviceSQL string: ShortASCII with length ((0+1)<<1)|1 = 3

        // Initialize all offsets to point to the empty string
        let mut string_offsets: Vec<u16> = vec![empty_string_offset; 21];

        // String 14: analyze_path (CRITICAL)
        let anlz_path_str = track_meta.anlz_path.to_string_lossy();
        if !anlz_path_str.is_empty() {
            string_offsets[14] = (string_data_start + string_data.len()) as u16;
            string_data.extend_from_slice(&encode_device_sql(&anlz_path_str));
        }

        // String 17: title (CRITICAL)
        if !track.title.is_empty() {
            string_offsets[17] = (string_data_start + string_data.len()) as u16;
            string_data.extend_from_slice(&encode_device_sql(&track.title));
        }

        // String 19: filename
        let filename = track_meta.file_path.file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !filename.is_empty() {
            string_offsets[19] = (string_data_start + string_data.len()) as u16;
            string_data.extend_from_slice(&encode_device_sql(&filename));
        }

        // String 20: file_path (CRITICAL)
        let file_path_str = track_meta.file_path.to_string_lossy();
        if !file_path_str.is_empty() {
            string_offsets[20] = (string_data_start + string_data.len()) as u16;
            string_data.extend_from_slice(&encode_device_sql(&file_path_str));
        }

        // Write string offset array
        for offset in &string_offsets {
            heap.extend_from_slice(&offset.to_le_bytes());
        }

        // Verify offset array position
        assert_eq!(heap.len() - row_start, 0x88, "String offset array should end at 0x88");

        // Write string data
        heap.extend_from_slice(&string_data);

        row_offsets.push((HEAP_START + row_start) as u16);
    }

    file.write_all(&heap)?;

    let current_pos = file.stream_position()? - page_start;
    let index_space_needed = row_offsets.len() * 2 + (row_offsets.len() + 15) / 16 * 2;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    let num_groups = (tracks.len() + 15) / 16;
    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(tracks.len());
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
    }

    let final_pos = file.stream_position()?;
    let expected_pos = page_start + PAGE_SIZE as u64;
    if final_pos < expected_pos {
        file.write_all(&vec![0u8; (expected_pos - final_pos) as usize])?;
    }

    Ok(())
}

/// Write playlist tree table
fn write_playlist_tree_table(file: &mut File, playlists: &[Playlist]) -> Result<()> {
    log::debug!("Writing playlist tree table: {} playlists", playlists.len());

    let page_start = file.stream_position()?;
    write_page_header(file, 4, TableType::PlaylistTree as u32, playlists.len() as u32)?;

    // PlaylistTreeNode row structure (inline strings, NOT offset-based!):
    // - u32: parent_id (0 = root)
    // - u32: unknown (0)
    // - u32: sort_order
    // - u32: id (playlist ID)
    // - u32: node_is_folder (0 = playlist, non-zero = folder)
    // - DeviceSQLString: name (INLINE, not offset-based!)

    let mut heap = Vec::new();
    let mut row_offsets = Vec::new();

    for (idx, playlist) in playlists.iter().enumerate() {
        let row_start = heap.len();

        // Fixed fields (20 bytes)
        heap.extend_from_slice(&0u32.to_le_bytes()); // parent_id (0 = root)
        heap.extend_from_slice(&0u32.to_le_bytes()); // unknown
        heap.extend_from_slice(&((idx + 1) as u32).to_le_bytes()); // sort_order (use idx as order)
        heap.extend_from_slice(&((idx + 1) as u32).to_le_bytes()); // playlist ID (1-based)
        heap.extend_from_slice(&0u32.to_le_bytes()); // node_is_folder (0 = playlist, not folder)

        // Encode and append string data INLINE (not offset-based!)
        let encoded_name = encode_device_sql(&playlist.name);
        heap.extend_from_slice(&encoded_name);

        row_offsets.push((HEAP_START + row_start) as u16);
    }

    file.write_all(&heap)?;

    let current_pos = file.stream_position()? - page_start;
    let index_space_needed = row_offsets.len() * 2 + (row_offsets.len() + 15) / 16 * 2;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    let num_groups = (playlists.len() + 15) / 16;
    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(playlists.len());
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
    }

    let final_pos = file.stream_position()?;
    let expected_pos = page_start + PAGE_SIZE as u64;
    if final_pos < expected_pos {
        file.write_all(&vec![0u8; (expected_pos - final_pos) as usize])?;
    }

    Ok(())
}

/// Write playlist entries table
fn write_playlist_entries_table(
    file: &mut File,
    playlists: &[Playlist],
    track_ids: &HashMap<String, u32>,
) -> Result<()> {
    // Count total entries
    let total_entries: usize = playlists.iter().map(|p| p.entries.len()).sum();

    log::debug!("Writing playlist entries table: {} entries", total_entries);

    let page_start = file.stream_position()?;
    write_page_header(file, 5, TableType::PlaylistEntries as u32, total_entries as u32)?;

    let mut heap = Vec::new();
    let mut row_offsets = Vec::new();

    for (playlist_idx, playlist) in playlists.iter().enumerate() {
        let playlist_id = (playlist_idx + 1) as u32;

        for entry in &playlist.entries {
            let row_start = heap.len();

            // PlaylistEntry row structure (simple, no subtype/index_shift):
            // - u32: entry_index (position in playlist)
            // - u32: track_id
            // - u32: playlist_id

            // Position in playlist (entry_index)
            heap.extend_from_slice(&entry.position.to_le_bytes());

            // Track ID reference
            let track_id = track_ids.get(&entry.track_id).unwrap_or(&0);
            heap.extend_from_slice(&track_id.to_le_bytes());

            // Playlist ID reference
            heap.extend_from_slice(&playlist_id.to_le_bytes());

            row_offsets.push((HEAP_START + row_start) as u16);
        }
    }

    file.write_all(&heap)?;

    let current_pos = file.stream_position()? - page_start;
    let index_space_needed = row_offsets.len() * 2 + (row_offsets.len() + 15) / 16 * 2;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    let num_groups = (total_entries + 15) / 16;
    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(total_entries);
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
    }

    let final_pos = file.stream_position()?;
    let expected_pos = page_start + PAGE_SIZE as u64;
    if final_pos < expected_pos {
        file.write_all(&vec![0u8; (expected_pos - final_pos) as usize])?;
    }

    Ok(())
}
