# Pioneer Exporter Implementation Strategy

This document describes the phased implementation approach for the Rhythmbox → Pioneer USB exporter.

## Current Status (2025-12-18)

**Phase:** Phase 1 COMPLETE + Phase 2.1 IN PROGRESS (BPM detection implemented but not displaying)
**Status:** ⚠️ BPM detection works, data is written correctly, but XDJ-XZ does not display BPM

### What Works
- XDJ-XZ recognizes USB and displays playlists
- Tracks load and play correctly
- Artist/Album/Title metadata displays properly
- Accented characters (UTF-16LE encoding) display correctly
- Multi-page track tables (tested with 45+ tracks across 4 pages)
- **BPM detection** via aubio-rs (detects correct tempo values)
- **PQTZ beatgrid** written to ANLZ .DAT files
- **Tempo field** written to PDB track table (offset 0x38-0x3B)

### Current Issue: BPM Not Displaying on XDJ-XZ

**What we've implemented:**
1. ✅ BPM detection using `aubio-rs` + `symphonia` (correct values detected)
2. ✅ Tempo field in PDB track row at offset 0x38-0x3B (BPM × 100, little-endian)
3. ✅ PQTZ beatgrid section in ANLZ .DAT files (big-endian, correct format per Deep Symmetry docs)
4. ✅ PVBR section in ANLZ .DAT files
5. ✅ PPTH path with UTF-16BE encoding and NUL terminator
6. ✅ `analyze_path` string (index 14) pointing to ANLZ file
7. ✅ `analyze_date` string (index 15) set to current date

**Verified correct in hex dumps:**
- PDB tempo: `b136 0000` = 0x000036b1 = 14001 = 140.01 BPM ✓
- PQTZ header: magic, len_header=24, unknown2=0x00800000 ✓
- PQTZ beats: tempo=0x36b1, times incrementing correctly ✓
- ANLZ path matches actual file location ✓

**What we've tried that didn't help:**
- Adding PVBR section to ANLZ
- Adding NUL terminator to PPTH path
- Setting analyze_date string
- Browsing from PLAYLIST view (not FOLDER)
- Loading track onto deck

**Unknown factors:**
- The `bitmask` field at offset 0x04-0x07 in track row (currently 0x0700) - meaning undocumented
- Unknown fields in PMAI header after file_length
- Possible additional ANLZ sections required (PCOB cue list?)
- Possible firmware-specific requirements for XDJ-XZ

**Next steps to investigate:**
1. Compare against a real Rekordbox export with BPM (need actual Rekordbox software)
2. Try adding empty PCOB (cue list) section
3. Research XDJ-XZ specific requirements
4. Check if waveform sections (PWAV/PWV2) are required for BPM display

### Current Limitations (remaining Phase 2 features)
- **BPM display**: Detection works but XDJ doesn't show it (investigation ongoing)
- **Waveforms**: Not implemented (PWAV, PWV2, PWV3, etc.)
- **Key detection**: Not implemented
- **Album artwork**: Not extracted/displayed

---

## Phase 1 Summary (COMPLETE)

### Key Achievements
1. **PDB file generation** - All 20 table types with correct page layout
2. **Multi-page track support** - Dynamic page allocation (~12 tracks/page)
3. **UTF-16LE string encoding** - Automatic detection for non-ASCII characters
4. **Reference data for sensitive tables** - Columns and HistoryPlaylists use byte-perfect copies
5. **ANLZ stub files** - Minimal valid .DAT/.EXT pairs
6. **USB directory structure** - PIONEER/rekordbox/, PIONEER/USBANLZ/, Contents/

### Critical Implementation Details

#### Sensitive Tables (use reference binary data)
Two tables require exact byte-level compatibility:
- **Columns table (page 34)** - `src/pdb/reference_columns.bin`
- **HistoryPlaylists table (page 36)** - `src/pdb/reference_history_playlists.bin`

#### Track Page Allocation
- ~12 tracks per page (conservative estimate due to variable row size)
- Reference pages: 2, 51 (used first)
- Additional pages allocated starting at page 56
- Empty candidate page at 55

#### String Encoding (DeviceSQL)
```rust
// Short ASCII: len ≤ 126, ASCII only
header = ((len + 1) << 1) | 1

// Long ASCII: len > 126, ASCII only
flags = 0x40, length = byte_len + 4

// Long UTF-16LE: contains non-ASCII (accents, unicode)
flags = 0x90, length = byte_len + 4
```

---

## XDJ Hardware Constraints (from Pioneer manual)

**Supported Audio Formats:**
- MP3, AAC, WAV, AIFF, FLAC

**ID3 Tag Support:**
- v1, v1.1, v2.2.0, v2.3.0, v2.4.0

**Artwork Requirements:**
- JPEG only (.jpg or .jpeg extension)
- Maximum size: 800×800 pixels
- Embedded in audio file metadata

---

## Phase 2 Plan: Audio Analysis Features

### 2.1 BPM Detection (Priority: HIGH) - IMPLEMENTED (display issue pending)

**Goal:** Detect tempo and populate PDB tempo field + ANLZ beatgrid

**Status:** ✅ Detection works, ⚠️ XDJ display not working

**Implementation (complete):**
- `src/analysis/bpm.rs` - BPM detection using aubio-rs + symphonia
- `src/analysis/real.rs` - RealAnalyzer that uses BPM detection
- `src/anlz/writer.rs` - PQTZ beatgrid generation
- `src/pdb/writer.rs` - Tempo field at offset 0x38-0x3B

**How it works:**
1. Decode audio to mono PCM using `symphonia`
2. Run through `aubio-rs` Tempo detector (SpecFlux onset mode)
3. Generate PQTZ beatgrid with constant tempo (beat_number 1-4, tempo×100, time_ms)
4. Write tempo to PDB track row
5. Write PQTZ section to ANLZ .DAT file

**CLI usage:**
```bash
# With BPM detection (default)
cargo run --release -- --output /path/to/usb --playlist "MyPlaylist"

# Without BPM detection (faster, Phase 1 behavior)
cargo run --release -- --output /path/to/usb --playlist "MyPlaylist" --no-bpm
```

**Caching Strategy (TODO):**
- Write detected BPM to ID3 TBPM tag (MP3) or equivalent Vorbis comment
- Read existing tag before analysis to skip re-detection
- Source file owns the canonical BPM value

**Libraries used:**
| Library | Purpose | License | Notes |
|---------|---------|---------|-------|
| `aubio-rs` | Tempo detection | GPL-3.0 | Uses system libaubio via pkg-config |
| `symphonia` | Audio decoding | MPL-2.0 | Pure Rust, good codec coverage |
| `lofty` | Tag reading/writing | MIT/Apache | For future TBPM caching |
| `chrono` | Date formatting | MIT/Apache | For analyze_date field |

### 2.2 Key Detection (Priority: HIGH)

**Goal:** Detect musical key and populate PDB key_id field

**Approach:**
- Use `libkeyfinder` via FFI or CLI wrapper
- Map to Rekordbox key notation (Open Key: 1A-12B)
- Store in Keys table with proper ID reference

**Caching Strategy:**
- Write to ID3 TKEY tag (standard) or Vorbis INITIALKEY
- Read existing tag before analysis

**Recommended Libraries:**
| Library | Purpose | License | Notes |
|---------|---------|---------|-------|
| `libkeyfinder` | Key detection | GPL-3.0+ | Widely used (Mixxx), reliable |
| QM Vamp key estimator | Alternative | BSD | Part of QM plugin suite |

**Alternatives:**
- Essentia KeyExtractor (AGPL) - high quality, optional external backend

### 2.3 Waveform Generation (Priority: HIGH)

**Goal:** Generate preview and detail waveforms for ANLZ files

**Waveform Types (per Deep Symmetry docs):**
| Tag | Description | Resolution |
|-----|-------------|------------|
| PWAV | Preview waveform (blue) | 400 points |
| PWV2 | Preview waveform (RGB) | 400 points |
| PWV3 | Detail waveform | ~1 point/150 samples |
| PWV4 | Detail waveform (color) | Variable |
| PWV5 | High-res detail | Variable |

**Approach:**
1. Decode audio to PCM using `symphonia`
2. Compute amplitude/peaks using `dasp` (RMS, peak extraction)
3. For colored waveforms: FFT with `rustfft` + `realfft` for band energy (low/mid/high)
4. Write to ANLZ .DAT and .EXT files

**Recommended Libraries:**
| Library | Purpose | License | Notes |
|---------|---------|---------|-------|
| `symphonia` | Decode to PCM | MPL-2.0 | Primary decoder |
| `dasp` | Signal processing | MIT/Apache | RMS, windowing, peaks |
| `rustfft` + `realfft` | FFT | MIT/Apache | Band energy for RGB waveforms |

**Alternatives:**
- BBC `audiowaveform` CLI - fast waveform generation, then convert to ANLZ format

**Caching Strategy:**
- Store computed waveforms in SQLite cache
- Key by file content hash (SHA256 of first 1MB + file size)
- Location: `~/.cache/pioneer-exporter/`

### 2.4 Album Artwork (Priority: MEDIUM)

**Goal:** Extract embedded artwork and create Rekordbox artwork entries

**XDJ Constraints:**
- JPEG only (.jpg/.jpeg)
- Maximum 800×800 pixels

**Approach:**
1. Extract cover art using `lofty` (ID3 APIC, Vorbis PICTURE, MP4, etc.)
2. Resize to standard dimensions using `image` crate
3. Convert to JPEG if necessary
4. Write to PIONEER/Artwork/ directory
5. Update Artwork table with references

**Artwork Sizes (Rekordbox):**
- Thumbnail: 80×80
- Small: 160×160
- Large: 240×240 (or up to 800×800 for XDJ)

**Recommended Libraries:**
| Library | Purpose | License | Notes |
|---------|---------|---------|-------|
| `lofty` | Tag reading/writing | MIT/Apache | Multi-format, extracts APIC |
| `id3` | MP3-only tags | MIT | Lightweight alternative |
| `image` | Resize + JPEG encode | MIT | Standard image processing |

### 2.5 Beatgrid (Priority: LOW)

**Goal:** Generate beat grid for PQTZ section in ANLZ files

**Approach:**
- Reuse beat timestamps from `aubio-rs` Tempo detection
- Quantize to grid, handle tempo changes
- Write PQTZ section to ANLZ .DAT files

**Note:** Beatgrid depends on BPM detection, implement after 2.1 is stable.

---

## Phase 3 Plan: Performance & Caching

### 3.1 Analysis Cache

**Problem:** Large libraries (100-200GB, thousands of tracks) need efficient caching

**Solution: Multi-tier caching**

```
Tier 1: Source File Tags (BPM, Key)
├── Authoritative source
├── Survives file moves
└── Standard tags (TBPM, TKEY)

Tier 2: Local Cache Database (Waveforms, Beatgrids)
├── SQLite database: ~/.cache/pioneer-exporter/cache.db
├── Keyed by file content hash (SHA256 of first 1MB + size)
├── Stores: waveform data, beatgrid, analysis timestamp
└── Portable between machines

Tier 3: Export Cache (Destination)
├── Track already on USB? Skip copy
├── Compare by hash or mtime+size
└── Incremental exports
```

### 3.2 Incremental Export

**Skip work that's already done:**
1. Audio files already on USB → skip copy
2. BPM/Key already in ID3 tags → skip analysis
3. Waveform in cache → skip generation
4. Track unchanged since last export → skip entirely

**Implementation:**
- Maintain export manifest (JSON) on USB
- Track file hashes and analysis versions
- Compare before processing

### 3.3 Parallel Processing

**CPU-intensive operations benefit from parallelism:**
- Audio decoding
- BPM detection
- Key detection
- Waveform generation

**Implementation:**
- Use `rayon` for parallel iterators
- Process N tracks concurrently (configurable, default: CPU cores)
- Progress bar with per-track status

---

## Phase 4 Plan: Distribution & GUI

### 4.1 Binary Distribution

**Goal:** Provide self-contained binaries for users who can't build from source

**Approach:**
- GitHub Releases with pre-built binaries
- Target platforms: Linux x86_64, Linux ARM64
- Use `cargo-bundle` or manual packaging
- Consider AppImage for Linux universal binary

**Build requirements documentation:**
- Rust toolchain version
- System dependencies (libssl, etc.)
- Optional: audio libraries for Phase 2

### 4.2 Simple GUI

**Goal:** User-friendly interface for non-technical users

**Features:**
- Playlist selection (checkboxes)
- Output directory picker
- Options:
  - Allow/disallow ID3 tag modification
  - Cache location setting
  - Analysis quality (fast/balanced/accurate)
- Progress display
- Export log

**Technology options:**
- `egui` - Pure Rust, immediate mode GUI
- `iced` - Elm-inspired Rust GUI
- `tauri` - Web frontend + Rust backend
- GTK4 via `gtk4-rs` (native Linux feel)

**Recommendation:** Start with `egui` for simplicity, consider `tauri` for richer UI later

### 4.3 Configuration File

```toml
# ~/.config/pioneer-exporter/config.toml

[general]
rhythmbox_database = "~/.local/share/rhythmbox/rhythmdb.xml"
rhythmbox_playlists = "~/.local/share/rhythmbox/playlists.xml"

[cache]
directory = "~/.cache/pioneer-exporter"
max_size_gb = 10

[analysis]
modify_source_tags = true  # Write BPM/Key to source files
parallel_jobs = 0          # 0 = auto (CPU count)
waveform_quality = "balanced"  # fast, balanced, accurate

[export]
skip_existing_audio = true
verify_after_export = true
```

---

## Implementation Priority

### Immediate (Phase 2.1)
1. **BPM Detection** - Highest user value, enables beat-matching
2. **ID3 tag caching** - Store BPM in source files

### Short-term (Phase 2.2-2.3)
3. **Key Detection** - Harmonic mixing support
4. **Preview Waveform** - Basic waveform display on XDJ

### Medium-term (Phase 3)
5. **Full Waveform Set** - All waveform types
6. **Incremental Export** - Performance for large libraries
7. **Album Artwork** - Visual polish

### Long-term (Phase 4)
8. **GUI Application** - User-friendly interface
9. **Binary Distribution** - Easy installation
10. **Other library sources** - iTunes, Traktor, etc.

---

## License Considerations

### Phase 2 Library Licenses

| Library | License | Bundling Impact |
|---------|---------|-----------------|
| `symphonia` | MPL-2.0 | Safe to bundle |
| `dasp` | MIT/Apache | Safe to bundle |
| `rustfft` | MIT/Apache | Safe to bundle |
| `lofty` | MIT/Apache | Safe to bundle |
| `image` | MIT | Safe to bundle |
| `rubato` | MIT | Safe to bundle |
| `aubio-rs` | GPL-3.0 | **Requires GPL for binary** |
| `libkeyfinder` | GPL-3.0+ | **Requires GPL for binary** |

### Options for GPL-Free Distribution

1. **Accept GPL** - Distribute under GPL-3.0 (most straightforward)
2. **External analyzers** - Ship GPL tools as separate binaries, call via subprocess
3. **Optional features** - Make GPL dependencies optional at compile time
4. **Alternative algorithms** - Use permissively-licensed alternatives (if quality sufficient)

### Recommendation

For a DJ tool, GPL is generally acceptable since:
- Target users are end-users, not developers embedding the library
- Mixxx (popular open-source DJ software) uses GPL
- The analysis quality of aubio/libkeyfinder is well-proven

If AGPL libraries (Essentia) are needed, use as external subprocess to avoid license propagation.

---

## Technical Reference

### Track Row Structure
```
Header (94 bytes):
  0x00-0x01: subtype (0x0024)
  0x02-0x03: index_shift
  0x04-0x07: bitmask
  0x08-0x0B: sample_rate
  0x0C-0x0F: composer_id
  0x10-0x13: file_size
  0x14-0x1B: unknown fields
  0x1C-0x1F: artwork_id
  0x20-0x23: key_id
  0x24-0x2F: label/remixer IDs
  0x30-0x33: bitrate
  0x34-0x37: track_number
  0x38-0x3B: tempo (BPM × 100)
  0x3C-0x4B: genre/album/artist IDs
  0x4C-0x5D: remaining fields

String offsets (42 bytes): 21 × u16
String data follows
```

### ANLZ File Tags
```
.DAT file:
  PMAI - File header
  PPTH - Path reference
  PVBR - Variable bitrate info
  PQTZ - Beat grid
  PWAV - Preview waveform (blue)
  PWV2 - Preview waveform (RGB)

.EXT file:
  PMAI - File header
  PPTH - Path reference
  PWV3 - Detail waveform
  PWV4 - Detail waveform (color)
  PWV5 - High-res detail waveform
  PSSI - Song structure
```

### Key Documentation
- [Deep Symmetry - PDB Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- [Deep Symmetry - ANLZ Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html)
- [rekordcrate Rust Library](https://holzhaus.github.io/rekordcrate/)

---

## Development Notes

### Running Tests
```bash
cargo test --lib          # Unit tests
cargo test --test '*'     # Integration tests
```

### Hardware Testing
Always test exports on actual XDJ hardware - validation with rekordcrate is necessary but not sufficient.

### Debug Tips
- Use `xxd` to compare binary files byte-by-byte
- Compare against reference export from actual Rekordbox
- Page 34 (Columns) and page 36 (HistoryPlaylists) are XDJ-sensitive
