//! Rhythmbox playlists (playlists.xml) parser
//!
//! Supports both static playlists (explicit track list) and
//! smart/automatic playlists (filter criteria applied to library).

use crate::model::{Playlist, Track};
use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

/// Smart playlist filter criterion
#[derive(Debug, Clone)]
enum Criterion {
    Equals { prop: String, value: String },
    Like { prop: String, value: String },
    Less { prop: String, value: String },
    Greater { prop: String, value: String },
    And(Vec<Criterion>),
    Or(Vec<Criterion>),
}

impl Criterion {
    /// Evaluate criterion against a track
    fn matches(&self, track: &Track) -> bool {
        match self {
            Criterion::Equals { prop, value } => {
                let track_value = get_track_property(track, prop);
                track_value.eq_ignore_ascii_case(value)
            }
            Criterion::Like { prop, value } => {
                let track_value = get_track_property(track, prop);
                track_value.to_lowercase().contains(&value.to_lowercase())
            }
            Criterion::Less { prop, value } => {
                if let (Ok(track_num), Ok(cmp_num)) = (
                    get_track_property(track, prop).parse::<i64>(),
                    value.parse::<i64>(),
                ) {
                    track_num < cmp_num
                } else {
                    false
                }
            }
            Criterion::Greater { prop, value } => {
                if let (Ok(track_num), Ok(cmp_num)) = (
                    get_track_property(track, prop).parse::<i64>(),
                    value.parse::<i64>(),
                ) {
                    track_num > cmp_num
                } else {
                    false
                }
            }
            Criterion::And(criteria) => criteria.iter().all(|c| c.matches(track)),
            Criterion::Or(criteria) => criteria.iter().any(|c| c.matches(track)),
        }
    }
}

/// Get a property value from a track
fn get_track_property(track: &Track, prop: &str) -> String {
    match prop {
        "type" => "song".to_string(), // All our tracks are songs
        "genre" | "genre-folded" => track.genre.clone().unwrap_or_default(),
        "artist" | "artist-folded" => track.artist.clone(),
        "album" | "album-folded" => track.album.clone(),
        "title" | "title-folded" => track.title.clone(),
        "duration" => (track.duration_ms / 1000).to_string(), // Convert ms to seconds
        "year" => track.year.map(|y| y.to_string()).unwrap_or_default(),
        "rating" => "0".to_string(), // We don't track ratings
        "play-count" => "0".to_string(),
        _ => String::new(),
    }
}

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
    let mut current_playlist_type = String::new();
    let mut current_element = String::new();
    let mut buf = Vec::new();

    // For smart playlists: track criteria parsing
    let mut criteria_stack: Vec<(String, Vec<Criterion>)> = Vec::new(); // (type, children)
    let mut current_prop = String::new();
    let mut current_criterion_type = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let name_str = String::from_utf8_lossy(name.as_ref()).to_string();

                match name.as_ref() {
                    b"playlist" => {
                        let mut playlist_name = String::from("Unnamed");
                        current_playlist_type = String::from("static");

                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                match attr.key.as_ref() {
                                    b"name" => {
                                        playlist_name =
                                            String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    b"type" => {
                                        current_playlist_type =
                                            String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    _ => {}
                                }
                            }
                        }

                        current_playlist = Some(Playlist::new(playlist_name));
                        criteria_stack.clear();
                    }
                    b"location" => {
                        if current_playlist.is_some() && current_playlist_type == "static" {
                            current_element = "location".to_string();
                        }
                    }
                    b"conjunction" => {
                        criteria_stack.push(("and".to_string(), Vec::new()));
                    }
                    b"disjunction" => {
                        criteria_stack.push(("or".to_string(), Vec::new()));
                    }
                    b"subquery" => {
                        // Subquery is just a container, treat as AND
                        criteria_stack.push(("and".to_string(), Vec::new()));
                    }
                    b"equals" | b"like" | b"less" | b"greater" => {
                        current_criterion_type = name_str.clone();
                        current_prop.clear();
                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"prop" {
                                    current_prop = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        current_element = name_str;
                    }
                    _ => {}
                }
            }

            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().to_string();

                if current_element == "location" {
                    if let Some(ref mut playlist) = current_playlist {
                        if let Some(file_path) = uri_to_path(&text) {
                            if let Some(track_id) = path_to_id.get(&file_path) {
                                playlist.add_track(track_id.clone());
                            } else {
                                log::debug!("Track not found for path: {:?}", file_path);
                            }
                        }
                    }
                } else if matches!(current_element.as_str(), "equals" | "like" | "less" | "greater") {
                    // Build criterion and add to current stack
                    if !current_prop.is_empty() && !criteria_stack.is_empty() {
                        let criterion = match current_criterion_type.as_str() {
                            "equals" => Criterion::Equals { prop: current_prop.clone(), value: text },
                            "like" => Criterion::Like { prop: current_prop.clone(), value: text },
                            "less" => Criterion::Less { prop: current_prop.clone(), value: text },
                            "greater" => Criterion::Greater { prop: current_prop.clone(), value: text },
                            _ => return Ok(playlists), // Shouldn't happen
                        };
                        if let Some((_, ref mut children)) = criteria_stack.last_mut() {
                            children.push(criterion);
                        }
                    }
                }
            }

            Ok(Event::End(e)) => {
                let name = e.name();
                match name.as_ref() {
                    b"playlist" => {
                        if let Some(mut playlist) = current_playlist.take() {
                            // For smart playlists, apply criteria to all tracks
                            if current_playlist_type == "automatic" && !criteria_stack.is_empty() {
                                // Build final criterion from stack
                                let root_criterion = build_criterion_from_stack(&mut criteria_stack);

                                // Apply to all tracks
                                let mut match_count = 0;
                                for track in tracks {
                                    if root_criterion.as_ref().map(|c| c.matches(track)).unwrap_or(false) {
                                        playlist.add_track(track.id.clone());
                                        match_count += 1;
                                    }
                                }
                                log::debug!(
                                    "Smart playlist '{}': {} tracks matched",
                                    playlist.name,
                                    match_count
                                );
                            }

                            if !playlist.is_empty() {
                                playlists.push(playlist);
                            } else {
                                log::debug!("Skipping empty playlist: {}", playlist.name);
                            }
                        }
                        current_playlist_type.clear();
                    }
                    b"location" | b"equals" | b"like" | b"less" | b"greater" => {
                        current_element.clear();
                    }
                    b"conjunction" | b"disjunction" | b"subquery" => {
                        // Pop from stack and add to parent
                        if let Some((ctype, children)) = criteria_stack.pop() {
                            let compound = if ctype == "and" {
                                Criterion::And(children)
                            } else {
                                Criterion::Or(children)
                            };

                            if let Some((_, ref mut parent_children)) = criteria_stack.last_mut() {
                                parent_children.push(compound);
                            } else {
                                // Root level - push back as single item
                                criteria_stack.push((ctype, vec![compound]));
                            }
                        }
                    }
                    _ => {}
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

/// Build a single criterion from the parsing stack
fn build_criterion_from_stack(stack: &mut Vec<(String, Vec<Criterion>)>) -> Option<Criterion> {
    if stack.is_empty() {
        return None;
    }

    // Flatten all remaining items into one AND
    let mut all_criteria = Vec::new();
    while let Some((ctype, children)) = stack.pop() {
        if children.len() == 1 {
            all_criteria.push(children.into_iter().next().unwrap());
        } else if !children.is_empty() {
            let compound = if ctype == "and" {
                Criterion::And(children)
            } else {
                Criterion::Or(children)
            };
            all_criteria.push(compound);
        }
    }

    if all_criteria.len() == 1 {
        Some(all_criteria.pop().unwrap())
    } else if !all_criteria.is_empty() {
        Some(Criterion::And(all_criteria))
    } else {
        None
    }
}

/// Convert file:// URI to PathBuf
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    uri.strip_prefix("file://")
        .and_then(|path| urlencoding::decode(path).ok())
        .map(|decoded| PathBuf::from(decoded.into_owned()))
}
