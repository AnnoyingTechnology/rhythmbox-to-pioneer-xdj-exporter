//! Rhythmbox library parsing
//!
//! Parses Rhythmbox's XML database and playlist files to extract
//! track metadata and playlist structure.

mod database;
mod playlists;
mod model;

pub use database::parse_database;
pub use playlists::parse_playlists;

use crate::model::Library;
use anyhow::Result;
use std::path::Path;

/// Parse a complete Rhythmbox library from its XML files
///
/// # Arguments
/// * `db_path` - Path to rhythmdb.xml (typically ~/.local/share/rhythmbox/rhythmdb.xml)
/// * `playlists_path` - Path to playlists.xml (typically ~/.local/share/rhythmbox/playlists.xml)
///
/// # Returns
/// A unified Library containing all tracks and playlists
pub fn parse_library(db_path: &Path, playlists_path: &Path) -> Result<Library> {
    log::info!("Parsing Rhythmbox database from {:?}", db_path);
    let tracks = database::parse_database(db_path)?;

    log::info!("Parsing Rhythmbox playlists from {:?}", playlists_path);
    let playlists = playlists::parse_playlists(playlists_path, &tracks)?;

    let mut library = Library::new();

    for track in tracks {
        library.add_track(track);
    }

    for playlist in playlists {
        library.add_playlist(playlist);
    }

    log::info!(
        "Loaded library: {} tracks, {} playlists",
        library.track_count(),
        library.playlist_count()
    );

    Ok(library)
}
