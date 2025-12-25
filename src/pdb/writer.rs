//! PDB file writer implementation
//!
//! Phase 1: Minimal implementation with simplified single-page tables
//! Phase 2: Full multi-page support, all metadata fields, proper indexing

use crate::analysis::AnalysisResult;
use crate::model::{Playlist, Track};
use super::types::{TableType, FileType};
use super::strings::encode_device_sql;
use anyhow::{bail, Context, Result};
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

struct TableLayout {
    table: TableType,
    header_page: u32,
    data_pages: &'static [u32],
    empty_candidate: u32,
    last_page: u32,
}

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

// Reference export uses 41 pages, with next_unused_page pointing to page 53
// (empty candidate pages 41-52 don't actually exist in file but are pointer values)
const REFERENCE_NEXT_UNUSED_PAGE: u32 = 53;

const TABLE_LAYOUTS: &[TableLayout] = &[
    // Tables with data (header + data page)
    TableLayout { table: TableType::Tracks, header_page: 1, data_pages: &[2], empty_candidate: 51, last_page: 2 },
    TableLayout { table: TableType::Genres, header_page: 3, data_pages: &[4], empty_candidate: 48, last_page: 4 },
    TableLayout { table: TableType::Artists, header_page: 5, data_pages: &[6], empty_candidate: 47, last_page: 6 },
    TableLayout { table: TableType::Albums, header_page: 7, data_pages: &[8], empty_candidate: 49, last_page: 8 },
    // Labels: empty table - header only, no data page (first==last in reference)
    TableLayout { table: TableType::Labels, header_page: 9, data_pages: &[], empty_candidate: 10, last_page: 9 },
    TableLayout { table: TableType::Keys, header_page: 11, data_pages: &[12], empty_candidate: 50, last_page: 12 },
    TableLayout { table: TableType::Colors, header_page: 13, data_pages: &[14], empty_candidate: 42, last_page: 14 },
    TableLayout { table: TableType::PlaylistTree, header_page: 15, data_pages: &[16], empty_candidate: 46, last_page: 16 },
    TableLayout { table: TableType::PlaylistEntries, header_page: 17, data_pages: &[18], empty_candidate: 52, last_page: 18 },
    // Empty placeholder tables
    TableLayout { table: TableType::Unknown09, header_page: 19, data_pages: &[], empty_candidate: 20, last_page: 19 },
    TableLayout { table: TableType::Unknown0A, header_page: 21, data_pages: &[], empty_candidate: 22, last_page: 21 },
    TableLayout { table: TableType::Unknown0B, header_page: 23, data_pages: &[], empty_candidate: 24, last_page: 23 },
    TableLayout { table: TableType::Unknown0C, header_page: 25, data_pages: &[], empty_candidate: 26, last_page: 25 },
    // Artwork: empty table - header only, no data page (first==last in reference)
    TableLayout { table: TableType::Artwork, header_page: 27, data_pages: &[], empty_candidate: 28, last_page: 27 },
    TableLayout { table: TableType::Unknown0E, header_page: 29, data_pages: &[], empty_candidate: 30, last_page: 29 },
    TableLayout { table: TableType::Unknown0F, header_page: 31, data_pages: &[], empty_candidate: 32, last_page: 31 },
    TableLayout { table: TableType::Columns, header_page: 33, data_pages: &[34], empty_candidate: 43, last_page: 34 },
    TableLayout { table: TableType::HistoryPlaylists, header_page: 35, data_pages: &[36], empty_candidate: 44, last_page: 36 },
    TableLayout { table: TableType::HistoryEntries, header_page: 37, data_pages: &[38], empty_candidate: 45, last_page: 38 },
    TableLayout { table: TableType::History, header_page: 39, data_pages: &[40], empty_candidate: 41, last_page: 40 },
];

fn seek_to_page(file: &mut File, page_index: u32) -> Result<u64> {
    let offset = page_index as u64 * PAGE_SIZE as u64;
    file.seek(SeekFrom::Start(offset))?;
    Ok(offset)
}

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

    // Write file header and pre-size the file so all reference pages exist
    write_file_header(&mut file, num_tables)?;

    // Calculate track page allocation
    // Conservative estimate: ~12 tracks per page to avoid overflow
    // (track rows are variable size due to strings)
    // ~350-400 bytes per track, page capacity ~4000 bytes = ~10 tracks max
    const TRACKS_PER_PAGE: usize = 10;

    // Split tracks into chunks
    let mut track_chunks: Vec<&[TrackMetadata]> = Vec::new();
    let mut remaining = &tracks[..];
    while !remaining.is_empty() {
        let chunk_size = remaining.len().min(TRACKS_PER_PAGE);
        track_chunks.push(&remaining[..chunk_size]);
        remaining = &remaining[chunk_size..];
    }

    // Track data pages: use reference page 2 (reference uses 2 and 51 but we only need one for 3 tracks)
    let mut track_data_pages: Vec<u32> = vec![2];
    // For more tracks, allocate pages starting after page 40 (last reference data page)
    let mut next_alloc_page = 41u32;  // Start allocating after all reference pages

    // Add more pages if needed (beyond the 1 reference page)
    while track_data_pages.len() < track_chunks.len() {
        track_data_pages.push(next_alloc_page);
        next_alloc_page += 1;
    }

    // File size: 41 pages for reference-compatible export (pages 0-40)
    // Empty candidate pages (41-52) don't need to exist in the file
    let file_page_count = 41u32;
    file.set_len((file_page_count as u64) * PAGE_SIZE as u64)?;

    log::debug!("Tracks: {} total, {} chunks, pages: {:?}",
        tracks.len(), track_chunks.len(), &track_data_pages[..track_chunks.len()]);

    for layout in TABLE_LAYOUTS {
        match layout.table {
            TableType::Tracks => {
                // Header page - point to first track data page
                seek_to_page(&mut file, layout.header_page)?;
                write_page_header(
                    &mut file,
                    layout.header_page,
                    TableType::Tracks as u32,
                    track_data_pages[0],
                    0,
                    0x1fff,
                    0,
                    0,
                    0x64,
                    0x01,   // unknown1 - reference has 0x01, not 0x3e
                    0,
                    0x1fff,
                    0x03ec,
                    0x0000, // unknown7 - reference has 0x00, not 0x01
                )?;
                write_header_page_content(&mut file, layout.header_page, Some(track_data_pages[0]), TableType::Tracks)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                // Write each track data page
                let mut current_track_id = 1u32;
                for (chunk_idx, track_chunk) in track_chunks.iter().enumerate() {
                    if track_chunk.is_empty() {
                        continue;
                    }

                    let page_num = track_data_pages[chunk_idx];
                    // Only link to next page if there's actually a next chunk to write
                    let next_page = if chunk_idx + 1 < track_chunks.len() {
                        track_data_pages[chunk_idx + 1]
                    } else {
                        layout.empty_candidate
                    };

                    // Page parameters based on reference export
                    let page_unknown1 = 0x001cu32;  // Matches reference: 0x1c
                    let unknown4 = 0x00u8;  // Matches reference: 0x00

                    seek_to_page(&mut file, page_num)?;
                    write_tracks_table(
                        &mut file,
                        track_chunk,
                        &entities,
                        page_num,
                        next_page,
                        page_unknown1,
                        unknown4,
                        current_track_id,
                    )?;

                    current_track_id += track_chunk.len() as u32;
                }

                // Empty candidate pages don't need actual content - they're just pointer values
            }
            TableType::Genres => {
                seek_to_page(&mut file, layout.header_page)?;
                write_page_header(
                    &mut file,
                    layout.header_page,
                    TableType::Genres as u32,
                    layout.data_pages[0],
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
                write_header_page_content(&mut file, layout.header_page, Some(layout.data_pages[0]), layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                seek_to_page(&mut file, layout.data_pages[0])?;
                write_genres_table(&mut file, &entities.genres, layout.data_pages[0], layout.empty_candidate)?;
            }
            TableType::Artists => {
                seek_to_page(&mut file, layout.header_page)?;
                write_page_header(
                    &mut file,
                    layout.header_page,
                    TableType::Artists as u32,
                    layout.data_pages[0],
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
                write_header_page_content(&mut file, layout.header_page, Some(layout.data_pages[0]), layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                seek_to_page(&mut file, layout.data_pages[0])?;
                write_artists_table(&mut file, &entities.artists, layout.data_pages[0], layout.empty_candidate)?;
            }
            TableType::Albums => {
                seek_to_page(&mut file, layout.header_page)?;
                write_page_header(
                    &mut file,
                    layout.header_page,
                    TableType::Albums as u32,
                    layout.data_pages[0],
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
                write_header_page_content(&mut file, layout.header_page, Some(layout.data_pages[0]), layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                seek_to_page(&mut file, layout.data_pages[0])?;
                write_albums_table(&mut file, &entities, layout.data_pages[0], layout.empty_candidate)?;
            }
            // Labels is an empty table (no data) - handled in the empty tables branch below
            TableType::Keys => {
                seek_to_page(&mut file, layout.header_page)?;
                write_page_header(
                    &mut file,
                    layout.header_page,
                    layout.table as u32,
                    layout.data_pages[0],
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
                write_header_page_content(&mut file, layout.header_page, Some(layout.data_pages[0]), layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                seek_to_page(&mut file, layout.data_pages[0])?;
                write_keys_table(&mut file, layout.data_pages[0], layout.empty_candidate)?;
            }
            TableType::Colors => {
                seek_to_page(&mut file, layout.header_page)?;
                write_page_header(
                    &mut file,
                    layout.header_page,
                    TableType::Colors as u32,
                    layout.data_pages[0],
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
                write_header_page_content(&mut file, layout.header_page, Some(layout.data_pages[0]), layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                seek_to_page(&mut file, layout.data_pages[0])?;
                write_colors_table(&mut file, layout.data_pages[0], layout.empty_candidate)?;
            }
            TableType::PlaylistTree => {
                seek_to_page(&mut file, layout.header_page)?;
                write_page_header(
                    &mut file,
                    layout.header_page,
                    TableType::PlaylistTree as u32,
                    layout.data_pages[0],
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
                write_header_page_content(&mut file, layout.header_page, Some(layout.data_pages[0]), layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                seek_to_page(&mut file, layout.data_pages[0])?;
                write_playlist_tree_table(&mut file, playlists, layout.data_pages[0], layout.empty_candidate)?;
            }
            TableType::PlaylistEntries => {
                seek_to_page(&mut file, layout.header_page)?;
                write_page_header(
                    &mut file,
                    layout.header_page,
                    TableType::PlaylistEntries as u32,
                    layout.data_pages[0],
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
                write_header_page_content(&mut file, layout.header_page, Some(layout.data_pages[0]), layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                seek_to_page(&mut file, layout.data_pages[0])?;
                write_playlist_entries_table(&mut file, playlists, &entities.track_ids, layout.data_pages[0], layout.empty_candidate)?;
            }
            TableType::Columns => {
                seek_to_page(&mut file, layout.header_page)?;
                write_page_header(
                    &mut file,
                    layout.header_page,
                    TableType::Columns as u32,
                    layout.data_pages[0],
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
                write_header_page_content(&mut file, layout.header_page, Some(layout.data_pages[0]), layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                seek_to_page(&mut file, layout.data_pages[0])?;
                write_columns_table(&mut file, &entities.columns, layout.data_pages[0], layout.empty_candidate)?;
            }
            TableType::History => {
                // History table - REQUIRED for XDJ to recognize USB
                seek_to_page(&mut file, layout.header_page)?;
                write_page_header(
                    &mut file,
                    layout.header_page,
                    TableType::History as u32,
                    layout.data_pages[0],
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
                write_header_page_content(&mut file, layout.header_page, Some(layout.data_pages[0]), layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                seek_to_page(&mut file, layout.data_pages[0])?;
                file.write_all(REFERENCE_HISTORY_PAGE)?;
            }
            TableType::HistoryEntries => {
                // HistoryEntries table - REQUIRED for XDJ to recognize USB
                seek_to_page(&mut file, layout.header_page)?;
                let next_page = layout.data_pages.get(0).copied().unwrap_or(layout.empty_candidate);
                write_page_header(
                    &mut file,
                    layout.header_page,
                    layout.table as u32,
                    next_page,
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
                let first_data_page = layout.data_pages.get(0).copied();
                write_header_page_content(&mut file, layout.header_page, first_data_page, layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                if let Some(&data_page) = layout.data_pages.get(0) {
                    seek_to_page(&mut file, data_page)?;
                    file.write_all(REFERENCE_HISTORY_ENTRIES_PAGE)?;
                }
            }
            TableType::HistoryPlaylists => {
                // HistoryPlaylists table - REQUIRED for XDJ to recognize USB
                seek_to_page(&mut file, layout.header_page)?;
                let next_page = layout.data_pages.get(0).copied().unwrap_or(layout.empty_candidate);
                write_page_header(
                    &mut file,
                    layout.header_page,
                    layout.table as u32,
                    next_page,
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
                let first_data_page = layout.data_pages.get(0).copied();
                write_header_page_content(&mut file, layout.header_page, first_data_page, layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                if let Some(&data_page) = layout.data_pages.get(0) {
                    seek_to_page(&mut file, data_page)?;
                    file.write_all(REFERENCE_HISTORY_PLAYLISTS_PAGE)?;
                }
            }
            TableType::Labels  // Empty table - no label data
            | TableType::Artwork  // Empty table - no artwork data
            | TableType::Unknown09
            | TableType::Unknown0A
            | TableType::Unknown0B
            | TableType::Unknown0C
            | TableType::Unknown0E
            | TableType::Unknown0F => {
                seek_to_page(&mut file, layout.header_page)?;
                let next_page = layout.data_pages.get(0).copied().unwrap_or(layout.empty_candidate);
                write_page_header(
                    &mut file,
                    layout.header_page,
                    layout.table as u32,
                    next_page,
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
                // For tables with no data pages, use None for first_data_page
                let first_data_page = layout.data_pages.get(0).copied();
                write_header_page_content(&mut file, layout.header_page, first_data_page, layout.table)?;
                patch_page_usage(&mut file, layout.header_page as u64 * PAGE_SIZE as u64, 0, 0)?;

                if let Some(&data_page) = layout.data_pages.get(0) {
                    seek_to_page(&mut file, data_page)?;
                    write_blank_page(&mut file, data_page, layout.table as u32, layout.empty_candidate, 0x24)?;
                }
            }
        }
    }

    // Write table pointers back into the header
    // For tracks, use the actual last data page (based on how many chunks we wrote)
    let actual_track_last_page = if track_chunks.is_empty() {
        TABLE_LAYOUTS[0].header_page
    } else {
        track_data_pages[track_chunks.len() - 1]
    };

    file.seek(SeekFrom::Start(0x1c))?;
    for layout in TABLE_LAYOUTS {
        let last_page = if layout.table == TableType::Tracks {
            actual_track_last_page
        } else {
            layout.last_page
        };
        write_table_pointer(
            &mut file,
            layout.table as u32,
            layout.empty_candidate,
            layout.header_page,
            last_page,
        )?;
    }

    // Patch header metadata to match reference export
    // 0x0c: next_unused_page (53 = 0x35 in reference)
    // 0x10: unknown (5 in reference)
    // 0x14: sequence (31 = 0x1f in reference)
    file.seek(SeekFrom::Start(0x0c))?;
    file.write_all(&REFERENCE_NEXT_UNUSED_PAGE.to_le_bytes())?;
    file.write_all(&5u32.to_le_bytes())?;
    file.write_all(&31u32.to_le_bytes())?;  // Match reference sequence

    log::info!("PDB file written successfully");
    Ok(())
}

/// Reference exportExt.pdb data - contains waveform preview data that is
/// required for Pioneer DJ hardware to display waveforms. This file is identical
/// between different Rekordbox exports (3-track and 20-track exports have the
/// same exportExt.pdb), so we use static reference data like the History tables.
const REFERENCE_EXPORTEXT: &[u8] = include_bytes!("../../examples/reference_exportext.pdb");

/// Write exportExt.pdb - extended database file required by Pioneer hardware
///
/// Uses reference data that contains waveform preview metadata. This data is
/// identical across different exports (like History tables), so we copy it
/// directly rather than generating it.
pub fn write_pdb_ext(path: &Path) -> Result<()> {
    log::info!("Writing exportExt.pdb file: {:?}", path);

    std::fs::write(path, REFERENCE_EXPORTEXT)
        .with_context(|| format!("Failed to write exportExt.pdb file: {:?}", path))?;

    log::info!("exportExt.pdb written successfully (reference data: {} bytes)", REFERENCE_EXPORTEXT.len());
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

fn row_group_count(num_rows: usize) -> usize {
    (num_rows + 15) / 16
}

/// Pad heap to 4-byte alignment after writing a row
/// Returns the number of padding bytes added
fn align_to_4(heap: &mut Vec<u8>) -> usize {
    let remainder = heap.len() % 4;
    if remainder != 0 {
        let padding = 4 - remainder;
        heap.extend(std::iter::repeat(0u8).take(padding));
        padding
    } else {
        0
    }
}

fn row_group_bytes(num_rows: usize) -> usize {
    // Full groups: 16 offsets * 2 bytes + 2 (present) + 2 (unknown) = 36 bytes
    // Partial groups: N offsets * 2 bytes + 2 (present) + 2 (unknown) = N*2 + 4 bytes
    let full_groups = num_rows / 16;
    let partial_rows = num_rows % 16;
    let full_bytes = full_groups * 36;
    let partial_bytes = if partial_rows > 0 { partial_rows * 2 + 4 } else { 0 };
    full_bytes + partial_bytes
}

fn page_padding(heap_len: usize, num_rows: usize) -> Result<usize> {
    let padding = PAGE_SIZE as isize - HEAP_START as isize - heap_len as isize - row_group_bytes(num_rows) as isize;
    if padding < 0 {
        bail!("Page overflow: heap {} rows {} exceed page capacity", heap_len, num_rows);
    }
    Ok(padding as usize)
}

fn write_row_groups<F>(file: &mut File, num_rows: usize, row_offsets: &[u16], unknown_fn: F) -> Result<()>
where
    F: Fn(u16) -> u16,
{
    let num_groups = row_group_count(num_rows);
    // Write groups in REVERSE order: last group first (closest to heap), group 0 last (at page boundary)
    // This matches Rekordbox's incremental append behavior
    for group_idx in (0..num_groups).rev() {
        let start_row = group_idx * 16;
        let end_row = (start_row + 16).min(num_rows);
        let rows_in_group = end_row - start_row;

        // Collect actual offsets for this group
        let mut offsets = Vec::with_capacity(rows_in_group);
        let mut flags = 0u16;
        for (slot, offset) in row_offsets[start_row..end_row].iter().enumerate() {
            offsets.push(*offset);
            flags |= 1 << slot;
        }

        // Write offsets in reverse order (only actual row count, not padded to 16)
        for off in offsets.iter().rev() {
            file.write_all(&off.to_le_bytes())?;
        }

        let unknown = unknown_fn(flags);
        file.write_all(&flags.to_le_bytes())?;
        file.write_all(&unknown.to_le_bytes())?;
    }

    Ok(())
}

fn row_group_unknown_high_bit(flags: u16) -> u16 {
    // Full groups (all 16 slots used) have unknown=0
    // Partial groups have unknown = 2^highest_set_bit
    if flags == 0 || flags == 0xffff {
        0
    } else {
        let leading = flags.leading_zeros() as u16;
        let idx = 15u16.saturating_sub(leading);
        1u16 << idx
    }
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
fn write_empty_candidate_page(file: &mut File, page_index: u32) -> Result<()> {
    seek_to_page(file, page_index)?;
    file.write_all(&vec![0u8; PAGE_SIZE as usize])?;
    Ok(())
}

/// Write the header page content that appears after the 40-byte page header
/// This structure is present in all Rekordbox header pages and contains pointers
fn write_header_page_content(file: &mut File, header_page: u32, first_data_page: Option<u32>, table_type: TableType) -> Result<()> {
    // Position is at 0x28 (right after page header)
    // Structure observed in reference export:
    // +0x00: u32 header_page (this page index)
    // +0x04: u32 first_data_page (or 0x03ffffff if no data)
    // +0x08: u32 sentinel (0x03ffffff)
    // +0x0c: u32 zero
    // +0x10: u32 varies by table type
    // +0x14: u32 0x1ffff8ff (repeated to fill pattern) or special for History
    // Note: page 1 (Tracks) has 0x00000000 at +0x10, other pages have 0x1fff0000
    // History (page 39) has special values: 01 00 ff 1f 40 01 00 00

    file.write_all(&header_page.to_le_bytes())?;
    let data_page = first_data_page.unwrap_or(0x03ff_ffff);
    file.write_all(&data_page.to_le_bytes())?;
    file.write_all(&0x03ff_ffffu32.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;

    // Different patterns for different table types
    match table_type {
        TableType::Tracks => {
            // Tracks (page 1): 00 00 ff 1f (0x1fff0000 in little endian)
            file.write_all(&0x1fff_0000u32.to_le_bytes())?;
        }
        TableType::History => {
            // History (page 39): 01 00 ff 1f 40 01 00 00 (special 8-byte sequence)
            file.write_all(&[0x01, 0x00, 0xff, 0x1f])?;
            file.write_all(&[0x40, 0x01, 0x00, 0x00])?;
            // Then continue with pattern (but we already wrote 8 extra bytes)
            let pattern: [u8; 4] = [0xf8, 0xff, 0xff, 0x1f];
            let remaining_bytes = PAGE_SIZE as usize - 0x28 - 24; // 24 bytes written above
            let pattern_fill_bytes = remaining_bytes - 20; // Leave last 20 bytes as zeros
            let pattern_count = pattern_fill_bytes / 4;
            for _ in 0..pattern_count {
                file.write_all(&pattern)?;
            }
            file.write_all(&[0u8; 20])?;
            return Ok(());
        }
        _ => {
            // Other pages: 00 00 ff 1f
            file.write_all(&0x1fff_0000u32.to_le_bytes())?;
        }
    }

    // Fill remaining with the pattern 0x1ffff8ff (f8 ff ff 1f in little endian)
    // But leave the last 20 bytes as zeros (observed in reference: pattern ends at 0xfec, zeros from 0xfec to 0xfff)
    let pattern: [u8; 4] = [0xf8, 0xff, 0xff, 0x1f]; // Exact bytes from reference
    let remaining_bytes = PAGE_SIZE as usize - 0x28 - 20; // 20 bytes written above
    let pattern_fill_bytes = remaining_bytes - 20; // Leave last 20 bytes as zeros
    let pattern_count = pattern_fill_bytes / 4;
    for _ in 0..pattern_count {
        file.write_all(&pattern)?;
    }
    // Write 20 trailing zeros
    file.write_all(&[0u8; 20])?;

    Ok(())
}

/// Write genres table (id + name)
fn write_genres_table(file: &mut File, genres: &[String], page_index: u32, next_page: u32) -> Result<()> {
    log::debug!("Writing genres table: {} genres", genres.len());

    let num_rows_small = genres.len().min(0xff) as u8;
    // num_rows_large is (num_rows - 1) for data pages with rows, per reference export
    let num_rows_large = if genres.is_empty() { 0 } else { (genres.len() - 1) as u16 };

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Genres as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x60,  // Match reference
        0x00,  // Match reference
        0x24,
        0x19,  // Match reference (was 0x3b)
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
        align_to_4(&mut heap); // Pad row to 4-byte alignment
        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    let padding_needed = page_padding(heap.len(), genres.len())?;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed])?;
    }

    write_row_groups(file, genres.len(), &row_offsets, row_group_unknown_high_bit)?;

    let free_size = padding_needed as u16;
    let used_size = heap.len() as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}

/// Write artists table
fn write_artists_table(file: &mut File, artists: &[String], page_index: u32, next_page: u32) -> Result<()> {
    log::debug!("Writing artists table: {} artists", artists.len());

    let num_rows_small = artists.len().min(0xff) as u8;
    // num_rows_large is (num_rows - 1) for data pages with rows, per reference export
    let num_rows_large = if artists.is_empty() { 0 } else { (artists.len() - 1) as u16 };

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Artists as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x60,  // Match reference
        0x00,  // Match reference
        0x24,
        0x18,  // Match reference (was 0x3a)
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
        let idx_shift = (idx as u16) * 0x20;  // index_shift increments by 0x20 per row
        heap.extend_from_slice(&idx_shift.to_le_bytes());
        heap.extend_from_slice(&((idx + 1) as u32).to_le_bytes()); // ID (1-based)

        // Offset array (2 bytes)
        heap.push(0x03u8); // offset[0] (constant value 0x03 for u8 offset arrays)

        // Calculate offset to name string from row start
        let name_offset = 10u8; // String starts at byte 10 (after 8-byte header + 2-byte offset array)
        heap.push(name_offset); // offset[1] (name offset from row start)

        // Encode and append string data at the offset
        let encoded_name = encode_device_sql(artist);
        heap.extend_from_slice(&encoded_name);

        // Pad row to 28 bytes (0x1C) to match reference
        let row_size = heap.len() - row_start;
        const ARTIST_ROW_SIZE: usize = 28;
        if row_size < ARTIST_ROW_SIZE {
            heap.extend(std::iter::repeat(0u8).take(ARTIST_ROW_SIZE - row_size));
        }

        row_offsets.push(row_start as u16);
    }

    // Write heap
    file.write_all(&heap)?;

    let padding_needed = page_padding(heap.len(), artists.len())?;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed])?;
    }

    write_row_groups(file, artists.len(), &row_offsets, row_group_unknown_high_bit)?;

    // Patch usage sizes now that we know padding/heap lengths
    let free_size = padding_needed as u16;
    let used_size = heap.len() as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}

/// Write albums table (different structure from artists!)
fn write_albums_table(file: &mut File, entities: &EntityTables, page_index: u32, next_page: u32) -> Result<()> {
    let albums = &entities.albums;
    log::debug!("Writing albums table: {} albums", albums.len());

    let num_rows_small = albums.len().min(0xff) as u8;
    // num_rows_large is (num_rows - 1) for data pages with rows, per reference export
    let num_rows_large = if albums.is_empty() { 0 } else { (albums.len() - 1) as u16 };

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Albums as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x60,  // Match reference (was 0xe0)
        0x00,
        0x24,
        0x1a,  // Match reference (was 0x3c)
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
        let idx_shift = (idx as u16) * 0x20;  // index_shift increments by 0x20 per row
        heap.extend_from_slice(&idx_shift.to_le_bytes());
        heap.extend_from_slice(&0u32.to_le_bytes()); // unknown2
        // Reference exports have artist_id=0 in album rows (no artist linkage)
        heap.extend_from_slice(&0u32.to_le_bytes()); // artist_id (always 0 to match reference)
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

        // Pad row to 40 bytes (0x28) to match reference
        let row_size = heap.len() - row_start;
        const ALBUM_ROW_SIZE: usize = 40;
        if row_size < ALBUM_ROW_SIZE {
            heap.extend(std::iter::repeat(0u8).take(ALBUM_ROW_SIZE - row_size));
        }

        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    let padding_needed = page_padding(heap.len(), albums.len())?;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed])?;
    }

    write_row_groups(file, albums.len(), &row_offsets, row_group_unknown_high_bit)?;

    // Patch usage sizes for this page
    let free_size = padding_needed as u16;
    let used_size = heap.len() as u16;
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
    page_unknown1: u32,
    unknown4: u8,
    start_track_id: u32,
) -> Result<()> {
    log::debug!("Writing tracks table: {} tracks (starting ID: {})", tracks.len(), start_track_id);

    let num_rows_small = tracks.len().min(0xff) as u8;
    // num_rows_large is (num_rows - 1) for data pages with rows, per reference export
    let num_rows_large = if tracks.is_empty() { 0 } else { (tracks.len() - 1) as u16 };

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Tracks as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x60,      // unknown3 - matches reference (0x60)
        unknown4,  // unknown4 - 0x00 from reference
        0x24,      // page_flags
        page_unknown1,
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
        let track_id = start_track_id + idx as u32;

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
        // Reference shows track_id + 20 (e.g., track 1 = 21, track 2 = 22, track 3 = 23)
        heap.extend_from_slice(&((track_id + 20) as u32).to_le_bytes());

        // 0x18-0x19: u3 (unknown, constant 0xe5b6 in reference exports)
        heap.extend_from_slice(&0xe5b6u16.to_le_bytes());

        // 0x1A-0x1B: u4 (unknown, constant 0x6a76 in reference exports)
        heap.extend_from_slice(&0x6a76u16.to_le_bytes());

        // 0x1C-0x1F: artwork_id
        heap.extend_from_slice(&0u32.to_le_bytes());

        // 0x20-0x23: key_id
        // Use detected key from analysis, or track metadata, or 0 (no key)
        let key_id = track_meta
            .analysis
            .key
            .or(track.key)
            .map(|k| k.to_rekordbox_id())
            .unwrap_or(0);
        heap.extend_from_slice(&key_id.to_le_bytes());

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

        // 0x38-0x3B: tempo (BPM * 100) - prefer analyzed BPM, fallback to track metadata
        let tempo = track_meta
            .analysis
            .bpm
            .or(track.bpm)
            .map(|bpm| (bpm * 100.0) as u32)
            .unwrap_or(0);
        heap.extend_from_slice(&tempo.to_le_bytes());

        // 0x3C-0x3F: genre_id
        let genre_id = track
            .genre
            .as_ref()
            .and_then(|g| entities.genre_map.get(g))
            .copied()
            .unwrap_or(1);  // Default to 1 if no genre
        heap.extend_from_slice(&genre_id.to_le_bytes());

        // 0x40-0x43: album_id
        heap.extend_from_slice(&album_id.to_le_bytes());

        // 0x44-0x47: artist_id
        heap.extend_from_slice(&artist_id.to_le_bytes());

        // 0x48-0x4B: id (track ID)
        heap.extend_from_slice(&track_id.to_le_bytes());

        // 0x4C-0x4D: disc_number (0 = not set, matches reference export)
        heap.extend_from_slice(&0u16.to_le_bytes());

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

        // 0x59: rating (0-5 stars as per Deep Symmetry docs)
        let rating = track.rating.unwrap_or(0).min(5);
        heap.push(rating);

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
        // 0: isrc, 1: lyricist, 2: unknown2 (sample depth?), 3: unknown3 (flag),
        // 4: unknown4, 5: message, 6: publish_track_info, 7: autoload_hotcues,
        // 8-9: unknown, 10: date_added, 11: release_date, 12: mix_name, 13: unknown,
        // 14: analyze_path, 15: analyze_date, 16: comment, 17: title,
        // 18: unknown, 19: filename, 20: file_path
        let mut string_data = Vec::new();

        // Initialize string offsets - we'll set each one as we add strings
        // Each string gets its own position, matching reference pattern
        let mut string_offsets: Vec<u16> = vec![0; 21];

        // Helper to add a string and record its offset
        let add_string = |index: usize, data: &[u8], offsets: &mut Vec<u16>, buffer: &mut Vec<u8>| {
            offsets[index] = (string_data_start + buffer.len()) as u16;
            buffer.extend_from_slice(data);
        };

        // String 0: isrc (empty)
        add_string(0, &[0x03], &mut string_offsets, &mut string_data);

        // String 1: lyricist (empty)
        add_string(1, &[0x03], &mut string_offsets, &mut string_data);

        // String 2: unknown2 - value "4" (matches reference export)
        add_string(2, &encode_device_sql("4"), &mut string_offsets, &mut string_data);

        // String 3: unknown3 - flag byte 0x01 (matches reference pattern)
        // This is encoded as DeviceSQL short string with 1 byte content
        add_string(3, &[0x05, 0x01], &mut string_offsets, &mut string_data);

        // String 4: unknown4 (empty)
        add_string(4, &[0x03], &mut string_offsets, &mut string_data);

        // String 5: message (empty)
        add_string(5, &[0x03], &mut string_offsets, &mut string_data);

        // String 6: publish_track_info (empty)
        add_string(6, &[0x03], &mut string_offsets, &mut string_data);

        // String 7: autoload_hotcues - "ON" (critical for proper XDJ behavior)
        add_string(7, &encode_device_sql("ON"), &mut string_offsets, &mut string_data);

        // String 8: unknown8 (empty)
        add_string(8, &[0x03], &mut string_offsets, &mut string_data);

        // String 9: unknown9 (empty)
        add_string(9, &[0x03], &mut string_offsets, &mut string_data);

        // String 10: date_added (format: YYYY-MM-DD)
        let date_added = chrono::Local::now().format("%Y-%m-%d").to_string();
        add_string(10, &encode_device_sql(&date_added), &mut string_offsets, &mut string_data);

        // String 11: release_date (empty)
        add_string(11, &[0x03], &mut string_offsets, &mut string_data);

        // String 12: mix_name (empty)
        add_string(12, &[0x03], &mut string_offsets, &mut string_data);

        // String 13: unknown13 (empty)
        add_string(13, &[0x03], &mut string_offsets, &mut string_data);

        // String 14: analyze_path
        let anlz_path_str = track_meta.anlz_path.to_string_lossy();
        add_string(14, &encode_device_sql(&anlz_path_str), &mut string_offsets, &mut string_data);

        // String 15: analyze_date (format: YYYY-MM-DD)
        let analyze_date = chrono::Local::now().format("%Y-%m-%d").to_string();
        add_string(15, &encode_device_sql(&analyze_date), &mut string_offsets, &mut string_data);

        // String 16: comment - ALWAYS empty for now (some Rhythmbox comments contain garbage)
        add_string(16, &[0x03], &mut string_offsets, &mut string_data);

        // String 17: title (CRITICAL)
        add_string(17, &encode_device_sql(&track.title), &mut string_offsets, &mut string_data);

        // String 18: unknown18 (empty)
        add_string(18, &[0x03], &mut string_offsets, &mut string_data);

        // String 19: filename
        let filename = track_meta.file_path.file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        add_string(19, &encode_device_sql(&filename), &mut string_offsets, &mut string_data);

        // String 20: file_path (CRITICAL)
        let file_path_str = track_meta.file_path.to_string_lossy();
        add_string(20, &encode_device_sql(&file_path_str), &mut string_offsets, &mut string_data);

        // Write string offset array
        for offset in &string_offsets {
            heap.extend_from_slice(&offset.to_le_bytes());
        }

        // Verify offset array position
        assert_eq!(heap.len() - row_start, 0x88, "String offset array should end at 0x88");

        // Write string data
        heap.extend_from_slice(&string_data);
        align_to_4(&mut heap); // Pad row to 4-byte alignment

        // For reference-matching, add extra padding to align rows to reference positions
        // Reference row offsets: 0x000, 0x158, 0x2B4 (for first 3 tracks)
        const REFERENCE_ROW_OFFSETS: &[usize] = &[0x000, 0x158, 0x2B4, 0x410]; // Add extra for potential 4th track
        if idx + 1 < REFERENCE_ROW_OFFSETS.len() && idx + 1 < tracks.len() {
            let next_expected = REFERENCE_ROW_OFFSETS[idx + 1];
            let current_pos = heap.len();
            if next_expected > current_pos {
                let padding = next_expected - current_pos;
                heap.extend(std::iter::repeat(0u8).take(padding));
            }
        }

        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    let padding_needed = page_padding(heap.len(), tracks.len())?;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed])?;
    }

    // Row group unknown field: use highest bit pattern (same as other tables)
    write_row_groups(file, tracks.len(), &row_offsets, row_group_unknown_high_bit)?;

    let free_size = padding_needed as u16;
    let used_size = heap.len() as u16;
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
    // num_rows_large is (num_rows - 1) for data pages with rows, per reference export
    let num_rows_large = if playlists.is_empty() { 0 } else { (playlists.len() - 1) as u16 };
    write_page_header(
        file,
        page_index,
        TableType::PlaylistTree as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x60,  // Match reference (was 0x40)
        0x00,
        0x24,
        0x08,  // Match reference (was 0x07)
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
        align_to_4(&mut heap); // Pad row to 4-byte alignment

        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    let padding_needed = page_padding(heap.len(), playlists.len())?;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed])?;
    }

    write_row_groups(file, playlists.len(), &row_offsets, row_group_unknown_high_bit)?;

    let free_size = padding_needed as u16;
    let used_size = heap.len() as u16;
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
    // num_rows_large is (num_rows - 1) for data pages with rows, per reference export
    let num_rows_large = if total_entries == 0 { 0 } else { (total_entries - 1) as u16 };

    log::debug!("Writing playlist entries table: {} entries", total_entries);

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::PlaylistEntries as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0xc0,  // Match reference (was 0xa0)
        0x00,  // Match reference (was 0x01)
        0x24,
        0x1d,  // Match reference (was 0x42)
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

    let padding_needed = page_padding(heap.len(), total_entries)?;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed])?;
    }

    write_row_groups(file, total_entries, &row_offsets, row_group_unknown_high_bit)?;

    let free_size = padding_needed as u16;
    let used_size = heap.len() as u16;
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
    let num_rows_large = 0u16; // Reference often has 0

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Colors as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x00,
        0x01,
        0x24,
        0x0002,
        0,
        0x0008,
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
        align_to_4(&mut heap); // Pad row to 4-byte alignment

        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    let padding_needed = page_padding(heap.len(), num_rows)?;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed])?;
    }

    write_row_groups(file, num_rows, &row_offsets, |flags| flags)?;

    let free_size = padding_needed as u16;
    let used_size = heap.len() as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}

/// Write keys table with all 24 musical keys
/// Key row structure (matches reference exactly):
///   - u32: key_id (1-based)
///   - u32: key_id2 (same as key_id)
///   - DeviceSQLString: name (e.g., "Am", "Em")
fn write_keys_table(file: &mut File, page_index: u32, next_page: u32) -> Result<()> {
    // All 24 musical keys (12 minor + 12 major)
    // Rekordbox key IDs follow this ordering
    let keys = [
        // Minor keys (1-12)
        (1u32, "Am"),
        (2u32, "Bbm"),
        (3u32, "Bm"),
        (4u32, "Cm"),
        (5u32, "Dbm"),
        (6u32, "Dm"),
        (7u32, "Ebm"),
        (8u32, "Em"),
        (9u32, "Fm"),
        (10u32, "Gbm"),
        (11u32, "Gm"),
        (12u32, "Abm"),
        // Major keys (13-24)
        (13u32, "A"),
        (14u32, "Bb"),
        (15u32, "B"),
        (16u32, "C"),
        (17u32, "Db"),
        (18u32, "D"),
        (19u32, "Eb"),
        (20u32, "E"),
        (21u32, "F"),
        (22u32, "Gb"),
        (23u32, "G"),
        (24u32, "Ab"),
    ];

    log::debug!("Writing keys table: {} keys", keys.len());

    let num_rows = keys.len();
    let num_rows_small = num_rows.min(0xff) as u8;
    // num_rows_large is (num_rows - 1) for data pages with rows, per reference export
    let num_rows_large = if keys.is_empty() { 0 } else { (keys.len() - 1) as u16 };

    let page_start = file.stream_position()?;
    write_page_header(
        file,
        page_index,
        TableType::Keys as u32,
        next_page,
        num_rows_small,
        num_rows_large,
        0x60,  // Match reference (was 0x20)
        0x00,
        0x24,
        0x1b, // Match reference (was 0x0a)
        0,
        0x0001, // unknown5
        0,
        0,
    )?;

    let mut heap = Vec::new();
    let mut row_offsets = Vec::new();

    for (key_id, name) in &keys {
        let row_start = heap.len();

        // Key row structure (8 bytes header + string)
        heap.extend_from_slice(&key_id.to_le_bytes()); // key_id
        heap.extend_from_slice(&key_id.to_le_bytes()); // key_id2 (same value)

        // Encode and append name string (DeviceSQL short string)
        let encoded_name = encode_device_sql(name);
        heap.extend_from_slice(&encoded_name);
        align_to_4(&mut heap); // Pad row to 4-byte alignment

        row_offsets.push(row_start as u16);
    }

    file.write_all(&heap)?;

    let padding_needed = page_padding(heap.len(), num_rows)?;
    if padding_needed > 0 {
        file.write_all(&vec![0u8; padding_needed])?;
    }

    // Keys row group unknown: use highest bit pattern (matches reference)
    write_row_groups(file, num_rows, &row_offsets, row_group_unknown_high_bit)?;

    let free_size = padding_needed as u16;
    let used_size = heap.len() as u16;
    patch_page_usage(file, page_start, free_size, used_size)?;

    Ok(())
}

/// Reference columns table data page (page 34)
/// Extracted from examples/PIONEER/rekordbox/export.pdb
/// Contains 27 standard column definitions for XDJ browser
/// The columns table has a complex row group structure that the XDJ is very
/// sensitive to, so we use the known-good reference data directly.
const REFERENCE_COLUMNS_PAGE: &[u8; 4096] = include_bytes!("reference_columns.bin");

/// Reference history playlists data page (page 36)
/// Extracted from examples/PIONEER/rekordbox/export.pdb
/// Contains history playlist entries that XDJ expects to be populated
const REFERENCE_HISTORY_PLAYLISTS_PAGE: &[u8; 4096] = include_bytes!("reference_history_playlists.bin");

/// Reference history entries data page (page 38)
const REFERENCE_HISTORY_ENTRIES_PAGE: &[u8; 4096] = include_bytes!("reference_history_entries.bin");

/// Reference history data page (page 40)
const REFERENCE_HISTORY_PAGE: &[u8; 4096] = include_bytes!("reference_history.bin");

/// Write columns table (browse categories)
/// Uses the reference page data directly since XDJ is sensitive to row group layout
fn write_columns_table(file: &mut File, _columns: &[ColumnEntry], _page_index: u32, _next_page: u32) -> Result<()> {
    log::debug!("Writing columns table: using reference data (27 entries)");

    // Write the reference page data directly
    // This ensures byte-perfect compatibility with XDJ hardware
    file.write_all(REFERENCE_COLUMNS_PAGE)?;

    Ok(())
}
