# Artwork Implementation

This document covers album artwork support for Pioneer USB exports.

## Current Status (2025-12-27)

**Status:** IMPLEMENTED AND WORKING

### What Works
- Artwork extraction from audio files (lofty)
- Image resizing to 80x80 and 240x240 (image crate)
- JPEG encoding for Pioneer format
- PDB Artwork table integration
- Track rows reference artwork_id correctly
- Deduplication of identical artworks
- Dynamic page allocation for large exports

### Tested Configurations
- Small exports (35 tracks, 7 artworks): PASS
- Large exports (119 tracks, 62 artworks): PASS
- Mixed exports with track/artist/album overflow: PASS

---

## Pioneer Artwork Format

### File Structure
```
PIONEER/Artwork/00001/
├── a1.jpg      (80x80 main artwork)
├── a1_m.jpg    (240x240 large thumbnail)
├── a2.jpg
├── a2_m.jpg
└── ...
```

### Naming Convention
- `a{id}.jpg` - Main artwork (80x80 pixels)
- `a{id}_m.jpg` - Large thumbnail (240x240 pixels)
- Artwork IDs are 1-indexed and match PDB artwork_id field

---

## PDB Artwork Table Structure

### Table Location
- Table type 13 (Artwork)
- Header page: 27
- Data page: 28 (when artworks exist)
- empty_candidate: dynamically calculated

### With Artwork
- first=27, last=28
- empty_candidate = after all entity overflow pages

### Without Artwork
- first=27, last=27, empty_candidate=28

### Artwork Row Format
```
4 bytes: artwork_id (u32)
DeviceSQL string: path (e.g., "/PIONEER/Artwork/00001/a1.jpg")
Padding to 36 bytes
```

---

## Track artwork_id Field

Track rows have an `artwork_id` field at offset 0x1C-0x1F:
- `0` = no artwork
- `1+` = references artwork row in Artwork table

---

## Dynamic Page Allocation

For large exports, artwork_empty_candidate is calculated dynamically:

```rust
let artwork_empty_candidate = if has_artworks {
    let max_used = *[
        actual_track_empty_candidate,
        actual_artist_empty_candidate,
        actual_album_empty_candidate,
    ].iter().max().unwrap();
    if needs_extra_pages {
        max_used + 1  // After all entity overflow
    } else {
        53u32  // Reference behavior for small exports
    }
} else {
    28u32
};
```

This prevents page conflicts when track/artist/album overflow uses pages 53+.

---

## Libraries Used

- `lofty` - Extract embedded artwork from audio files (ID3 APIC, Vorbis/FLAC pictures)
- `image` - Resize and JPEG encode
- `md5` - Hash for artwork deduplication

---

## Code Location

- `src/artwork.rs` - ArtworkManager for extraction, processing, deduplication
- `src/pdb/writer.rs` - Artwork table writing, track artwork_id integration
- `src/export/pipeline.rs` - Artwork integration in export flow

---

## References

- [Deep Symmetry - PDB Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- See HISTORY.md for detailed debugging sessions
