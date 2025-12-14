use super::{Playlist, Track};
use std::collections::HashMap;

/// Complete music library containing tracks and playlists
#[derive(Debug, Clone)]
pub struct Library {
    /// All tracks indexed by their ID
    tracks: HashMap<String, Track>,

    /// All playlists
    playlists: Vec<Playlist>,
}

impl Library {
    /// Create a new empty library
    pub fn new() -> Self {
        Self {
            tracks: HashMap::new(),
            playlists: Vec::new(),
        }
    }

    /// Add a track to the library
    pub fn add_track(&mut self, track: Track) {
        self.tracks.insert(track.id.clone(), track);
    }

    /// Add a playlist to the library
    pub fn add_playlist(&mut self, playlist: Playlist) {
        self.playlists.push(playlist);
    }

    /// Get a track by ID
    pub fn get_track(&self, id: &str) -> Option<&Track> {
        self.tracks.get(id)
    }

    /// Get all tracks
    pub fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.tracks.values()
    }

    /// Get all playlists
    pub fn playlists(&self) -> &[Playlist] {
        &self.playlists
    }

    /// Total number of tracks
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    /// Total number of playlists
    pub fn playlist_count(&self) -> usize {
        self.playlists.len()
    }
}

impl Default for Library {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_library_creation() {
        let lib = Library::new();
        assert_eq!(lib.track_count(), 0);
        assert_eq!(lib.playlist_count(), 0);
    }

    #[test]
    fn test_add_track() {
        let mut lib = Library::new();

        let track = Track {
            id: "test123".to_string(),
            title: "Test Song".to_string(),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            genre: Some("Electronic".to_string()),
            duration_ms: 180000,
            bpm: Some(128.0),
            key: None,
            file_path: PathBuf::from("/music/test.mp3"),
            file_size: 5000000,
            track_number: Some(1),
            year: Some(2024),
            comment: None,
        };

        lib.add_track(track.clone());

        assert_eq!(lib.track_count(), 1);
        assert!(lib.get_track("test123").is_some());
        assert_eq!(lib.get_track("test123").unwrap().title, "Test Song");
    }

    #[test]
    fn test_add_playlist() {
        let mut lib = Library::new();

        let playlist = Playlist::new("My Playlist".to_string());
        lib.add_playlist(playlist);

        assert_eq!(lib.playlist_count(), 1);
        assert_eq!(lib.playlists()[0].name, "My Playlist");
    }
}
