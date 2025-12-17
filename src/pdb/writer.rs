//! PDB file writer implementation
//!
//! Phase 1: Minimal implementation with simplified single-page tables
//! Phase 2: Full multi-page support, all metadata fields, proper indexing

use crate::analysis::AnalysisResult;
use crate::model::{Playlist, Track};
use super::types::{TableType, FileType};
use super::strings::{encode_device_sql, encode_device_sql_utf16_annotated};
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

// Table write order (matches rekordbox exports)
const TABLE_SEQUENCE: [TableType; 20] = [
    TableType::Tracks,
    TableType::Genres,
    TableType::Artists,
    TableType::Albums,
    TableType::Labels,
    TableType::Keys,
    TableType::Colors,
    TableType::PlaylistTree,
    TableType::PlaylistEntries,
    TableType::Unknown09,
    TableType::Unknown0A,
    TableType::Unknown0B,
    TableType::Unknown0C,
    TableType::Artwork,
    TableType::Unknown0E,
    TableType::Unknown0F,
    TableType::Columns,
    TableType::HistoryPlaylists,
    TableType::HistoryEntries,
    TableType::History,
];

/// Write a complete PDB file
///
/// Phase 1: Simplified implementation with minimal metadata
/// All Rekordbox table types are present; unimplemented ones are empty placeholders
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
    log::info!(
        "  Artists: {}, Albums: {}, Genres: {}",
        entities.artists.len(),
        entities.albums.len(),
        entities.genres.len()
    );

    // Rekordbox exports include 20 table pointers (0x00-0x13)
    let num_tables = TABLE_SEQUENCE.len() as u32;

    // Write file header
    write_file_header(&mut file, num_tables)?;

    // Write tables - each table has: header page, data page, empty candidate page
    // Page 0 is the file header, table pages start at page 1
    let mut current_page = 1u32;
    // Track (table_type, first_page, last_page, empty_candidate) for each table
    let mut table_pages: Vec<(TableType, u32, u32, u32)> = Vec::with_capacity(TABLE_SEQUENCE.len());

    for table_type in TABLE_SEQUENCE {
        let header_page = current_page;
        let data_page = current_page + 1;
        let empty_page = current_page + 2;  // Empty candidate page after data

        match table_type {
            TableType::Tracks => {
                // Header page (no rows, points to data)
                write_page_header(
                    &mut file,
                    header_page,
                    table_type as u32,
                    data_page,
                    0,            // num_rows_small
                    0x1fff,       // num_rows_large sentinel
                    0,            // unknown3
                    0,            // unknown4
                    0x64,         // page_flags (header)
                    0x3e,         // unknown1 (matches reference)
                    0,            // unknown2
                    0x1fff,       // unknown5 sentinel
                    0x03ec,       // unknown6
                    0x0001,       // unknown7 (tracks header)
                )?;
                // Pad header page to full size
                let padding = PAGE_SIZE as usize - 0x28;
                file.write_all(&vec![0u8; padding])?;
                patch_page_usage(&mut file, header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                // Data page points to empty candidate
                write_tracks_table(&mut file, tracks, &entities, data_page, empty_page)?;

                // Empty candidate page (type 0 = Tracks, no data)
                write_empty_candidate_page(&mut file)?;
            }
            TableType::Genres => {
                write_page_header(
                    &mut file,
                    header_page,
                    table_type as u32,
                    data_page,
                    0,
                    0x1fff,
                    0,
                    0,
                    0x64,
                    1,
                    0,
                    0x1fff,
                    0x03ec,
                    0,
                )?;
                let padding = PAGE_SIZE as usize - 0x28;
                file.write_all(&vec![0u8; padding])?;
                patch_page_usage(&mut file, header_page as u64 * PAGE_SIZE as u64, 0, 0)?;
                write_genres_table(&mut file, &entities.genres, data_page, empty_page)?;
                write_empty_candidate_page(&mut file)?;
            }
            TableType::Artists => {
                write_page_header(
                    &mut file,
                    header_page,
                    table_type as u32,
                    data_page,
                    0,
                    0x1fff,
                    0,
                    0,
                    0x64,
                    1,
                    0,
                    0x1fff,
                    0x03ec,
                    0,
                )?;
                let padding = PAGE_SIZE as usize - 0x28;
                file.write_all(&vec![0u8; padding])?;
                patch_page_usage(&mut file, header_page as u64 * PAGE_SIZE as u64, 0, 0)?;
                write_artists_table(&mut file, &entities.artists, data_page, empty_page)?;
                write_empty_candidate_page(&mut file)?;
            }
            TableType::Albums => {
                write_page_header(
                    &mut file,
                    header_page,
                    table_type as u32,
                    data_page,
                    0,
                    0x1fff,
                    0,
                    0,
                    0x64,
                    1,
                    0,
                    0x1fff,
                    0x03ec,
                    0,
                )?;
                let padding = PAGE_SIZE as usize - 0x28;
                file.write_all(&vec![0u8; padding])?;
                patch_page_usage(&mut file, header_page as u64 * PAGE_SIZE as u64, 0, 0)?;
                write_albums_table(&mut file, &entities, data_page, empty_page)?;
                write_empty_candidate_page(&mut file)?;
            }
            TableType::Labels | TableType::Keys => {
                // Empty stub tables for Labels and Keys
                write_page_header(
                    &mut file,
                    header_page,
                    table_type as u32,
                    data_page,
                    0,
                    0x1fff,
                    0,
                    0,
                    0x64,
                    1,
                    0,
                    0x1fff,
                    0x03ec,
                    0,
                )?;
                let padding = PAGE_SIZE as usize - 0x28;
                file.write_all(&vec![0u8; padding])?;
                patch_page_usage(&mut file, header_page as u64 * PAGE_SIZE as u64, 0, 0)?;
                write_blank_page(&mut file, data_page, table_type as u32, empty_page, 0x24)?;
                write_empty_candidate_page(&mut file)?;
            }
            TableType::Colors => {
                // Write header page for Colors table
                write_page_header(
                    &mut file,
                    header_page,
                    table_type as u32,
                    data_page,
                    0,
                    0x1fff,
                    0,
                    0,
                    0x64,
                    1,
                    0,
                    0x1fff,
                    0x03ec,
                    0,
                )?;
                let padding = PAGE_SIZE as usize - 0x28;
                file.write_all(&vec![0u8; padding])?;
                patch_page_usage(&mut file, header_page as u64 * PAGE_SIZE as u64, 0, 0)?;
                // Write Colors data page with 8 preset colors
                write_colors_table(&mut file, data_page, empty_page)?;
                write_empty_candidate_page(&mut file)?;
            }
            TableType::PlaylistTree => {
                write_page_header(
                    &mut file,
                    header_page,
                    table_type as u32,
                    data_page,
                    0,
                    0x1fff,
                    0,
                    0,
                    0x64,
                    1,
                    0,
                    0x1fff,
                    0x03ec,
                    0,
                )?;
                let padding = PAGE_SIZE as usize - 0x28;
                file.write_all(&vec![0u8; padding])?;
                patch_page_usage(&mut file, header_page as u64 * PAGE_SIZE as u64, 0, 0)?;
                write_playlist_tree_table(&mut file, playlists, data_page, empty_page)?;
                write_empty_candidate_page(&mut file)?;
            }
            TableType::PlaylistEntries => {
                write_page_header(
                    &mut file,
                    header_page,
                    table_type as u32,
                    data_page,
                    0,
                    0x1fff,
                    0,
                    0,
                    0x64,
                    1,
                    0,
                    0x1fff,
                    0x03ec,
                    0,
                )?;
                let padding = PAGE_SIZE as usize - 0x28;
                file.write_all(&vec![0u8; padding])?;
                patch_page_usage(&mut file, header_page as u64 * PAGE_SIZE as u64, 0, 0)?;
                write_playlist_entries_table(&mut file, playlists, &entities.track_ids, data_page, empty_page)?;
                write_empty_candidate_page(&mut file)?;
            }
            TableType::Columns => {
                // Write empty Columns table for now (stub)
                // TODO: Fix row index format to match reference export
                write_page_header(
                    &mut file,
                    header_page,
                    table_type as u32,
                    data_page,
                    0,
                    0x1fff,
                    0,
                    0,
                    0x64,
                    1,
                    0,
                    0x1fff,
                    0x03ec,
                    0,
                )?;
                let padding = PAGE_SIZE as usize - 0x28;
                file.write_all(&vec![0u8; padding])?;
                patch_page_usage(&mut file, header_page as u64 * PAGE_SIZE as u64, 0, 0)?;
                // Write empty data page instead of columns data
                write_blank_page(&mut file, data_page, table_type as u32, empty_page, 0x24)?;
                write_empty_candidate_page(&mut file)?;
            }
            TableType::History => {
                write_page_header(
                    &mut file,
                    header_page,
                    table_type as u32,
                    data_page,
                    0,
                    0x1fff,
                    0,
                    0,
                    0x64,
                    0x12,
                    0,
                    0x1fff,
                    0x03ec,
                    0x0001,
                )?;
                let padding = PAGE_SIZE as usize - 0x28;
                file.write_all(&vec![0u8; padding])?;
                patch_page_usage(&mut file, header_page as u64 * PAGE_SIZE as u64, 0, 0)?;
                write_blank_page(&mut file, data_page, table_type as u32, empty_page, 0x24)?;
                write_empty_candidate_page(&mut file)?;
            }
            TableType::Artwork
            | TableType::HistoryPlaylists
            | TableType::HistoryEntries
            | TableType::Unknown09
            | TableType::Unknown0A
            | TableType::Unknown0B
            | TableType::Unknown0C
            | TableType::Unknown0E
            | TableType::Unknown0F => {
                write_page_header(
                    &mut file,
                    header_page,
                    table_type as u32,
                    data_page,
                    0,
                    0x1fff,
                    0,
                    0,
                    0x64,
                    1,
                    0,
                    0x1fff,
                    0x03ec,
                    0,
                )?;
                let padding = PAGE_SIZE as usize - 0x28;
                file.write_all(&vec![0u8; padding])?;
                patch_page_usage(&mut file, header_page as u64 * PAGE_SIZE as u64, 0, 0)?;
                write_blank_page(&mut file, data_page, table_type as u32, empty_page, 0x24)?;
                write_empty_candidate_page(&mut file)?;
            }
        }

        // Now we have: header_page, data_page, empty_page
        // first_page = header_page, last_page = data_page, empty_candidate = empty_page
        table_pages.push((table_type, header_page, data_page, empty_page));
        current_page += 3;  // header + data + empty
    }

    // Go back and write table pointers in header using the recorded pages
    file.seek(SeekFrom::Start(0x1c))?;
    for (table_type, first_page, last_page, empty_candidate) in table_pages {
        write_table_pointer(&mut file, table_type as u32, empty_candidate, first_page, last_page)?;
    }

    // Patch header metadata now that we know the final page count
    let next_unused_page = current_page;
    let unknown_field = 5u32; // matches reference export header
    let sequence = 0x44u32; // observed in reference export
    file.seek(SeekFrom::Start(0x0c))?;
    file.write_all(&next_unused_page.to_le_bytes())?;
    file.write_all(&unknown_field.to_le_bytes())?;
    file.write_all(&sequence.to_le_bytes())?;

    log::info!("PDB file written successfully");
    Ok(())
}

/// Write exportExt.pdb - extended database file required by some Pioneer hardware
/// This is a minimal stub with 9 empty tables
pub fn write_pdb_ext(path: &Path) -> Result<()> {
    log::info!("Writing exportExt.pdb file: {:?}", path);

    let mut file = File::create(path)
        .with_context(|| format!("Failed to create exportExt.pdb file: {:?}", path))?;

    // exportExt.pdb has 9 tables (observed from reference)
    let num_tables = 9u32;

    // Write file header (same structure as export.pdb)
    file.write_all(&0u32.to_le_bytes())?;  // unknown1
    file.write_all(&PAGE_SIZE.to_le_bytes())?;  // page_size
    file.write_all(&num_tables.to_le_bytes())?;  // num_tables
    file.write_all(&0u32.to_le_bytes())?;  // next_unused_page (will patch)
    file.write_all(&5u32.to_le_bytes())?;  // unknown
    file.write_all(&4u32.to_le_bytes())?;  // sequence
    file.write_all(&0u32.to_le_bytes())?;  // gap

    // Table types for exportExt (based on reference analysis)
    let ext_tables = [
        0u32, 1, 2, 3, 6, 7, 8, 19, 20,  // Types from reference
    ];

    // Write table pointers (will patch later)
    let table_ptr_start = file.stream_position()?;
    for _ in 0..num_tables {
        file.write_all(&[0u8; 16])?;  // Placeholder
    }

    // Pad header page
    let current_pos = file.stream_position()?;
    let header_padding = PAGE_SIZE as u64 - current_pos;
    file.write_all(&vec![0u8; header_padding as usize])?;

    // Write empty table pages (header + data for each)
    let mut current_page = 1u32;
    let mut table_pages = Vec::new();

    for &table_type in &ext_tables {
        let first_page = current_page;
        let last_page = current_page;  // Single page per table

        // Write a minimal empty page
        let page_start = file.stream_position()?;
        file.write_all(&0u32.to_le_bytes())?;  // unknown
        file.write_all(&current_page.to_le_bytes())?;  // page_index
        file.write_all(&table_type.to_le_bytes())?;  // page_type
        file.write_all(&(current_page + 1).to_le_bytes())?;  // next_page (empty candidate)
        // Rest of page header
        file.write_all(&[0u8; 24])?;
        // Pad to full page
        let remaining = PAGE_SIZE as u64 - (file.stream_position()? - page_start);
        file.write_all(&vec![0u8; remaining as usize])?;

        table_pages.push((table_type, first_page, last_page, current_page + 1));
        current_page += 2;  // page + empty candidate

        // Write empty candidate page
        write_empty_candidate_page(&mut file)?;
    }

    // Go back and write table pointers
    file.seek(SeekFrom::Start(table_ptr_start))?;
    for (table_type, first_page, last_page, empty_candidate) in &table_pages {
        file.write_all(&table_type.to_le_bytes())?;
        file.write_all(&empty_candidate.to_le_bytes())?;
        file.write_all(&first_page.to_le_bytes())?;
        file.write_all(&last_page.to_le_bytes())?;
    }

    // Patch next_unused_page in header
    file.seek(SeekFrom::Start(0x0c))?;
    file.write_all(&current_page.to_le_bytes())?;

    log::info!("exportExt.pdb written successfully");
    Ok(())
}

/// Entity tables (deduplicated)
struct EntityTables {
    artists: Vec<String>,
    albums: Vec<String>,
    genres: Vec<String>,
    artist_map: HashMap<String, u32>,
    album_map: HashMap<String, u32>,
    album_artist_map: HashMap<String, u32>,
    genre_map: HashMap<String, u32>,
    track_ids: HashMap<String, u32>, // Maps Track.id to PDB row ID
    columns: Vec<ColumnEntry>,
}

struct ColumnEntry {
    id: u16,
    flags: u16,
    name: String,
}

/// Build deduplicated entity tables from tracks
fn build_entity_tables(tracks: &[TrackMetadata]) -> Result<EntityTables> {
    let mut artists = Vec::new();
    let mut albums = Vec::new();
    let mut genres = Vec::new();
    let mut artist_map = HashMap::new();
    let mut album_map = HashMap::new();
    let mut album_artist_map = HashMap::new();
    let mut genre_map = HashMap::new();
    let mut track_ids = HashMap::new();
    let mut columns = Vec::new();

    for (track_idx, track_meta) in tracks.iter().enumerate() {
        let track = &track_meta.track;

        // Track ID (1-based)
        track_ids.insert(track.id.clone(), (track_idx + 1) as u32);

        // Artist (deduplicate)
        let artist_id = *artist_map.entry(track.artist.clone())
            .or_insert_with(|| {
                let new_id = (artists.len() + 1) as u32;
                artists.push(track.artist.clone());
                new_id
            });

        // Album (deduplicate)
        if !album_map.contains_key(&track.album) {
            let album_id = (albums.len() + 1) as u32;
            album_map.insert(track.album.clone(), album_id);
            albums.push(track.album.clone());
            album_artist_map.insert(track.album.clone(), artist_id);
        }

        // Genre (optional)
        if let Some(genre) = &track.genre {
            if !genre_map.contains_key(genre) {
                let genre_id = (genres.len() + 1) as u32;
                genre_map.insert(genre.clone(), genre_id);
                genres.push(genre.clone());
            }
        }
    }

    // Columns table definition observed in the reference Rekordbox export
    let column_defs: &[(u16, u16, &str)] = &[
        (17, 132, "PLAYLIST"),
        (18, 152, "HOT CUE BANK"),
        (19, 149, "HISTORY"),
        (20, 145, "SEARCH"),
        (21, 150, "COMMENTS"),
        (22, 140, "DATE ADDED"),
        (23, 151, "DJ PLAY COUNT"),
        (24, 144, "FOLDER"),
        (25, 161, "DEFAULT"),
        (26, 162, "ALPHABET"),
        (27, 170, "MATCHING"),
        (1, 128, "GENRE"),
        (2, 129, "ARTIST"),
        (3, 130, "ALBUM"),
        (4, 131, "TRACK"),
        (5, 133, "BPM"),
        (6, 134, "RATING"),
        (7, 135, "YEAR"),
        (8, 136, "REMIXER"),
        (9, 137, "LABEL"),
        (10, 138, "ORIGINAL ARTIST"),
        (11, 139, "KEY"),
        (12, 141, "CUE"),
        (13, 142, "COLOR"),
        (14, 146, "TIME"),
        (15, 147, "BITRATE"),
        (16, 148, "FILENAME"),
    ];
    for (id, flags, name) in column_defs {
        columns.push(ColumnEntry {
            id: *id,
            flags: *flags,
            name: name.to_string(),
        });
    }

    Ok(EntityTables {
        artists,
        albums,
        genres,
        artist_map,
        album_map,
        album_artist_map,
        genre_map,
        track_ids,
        columns,
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
fn write_table_pointer(file: &mut File, table_type: u32, empty_candidate: u32, first_page: u32, last_page: u32) -> Result<()> {
    file.write_all(&table_type.to_le_bytes())?; // type
    file.write_all(&empty_candidate.to_le_bytes())?; // empty_candidate page
    file.write_all(&first_page.to_le_bytes())?; // first_page
    file.write_all(&last_page.to_le_bytes())?; // last_page
    Ok(())
}

/// Write page header
fn write_page_header(
    file: &mut File,
    page_index: u32,
    table_type: u32,
    next_page: u32,
    num_rows_small: u8,
    num_rows_large: u16,
    unknown3: u8,
    unknown4: u8,
    page_flags: u8,
    unknown1: u32,
    unknown2: u32,
    unknown5: u16,
    unknown6: u16,
    unknown7: u16,
) -> Result<()> {
    // Bytes 0x00-0x03: padding
    file.write_all(&[0u8; 4])?;

    // Bytes 0x04-0x07: page_index
    file.write_all(&page_index.to_le_bytes())?;

    // Bytes 0x08-0x0b: type
    file.write_all(&table_type.to_le_bytes())?;

    // Bytes 0x0c-0x0f: next_page (0 = last page)
    file.write_all(&next_page.to_le_bytes())?;

    // Bytes 0x10-0x13: unknown1
    file.write_all(&unknown1.to_le_bytes())?;

    // Bytes 0x14-0x17: unknown2
    file.write_all(&unknown2.to_le_bytes())?;

    // Bytes 0x18-0x1a: num_rows_small, unknown3, unknown4
    file.write_all(&[num_rows_small])?;
    file.write_all(&[unknown3])?; // unknown3
    file.write_all(&[unknown4])?; // unknown4

    // Byte 0x1b: page_flags
    file.write_all(&[page_flags])?;

    // Bytes 0x1c-0x1d: free_size (patched later)
    file.write_all(&[0u8; 2])?;

    // Bytes 0x1e-0x1f: used_size (patched later)
    file.write_all(&[0u8; 2])?;

    // Bytes 0x20-0x21: unknown5
    file.write_all(&unknown5.to_le_bytes())?;

    // Bytes 0x22-0x23: num_rows_large
    file.write_all(&num_rows_large.to_le_bytes())?;

    // Bytes 0x24-0x25: unknown6
    file.write_all(&unknown6.to_le_bytes())?;

    // Bytes 0x26-0x27: unknown7
    file.write_all(&unknown7.to_le_bytes())?;

    Ok(())
}

/// Patch free/used sizes after writing page contents
fn patch_page_usage(file: &mut File, page_start: u64, free_size: u16, used_size: u16) -> Result<()> {
    // free_size at 0x1c
    file.seek(SeekFrom::Start(page_start + 0x1c))?;
    file.write_all(&free_size.to_le_bytes())?;

    // used_size at 0x1e
    file.seek(SeekFrom::Start(page_start + 0x1e))?;
    file.write_all(&used_size.to_le_bytes())?;

    // Seek back to end of page for subsequent writes/checks
    file.seek(SeekFrom::Start(page_start + PAGE_SIZE as u64))?;
    Ok(())
}

/// Write a blank page (no rows)
///
/// If is_empty_candidate is true, writes a completely zeroed page (empty candidate)
/// Otherwise writes a header-like blank page with proper structure
fn write_blank_page(
    file: &mut File,
    page_index: u32,
    table_type: u32,
    next_page: u32,
    page_flags: u8,
) -> Result<()> {
    log::debug!(
        "Writing blank page {} (type {}), next_page {}",
        page_index,
        table_type,
        next_page
    );

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        table_type,
        next_page,
        0,
        0,
        0,
        0,
        page_flags,
        1,
        0,
        1,
        0,
        0,
    )?;

    // Pad to full page size
    let current_pos = file.stream_position()? - page_start;
    let padding_needed = PAGE_SIZE as u64 - current_pos;
    file.write_all(&vec![0u8; padding_needed as usize])?;

    // Header pages: keep free/used at 0 like reference exports
    let free_size = 0u16;
    let used_size = 0u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}

/// Write an empty candidate page (completely zeroed)
/// Reference exports have empty candidate pages that are all zeros
fn write_empty_candidate_page(file: &mut File) -> Result<()> {
    // Empty candidate pages are completely zeroed - 4096 bytes of zeros
    file.write_all(&vec![0u8; PAGE_SIZE as usize])?;
    Ok(())
}

/// Write genres table (id + name)
fn write_genres_table(file: &mut File, genres: &[String], page_index: u32, next_page: u32) -> Result<()> {
    log::debug!("Writing genres table: {} genres", genres.len());

    let num_rows_small = genres.len().min(0xff) as u8;
    let num_rows_large = genres.len().min(0xffff) as u16;

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Genres as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x20,
        0x01,
        0x24,
        0x3b,
        0,
        0x0001,
        0,
        0,
    )?;

    let mut heap = Vec::new();
    let mut row_offsets = Vec::new();

    for (idx, genre) in genres.iter().enumerate() {
        let row_start = heap.len();
        heap.extend_from_slice(&((idx + 1) as u32).to_le_bytes()); // id
        let encoded_name = encode_device_sql(genre);
        heap.extend_from_slice(&encoded_name);
        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    let current_pos = file.stream_position()? - page_start;
    let num_groups = (genres.len() + 15) / 16;
    let index_space_needed = row_offsets.len() * 2 + num_groups * 4;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(genres.len());
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
        file.write_all(&0u16.to_le_bytes())?;
    }

    let free_size = padding_needed as u16;
    let used_size = (HEAP_START + heap.len()) as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}

/// Write artists table
fn write_artists_table(file: &mut File, artists: &[String], page_index: u32, next_page: u32) -> Result<()> {
    log::debug!("Writing artists table: {} artists", artists.len());

    let num_rows_small = artists.len().min(0xff) as u8;
    let num_rows_large = artists.len().min(0xffff) as u16;

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Artists as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x20,
        0x01,
        0x24,
        0x3a,
        0,
        0x0001,
        0,
        0,
    )?;

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
        heap.extend_from_slice(&0x0100u16.to_le_bytes()); // index_shift (matches reference)
        heap.extend_from_slice(&((idx + 1) as u32).to_le_bytes()); // ID (1-based)

        // Offset array (2 bytes)
        heap.push(0x03u8); // offset[0] (constant value 0x03 for u8 offset arrays)

        // Calculate offset to name string from row start
        let name_offset = 10u8; // String starts at byte 10 (after 8-byte header + 2-byte offset array)
        heap.push(name_offset); // offset[1] (name offset from row start)

        // Encode and append string data at the offset
        let encoded_name = encode_device_sql(artist);
        heap.extend_from_slice(&encoded_name);

        row_offsets.push(row_start as u16);
    }

    // Write heap
    file.write_all(&heap)?;

    // Pad to near end of page for row index
    let current_pos = file.stream_position()? - page_start;
    let num_groups = (artists.len() + 15) / 16;
    let index_space_needed = row_offsets.len() * 2 + num_groups * 4;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    // Write row index (offsets from end)
    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    // Write row presence flags (+ unknown)
    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(artists.len());
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
        file.write_all(&0u16.to_le_bytes())?; // unknown padding
    }

    // Patch usage sizes now that we know padding/heap lengths
    let free_size = padding_needed as u16;
    let used_size = (HEAP_START + heap.len()) as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}

/// Write albums table (different structure from artists!)
fn write_albums_table(file: &mut File, entities: &EntityTables, page_index: u32, next_page: u32) -> Result<()> {
    let albums = &entities.albums;
    log::debug!("Writing albums table: {} albums", albums.len());

    let num_rows_small = albums.len().min(0xff) as u8;
    let num_rows_large = albums.len().min(0xffff) as u16;

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Albums as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0xe0,
        0x00,
        0x24,
        0x3c,
        0,
        0x0001,
        0,
        0,
    )?;

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
        heap.extend_from_slice(&0x00c0u16.to_le_bytes()); // index_shift (matches reference)
        heap.extend_from_slice(&0u32.to_le_bytes()); // unknown2
        let album_artist_id = entities
            .album_artist_map
            .get(album)
            .copied()
            .unwrap_or(0);
        heap.extend_from_slice(&album_artist_id.to_le_bytes()); // artist_id reference
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

        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    let current_pos = file.stream_position()? - page_start;
    let num_groups = (albums.len() + 15) / 16;
    let index_space_needed = row_offsets.len() * 2 + num_groups * 4;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(albums.len());
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
        file.write_all(&0u16.to_le_bytes())?;
    }

    // Patch usage sizes for this page
    let free_size = padding_needed as u16;
    let used_size = (HEAP_START + heap.len()) as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

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
fn write_tracks_table(
    file: &mut File,
    tracks: &[TrackMetadata],
    entities: &EntityTables,
    page_index: u32,
    next_page: u32,
) -> Result<()> {
    log::debug!("Writing tracks table: {} tracks", tracks.len());

    let num_rows_small = tracks.len().min(0xff) as u8;
    let num_rows_large = tracks.len().min(0xffff) as u16;
    let unknown4 = if tracks.len() > 1 { 0x01 } else { 0x00 };

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Tracks as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x20,
        unknown4,
        0x24,
        0x3e,
        0,
        0x0001,
        0,
        0,
    )?;

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

        // 0x02-0x03: index_shift - increments by 0x20 for each row
        let index_shift = (idx as u16) * 0x20;
        heap.extend_from_slice(&index_shift.to_le_bytes());

        // 0x04-0x07: bitmask (unknown, observed 0x0700)
        heap.extend_from_slice(&0x0700u32.to_le_bytes());

        // 0x08-0x0B: sample_rate
        heap.extend_from_slice(&44100u32.to_le_bytes());

        // 0x0C-0x0F: composer_id
        heap.extend_from_slice(&0u32.to_le_bytes());

        // 0x10-0x13: file_size
        heap.extend_from_slice(&(track.file_size as u32).to_le_bytes());

        // 0x14-0x17: u2 (unknown, correlates with track id in reference exports)
        heap.extend_from_slice(&((track_id + 6) as u32).to_le_bytes());

        // 0x18-0x19: u3 (unknown, constant 0xe5b6 in reference exports)
        heap.extend_from_slice(&0xe5b6u16.to_le_bytes());

        // 0x1A-0x1B: u4 (unknown, constant 0x6a76 in reference exports)
        heap.extend_from_slice(&0x6a76u16.to_le_bytes());

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

        // 0x30-0x33: bitrate (MP3 default to 320kbps, otherwise 0)
        let bitrate = if file_type == FileType::Mp3 as u16 { 320u32 } else { 0u32 };
        heap.extend_from_slice(&bitrate.to_le_bytes());

        // 0x34-0x37: track_number (u32!)
        let track_number = track.track_number.unwrap_or(0) as u32;
        heap.extend_from_slice(&track_number.to_le_bytes());

        // 0x38-0x3B: tempo (BPM * 100)
        let tempo = track.bpm.map(|bpm| (bpm * 100.0) as u32).unwrap_or(0);
        heap.extend_from_slice(&tempo.to_le_bytes());

        // 0x3C-0x3F: genre_id
        let genre_id = track
            .genre
            .as_ref()
            .and_then(|g| entities.genre_map.get(g))
            .copied()
            .unwrap_or(0);
        heap.extend_from_slice(&genre_id.to_le_bytes());

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

        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    let current_pos = file.stream_position()? - page_start;
    let num_groups = (tracks.len() + 15) / 16;
    let index_space_needed = row_offsets.len() * 2 + num_groups * 4; // offsets + (flags+unknown) per group
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(tracks.len());
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
        file.write_all(&0u16.to_le_bytes())?; // unknown padding
    }

    let free_size = padding_needed as u16;
    let used_size = (HEAP_START + heap.len()) as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}

/// Write playlist tree table
fn write_playlist_tree_table(file: &mut File, playlists: &[Playlist], page_index: u32, next_page: u32) -> Result<()> {
    log::debug!("Writing playlist tree table: {} playlists", playlists.len());

    let page_start = file.stream_position()?;
    // No ROOT folder - playlists sit directly at root level (parent_id=0)
    let num_rows = playlists.len() as u32;
    let num_rows_small = num_rows.min(0xff) as u8;
    let num_rows_large = num_rows.min(0xffff) as u16;
    write_page_header(
        file,
        page_index,
        TableType::PlaylistTree as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x40,
        0x00,
        0x24,
        0x0007,
        0,
        0x0001,
        0,
        0,
    )?;

    // PlaylistTreeNode row structure (inline strings, NOT offset-based!):
    // - u32: parent_id (0 = root)
    // - u32: unknown (0)
    // - u32: sort_order
    // - u32: id (playlist ID)
    // - u32: node_is_folder (0 = playlist, non-zero = folder)
    // - DeviceSQLString: name (INLINE, not offset-based!)

    let mut heap = Vec::new();
    let mut row_offsets = Vec::new();

    // No ROOT folder - playlists sit directly at root level with parent_id=0
    // Playlist IDs start at 1, sort_order starts at 0
    for (idx, playlist) in playlists.iter().enumerate() {
        let row_start = heap.len();

        // Fixed fields (20 bytes)
        heap.extend_from_slice(&0u32.to_le_bytes()); // parent_id (0 = root level)
        heap.extend_from_slice(&0u32.to_le_bytes()); // unknown
        heap.extend_from_slice(&(idx as u32).to_le_bytes()); // sort_order (0-based)
        heap.extend_from_slice(&((idx + 1) as u32).to_le_bytes()); // playlist ID (1-based)
        heap.extend_from_slice(&0u32.to_le_bytes()); // node_is_folder (0 = playlist, not folder)

        // Encode and append string data INLINE (not offset-based!)
        let encoded_name = encode_device_sql(&playlist.name);
        heap.extend_from_slice(&encoded_name);

        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    let current_pos = file.stream_position()? - page_start;
    let total_rows = playlists.len();
    let num_groups = (total_rows + 15) / 16;
    let index_space_needed = row_offsets.len() * 2 + num_groups * 4;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(total_rows);
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
        file.write_all(&0u16.to_le_bytes())?;
    }

    let free_size = padding_needed as u16;
    let used_size = (HEAP_START + heap.len()) as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}

/// Write playlist entries table
fn write_playlist_entries_table(
    file: &mut File,
    playlists: &[Playlist],
    track_ids: &HashMap<String, u32>,
    page_index: u32,
    next_page: u32,
) -> Result<()> {
    // Count total entries
    let total_entries: usize = playlists.iter().map(|p| p.entries.len()).sum();
    let num_rows_small = total_entries.min(0xff) as u8;
    let num_rows_large = total_entries.min(0xffff) as u16;

    log::debug!("Writing playlist entries table: {} entries", total_entries);

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::PlaylistEntries as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0xa0,
        0x01,
        0x24,
        0x0042,
        0,
        0x0001,
        0,
        0,
    )?;

    let mut heap = Vec::new();
    let mut row_offsets = Vec::new();

    for (playlist_idx, playlist) in playlists.iter().enumerate() {
        // Playlist IDs start at 1 (no ROOT folder)
        let playlist_id = (playlist_idx + 1) as u32;

        for entry in &playlist.entries {
            let row_start = heap.len();

            // PlaylistEntry row structure (simple, no subtype/index_shift):
            // - u32: entry_index (position in playlist, 1-based)
            // - u32: track_id
            // - u32: playlist_id

            // Position in playlist (entry_index) - 1-based
            let entry_index = entry.position + 1;
            heap.extend_from_slice(&entry_index.to_le_bytes());

            // Track ID reference
            let track_id = track_ids.get(&entry.track_id).unwrap_or(&0);
            heap.extend_from_slice(&track_id.to_le_bytes());

            // Playlist ID reference
            heap.extend_from_slice(&playlist_id.to_le_bytes());

            row_offsets.push(row_start as u16);
        }
    }

    file.write_all(&heap)?;

    let current_pos = file.stream_position()? - page_start;
    let num_groups = (total_entries + 15) / 16;
    let index_space_needed = row_offsets.len() * 2 + num_groups * 4;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(total_entries);
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
        file.write_all(&0u16.to_le_bytes())?;
    }

    let free_size = padding_needed as u16;
    let used_size = (HEAP_START + heap.len()) as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}

/// Write colors table with 8 preset Rekordbox colors
/// Color row structure:
///   - u32: unknown1 (0)
///   - u8: unknown2 (0)
///   - u8: color_index (1=Pink, 2=Red, ..., 8=Purple)
///   - u16: unknown3 (0)
///   - DeviceSQLString: name
fn write_colors_table(file: &mut File, page_index: u32, next_page: u32) -> Result<()> {
    // 8 preset colors in Rekordbox order
    let colors = [
        (1u8, "Pink"),
        (2u8, "Red"),
        (3u8, "Orange"),
        (4u8, "Yellow"),
        (5u8, "Green"),
        (6u8, "Aqua"),
        (7u8, "Blue"),
        (8u8, "Purple"),
    ];

    log::debug!("Writing colors table: {} colors", colors.len());

    let num_rows = colors.len();
    let num_rows_small = num_rows.min(0xff) as u8;
    let num_rows_large = num_rows.min(0xffff) as u16;

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Colors as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x7c,  // free_size from reference
        0x00,
        0x24,
        0x0048,  // unknown4 - matches reference
        0,
        0x0001,
        0,
        0,
    )?;

    let mut heap = Vec::new();
    let mut row_offsets = Vec::new();

    for (color_index, name) in &colors {
        let row_start = heap.len();

        // Color row structure (8 bytes header + string)
        heap.extend_from_slice(&0u32.to_le_bytes()); // unknown1
        heap.push(0u8); // unknown2
        heap.push(*color_index); // color_index
        heap.extend_from_slice(&0u16.to_le_bytes()); // unknown3

        // Encode and append name string
        let encoded_name = encode_device_sql(name);
        heap.extend_from_slice(&encoded_name);

        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    // Pad to near end of page for row index
    let current_pos = file.stream_position()? - page_start;
    let num_groups = (num_rows + 15) / 16;
    let index_space_needed = row_offsets.len() * 2 + num_groups * 4;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    // Write row offsets in reverse order
    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    // Write row presence flags
    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(num_rows);
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
        file.write_all(&0xffu16.to_le_bytes())?; // unknown16 - reference has 0xff
    }

    let free_size = padding_needed as u16;
    let used_size = (HEAP_START + heap.len()) as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}

/// Write columns table (browse categories)
fn write_columns_table(file: &mut File, columns: &[ColumnEntry], page_index: u32, next_page: u32) -> Result<()> {
    log::debug!("Writing columns table: {} entries", columns.len());

    let num_rows_small = columns.len().min(0xff) as u8;
    let num_rows_large = columns.len().min(0xffff) as u16;

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Columns as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x60,
        0x03,
        0x24,
        0x0003,
        0,
        num_rows_small as u16,
        0,
        0,
    )?;

    let mut heap = Vec::new();
    let mut row_offsets = Vec::new();

    for col in columns {
        let row_start = heap.len();

        // u16 id
        heap.extend_from_slice(&col.id.to_le_bytes());
        // u16 flags
        heap.extend_from_slice(&col.flags.to_le_bytes());

        // Name as annotated UTF-16 DeviceSQL string
        let encoded_name = encode_device_sql_utf16_annotated(&col.name);
        heap.extend_from_slice(&encoded_name);

        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    let current_pos = file.stream_position()? - page_start;
    let num_groups = (columns.len() + 15) / 16;
    let index_space_needed = row_offsets.len() * 2 + num_groups * 4;
    let padding_needed = PAGE_SIZE as u64 - current_pos - index_space_needed as u64;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed as usize])?;
    }

    for offset in row_offsets.iter().rev() {
        file.write_all(&offset.to_le_bytes())?;
    }

    for group_idx in 0..num_groups {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(columns.len());
        let mut flags = 0u16;
        for row in start_row..end_row {
            flags |= 1 << (row - start_row);
        }
        file.write_all(&flags.to_le_bytes())?;
        file.write_all(&0u16.to_le_bytes())?;
    }

    let free_size = padding_needed as u16;
    let used_size = (HEAP_START + heap.len()) as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}
