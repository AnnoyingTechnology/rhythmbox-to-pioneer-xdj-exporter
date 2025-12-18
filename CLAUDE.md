# Pioneer Exporter Implementation Strategy

This document describes the phased implementation approach for the Rhythmbox → Pioneer USB exporter.

## Current Status (2025-12-18)

**Phase:** Phase 1 – WORKING ON HARDWARE! 🎉
**Status:** ✅ PDB passes rekordcrate validation; ✅ XDJ-XZ loads and displays tracks/playlists correctly
**Reference:** A valid Rekordbox export is available at `examples/PIONEER/rekordbox/export.pdb` (playlists REKORDBOX1/2) and must be treated as the source of truth for byte-level comparison.

### What Works
- XDJ-XZ recognizes USB and displays playlists (REKORDBOX1, REKORDBOX2)
- Tracks load and play correctly
- Artist/Album/Title metadata displays
- rekordcrate validation passes

### Known Issues (minor)
- **UTF-8/Accented characters broken**: Characters with accents display incorrectly (typical UTF-8 encoding issue)
- **BPM/tempo**: Shows 0 (Rhythmbox data not populated)
- **Waveforms**: Not displayed (Phase 2 feature)

## 2025-12-18 BREAKTHROUGH: Hardware Success

### Root Cause Analysis
Through systematic isolation testing, we identified **two critical tables** that the XDJ is extremely sensitive to:

1. **Columns table (page 34)** - Must have specific row group structure
   - Our dynamically generated columns had wrong row group ordering
   - Reference has 27 entries with specific row indices mapping
   - Row groups store offsets in reverse order within each group
   - Rows 16-20 are "deleted" placeholders pointing to offset 0

2. **HistoryPlaylists table (page 36)** - Must be populated
   - Our version was completely empty (all zeros)
   - Reference has 21 history playlist entries
   - XDJ requires this table to have valid content

### The Fix
Both tables now use **reference page data** directly (byte-for-byte copy from `examples/PIONEER/rekordbox/export.pdb`):
- `src/pdb/reference_columns.bin` - 4096 bytes, page 34
- `src/pdb/reference_history_playlists.bin` - 4096 bytes, page 36

This ensures byte-perfect compatibility with XDJ hardware expectations.

### Isolation Testing Methodology
Created hybrid PDBs to isolate which data pages caused failures:
1. Started with reference PDB (known working)
2. Replaced individual data pages with our generated versions
3. Tested each combination on XDJ hardware

**Results:**
- ✅ Tracks (pages 2, 51) - WORKS
- ✅ Genres (page 4) - WORKS
- ✅ Artists (page 6) - WORKS
- ✅ Albums (page 8) - WORKS
- ✅ Colors (page 14) - WORKS
- ✅ PlaylistTree (page 16) - WORKS
- ✅ PlaylistEntries (page 18) - WORKS
- ✅ Labels (page 10) - WORKS
- ✅ Keys (page 12) - WORKS
- ✅ Artwork (page 28) - WORKS
- ✅ HistoryEntries (page 38) - WORKS
- ✅ History (page 40) - WORKS
- ❌ Columns (page 34) - FAILS → Fixed with reference data
- ❌ HistoryPlaylists (page 36) - FAILS → Fixed with reference data

### Files Modified
- `src/pdb/writer.rs` - Uses reference binary data for Columns and HistoryPlaylists tables
- `src/pdb/reference_columns.bin` - Reference columns page (NEW)
- `src/pdb/reference_history_playlists.bin` - Reference history playlists page (NEW)

## Development History (Condensed)

### Key Fixes Applied (chronological)
1. **Page numbering** - Fixed `current_page` starting at 0 instead of 1
2. **Track row structure** - Rewrote to 94-byte header per Deep Symmetry docs
3. **Empty string handling** - Created valid empty DeviceSQL string at offset 0
4. **Page layout** - Matched reference pointer map (non-contiguous page indices)
5. **Row alignment** - Added 4-byte alignment for all rows
6. **Header page content** - Added B-tree pointers after 40-byte page header
7. **Columns table** - Using reference binary data (row group structure critical)
8. **HistoryPlaylists table** - Using reference binary data (must be populated)

### Track Row Structure (Corrected)

```
Header (94 bytes = 0x5E):
  0x00-0x01: subtype (0x0024)
  0x02-0x03: index_shift
  0x04-0x07: bitmask
  0x08-0x0B: sample_rate (44100)
  0x0C-0x0F: composer_id
  0x10-0x13: file_size
  0x14-0x17: u2
  0x18-0x1B: u3, u4
  0x1C-0x1F: artwork_id
  0x20-0x23: key_id
  0x24-0x27: original_artist_id
  0x28-0x2B: label_id
  0x2C-0x2F: remixer_id
  0x30-0x33: bitrate
  0x34-0x37: track_number (u32!)
  0x38-0x3B: tempo (BPM * 100)
  0x3C-0x3F: genre_id
  0x40-0x43: album_id
  0x44-0x47: artist_id
  0x48-0x4B: id
  0x4C-0x4D: disc_number
  0x4E-0x4F: play_count
  0x50-0x51: year
  0x52-0x53: sample_depth
  0x54-0x55: duration (seconds)
  0x56-0x57: u5 (0x0029)
  0x58: color_id
  0x59: rating
  0x5A-0x5B: file_type
  0x5C-0x5D: u7 (0x0003)

String offsets (42 bytes = 21 x u16):
  0x5E-0x87: 21 string offsets

String data starts at 0x88
```

### String Index Mapping

```
Index  Field
-----  -----
0      isrc
1      lyricist
2-4    unknown
5      message
6      publish_track_info
7      autoload_hotcues
8-9    unknown
10     date_added
11     release_date
12     mix_name
13     unknown
14     analyze_path  ← CRITICAL
15     analyze_date
16     comment
17     title         ← CRITICAL
18     unknown
19     filename      ← CRITICAL
20     file_path     ← CRITICAL
```

### Current Export Capabilities

**Working Features:**
- ✅ Rhythmbox XML parsing (9,298 tracks, 34 playlists)
- ✅ PDB file generation with all 20 table types
- ✅ All entity tables: Artists, Albums, Tracks, Genres, Labels, Keys, Colors
- ✅ Playlist tables: PlaylistTree, PlaylistEntries
- ✅ System tables: Columns, HistoryPlaylists, HistoryEntries, History, Artwork
- ✅ DeviceSQL string encoding (ShortASCII and Long formats)
- ✅ ANLZ stub files (.DAT/.EXT pairs with PMAI headers)
- ✅ USB directory structure (PIONEER/rekordbox/, PIONEER/USBANLZ/, Music/)
- ✅ Audio file copying with correct paths
- ✅ CLI with playlist filtering (--playlist flag)
- ✅ rekordcrate validation passes
- ✅ XDJ-XZ hardware validation passes

**Usage:**
```bash
# Export specific playlists
cargo run --release -- --output /path/to/usb --playlist PLAYLIST1 --playlist PLAYLIST2

# Copy to USB
cp -r /path/to/output/* /media/usb/
```

**Test Export (REKORDBOX1/2 - 10 Tracks):**
```
/tmp/pioneer_test/
├── Music/
│   └── [10 audio files]
├── PIONEER/
│   ├── USBANLZ/
│   │   └── [.DAT/.EXT pairs for each track]
│   └── rekordbox/
│       └── export.pdb (229 KB, 56 pages)
```

### Reference Documentation

**DeviceSQL String Format** (from rekordcrate source):
```rust
ShortASCII {
    header: u8 = ((content.len() + 1) << 1) | 1,
    content: Vec<u8>,  // no null terminator
}

Long {
    flags: u8,  // 0x40 for ASCII, 0x90 for UTF-16
    length: u16,  // content.byte_count() + 4 (includes 4-byte header)
    padding: 0u8,
    content: LongBody,  // actual string data
}
```

**Album Row Structure** (from rekordcrate source):
```rust
pub struct Album {
    base_offset: u64,  // virtual - file position at row start
    unknown1: u16,     // 0x0080
    index_shift: u16,  // 0x0000
    unknown2: u32,     // 0x00000000
    artist_id: ArtistId,
    id: AlbumId,
    unknown3: u32,     // 0x00000000
    unknown4: u8,      // 0x00
    #[br(offset = base_offset, parse_with = FilePtr8::parse)]
    name: DeviceSQLString,
}
```

Total header: 22 bytes before offset pointer

**FilePtr8 Behavior** (hypothesis - needs verification):
- Reads 1-byte offset at current position (after 21-byte header)
- Seeks to `base_offset + offset_value`
- Reads DeviceSQLString at that position

---

## Two-Phase Development Approach

### Phase 1: Core Export System (MVP)
**Goal:** Create a working USB export that the XDJ-XZ can read, with basic metadata but minimal analysis.

**Deliverables:**
- Rhythmbox library parsing (rhythmdb.xml + playlists.xml)
- Basic PDB writer (tracks, artists, albums, genres, playlists)
- Stub ANLZ files (minimal valid structure)
- USB file organization (PIONEER/rekordbox/, PIONEER/USBANLZ/)
- Audio file copying to USB
- Validation: XDJ-XZ loads tracks and shows playlists

**What's NOT included in Phase 1:**
- BPM detection (will use Rhythmbox BPM if available, otherwise 0/unknown)
- Key detection (empty/unknown key)
- Waveform generation (minimal stub waveforms or empty)
- Beatgrid analysis (basic/no beatgrid)

**Acceptance criteria:**
- XDJ-XZ recognizes USB stick
- Playlists appear correctly
- Tracks load and play
- Basic metadata visible (artist, title, album)

### Phase 2: Analysis Features
**Goal:** Add audio analysis capabilities for professional DJ use.

**Deliverables:**
- Audio decoding and normalization pipeline
- BPM detection and beatgrid generation
- Musical key detection and key table population
- Full waveform generation (preview, detail, colored variants)
- Enhanced ANLZ files with complete analysis data

**Acceptance criteria:**
- Waveforms display on XDJ-XZ
- Beatgrid visible and accurate
- Key detection working and filterable
- Analysis quality comparable to Rekordbox

## Architectural Principles

### 1. Separation of Concerns

```
┌─────────────────────────────────────────────────────────┐
│                    Export Pipeline                      │
│  (orchestrates the overall export process)              │
└─────────────────────────────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
          ▼               ▼               ▼
    ┌──────────┐   ┌──────────┐   ┌──────────┐
    │ Input    │   │ Analysis │   │ Output   │
    │ Layer    │   │ Layer    │   │ Layer    │
    └──────────┘   └──────────┘   └──────────┘
```

**Input Layer:** Rhythmbox parsing
- `rhythmbox::database` - Parse rhythmdb.xml
- `rhythmbox::playlists` - Parse playlists.xml
- Output: Unified track/playlist data model

**Analysis Layer:** Audio processing (extensible)
- `analysis::traits` - Abstract interfaces
- `analysis::stub` - Phase 1 stub implementations
- `analysis::audio` - Phase 2 real implementations
- Plugin point for future enhancements

**Output Layer:** Binary format writing
- `pdb::writer` - PDB file generation
- `anlz::writer` - ANLZ file generation
- `export::organizer` - USB file structure

### 2. Trait-Based Analysis Abstraction

Allows Phase 1 to use stubs and Phase 2 to plug in real implementations:

```rust
pub trait AudioAnalyzer {
    fn analyze(&self, audio_path: &Path) -> Result<AnalysisResult>;
}

pub struct AnalysisResult {
    pub bpm: Option<f32>,
    pub key: Option<MusicalKey>,
    pub beatgrid: Option<BeatGrid>,
    pub waveforms: WaveformData,
}

// Phase 1: Stub implementation
pub struct StubAnalyzer;
impl AudioAnalyzer for StubAnalyzer {
    fn analyze(&self, audio_path: &Path) -> Result<AnalysisResult> {
        Ok(AnalysisResult {
            bpm: None,
            key: None,
            beatgrid: None,
            waveforms: WaveformData::empty_stub(),
        })
    }
}

// Phase 2: Real implementation
pub struct FullAnalyzer {
    decoder: AudioDecoder,
    beat_detector: BeatDetector,
    key_detector: KeyDetector,
    waveform_generator: WaveformGenerator,
}
impl AudioAnalyzer for FullAnalyzer {
    fn analyze(&self, audio_path: &Path) -> Result<AnalysisResult> {
        // Real audio analysis
    }
}
```

### 3. Incremental PDB/ANLZ Writing

The PDB and ANLZ writers accept optional data:

```rust
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: Option<String>,
    pub duration_ms: u32,
    pub bpm: Option<f32>,        // Phase 1: None, Phase 2: detected
    pub key: Option<MusicalKey>, // Phase 1: None, Phase 2: detected
    pub analyze_path: String,
}

pub struct AnlzData {
    pub beatgrid: Option<BeatGrid>,      // Phase 1: None
    pub waveforms: WaveformSet,          // Phase 1: stub, Phase 2: full
}
```

Writers handle missing data gracefully:
- BPM field: 0 or omit if None
- Key: blank/unknown entry
- Waveforms: minimal valid structure vs. full data

### 4. Data Model Independence

Internal data structures decoupled from file formats:

```
Rhythmbox Model → Unified Track Model → PDB/ANLZ Format
                         ↑
                   Analysis Results
```

This allows:
- Future support for other music library sources (iTunes, Traktor, etc.)
- Format version updates without touching business logic
- Testing with mock data

### 5. File Format Writing Strategy

Use `rekordcrate` as reference but implement writers from scratch:
- Study rekordcrate's parsing code to understand structure
- Mirror the data structures (tables, tags, sections)
- Implement encoding (reverse of parsing)
- Validate by round-trip: write → parse with rekordcrate → compare

## Project Structure

```
pioneer-exporter/
├── Cargo.toml
├── src/
│   ├── main.rs                    # CLI entry point
│   ├── lib.rs                     # Library interface
│   │
│   ├── rhythmbox/
│   │   ├── mod.rs
│   │   ├── database.rs            # Parse rhythmdb.xml
│   │   ├── playlists.rs           # Parse playlists.xml
│   │   └── model.rs               # Rhythmbox data structures
│   │
│   ├── model/
│   │   ├── mod.rs
│   │   ├── track.rs               # Unified track representation
│   │   ├── playlist.rs            # Unified playlist representation
│   │   └── library.rs             # Complete library model
│   │
│   ├── analysis/
│   │   ├── mod.rs
│   │   ├── traits.rs              # AudioAnalyzer trait
│   │   ├── stub.rs                # Phase 1: stub implementation
│   │   ├── audio.rs               # Phase 2: audio decoding
│   │   ├── beat.rs                # Phase 2: beat detection
│   │   ├── key.rs                 # Phase 2: key detection
│   │   └── waveform.rs            # Phase 2: waveform generation
│   │
│   ├── pdb/
│   │   ├── mod.rs
│   │   ├── writer.rs              # PDB file writer
│   │   ├── tables.rs              # Table structures
│   │   └── strings.rs             # String indexing
│   │
│   ├── anlz/
│   │   ├── mod.rs
│   │   ├── writer.rs              # ANLZ file writer
│   │   ├── tags.rs                # Tag structures
│   │   └── stub.rs                # Phase 1: minimal ANLZ
│   │
│   ├── export/
│   │   ├── mod.rs
│   │   ├── pipeline.rs            # Main export orchestration
│   │   ├── organizer.rs           # USB file organization
│   │   └── config.rs              # Export configuration
│   │
│   └── validation/
│       ├── mod.rs
│       └── roundtrip.rs           # Rekordcrate validation
│
└── tests/
    ├── integration/
    │   ├── rhythmbox_parsing.rs
    │   ├── pdb_writing.rs
    │   └── full_export.rs
    └── fixtures/
        ├── sample_rhythmdb.xml
        └── sample_playlists.xml
```

## Phase 1 Implementation Order

1. **Project Setup** ✅
   - Initialize Cargo project
   - Add dependencies: `quick-xml`, `rekordcrate`, `anyhow`, `clap`

2. **Rhythmbox Parsing** ✅
   - Parse rhythmdb.xml → track list
   - Parse playlists.xml → playlist structure
   - Build unified model

3. **Stub Analysis** ✅
   - Implement `StubAnalyzer` returning empty/minimal data
   - Extract basic metadata (duration from Rhythmbox)

4. **PDB Writer** ✅
   - Track table (title, artist, album, duration, file path) ✅
   - Artist/Album/Genre tables ✅
   - Playlist tree and entries ✅
   - Columns table (using reference data) ✅
   - HistoryPlaylists table (using reference data) ✅
   - All 20 table types with correct page layout ✅

5. **ANLZ Writer (Stub)** ✅
   - Create valid PMAI header
   - Minimal stub files (.DAT/.EXT pairs)
   - XDJ loads tracks without full analysis data

6. **USB Organization** ✅
   - Create directory structure
   - Copy audio files with correct paths
   - Link PDB analyze_path to ANLZ files

7. **CLI Interface** ✅
   - Arguments: source library path, target USB mount
   - Playlist filtering (--playlist flag)
   - Progress reporting
   - Error handling

8. **Validation** ✅
   - Parse generated PDB with rekordcrate ✅
   - Hardware test on XDJ-XZ ✅ (2025-12-18)

## Phase 2 Extension Points

When ready for Phase 2, replace `StubAnalyzer` with `FullAnalyzer`:

1. Implement audio decoding
2. Integrate beat detection library
3. Integrate key detection library
4. Implement waveform generators
5. Update ANLZ writer with real data
6. Update PDB writer to include BPM/key fields

The export pipeline code remains unchanged - just swap the analyzer implementation.

## Testing Strategy

**Phase 1:**
- Unit tests for XML parsing
- Unit tests for PDB/ANLZ writing
- Integration test: export single track, validate structure
- Hardware test: load on XDJ-XZ

**Phase 2:**
- Add audio analysis accuracy tests
- Compare BPM/key against known references
- Visual waveform validation
- Full library export test

## Success Metrics

**Phase 1 Complete:** ✅ ACHIEVED (2025-12-18)
- [x] Can export Rhythmbox library to USB
- [x] XDJ-XZ recognizes and loads tracks
- [x] Playlists appear correctly
- [x] rekordcrate successfully parses output

**Phase 1.5 (Polish):**
- [ ] Fix UTF-8/accented character encoding
- [ ] Test with larger playlists
- [ ] Verify all track metadata displays correctly

**Phase 2 Complete:**
- [ ] BPM displayed and accurate
- [ ] Key displayed and filterable
- [ ] Waveforms render correctly
- [ ] Beatgrid aligned with audio
- [ ] Export quality comparable to Rekordbox

## Key Reference Documentation

The primary technical reference for implementing the Pioneer export format:

**Deep Symmetry - DJ Link Ecosystem Analysis**
- Database Exports (PDB format): https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html
- Analysis Files (ANLZ format): https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html

This documentation reverse-engineers the Rekordbox export format and is the authoritative source for:
- PDB file structure (page-based database, table layouts, string indexing)
- ANLZ file structure (PMAI headers, tagged sections, waveform/beatgrid formats)
- Field encodings and data types
- Referential integrity between tables

The `rekordcrate` Rust library (https://holzhaus.github.io/rekordcrate/) implements parsers based on this documentation and serves as our validation tool and structural reference.

## Dependencies

### Phase 1
- `quick-xml` - XML parsing
- `rekordcrate` - PDB/ANLZ format reference and validation
- `anyhow` - Error handling
- `clap` - CLI argument parsing
- `walkdir` - File tree traversal

### Phase 2 (to be added)
- `symphonia` or `ffmpeg-next` - Audio decoding
- Essentia or aubio bindings - Beat/tempo detection
- libkeyfinder or Essentia bindings - Key detection
- `rustfft` - FFT for waveform frequency analysis

## Notes

- Keep Phase 1 lean: avoid premature optimization
- Focus on correctness: exact byte-level format adherence
- Validate frequently: round-trip with rekordcrate after each component
- Document format decisions: link to Deep Symmetry docs for each PDB/ANLZ field
- Hardware test early: catch compatibility issues before Phase 2
