# Artwork Implementation

This document covers album artwork support for Pioneer USB exports.

## Current Status (2025-12-26)

**Status:** Not Implemented (causes database corruption)

### What Works
- Artwork extraction from audio files (lofty)
- Image resizing (image crate)
- JPEG encoding for Pioneer format

### What Doesn't Work
- PDB Artwork table integration causes "corrupted database" errors in Rekordbox
- Single-track exports with artwork are corrupted
- Investigation ongoing

---

## Pioneer Artwork Format

### File Structure
```
PIONEER/Artwork/00001/
├── a1.jpg      (80x80 main artwork)
├── a1_m.jpg    (56x56 thumbnail)
├── a2.jpg
├── a2_m.jpg
└── ...
```

### Naming Convention
- `a{id}.jpg` - Main artwork (80x80 pixels)
- `a{id}_m.jpg` - Thumbnail (56x56 pixels)
- Artwork IDs are 1-indexed and match PDB artwork_id field

---

## PDB Artwork Table Structure

### Reference Analysis (reference-84 with artwork)

**Table Location:**
- File header table type 14: first=27, last=28, empty_candidate=53
- Page 27: header page (page_type=13)
- Page 28: data page with artwork rows (page_type=13)

**Without Artwork (reference-1):**
- first=27, last=27, empty_candidate=28
- Page 28: ALL ZEROS

### Artwork Row Format
```
4 bytes: artwork_id (u32)
DeviceSQL string: path (e.g., "/PIONEER/Artwork/00001/a1.jpg")
```

### Row Offsets (Page 28)
Rows are stored at fixed intervals of ~36 bytes each.

---

## Implementation Attempts (Failed)

### Attempt 1: Static page allocation
- Set artwork data page to 28
- Set empty_candidate to 53
- **Result:** Corrupted for small exports (page 53 doesn't exist)

### Attempt 2: Dynamic page allocation
- Allocate artwork data at page 53+
- Leave page 28 as zeros
- **Result:** Still corrupted

### Root Cause (Suspected)
The Artwork table (type 14 in file header) interacts with other tables in ways not fully understood. Simply adding data pages breaks validation.

---

## Track artwork_id Field

Track rows have an `artwork_id` field at offset 0x1C-0x1F:
- `0` = no artwork
- `1+` = references artwork row in Artwork table

Currently hardcoded to `0` (no artwork).

---

## Future Implementation Plan

1. **Understand reference structure completely**
   - Binary diff reference-1 (no artwork) vs reference-84 (with artwork)
   - Identify ALL fields that change when artwork is present

2. **Minimal change approach**
   - Only modify fields that differ in reference exports
   - Don't add dynamic allocation unless proven necessary

3. **Test incrementally**
   - Test on Rekordbox 5 first (stricter validation)
   - Then test on XDJ-XZ hardware

---

## Libraries Used

- `lofty` - Extract embedded artwork from audio files (ID3 APIC, Vorbis/FLAC pictures)
- `image` - Resize and JPEG encode

---

## Code Location

- `src/artwork.rs` - Artwork extraction and resizing (exists but disabled)
- `src/pdb/writer.rs` - Artwork table writing (needs work)

---

## References

- [Deep Symmetry - PDB Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- See HISTORY.md for detailed debugging sessions
