//! Artwork extraction and processing for Pioneer exports
//!
//! Extracts embedded artwork from audio files, resizes to Pioneer format,
//! and saves to the appropriate USB directory structure.

use anyhow::{Context, Result};
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};
use lofty::file::TaggedFileExt;
use lofty::picture::PictureType;
use lofty::probe::Probe;
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Standard artwork size for Pioneer (small thumbnail)
pub const ARTWORK_SIZE_SMALL: u32 = 80;

/// High-resolution artwork size for Pioneer (large thumbnail)
pub const ARTWORK_SIZE_LARGE: u32 = 240;

/// Artwork data extracted from an audio file
#[derive(Debug, Clone)]
pub struct ArtworkData {
    /// Raw image bytes (original from audio file)
    pub data: Vec<u8>,
    /// MIME type of the image
    pub mime_type: String,
}

/// Processed artwork ready for export
#[derive(Debug)]
pub struct ProcessedArtwork {
    /// Artwork ID (matches the ID in PDB)
    pub id: u32,
    /// Path relative to PIONEER directory (e.g., "/PIONEER/Artwork/00001/a1.jpg")
    pub path: String,
    /// Small image bytes (80x80 JPEG)
    pub small_image: Vec<u8>,
    /// Large image bytes (240x240 JPEG)
    pub large_image: Vec<u8>,
}

/// Manages artwork extraction, deduplication, and export
pub struct ArtworkManager {
    /// Maps artwork hash to artwork ID for deduplication
    artwork_hashes: HashMap<String, u32>,
    /// List of unique artwork entries
    artworks: Vec<ProcessedArtwork>,
    /// Next artwork ID to assign
    next_id: u32,
}

impl ArtworkManager {
    /// Create a new artwork manager
    pub fn new() -> Self {
        Self {
            artwork_hashes: HashMap::new(),
            artworks: Vec::new(),
            next_id: 1,
        }
    }

    /// Extract artwork from an audio file
    /// Returns the artwork data if found, None otherwise
    pub fn extract_from_file(path: &Path) -> Result<Option<ArtworkData>> {
        let tagged_file = Probe::open(path)
            .with_context(|| format!("Failed to open audio file: {}", path.display()))?
            .read()
            .with_context(|| format!("Failed to read tags from: {}", path.display()))?;

        // Try to find artwork in any tag
        for tag in tagged_file.tags() {
            // Look for front cover first, then any picture
            if let Some(picture) = tag
                .pictures()
                .iter()
                .find(|p| p.pic_type() == PictureType::CoverFront)
                .or_else(|| tag.pictures().first())
            {
                return Ok(Some(ArtworkData {
                    data: picture.data().to_vec(),
                    mime_type: picture.mime_type().map(|m| m.to_string()).unwrap_or_else(|| "image/jpeg".to_string()),
                }));
            }
        }

        Ok(None)
    }

    /// Process artwork and add to the manager
    /// Returns the artwork ID (reuses existing ID if artwork is duplicate)
    pub fn add_artwork(&mut self, artwork: &ArtworkData) -> Result<u32> {
        // Create hash for deduplication
        let hash = format!("{:x}", md5::compute(&artwork.data));

        // Check if we already have this artwork
        if let Some(&existing_id) = self.artwork_hashes.get(&hash) {
            log::debug!("Reusing existing artwork ID {} (duplicate)", existing_id);
            return Ok(existing_id);
        }

        // Process the artwork
        let processed = self.process_artwork(&artwork.data)?;
        let id = self.next_id;

        // Create the processed artwork entry
        let artwork_entry = ProcessedArtwork {
            id,
            path: format!("/PIONEER/Artwork/00001/a{}.jpg", id),
            small_image: processed.0,
            large_image: processed.1,
        };

        self.artwork_hashes.insert(hash, id);
        self.artworks.push(artwork_entry);
        self.next_id += 1;

        log::debug!("Added new artwork ID {}", id);
        Ok(id)
    }

    /// Process artwork: decode, resize to 80x80 and 240x240, encode as JPEG
    fn process_artwork(&self, data: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        // Decode the image
        let img = image::load_from_memory(data)
            .context("Failed to decode artwork image")?;

        // Resize to small (80x80)
        let small = img.resize_exact(
            ARTWORK_SIZE_SMALL,
            ARTWORK_SIZE_SMALL,
            FilterType::Lanczos3,
        );

        // Resize to large (240x240)
        let large = img.resize_exact(
            ARTWORK_SIZE_LARGE,
            ARTWORK_SIZE_LARGE,
            FilterType::Lanczos3,
        );

        // Encode as JPEG
        let small_bytes = encode_jpeg(&small, 90)?;
        let large_bytes = encode_jpeg(&large, 90)?;

        Ok((small_bytes, large_bytes))
    }

    /// Write all artwork files to the output directory
    pub fn write_artwork_files(&self, output_dir: &Path) -> Result<()> {
        if self.artworks.is_empty() {
            log::info!("No artwork to write");
            return Ok(());
        }

        // Create artwork directory
        let artwork_dir = output_dir.join("PIONEER/Artwork/00001");
        fs::create_dir_all(&artwork_dir)
            .with_context(|| format!("Failed to create artwork directory: {}", artwork_dir.display()))?;

        for artwork in &self.artworks {
            // Write small image (aX.jpg)
            let small_path = artwork_dir.join(format!("a{}.jpg", artwork.id));
            fs::write(&small_path, &artwork.small_image)
                .with_context(|| format!("Failed to write artwork: {}", small_path.display()))?;

            // Write large image (aX_m.jpg)
            let large_path = artwork_dir.join(format!("a{}_m.jpg", artwork.id));
            fs::write(&large_path, &artwork.large_image)
                .with_context(|| format!("Failed to write artwork: {}", large_path.display()))?;

            log::debug!("Wrote artwork {} to {}", artwork.id, artwork_dir.display());
        }

        log::info!("Wrote {} artwork file(s)", self.artworks.len());
        Ok(())
    }

    /// Get all processed artworks for PDB writing
    pub fn get_artworks(&self) -> &[ProcessedArtwork] {
        &self.artworks
    }

    /// Get artwork count
    pub fn len(&self) -> usize {
        self.artworks.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.artworks.is_empty()
    }
}

impl Default for ArtworkManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Encode a DynamicImage as JPEG with given quality
fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let mut buffer = Cursor::new(Vec::new());
    img.write_to(&mut buffer, ImageFormat::Jpeg)
        .context("Failed to encode JPEG")?;
    Ok(buffer.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artwork_manager_new() {
        let manager = ArtworkManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
    }
}
