//! Rhythmbox playlists (playlists.xml) parser

use crate::model::{Playlist, Track};
use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

/// Parse playlists.xml and extract all playlists
///
/// # Arguments
/// * `path` - Path to playlists.xml
/// * `tracks` - All tracks from the library (to resolve playlist entries)
pub fn parse_playlists(path: &Path, tracks: &[Track]) -> Result<Vec<Playlist>> {
    // Build a map from file path to track ID for quick lookups
    let path_to_id: HashMap<PathBuf, String> = tracks
        .iter()
        .map(|t| (t.file_path.clone(), t.id.clone()))
        .collect();

    let file = File::open(path)
        .with_context(|| format!("Failed to open Rhythmbox playlists: {:?}", path))?;

    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.config_mut().trim_text(true);

    let mut playlists = Vec::new();
    let mut current_playlist: Option<Playlist> = None;
    let mut current_element = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                match name.as_ref() {
                    b"playlist" => {
                        // Get playlist name and type
                        let mut playlist_name = String::from("Unnamed");
                        let mut is_valid = true;

                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                match attr.key.as_ref() {
                                    b"name" => {
                                        playlist_name =
                                            String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    b"type" => {
                                        // Skip automatic playlists (we want static playlists)
                                        let ptype = String::from_utf8_lossy(&attr.value);
                                        if ptype == "automatic" {
                                            is_valid = false;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }

                        if is_valid {
                            current_playlist = Some(Playlist::new(playlist_name));
                        }
                    }
                    b"location" => {
                        // We're entering a location element
                        if current_playlist.is_some() {
                            current_element = "location".to_string();
                        }
                    }
                    _ => {}
                }
            }

            Ok(Event::Text(e)) => {
                // Text content inside an element
                if current_element == "location" {
                    if let Some(ref mut playlist) = current_playlist {
                        let location_uri = e.unescape().unwrap_or_default().to_string();

                        // Convert file:// URI to path
                        if let Some(file_path) = uri_to_path(&location_uri) {
                            // Look up track ID from path
                            if let Some(track_id) = path_to_id.get(&file_path) {
                                playlist.add_track(track_id.clone());
                            } else {
                                log::debug!("Track not found for path: {:?}", file_path);
                            }
                        }
                    }
                }
            }

            Ok(Event::End(e)) => {
                let name = e.name();
                if name.as_ref() == b"playlist" {
                    // Playlist complete
                    if let Some(playlist) = current_playlist.take() {
                        if !playlist.is_empty() {
                            playlists.push(playlist);
                        } else {
                            log::debug!("Skipping empty playlist: {}", playlist.name);
                        }
                    }
                } else if name.as_ref() == b"location" {
                    // End of location element
                    current_element.clear();
                }
            }

            Ok(Event::Eof) => break,
            Err(e) => {
                log::warn!("XML parsing error: {:?}", e);
            }
            _ => {}
        }

        buf.clear();
    }

    log::info!("Parsed {} playlists from Rhythmbox", playlists.len());
    Ok(playlists)
}

/// Convert file:// URI to PathBuf
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    uri.strip_prefix("file://")
        .and_then(|path| urlencoding::decode(path).ok())
        .map(|decoded| PathBuf::from(decoded.into_owned()))
}
