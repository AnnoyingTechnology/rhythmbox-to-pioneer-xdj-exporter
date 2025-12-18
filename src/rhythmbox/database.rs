//! Rhythmbox database (rhythmdb.xml) parser

use super::model::RhythmboxEntry;
use crate::model::Track;
use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Parse rhythmdb.xml and extract all music tracks
pub fn parse_database(path: &Path) -> Result<Vec<Track>> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open Rhythmbox database: {:?}", path))?;

    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.config_mut().trim_text(true);

    let mut tracks = Vec::new();
    let mut current_entry: Option<RhythmboxEntry> = None;
    let mut current_element = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                match name.as_ref() {
                    b"entry" => {
                        // Check if this is a song entry (type="song")
                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"type" && attr.value.as_ref() == b"song" {
                                    current_entry = Some(RhythmboxEntry::new());
                                    break;
                                }
                            }
                        }
                    }
                    _ => {
                        // Track element name for text content
                        if current_entry.is_some() {
                            current_element = String::from_utf8_lossy(name.as_ref()).to_string();
                        }
                    }
                }
            }

            Ok(Event::Text(e)) => {
                if let Some(ref mut entry) = current_entry {
                    let text = e.unescape().unwrap_or_default().to_string();

                    // Populate entry based on current element name
                    match current_element.as_str() {
                        "title" => entry.title = Some(text),
                        "artist" => entry.artist = Some(text),
                        "album" => entry.album = Some(text),
                        "genre" => entry.genre = Some(text),
                        "location" => entry.location = Some(text),
                        "duration" => {
                            if let Ok(duration) = text.parse::<u32>() {
                                entry.duration = Some(duration);
                            }
                        }
                        "track-number" => {
                            if let Ok(track_num) = text.parse::<u32>() {
                                entry.track_number = Some(track_num);
                            }
                        }
                        "beats-per-minute" => {
                            if let Ok(bpm) = text.parse::<f32>() {
                                entry.bpm = Some(bpm);
                            }
                        }
                        "date" => {
                            // Rhythmbox date field needs conversion
                            if let Ok(date) = text.parse::<u32>() {
                                // Convert Julian date to year (approximate)
                                if date > 0 {
                                    entry.year = Some(1970 + (date / 365));
                                }
                            }
                        }
                        "comment" => entry.comment = Some(text),
                        _ => {}
                    }
                }
            }

            Ok(Event::End(e)) => {
                let name = e.name();
                if name.as_ref() == b"entry" {
                    // Entry complete, convert to Track
                    if let Some(entry) = current_entry.take() {
                        if let Some(track) = convert_entry_to_track(&entry) {
                            tracks.push(track);
                        }
                    }
                    current_element.clear();
                } else {
                    // End of element, clear current element name
                    current_element.clear();
                }
            }

            Ok(Event::Eof) => break,
            Err(e) => {
                log::warn!(
                    "XML parsing error at position {}: {:?}",
                    reader.buffer_position(),
                    e
                );
            }
            _ => {}
        }

        buf.clear();
    }

    log::info!("Parsed {} tracks from Rhythmbox database", tracks.len());
    Ok(tracks)
}

/// Convert a RhythmboxEntry to our unified Track model
fn convert_entry_to_track(entry: &RhythmboxEntry) -> Option<Track> {
    // Need at minimum: title, location
    let title = entry.title.clone()?;
    let file_path = entry.get_file_path()?;

    // Generate a unique ID (use file path as basis)
    let id = format!("{:x}", md5::compute(file_path.to_string_lossy().as_bytes()));

    // Get file size
    let file_size = std::fs::metadata(&file_path).ok()?.len();

    Some(Track {
        id,
        title,
        artist: entry
            .artist
            .clone()
            .unwrap_or_else(|| "Unknown Artist".to_string()),
        album: entry
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string()),
        genre: entry.genre.clone(),
        duration_ms: entry.duration.unwrap_or(0) * 1000, // Convert seconds to ms
        bpm: entry.bpm,
        key: None, // Phase 1: no key detection
        file_path,
        file_size,
        track_number: entry.track_number,
        year: entry.year,
        comment: entry.comment.clone(),
    })
}
