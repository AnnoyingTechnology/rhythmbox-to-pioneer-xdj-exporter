# Pioneer Exporter - Phase 1 Status

## Current State (2025-12-17)
- **Hardware test status**: XDJ still shows empty USB browser (no error message)
- **Rhythmbox playlists aligned**: REKORDBOX1/REKORDBOX2 now match reference export track order exactly
- **rekordcrate validation: PASSES** - PDB header, album pages, track pages all parse successfully
- **Issue confirmed in PDB**: Testing with reference USBANLZ + our PDB still fails
- Reference Rekordbox export available at `examples/PIONEER/rekordbox/export.pdb` for byte-level comparison

### Recent Fixes Applied
- Empty candidate pages properly zeroed (all 4096 bytes = 0x00)
- Track row `index_shift` increments by 0x20 per row
- Track row `u3` constant: 0xe5b6
- Track row `u4` constant: 0x6a76
- Page chain structure: header → data → empty_candidate

### Debugging Tools Added
- `examples/list_tracks.rs` - Parse and list tracks from PDB via rekordcrate
- `examples/list_playlists.rs` - Parse and list playlists from PDB via rekordcrate

### Next Steps
1. Rebuild export with aligned Rhythmbox playlists
2. Byte-by-byte comparison of our PDB vs reference (same tracks enables direct diff)
3. Identify remaining field/structure differences causing hardware rejection

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
- Hardware still reports **“library is corrupted”**; device browser remains empty.
- Table pagination still single-page (header+data only); no empty-candidate pages or multi-page chains for large libraries.
- Data-page unknown fields/next_page chaining are only matched approximately; free/used space metrics may still differ from the reference export.
- Labels/keys/colors/history tables are stubbed; ANLZ files remain minimal headers (no waveforms/beatgrids).
- Track bitrate defaults are best-effort (MP3→320kbps, others→0) and may diverge from actual media metadata.

## 🔧 NEXT STEPS (Critical for Phase 1 to work)
1. **Mirror Rekordbox pagination/empty pages**
   - Add explicit empty-candidate pages per table and match next_page chaining to the valid export.
   - Support multi-page chains for large tables (tracks/playlists/columns).
2. **Tighten page header fields**
   - Align data-page unknown1/3/4/5 values and free/used sizes with the reference export.
   - Revisit header unknown7 usage (tracks/history) and unknown6 for non-track tables if needed.
3. **Validate against the new reference export**
   - Byte-compare page headers and row layouts vs `examples/PIONEER/rekordbox/export.pdb`.
   - Ensure column/genre IDs referenced by tracks resolve correctly on-device.
4. **Improve ANLZ stubs (optional)**
   - Ensure minimal but valid content to avoid rejection (even if waveforms remain empty).

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

## 🆘 Current Blocker (2025-12-17)
- XDJ shows **empty USB browser** (no error message), even though rekordcrate parses the PDB successfully.
- Confirmed the issue is in the PDB file (not ANLZ): using reference USBANLZ with our PDB still fails.
- Rhythmbox playlists now aligned with reference export (same 10 tracks, same order) to enable direct byte-level comparison.
- Latest hardware test export: `/tmp/pioneer_test` (built with the current writer).

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
