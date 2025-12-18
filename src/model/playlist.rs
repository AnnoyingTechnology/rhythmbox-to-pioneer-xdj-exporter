use serde::{Deserialize, Serialize};

/// Represents a playlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    /// Playlist name
    pub name: String,

    /// Playlist entries (ordered)
    pub entries: Vec<PlaylistEntry>,

    /// Whether this is a folder (can contain sub-playlists)
    /// Phase 1: flat playlists only, Phase 2: support folders
    pub is_folder: bool,
}

/// Entry in a playlist, referencing a track by ID
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistEntry {
    /// Track ID (references Track::id)
    pub track_id: String,

    /// Position in playlist (0-based)
    pub position: u32,
}

impl Playlist {
    /// Create a new empty playlist
    pub fn new(name: String) -> Self {
        Self {
            name,
            entries: Vec::new(),
            is_folder: false,
        }
    }

    /// Add a track to this playlist
    pub fn add_track(&mut self, track_id: String) {
        let position = self.entries.len() as u32;
        self.entries.push(PlaylistEntry { track_id, position });
    }

    /// Number of tracks in this playlist
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if playlist is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
