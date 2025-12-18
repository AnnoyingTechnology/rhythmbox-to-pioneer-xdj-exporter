# Pioneer Exporter Implementation Strategy

This document describes the phased implementation approach for the Rhythmbox → Pioneer USB exporter.

## Current Status (2025-12-18)

**Phase:** Phase 1 COMPLETE + Phase 1.5 COMPLETE
**Status:** ✅ Full export working on XDJ-XZ hardware with multi-page track support

### What Works
- XDJ-XZ recognizes USB and displays playlists
- Tracks load and play correctly
- Artist/Album/Title metadata displays properly
- Accented characters (UTF-16LE encoding) display correctly
- Multi-page track tables (tested with 45+ tracks across 4 pages)
- rekordcrate validation passes

### Current Limitations (Phase 2 features)
- **BPM/tempo**: Shows 0 (no detection yet)
- **Waveforms**: Not displayed (stub ANLZ files)
- **Beatgrid**: Not present
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

## Phase 2 Plan: Audio Analysis Features

### 2.1 BPM Detection

**Goal:** Detect tempo and populate PDB tempo field + ANLZ beatgrid

**Approach:**
- Use `aubio` or `librosa` bindings for beat detection
- Store BPM in track's tempo field (BPM × 100 as u32)
- Generate beatgrid for ANLZ files

**Caching Strategy:**
- Write detected BPM to ID3 TBPM tag (MP3) or equivalent
- Read existing BPM tag before analysis to skip re-detection
- Source file owns the canonical BPM value

**Libraries to evaluate:**
- `aubio-rs` - Rust bindings for aubio
- `symphonia` - Pure Rust audio decoding
- Python `librosa` via subprocess (fallback)

### 2.2 Key Detection

**Goal:** Detect musical key and populate PDB key_id field

**Approach:**
- Use `libkeyfinder` or Essentia for key detection
- Map to Rekordbox key notation (1A-12B, Open Key format)
- Store in Keys table with proper ID reference

**Caching Strategy:**
- Write to ID3 TKEY tag (standard) or custom tag
- Read existing key before analysis

**Libraries to evaluate:**
- `keyfinder-rs` - Rust bindings for libkeyfinder
- Essentia (C++ with potential Rust bindings)

### 2.3 Waveform Generation

**Goal:** Generate preview and detail waveforms for ANLZ files

**Waveform Types (per Deep Symmetry docs):**
1. **PWAV** - Preview waveform (400 data points, blue)
2. **PWV2** - Preview waveform (400 points, RGB)
3. **PWV3** - Detail waveform (variable, ~1 point/150 samples)
4. **PWV4** - Detail waveform with color
5. **PWV5** - High-resolution detail waveform

**Approach:**
- Decode audio to PCM samples
- Compute RMS/peak values at appropriate intervals
- Apply frequency analysis for colored waveforms
- Write to ANLZ .DAT and .EXT files

**Caching Strategy:**
- Store computed waveforms in cache directory
- Key by file hash (content-addressable)
- Location: `~/.cache/pioneer-exporter/waveforms/`

### 2.4 Album Artwork

**Goal:** Extract embedded artwork and create Rekordbox artwork entries

**Approach:**
- Extract cover art from audio file metadata (ID3 APIC, Vorbis PICTURE)
- Convert to JPEG if necessary, resize to standard dimensions
- Write to PIONEER/Artwork/ directory
- Update Artwork table with references
- Link tracks to artwork via artwork_id field

**Artwork dimensions (per Rekordbox):**
- Thumbnail: 80×80
- Small: 160×160
- Large: 240×240 or 320×320

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
