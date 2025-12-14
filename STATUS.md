# Pioneer Exporter - Phase 1 Status

## ✅ COMPLETED

### Architecture & Foundation
- [x] Rust project initialized with all dependencies
- [x] Modular code structure with clear separation of concerns
- [x] Trait-based analysis abstraction (allows Phase 1 stub → Phase 2 real)
- [x] Complete data model (Track, Playlist, Library, MusicalKey)
- [x] CLI with argument parsing

### Input Layer - Rhythmbox Parsing
- [x] Rhythmdb.xml parser (track metadata extraction)
- [x] Playlists.xml parser (playlist structure)
- [x] Unified Library model
- [x] URL decoding for file:// URIs
- [x] Track ID generation (MD5 hash of file path)

### Analysis Layer
- [x] AudioAnalyzer trait definition
- [x] StubAnalyzer implementation (returns empty/minimal data)
- [x] AnalysisResult, BeatGrid, WaveformData structures
- [x] Ready for Phase 2 real implementation

### Output Layer - File Writers

#### ANLZ Writer (Analysis Files)
- [x] Minimal .DAT file writer
- [x] Minimal .EXT file writer
- [x] Valid PMAI header structure (28 bytes)
- [ ] ⚠️ Waveform/beatgrid sections (Phase 1: empty, Phase 2: real data)

#### PDB Writer (Database)
- [x] File header with magic and table pointers
- [x] Page-based structure (4KB pages)
- [x] Page headers with row counts
- [x] DeviceSQL string encoding (short ASCII)
- [x] Artists table (deduplicated from tracks)
- [x] Albums table (deduplicated from tracks)
- [x] Tracks table (ID, title, artist_id, album_id, duration, file type)
- [x] Playlist tree table (playlist names and IDs)
- [x] Playlist entries table (track-to-playlist mappings)
- [x] Row indexing from page end
- [x] Row presence flags (bitmasks)
- [ ] ⚠️ Full track metadata fields (file paths, ANLZ paths, BPM, key) - Phase 1: minimal

### Export Pipeline
- [x] USB directory structure creation
  - PIONEER/rekordbox/
  - PIONEER/USBANLZ/
  - Music/
- [x] File organization and path management
- [x] Audio file copying
- [x] ANLZ file path generation
- [x] Complete export orchestration

### Build System
- [x] Compiles successfully
- [x] Only minor unused variable warnings (expected for stubs)

## ⚠️ KNOWN LIMITATIONS (Phase 1)

### PDB File
1. **Missing critical track fields:**
   - File path (needed for XDJ-XZ to find audio files)
   - ANLZ path (analyze_path field - links to waveform/beatgrid)
   - BPM field
   - Key field
   - Other metadata: bitrate, sample rate, comment, genre

2. **Row structure simplified:**
   - Real track rows have 21 string offset pointers
   - Real rows have complex offset-based field layout
   - Phase 1 uses simplified inline data

3. **Single-page limitation:**
   - Each table limited to one page (~100 rows max)
   - No multi-page support yet

### ANLZ Files
- Empty stub files (just headers)
- No waveform data
- No beatgrid data
- XDJ-XZ may reject empty ANLZ files

### Rhythmbox Parsing
- Playlist parsing incomplete (XML structure not fully handled)
- No playlist folder support

## 🔧 NEXT STEPS (Critical for Phase 1 to work)

### Priority 1: Complete PDB Track Table
The track table needs these critical fields for XDJ-XZ to work:

```rust
// Current track row: ID + artist_id + album_id + duration + title (inline)
// Needed: Proper row structure with string offsets

Track Row (actual format based on Deep Symmetry docs):
- Row header (subtype, index_shift, ID)
- Sample rate (u32)
- Bitrate (u16)
- Tempo (u16, BPM * 100)
- Year (u16)
- String offset array (21 x u16 offsets):
  [0] = ISRC
  [1] = Track title
  [2] = Artist name (or offset to artist table)
  [3] = Album name (or offset to album table)
  [4] = Label
  [5] = Key
  [6] = Original artist
  [7] = Remixer
  [8] = Comment
  [9] = Mix name
  [10] = Genre
  [11] = Album artist
  [12] = Composer
  [13] = File path (CRITICAL!)
  [14] = ANLZ path (CRITICAL!)
  [15-20] = Other fields
- String data in heap (referenced by offsets)
```

**Action:** Refactor `write_tracks_table()` to match actual track row format

### Priority 2: Fix Playlist Parser
Current playlist parser doesn't actually extract track entries.

**Action:** Complete `src/rhythmbox/playlists.rs` to properly parse `<location>` elements

### Priority 3: Test with Real Data
Create a test with 1-2 actual music files from Rhythmbox.

**Action:**
1. Find 1-2 MP3 files in Rhythmbox library
2. Export to test USB stick
3. Try loading on XDJ-XZ
4. Debug failures

### Priority 4: Validation with rekordcrate
Parse generated PDB with rekordcrate to validate structure.

**Action:** Implement `src/validation/roundtrip.rs`

## 📝 NOTES FOR DEBUGGING

### Testing Without Hardware
If XDJ-XZ not available for testing:
1. Use rekordcrate CLI tools to parse generated PDB
2. Compare hex dumps with known-good Rekordbox exports
3. Validate ANLZ files with rekordcrate

### Expected File Structure
```
/media/usb/
├── PIONEER/
│   ├── rekordbox/
│   │   └── export.pdb          # Database file
│   └── USBANLZ/
│       ├── ANLZ12345678.DAT    # Per-track analysis
│       └── ANLZ12345678.EXT
└── Music/
    ├── track1.mp3              # Audio files
    └── track2.mp3
```

### PDB Structure Reference
- Page 0: Artists table
- Page 1: Albums table
- Page 2: Tracks table
- Page 3: Playlist tree table
- Page 4: Playlist entries table

Each page is exactly 4096 bytes.

## 🎯 SUCCESS CRITERIA

### Phase 1 Minimum:
- [x] Project compiles
- [ ] Parses Rhythmbox library
- [ ] Generates valid PDB structure
- [ ] Generates ANLZ stub files
- [ ] Copies audio files
- [ ] rekordcrate can parse the PDB
- [ ] XDJ-XZ recognizes USB stick
- [ ] Tracks appear in XDJ-XZ browser (may not play if paths wrong)

### Phase 2 Goals:
- [ ] BPM detection working
- [ ] Key detection working
- [ ] Waveforms display on XDJ-XZ
- [ ] Beatgrid aligned
- [ ] Export quality comparable to Rekordbox

## 📚 KEY REFERENCES

- **Deep Symmetry PDB Format:** https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html
- **Deep Symmetry ANLZ Format:** https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html
- **rekordcrate Documentation:** https://holzhaus.github.io/rekordcrate/

## 🐛 KNOWN ISSUES

1. Playlist parser incomplete - doesn't extract track entries yet
2. Track rows missing file_path and analyze_path fields
3. ANLZ files are empty stubs (may cause XDJ-XZ to reject tracks)
4. No genre support yet
5. No key table support yet
6. String encoding only supports ASCII (UTF-16 needed for some metadata)

## ⏭️ IMMEDIATE TODO

1. Fix track row format to include file_path and analyze_path
2. Complete playlist parser
3. Test with 1-2 real tracks
4. Debug with rekordcrate
5. Hardware test on XDJ-XZ

---

**Last Updated:** 2025-12-14
**Phase:** 1 (Core Export System)
**Status:** Foundation complete, critical fields needed for hardware compatibility
