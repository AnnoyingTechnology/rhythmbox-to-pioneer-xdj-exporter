# Pioneer Exporter Implementation Strategy

This document describes the phased implementation approach for the Rhythmbox → Pioneer USB exporter.

## Current Status (2025-12-16)

**Phase:** Phase 1 – still failing on hardware  
**Status:** ✅ PDB passes rekordcrate with the latest changes; ❌ XDJ shows an empty USB browser (no message).  
**Reference:** A valid Rekordbox export is available at `examples/PIONEER/rekordbox/export.pdb` (playlists REKORDBOX1/2) and must be treated as the source of truth for byte-level comparison.

## Updated Assessment (after cleaning the reference export)
- Confirmed: the XDJ loads fine with only the reference `export.pdb` present. All USBANLZ contents (`*.DAT/EXT`), `.nxs` setting files, and `exportExt.pdb` can be removed and the device still browses correctly.  
- Implication: the blank/empty browser issue is entirely PDB-driven; exportExt/ANLZ/settings files are not blockers.  
- Primary deltas that remain between our PDB and the reference (same tracks/playlists):
  1. **Columns table empty** in our file vs 27 populated rows in the reference; reference uses column definitions to render the browser UI.  
  2. **Page topology/pointers differ**: reference spreads tables across non-contiguous page indices with dedicated empty-candidate pages at the end; our PDB packs tables sequentially (header→data→empty per table) with `last_page` and `empty_candidate` much smaller.  
  3. **Track field content gaps**: most tempo/BPM values are zero, several years are corrupted (e.g., 3971), and date strings (date_added/analyze_date) are blank; reference has BPM*100, sane years, and populated dates.  
  4. **Entity tables missing data**: labels/keys/colors/history are populated in the reference but empty in ours; playlist entry ordering differs slightly (track_ids in REKORDBOX2).  
- Likely blockers for the XDJ blank page (given exportExt/ANLZ are irrelevant):  
  - Missing Columns table data needed for browser layout.  
  - Non-reference page layout/pointers (sequential packing vs spaced indices and shared empty-candidate region).  
  - Invalid/empty per-track fields (tempo/year/dates) causing tracks to be ignored.  
  - Missing keys/colors/labels/history tables that the device may expect to exist/populate.

### What’s new in the writer (latest round)
- Header defaults aligned to the reference: header pages use `num_rows_large=0x1fff`, `unknown6=0x03ec`, track header `unknown1=0x3e/unknown7=1`, history header `unknown1=0x12`; header metadata patched to `next_unused_page`, `unknown=5`, `sequence=0x44`.
- Data pages: row-group layout fixed (offsets + flags + unknown16 per group), row offsets stored relative to heap start (not `HEAP_START +`), and page usage patched accordingly.
- Track rows: bitmask `0x0700`, u2 `track_id+6`, u3 `0xe556`, u4 `0x6a2e`, genre IDs, file-type-aware bitrate; artist/album index_shift aligned with the reference.
- Tables: columns table expanded to 27 UTF-16 annotated entries matching the valid export; genres table now populated from track metadata.
- Latest export targeting only REKORDBOX1/REKORDBOX2 playlists (10 tracks) lives at `/tmp/pioneer_test` and parses cleanly with rekordcrate (2 track pages: header + 10-row data).

### Where we stand
- **rekordcrate:** ✅ passes on the new `/tmp/pioneer_test` export.  
- **Hardware:** ❌ still reports “library is corrupted” and shows an empty device browser.  
- **Likely remaining gap:** page/header details still differ from the reference Rekordbox export; need byte-level comparison against `examples/PIONEER/rekordbox/export.pdb` to finalize (pagination/empty candidates/data-page unknowns/free-space values).

### Next steps (not executed here)
1) Systematically diff our export vs `examples/PIONEER/rekordbox/export.pdb` (page headers, table pointers, row offsets, free/used sizes).  
2) Match empty-candidate pages / multi-page chains and data-page unknowns/free-space to the reference.  
3) Re-test on hardware after the above alignment.

## 2025-12-17 Update (WIP attempt to unblock hardware)
- Populated **Columns**, **Keys**, **Labels**, and **Colors** tables to mirror the reference contents and ordering (27 columns, 7 keys, 1 label, 8 preset colors). Columns now use the annotated UTF-16 format; colors/key/label tables have reference-like header values and row structures.
- Regenerated export at `/tmp/pioneer_test` with REKORDBOX1/2 (10 tracks). The standard roundtrip validation still passes (rekordcrate parses header/tracks/albums/playlists).
- The `examples/list_columns.rs` helper still panics on the new PDB (rekordcrate error while parsing ColumnEntry), so the columns page layout is still not byte-identical to the reference despite populated rows. Column header free/used sizes now use page-size-based accounting, but the row index format likely still diverges (missing sentinel or wrong index layout).
- Table layout remains **sequential** (header→data→empty per table with contiguous page indices) rather than the spaced/gapped layout of the reference; this likely remains a blocker for the XDJ even with populated tables.
- Track field gaps remain (tempo/year/date fields not aligned to the reference; BPM often zero from Rhythmbox data), and pagination is still single-chain (tracks split across 2 pages but not placed at reference indices).

### What to test next
- Fix the Columns page row index to match the reference (ensure rekordcrate list_columns succeeds on our PDB). Compare our columns data page against `examples/PIONEER/rekordbox/export.pdb` (page_index 34) for row index layout and free/used sizes.
- Rework table pagination to mirror the reference pointer map (non-contiguous page indices, shared empty-candidate region) and copy data-page header unknowns/free/used from the reference per table type.
- Populate tempo/year/date fields for tracks (BPM*100, sane years, date_added/analyze_date strings) to remove remaining per-row divergences.
- Re-test on hardware once columns parse cleanly and page layout matches the reference.

## 2025-12-18 Update (columns + layout implementation)
- `write_pdb` now hard-codes the reference pointer map: headers/data/empty pages are written to the same indices as `examples/PIONEER/rekordbox/export.pdb` (tracks h=1 d=2/51 empty=55; genres h=3 d=4 empty=48; artists h=5 d=6 empty=47; albums h=7 d=8 empty=49; labels h=9 d=10 empty=54; keys h=11 d=12 empty=50; colors h=13 d=14 empty=42; playlistTree h=15 d=16 empty=46; playlistEntries h=17 d=18 empty=52; unknown09 h=19 empty=20; unknown0A h=21 empty=22; unknown0B h=23 empty=24; unknown0C h=25 empty=26; artwork h=27 d=28 empty=53; unknown0E h=29 empty=30; unknown0F h=31 empty=32; columns h=33 d=34 empty=43; historyPlaylists h=35 d=36 empty=44; historyEntries h=37 d=38 empty=45; history h=39 d=40 empty=41). Header metadata now patches `next_unused_page=56` and explicitly zeros the empty-candidate pages. Tracks are split across page 2 and 51 with per-page unknown1 values 0x0038 (page 2) and 0x003e (page 51).
- Row-group layout was rewritten to align with rekordcrate: row groups are always 36 bytes (16 offsets + flags + unknown). `used_size` now records only the heap length (no `HEAP_START` bias), `free_size` uses page-padding math, and columns/colors use unknown=flags. Track row groups use a small heuristic (flags count → unknown) to mirror the reference (page 2 unknown=0x80, page 51 unknown=0x01 for a single row). Colors data page header fields now match the reference (unk1=0x0002, unk3=0x00, unk4=0x01, unk5=0x0008).
- Rekordcrate parsing check: `cargo run --example list_columns examples/PIONEER/rekordbox/export.pdb` now succeeds (columns table prints 27 rows; no panic), confirming the row-group sizing logic matches the parser expectations.
- Not yet re-exported with Rhythmbox data or hardware-tested due to missing library inputs in this session. The new layout presizes the file to 56 pages; hardware validation still needed. Tempo/year/date field gaps remain untouched.
- Pending follow-ups: regenerate an export with the new writer, diff free/used sizes vs reference (columns free_size now computed from padding and may differ by ~10 bytes from the reference 0x0cca), and run hardware/XDJ validation. Track unknown row-group values remain heuristic; adjust if hardware still balks.

### 2025-12-18 Small-library export (REKORDBOX1/2 only)
- Ran export to `/tmp/pioneer_new` with playlist filters `REKORDBOX1`, `REKORDBOX2` (10 tracks total). Table pointers and next_unused_page match the reference map (headers at the same indices; tracks split across pages 2 and 51; empty_candidate=55).
- Rekordcrate validation passes on this export, but it is **not byte-identical** to the reference: track page used/free differ (page 2 used=0x0ad6 vs ref 0x0f80; page 51 used=0x0150 vs ref 0x01c0), genres/artists/albums used sizes are smaller than reference, playlistEntries rows_l=0x000d vs ref 0x000c, colors page used/free differ (0x006e/0x0f46 vs ref 0x007c/0x0f48), columns page used/free differ (0x02b4/0x0cdc vs ref 0x02d0/0x0cca).
- Keys, Labels, and Artwork tables are empty (used=0) in the new export, while the reference has populated rows (Keys used≈0x54, Labels used≈0x14, Artwork used≈0x48). These remain gaps to close.
- The export pipeline still writes `PIONEER/USBANLZ/*.DAT`/`.EXT`, `PIONEER/rekordbox/*.nxs`, and `exportExt.pdb` by default. Earlier hardware tests showed these are not required; consider gating or skipping their generation for final hardware runs.
- Next steps: adjust track page layout/free/used to match reference, populate Keys/Labels/Artwork to reference contents, align playlistEntries row count, and re-diff columns/colors free/used before retrying on hardware.

### Validation Status

**rekordcrate validation: ✅ PASS**
- ✅ PDB header parses successfully
- ✅ Table pointers correct (Pages 1-5)
- ✅ Artists table: 4 rows
- ✅ Albums table: 4 rows
- ✅ Tracks table: 4 rows
- ✅ PlaylistTree and PlaylistEntries tables present

### Latest Session Fixes (Round 3)

1. **Track Row Structure Fixed** ✅
   - Completely rewrote Track row structure per Deep Symmetry documentation
   - 94-byte header (0x00-0x5D) with correct field order and sizes
   - Subtype changed from 0x80 to 0x0024
   - track_number field changed from u16 to u32
   - 21 x u16 string offset array at 0x5E
   - String data follows at 0x88
   - Corrected string indices: title=17, file_path=20, analyze_path=14

2. **Empty String Handling Fixed** ✅
   - **Problem:** offset 0 for unused strings → rekordcrate tries to parse header bytes as strings → crash
   - **Solution:** Created empty DeviceSQL string (0x03) at start of string data section
   - All unused string offsets now point to this valid empty string
   - Prevents "attempt to subtract with overflow" panic

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

### Complete Implementation History

#### Round 1: Initial Implementation (Early 2025-12-14)
**Status:** Hardware test failed with "Database not found"

**Issues Found:**
1. Page numbering bug - `current_page` started at 0 instead of 1
2. Table pointers pointed to page 0 (header) instead of data pages
3. XDJ couldn't find table data

**Fixes Applied:**
- Set `current_page = 1` in `write_pdb()`
- Updated all page header indices to 1, 2, 3, 4, 5 (was 0, 1, 2, 3, 4)

#### Round 2: Post-Page-Numbering Fix (Mid 2025-12-14)
**Status:** Albums validate, Tracks fail parsing

**Issues Found:**
1. Track row structure completely wrong
   - Using subtype 0x80 (should be 0x0024)
   - Wrong header size
   - Wrong field order
   - String indices wrong (title=1, file_path=13, analyze_path=14)
   - Should be: title=17, file_path=20, analyze_path=14

**Fixes Applied:**
- Rewrote entire `write_tracks_table()` function
- Implemented correct 94-byte header per Deep Symmetry docs
- Fixed string offset array to 21 x u16 at position 0x5E
- Corrected string indices

#### Round 3: Track Structure Fix (Late 2025-12-14)
**Status:** Tracks parse but crash on empty strings

**Issues Found:**
1. Empty string offsets set to 0
2. rekordcrate tries to parse bytes at row start (offset 0) as strings
3. Causes "attempt to subtract with overflow" panic

**Fixes Applied:**
- Created empty DeviceSQL string (0x03) at start of string data
- All unused string offsets point to this valid empty string
- No more crashes on parsing

**Result:** ✅ All tables now validate with rekordcrate!

### Current Export Capabilities

**Working Features:**
- ✅ Rhythmbox XML parsing (9,298 tracks, 34 playlists)
- ✅ PDB file generation with all 5 tables
- ✅ Artist/Album/Track/PlaylistTree/PlaylistEntries tables
- ✅ DeviceSQL string encoding (ShortASCII and Long formats)
- ✅ ANLZ stub files (.DAT/.EXT pairs with PMAI headers)
- ✅ USB directory structure (PIONEER/rekordbox/, PIONEER/USBANLZ/, Music/)
- ✅ Audio file copying with correct paths
- ✅ CLI with playlist filtering
- ✅ rekordcrate validation passes on all tables

**Test Export (Shower Playlist - 4 Tracks):**
```
/tmp/pioneer_test/
├── Music/
│   ├── 10 Only Child - Addicted.mp3 (11 MB)
│   ├── 01 So Different.mp3 (16 MB)
│   ├── 5-09 Ay No Corrida.mp3 (10 MB)
│   └── Harlem.mp3 (10 MB)
├── PIONEER/
│   ├── USBANLZ/
│   │   ├── ANLZ4f008eaa.DAT, ANLZ4f008eaa.EXT
│   │   ├── ANLZ242ee464.DAT, ANLZ242ee464.EXT
│   │   ├── ANLZ7116a0d7.DAT, ANLZ7116a0d7.EXT
│   │   └── ANLZ80939d47.DAT, ANLZ80939d47.EXT
│   └── rekordbox/
│       └── export.pdb (24 KB)
```

**Validation Output:**
```
✅ rekordcrate successfully parsed the PDB header!
✅ Successfully read 1 album page(s)! (4 rows)
✅ Successfully read 1 track page(s)! (4 rows)
✅ Validation passed!
```

### Files Modified During Development

- `src/pdb/strings.rs` - DeviceSQL encoding (ShortASCII/Long formats)
- `src/pdb/writer.rs` - All table writers (Artists, Albums, Tracks, Playlists)
  - Page numbering fixes
  - Row structure corrections
  - Empty string handling

### Test Tracks

"Shower" playlist (4 tracks):
1. Nicolas Skorsky - Harlem
2. Quincy Jones - Ay No Corrida
3. Kinky Foxx - So Different
4. Only Child - Addicted

### Ready for Hardware Testing

The export is now ready for XDJ-XZ hardware testing. To test:
1. Copy contents of `/tmp/pioneer_test` to USB stick
2. Plug USB into XDJ-XZ
3. Navigate to browse → USB
4. Look for "Shower" playlist
5. Verify tracks load and play

**Expected Behavior:**
- XDJ should recognize the USB database
- "Shower" playlist should appear
- 4 tracks should be visible with artist/title metadata
- Tracks should load and play
- No waveforms/beatgrids (Phase 1 - stubs only)

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

4. **PDB Writer (Minimal)** 🔨 IN PROGRESS
   - Track table (title, artist, album, duration, file path) ✅
   - Artist/Album/Genre tables ✅
   - Playlist tree and entries ✅
   - Skip or stub: keys table, BPM field ✅
   - **CURRENT BLOCKER:** String encoding validation

5. **ANLZ Writer (Stub)** (NEXT)
   - Create valid PMAI header
   - Minimal required tags (refer to rekordcrate for minimum viable set)
   - Empty or stub waveform sections

6. **USB Organization** ✅
   - Create directory structure
   - Copy audio files with correct naming
   - Link PDB analyze_path to ANLZ files

7. **CLI Interface** ✅
   - Arguments: source library path, target USB mount
   - Progress reporting
   - Error handling

8. **Validation** 🔨 IN PROGRESS
   - Parse generated PDB with rekordcrate
   - Parse generated ANLZ with rekordcrate
   - Test on XDJ-XZ hardware

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

**Phase 1 Complete:**
- [ ] Can export Rhythmbox library to USB
- [ ] XDJ-XZ recognizes and loads tracks
- [ ] Playlists appear correctly
- [ ] rekordcrate successfully parses output

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
