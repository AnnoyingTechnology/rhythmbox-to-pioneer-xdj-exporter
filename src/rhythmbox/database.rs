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
                            // Rhythmbox uses days since year 1 AD (proleptic Gregorian)
                            // Formula: year ≈ (date * 400) / 146097 + 1
                            if let Ok(date) = text.parse::<u32>() {
                                if date > 0 {
                                    // More accurate year calculation
                                    let year = ((date as u64 * 400) / 146097 + 1) as u32;
                                    entry.year = Some(year);
                                }
                            }
                        }
                        "comment" => entry.comment = Some(text),
                        "rating" => {
                            // Rhythmbox stores rating as 0-5 (0=unrated, 1-5=stars)
                            if let Ok(rating) = text.parse::<u8>() {
                                entry.rating = Some(rating);
                            }
                        }
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

    // Read rating from audio file ID3 tags (POPM frame)
    // Falls back to Rhythmbox XML rating if ID3 read fails
    let rating = read_rating_from_file(&file_path).or(entry.rating);

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
        rating,
    })
}

/// Read rating from audio file ID3/POPM tags
/// Returns rating as 0-5 stars (converted from ID3's 0-255 scale)
fn read_rating_from_file(path: &std::path::Path) -> Option<u8> {
    use lofty::file::TaggedFileExt;
    use lofty::probe::Probe;
    use lofty::tag::{ItemKey, TagType};

    let tagged_file = Probe::open(path).ok()?.read().ok()?;

    // Try to get the ID3v2 tag directly (for MP3s)
    let tag = tagged_file.tag(TagType::Id3v2)?;

    // The tag is a unified Tag, try to get POPM via item key
    for item in tag.items() {
        if item.key() == &ItemKey::Popularimeter {
            if let lofty::tag::ItemValue::Binary(data) = item.value() {
                // Parse POPM frame: find null terminator after email, then rating byte
                if let Some(null_pos) = data.iter().position(|&b| b == 0) {
                    if data.len() > null_pos + 1 {
                        let rating_byte = data[null_pos + 1];
                        let stars = id3_rating_to_stars(rating_byte)?;
                        log::debug!("Read rating {} (ID3 {}) from {:?}", stars, rating_byte, path);
                        return Some(stars);
                    }
                }
            }
        }
    }

    // POPM isn't in unified items - need to access raw ID3v2 frames
    // Use the id3 crate directly for more reliable POPM access
    if let Ok(id3_tag) = id3::Tag::read_from_path(path) {
        // Look for POPM frames (there can be multiple with different emails)
        for frame in id3_tag.frames() {
            if frame.id() == "POPM" {
                if let id3::Content::Popularimeter(popm) = frame.content() {
                    let stars = id3_rating_to_stars(popm.rating)?;
                    log::debug!("Read rating {} (ID3 {}) from {:?}", stars, popm.rating, path);
                    return Some(stars);
                }
            }
        }
    }

    None
}

/// Convert ID3 rating (1-255) to stars (1-5)
/// Standard mapping: 1=1, 64=2, 128=3, 196=4, 255=5
/// 0 = unrated
fn id3_rating_to_stars(rating_byte: u8) -> Option<u8> {
    match rating_byte {
        0 => None,         // Unrated
        1..=31 => Some(1), // 1 star
        32..=95 => Some(2),  // 2 stars
        96..=159 => Some(3), // 3 stars
        160..=223 => Some(4), // 4 stars
        224..=255 => Some(5), // 5 stars
    }
}
