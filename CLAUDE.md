# Pioneer Exporter Implementation Strategy

## Current Status (2025-12-25)

**Phase:** PDB Structure Fixes - Size-Dependent Corruption
**XDJ Status:** ✅ WORKING - Playlists display, tracks play
**Rekordbox 5 Status:** ⚠️ PARTIAL - Small exports work, large exports corrupted

### Latest Test Results (2025-12-25):
| Export Size | Rekordbox 5 | Notes |
|-------------|-------------|-------|
| 1 track | ✅ Works | Reference-1 match |
| 4 tracks | ✅ Works | Small export OK |
| ~40 tracks (2 playlists) | ❌ Corrupted | Size-dependent issue |

**Key Finding:** Corruption is SIZE/LENGTH/CONTENT dependent. Small exports now work after structural fixes. Large exports still fail. This indicates scaling issues in multi-page or multi-entity handling.

### What Works on XDJ:
- ✅ USB is recognized
- ✅ Playlists are visible with correct count
- ✅ Tracks are playable
- ✅ All table chains valid (verified by rekordcrate)

### What's Broken:
- ❌ **Track count display**: XDJ shows "1 track" (from static History table data from reference-1)
- ❌ **Large export corruption**: Rekordbox 5 rejects exports with ~40+ tracks

### Fixes Applied Today (Session 2):
1. **File header values** - Fixed `next_unused_page=53` and `sequence=14` for single-track exports
2. **Page header scaling** - `unknown1` and `unknown3` now vary based on track count:
   - 1 track: unknown3=0x20, unknown1 small (0x07-0x0a per table)
   - 2+ tracks: unknown3=0x60, unknown1 larger (0x18-0x1b per table)
3. **History tables** - Updated reference data from reference-1 (1-track export)
4. **Function signatures** - Added `track_count` parameter to genre/artist/album/keys table writers

### Fixes Applied Today (Session 1):
1. **Page allocation conflict** - Was allocating new track pages at 41-52, but these are `empty_candidate` pointers for other tables. **Fixed by starting allocation at page 53.**
2. **Empty candidate pages 41-52** - When file extends beyond 40 pages, pages 41-52 must have proper headers with correct types.
3. **File size bug** - File was hardcoded to 41 pages. Now dynamically allocates based on track count.
4. **next_unused_page** - Now always 53 (reserves empty_candidate range even if file is smaller).

### Remaining Issues (Priority Order):
1. **Large export corruption** - Must investigate what breaks when scaling beyond ~10 tracks
   - Possible causes: multi-page row groups, page linking, entity count limits
   - Need to compare reference-20 (20 tracks) with our 20-track export
2. **Track count from History tables** - Static reference data shows wrong count
3. **Keys table** - We include all 24 keys, reference only includes used keys (may not cause corruption)

### Investigation Path for Large Export Corruption:
1. Generate exact 20-track export matching reference-20 structure
2. Binary compare page-by-page to find structural differences
3. Check row group handling for tables with >16 rows
4. Verify page linking for multi-page tables (Tracks, PlaylistEntries)
5. Test incremental sizes: 5, 10, 15, 20, 25, 30 tracks to find breakpoint

---

## Expert Analysis Summary (2025-12-25)

Consulted 4 experts on the waveform display issue. Key insights consolidated below.

### Consensus: PWV4 All-Zeros is the #1 Suspect

All experts agree that **PWV4 (color preview waveform) being all zeros is likely the root cause**.

- XDJ-XZ is a Nexus 2-era device that relies heavily on PWV4 for the scrolling waveform overview
- If PWV4 is present but "empty" (all zeros), firmware may treat waveform analysis as **missing/invalid** and hide waveform UI entirely (no fallback to monochrome)
- Reference-1 has 3178 non-zero bytes in PWV4; ours has 7200 zeros

**PWV4 Structure (6 bytes per entry, 1200 entries):**
```
Channel 0: Unknown (affects blue waveform whiteness) - try 0x40
Channel 1: Luminance boost - CRITICAL: MUST be non-zero (try 0x60)
Channel 2: Inverse intensity for blue - non-zero required (try 0x20)
Channel 3: Red component (0-127)
Channel 4: Green component (0-127)
Channel 5: Blue component + height (0-127)
```

### Other High-Priority Issues Identified

1. **Height values too low/zero** - Need minimum floor
   - PWAV: Reference shows height=2 (`0xa2`), ours shows height=0 (`0xa0`)
   - PWV2: Reference starts at `0x01`, ours starts at `0x00`
   - Pioneer players may interpret zero heights as "uncalculated" or "silent"
   - **Fix:** Clamp all heights to minimum 1-2, use log scaling

2. **PWV5 all-white may be rejected**
   - Ours: `ff80` (RGB=7,7,7, all white)
   - Reference: `e000`, `0b80`, `0b84` (varied colors)
   - Player may expect spectral coloring (lows→red, highs→blue)
   - **Fix:** Implement frequency-based coloring or at least varied colors

3. **Possible "Waveform Analyzed" bitmask in PDB**
   - Track row offset 0x04-0x07 contains a bitmask
   - Possible bit meanings: Bit 0=Beatgrid, Bit 1=Waveform, Bit 2=Key, Bit 3=Phrase
   - If "Waveform" bit is 0, player may skip waveform rendering
   - **Fix:** Compare our bitmask with reference; ensure bit 1 is set

4. **PVBR trailing bytes differ**
   - Reference: `d3 80` (VBR timing data)
   - Ours: `00 00`
   - May participate in "analysis validity / needle search safety" check
   - **Fix:** Copy PVBR bytes from reference as a test

### Key Diagnostic Test (All Experts Recommend)

**"Reference Injection Test" - Isolates Data vs PDB issue:**

1. Keep our generated `export.pdb` (points to our path P9CC/00C30E0D)
2. Copy **Reference ANLZ files** (from P05C/0001D2C0) into our path
3. Test on XDJ-XZ:
   - **YES waveforms** → Our PDB is fine; our waveform DATA is the problem
   - **NO waveforms** → Problem is in PDB flags/validation, not ANLZ data

This single test will eliminate half the possibilities.

### Other Ideas to Explore

- **Omit PWV4 entirely** (don't zero-fill) - forces fallback to monochrome?
- **Test with longer tracks** - 1-second sample may be too short (185 entries at 150/sec = 1.23s min)
- **exportExt.pdb ID mismatch** - Static copy may have wrong track IDs
- **Unknown PMAI header bytes (12-27)** - Could be checksums/flags
- **Firmware quirk** - XDJ-XZ specific; test on different hardware if possible

---

## Battle Plan: Waveform Fix (2025-12-25)

### Phase A: Diagnostic Tests (No Code Changes)

| # | Test | Expected Result | If YES | If NO |
|---|------|-----------------|--------|-------|
| A1 | Reference Injection: Copy reference ANLZ files to our path | Waveforms display | Data issue | PDB issue |
| A2 | Omit PWV4 entirely from EXT file | Fallback to PWAV? | PWV4 zeros = fatal | Structure issue |
| A3 | Use reference ANLZ path (P05C/0001D2C0) in our PDB | Waveforms display | Path calculation issue | Other PDB issue |

**A1 RESULT (2025-12-25): NO WAVEFORMS** → Problem is AT LEAST in PDB
- Copied reference ANLZ files to our path P9CC/00C30E0D
- Our PDB + reference ANLZ = still no waveforms
- This proves we have a PDB issue

**REVERSE TEST (2025-12-25): NO WAVEFORMS** → Problem is ALSO in ANLZ!
- Reference PDB (patched to point to P9CC/00C30E0D) + Our ANLZ = still no waveforms
- This proves we have BOTH PDB and ANLZ issues
- Must fix both to get waveforms working

**ANLZ Comparison Findings (2025-12-25):**
| Component | Reference | Ours | Issue |
|-----------|-----------|------|-------|
| DAT PQTZ size | 24 bytes (header-only) | 48 bytes (3 beats) | We generate beats for 1-sec sample |
| DAT PVBR trailing | `d3 80` | `00 00` | VBR timing data missing |
| DAT total size | 2362 bytes | 2386 bytes | 24 byte difference from PQTZ |
| EXT PWV4 | ALL ZEROS | `40 20 10...` pattern | Reference is zeros, ours is non-zero |
| PWAV heights | `a2` (height=2) | `a1` (height=1) | Minor, probably OK |

**Key insight:** Reference PWV4 is all zeros! The experts were wrong about PWV4 needing data.

---

## CRITICAL USER TESTS (2025-12-25) - Breakthrough!

User performed systematic tests on reference-1 export, swapping files with ours:

| Test | Result | Conclusion |
|------|--------|------------|
| Remove `exportExt.pdb` | ✅ Works | **exportExt.pdb NOT required** - stop generating |
| Remove `ANLZ0000.EXT` | ❌ Broken (narrow blue peaks only) | **EXT file is CRITICAL** for waveforms |
| Remove `ANLZ0000.DAT` | ✅ Works | DAT file is secondary/optional |
| Swap DAT with ours | ✅ Works | **Our DAT is valid** - not the problem |
| Swap EXT with ours | ⚠️ PARTIAL - Main screen works, needle search & jogwheel broken | **Our EXT is partially broken** |

### Key Inferences:

1. **exportExt.pdb is NOT needed** - Can disable generation entirely
2. **ANLZ0000.DAT is secondary** - Our DAT works fine, beatgrid/preview not blocking
3. **ANLZ0000.EXT is the critical file** - Must be correct for waveforms
4. **Our EXT is PARTIALLY working:**
   - ✅ **Main screen waveform** (PWV3/PWV5) - WORKS
   - ❌ **Needle search** (PWV4) - BROKEN
   - ❌ **Jogwheel** (PWV4?) - BROKEN

### Root Cause Identified and FIXED: PWV4 in EXT file

The main screen waveform uses **PWV3/PWV5** (detail waveforms) - our generation is correct!
The needle search and jogwheel use **PWV4** (color preview) - **WAS BROKEN, NOW FIXED!**

**Root Cause (2025-12-25):**
```rust
// OLD CODE - BROKEN:
let color_preview = Vec::new(); // Returns empty, written as all zeros

// NEW CODE - FIXED:
let color_preview = generate_pwv4(&samples, sample_rate); // Actually generates waveform data
```

The `generate_pwv4()` function existed but was never called! Line 58 in `waveform.rs` returned an empty Vec with a misleading comment saying "Reference exports have all zeros" - which was FALSE. Binary comparison showed reference PWV4 has ~349 lines of non-zero data!

**PWV4 Format (corrected from expert speculation):**
Each 6-byte entry has 3 columns for frequency bands (low/mid/high):
- Bytes 0-1: Low frequency (height 0-31, whiteness 0xF0-0xFF)
- Bytes 2-3: Mid frequency (height, whiteness)
- Bytes 4-5: High frequency (height, whiteness)

**Status:** Major progress! Small exports now work in Rekordbox 5. Large exports still show as corrupted. Track count display bug remains.

### Latest Test Results (2025-12-25):
- ✅ **Rekordbox 5 accepts small exports (5 tracks)** - no more "corrupted database" error!
- ✅ **Playlists are visible** in Rekordbox
- ✅ **Waveforms appear to be showing** (may be Rekordbox regenerating or reading ours)
- ❌ **Large exports (35 tracks) still show as corrupted** - scaling issue in PDB
- ❌ **USB1 screen always shows "3 tracks"** - hardcoded value somewhere

### Critical Test Results (2025-12-25 - Post PWV4 Fix):

| Test | Result | Conclusion |
|------|--------|------------|
| Our export (with PWV4 fix) | ❌ No waveforms | Still broken |
| Our export + reference DAT | ❌ No waveforms | DAT not the issue |
| Our export + reference EXT | ❌ No waveforms | EXT not the issue |

**Conclusion:** Even with reference ANLZ files, our PDB prevents waveforms from displaying. The problem is 100% in **export.pdb**.

### PDB Page Header Fix (2025-12-25):

Discovered page header differences between reference-1 (1 track) and our export:

| Field | Reference-1 | Ours (old) | Ours (fixed) |
|-------|-------------|------------|--------------|
| page_unknown1 | 0x0b (11) | 0x1c (28) | 0x0b (11) |
| unknown3 | 0x20 (32) | 0x60 (96) | 0x20 (32) |

**Pattern discovered:**
- `unknown3` = 0x20 × num_tracks (1 track = 0x20, 3 tracks = 0x60)
- `page_unknown1` = 0x0b for 1 track, 0x1c for 3+ tracks

**Fixed in `writer.rs`:**
```rust
// unknown3: 0x20 for 1 track, 0x60 for 2+ tracks
if tracks.len() <= 1 { 0x20u8 } else { 0x60u8 }

// page_unknown1 depends on track count
let page_unknown1 = match track_chunk.len() {
    0..=1 => 0x0bu32,
    2..=3 => 0x1cu32,
    4..=6 => 0x28u32,
    7..=10 => 0x34u32,
    _ => 0x3cu32, // 11+ tracks
};

// unknown4: 0x00 for small pages, 0x01 when page is getting full (11+ tracks)
let unknown4 = if track_chunk.len() >= 11 { 0x01u8 } else { 0x00u8 };

// sequence field scales with content
let total_entities = tracks.len() + artists.len() + albums.len() + genres.len();
let sequence = 14u32 + (total_entities as u32 * 4);
```

### Test Results (2025-12-25 - After Page Header + Sequence Fixes):

| Issue | Status | Notes |
|-------|--------|-------|
| Track count display | ❌ Still wrong | Shows "3 tracks" regardless of actual count |
| Waveforms | ❌ Still not showing | Despite PWV4 fix, no waveforms display |
| Large export (35 tracks) | ❌ Still corrupted in Rekordbox 5 | Sequence formula didn't help |

### Remaining Issues:

1. **"3 tracks" display on USB screen** - Coming from static `reference_history.bin` and `reference_history_entries.bin` files that were copied from a 3-track reference export. These contain hardcoded track counts.

2. **Large export corruption (35+ tracks)** - Sequence field fix didn't resolve. Possible causes:
   - Table pointer structure incorrect
   - Multi-page track allocation issues
   - Row group handling across pages
   - Unknown validation fields

3. **Waveforms still not displaying** - PWV4 is being generated with actual data (verified via hex dump), but XDJ still doesn't show waveforms. The PDB must have some flag or field that prevents waveform rendering.

---

## Priority: Fix DeviceSQL Compliance First (2025-12-25)

**Key Insight:** If Rekordbox 5 reports "corrupted database", this is a **DeviceSQL format violation**, not an XDJ-specific issue. We should:

1. **Study DeviceSQL specifications** - The PDB format is based on DeviceSQL. We need to understand the exact specification rather than reverse-engineering from hex dumps.

2. **Use dedicated validators** - Instead of guessing at field values, use tools like:
   - `rekordcrate` library's parser (we already use this for validation)
   - Deep Symmetry's crate-digger Kaitai struct definitions
   - Write our own validator that checks all DeviceSQL constraints

3. **Focus on validity first** - A perfectly valid DeviceSQL file is more likely to work on XDJ hardware. We've been chasing XDJ-specific symptoms while the underlying database format may be fundamentally broken.

**Resources to consult:**
- https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html (DeviceSQL page structure)
- https://github.com/Deep-Symmetry/crate-digger/blob/main/src/main/kaitai/rekordbox_pdb.ksy (Kaitai spec)
- rekordcrate library source code for parsing logic

**Next Steps:**
1. Read DeviceSQL spec thoroughly
2. Compare our output against spec requirements (not just reference bytes)
3. Identify which DeviceSQL rules we violate
4. Fix fundamental format issues before debugging XDJ features

---

### Phase B: PWV4 Fixes (Primary Suspect) - COMPLETED

| # | Fix | Details |
|---|-----|---------|
| B1 | Generate minimal non-zero PWV4 | Channels 0-5: `[0x40, 0x60, 0x20, 0x7F, 0x7F, 0x7F]` per entry |
| B2 | Copy reference PWV4 data | Hex-copy 7200 bytes from reference EXT |
| B3 | Implement proper PWV4 generation | FFT-based spectral coloring |

### Phase C: Height/Amplitude Fixes

| # | Fix | Details |
|---|-----|---------|
| C1 | Add height floor to all waveforms | `height = max(1, computed_height)` |
| C2 | Scale heights by 4-5x | Match reference range (2-22 vs our 0-5) |
| C3 | Use log scaling instead of linear | Matches Rekordbox's visual style |

### Phase D: Other Data Fixes

| # | Fix | Details |
|---|-----|---------|
| D1 | Implement varied PWV5 colors | Frequency-based RGB, not all white |
| D2 | Copy PVBR from reference | Test if it participates in validation |
| D3 | Match unknown2 string | Change "4" to "3" |

### Phase E: PDB Investigation

| # | Fix | Details |
|---|-----|---------|
| E1 | Check bitmask at track row 0x04-0x07 | Ensure "Waveform" bit is set |
| E2 | Generate dynamic exportExt.pdb | Avoid static copy ID mismatch |
| E3 | Binary diff entire export.pdb | Find any remaining differences |

### Recommended Order of Execution

1. **A1** (Reference Injection) - Most valuable diagnostic, takes 5 minutes
2. **A2** (Omit PWV4) - Quick test to confirm PWV4 zeros are fatal
3. **B2** (Copy reference PWV4) - If A1 shows data issue
4. **C1 + C2** (Height fixes) - Easy code changes
5. **B1** (Minimal PWV4) - If B2 works, implement proper generation
6. **E1** (Bitmask check) - If A1 shows PDB issue

---

## Audio Analysis (Completed)

**Powered by stratum-dsp** - unified BPM + key detection in a single pass.

Features:
- **Parallel processing** - uses rayon with (cores - 1) threads for multi-threaded analysis
  - 8 tracks in ~3 seconds (release build, 31 threads on 32-core system)
  - Use `cargo run --release` for best performance
- **BPM detection** with range constraint (default 70-170 BPM) - handles octave errors
- **Key detection** using chroma-based Krumhansl-Kessler template matching
- **Single audio decode** per track - efficient, no duplicate processing
- **Skips tracks with existing metadata** from ID3/Vorbis tags
- **Optional caching** to source files (`--cache-bpm`, `--cache-key`)
  - Works for FLAC files
  - MP3 skipped due to lofty library issues with TBPM/TKEY frames

CLI options:
```bash
cargo run -- --output /path/to/usb --playlist "MyPlaylist"           # BPM + key detection enabled
cargo run -- --output /path/to/usb --playlist "MyPlaylist" --no-bpm  # Skip BPM detection only
cargo run -- --output /path/to/usb --playlist "MyPlaylist" --no-key  # Skip key detection only
cargo run -- --output /path/to/usb --playlist "MyPlaylist" --no-bpm --no-key  # Stub mode (fast)
cargo run -- --output /path/to/usb --playlist "MyPlaylist" --cache-bpm --cache-key  # Cache to files
cargo run -- --output /path/to/usb --min-bpm 100 --max-bpm 180       # Custom BPM range
```

Dependencies:
- `stratum-dsp` - BPM + key detection (pure Rust, ~87% BPM accuracy, ~72% key accuracy)
- `symphonia` - audio decoding (MP3, FLAC, AAC, WAV, OGG)
- `lofty` - metadata read/write
- `rayon` - parallel processing

---

## Roadmap

### Phase 2 - Complete
- [x] BPM detection with range normalization
- [x] Key detection with correct Rekordbox ID mapping
- [x] Parallel track analysis (31 threads, ~5 tracks/sec)
- [x] Smart/automatic playlist support (genre, duration, artist filters)
- [x] Metadata caching (FLAC only, MP3 TODO)
- [x] Key ID fix (chromatic order from A: minor 1-12, major 13-24)
- [x] Filename sanitization for FAT32 (quotes, colons, etc. → underscore)

### Known Issues
- ~~**Some tracks show blank artist on XDJ**~~ - **FIXED** (see Row Group Fix below)
- ~~**FAT32 filename issues**~~ - **FIXED** (Phase 2.1)
  - Illegal chars replaced with underscores
  - Filenames truncated to 250 chars (preserving extension)
  - Path components truncated to 200 chars
  - Note: FAT32 case-insensitivity is NOT handled (same folder names with different cases may collide)
- **Performance is poor for large exports** - ~10 minutes for 84 tracks
  - 30GB RAM usage during analysis (stratum-dsp decodes full audio to memory)
  - All CPU cores maxed (31 threads on 32-core system)
  - Use `--max-parallel N` to limit concurrent analyses and reduce RAM usage
- **PDB shows as "corrupted" in Rekordbox 5** - Cannot be imported back into Rekordbox
- **Wrong track/playlist count on XDJ USB screen** - Counts displayed are incorrect
- **Several PDB tables use static reference data (hacks)**:
  - `reference_history.bin` (page 40) - History table
  - `reference_history_entries.bin` (page 38) - HistoryEntries table
  - `reference_history_playlists.bin` (page 36) - HistoryPlaylists table
  - `reference_columns.bin` (page 34) - Columns table
  - `exportExt.pdb` - Entire file copied from reference export
  - These are required for XDJ to recognize the USB but contain data from reference export, not our actual export

### Phase 2.1 - Complete
- [x] Rhythmbox track rating (stars) to PDB rating
  - Reads rating from ID3 POPM (Popularimeter) frames using `id3` crate
  - Converts ID3 rating (0-255) to stars (1-5): 1-31→1, 32-95→2, 96-159→3, 160-223→4, 224-255→5
  - Falls back to Rhythmbox XML rating if POPM not present
- [x] FAT32 filename sanitization (illegal chars, truncation to 250 chars)
- [x] FAT32 path component sanitization (truncation to 200 chars)
- [x] `--max-parallel` CLI option to limit concurrent analyses (memory optimization)

### Phase 3 - Waveforms (IN PROGRESS - See Battle Plan)
- [x] PWAV waveform preview (400 bytes, monochrome) - .DAT file
- [x] PWV2 tiny preview (100 bytes) - .DAT file
- [x] PWV3 waveform detail (150 entries/sec, monochrome) - .EXT file
- [x] PWV5 color waveform detail (150 entries/sec, 2 bytes/entry) - .EXT file
- [ ] **PWV4 color preview** - Currently all zeros (SUSPECTED ROOT CAUSE)
- [x] **ExportExt.pdb with reference data** (copied from reference-1)
- [x] **StubAnalyzer fix** - now generates actual waveforms instead of empty stubs
- [x] **Whiteness/height encoding fix** - PWAV uses whiteness=5, PWV3 uses whiteness=7
- [ ] **Height floor/scaling** - Heights too low (0-5 vs reference 2-22)
- [ ] **PWV5 varied colors** - Currently all white, may need frequency-based coloring
- [ ] PWV6/PWV7 3-band waveforms (CDJ-3000) - not needed for XDJ-XZ
- [ ] **WAVEFORMS NOT DISPLAYING** - See Battle Plan above for next steps

---

## Waveform Debugging Status (2025-12-24)

### What Works
- **Beatgrids display** on XDJ-XZ (beat markers visible across track)
- **ANLZ files are being read** (proven by beatgrid display from PQTZ section)
- **Waveform data is generated** (verified 185 entries in PWV3/PWV5 for 1-second sample)
- **Reference-1 export displays waveforms** when copied to USB as-is

### What Was Fixed (but waveforms still don't display)
1. **StubAnalyzer bug** - Was returning `WaveformData::minimal_stub()` (empty vectors) when using `--no-bpm --no-key`. Now calls `generate_waveforms()` to actually analyze audio.
2. **exportExt.pdb header bytes** - Bytes 16-19 must be `05 00 00 00` and bytes 20-23 must be `04 00 00 00`. Our reference file had these swapped. Fixed by copying from reference-1.

### What Was Verified (comparing our export vs reference-1 for Fresh.mp3)

| Component | Match? | Notes |
|-----------|--------|-------|
| EXT file structure | ✅ Yes | Same sections, same order, same sizes |
| PWV3 entry count | ✅ Yes | Both have 185 entries (0xb9) |
| PWV5 entry count | ✅ Yes | Both have 185 entries |
| PWV4 size | ✅ Yes | Both 7224 bytes (zeros) |
| DAT file structure | ✅ Yes | Same sections, same order |
| PQTZ section | ✅ Yes | Both have header-only (no beats for 1-sec sample) |
| exportExt.pdb header | ✅ Yes | Now has correct 05/04 byte order |
| Track row fixed fields | ✅ Yes | Structure matches, minor value differences |
| Track row strings | ✅ Yes | Same format, paths consistent |

### PDB Track Row Comparison (offset from row start at 0x2028)

| Field | Reference | Ours | Notes |
|-------|-----------|------|-------|
| file_size (0x10) | 34857 | 30491 | Different file copies |
| u2 (0x14) | 44 | 21 | Unknown field |
| key_id (0x20) | 1 | 0 | We use --no-key flag |
| bitrate (0x30) | 192 | 320 | We hardcode 320 |
| unknown2 string | "3" | "4" | Minor string difference |
| analyze_path | P05C/0001D2C0 | P9CC/00C30E0D | Different hash, but consistent |

### Key Differences That Remain
1. **ANLZ path differs** - Reference uses `P05C/0001D2C0`, ours uses `P9CC/00C30E0D`
   - Paths are consistent (PDB track row matches actual ANLZ location)
   - Different hash algorithm or track ID calculation
2. **Waveform DATA values differ** - Our algorithm produces different amplitude values than Rekordbox
   - Reference PWV3: `e0 e0 a0 80 a0 c0 60 a0 e0...` (varied)
   - Ours PWV3: `e0 e0 e0 e0 e0 e0 e0 e0 e1 e2...` (smoother)
3. **PWV5 colors are all white** - We hardcode RGB=(7,7,7) instead of frequency-based coloring
   - Reference: `e000`, `0b80`, `0b84` (varied colors)
   - Ours: `ff80`, `ff84` (all white)
4. **PVBR trailing bytes** - Reference has `d3 80`, ours has `00 00` (VBR timing data)

### Critical Question
**Why does copying reference-1 PIONEER folder work, but our generated export doesn't?**

The reference PIONEER folder contains:
- export.pdb with track pointing to P05C/0001D2C0
- ANLZ files at P05C/0001D2C0
- exportExt.pdb with correct header

Our export contains:
- export.pdb with track pointing to P9CC/00C30E0D
- ANLZ files at P9CC/00C30E0D
- exportExt.pdb with correct header (after fix)

Both are internally consistent. The issue must be either:
1. Something in export.pdb we haven't identified
2. The ANLZ path hash calculation differs from what XDJ expects
3. The waveform data encoding is wrong (despite correct structure)

### Next Steps to Try
1. **Use reference ANLZ path** - Hardcode P05C/0001D2C0 instead of computing hash
2. **Copy reference ANLZ files** to our path (P9CC/00C30E0D) with our PDB
3. **Match unknown2 string** - Change from "4" to "3"
4. **Try with BPM/key detection enabled** (without --no-bpm --no-key)
5. **Binary diff entire export.pdb** to find any other differences

### Test Reference: reference-1
Single track export created by Rekordbox:
- **Track:** Fresh.mp3 (1-second voice sample, ~30KB)
- **Playlist:** REKORDBOX4
- **Location on USB:** `/run/media/julien/USB/reference-1/`
- **ANLZ path:** `PIONEER/USBANLZ/P05C/0001D2C0/`

### Current Test Command
```bash
# Single track export for comparison
cargo run --release -- --output /tmp/test-single --playlist "REKORDBOX4" --no-bpm --no-key
```

---

Waveform encoding (as implemented - NEEDS FIXES 2025-12-25):
- PWAV: height (5 low bits) | whiteness (3 high bits) - whiteness=5 like reference
- PWV2: height (4 bits) - simple peak amplitude **⚠️ NEEDS MIN FLOOR**
- PWV3: height (5 low bits) | whiteness (3 high bits) - whiteness=7 like reference
- PWV5: RGB (3 bits each) | height (5 bits) - always white (7,7,7) **⚠️ NEEDS VARIED COLORS**
- PWV4: 1200 entries × 6 bytes - ALL ZEROS **⚠️ SUSPECTED ROOT CAUSE - NEEDS NON-ZERO DATA**

Implementation:
- Uses symphonia to decode audio to mono samples
- Calculates RMS and peak per time window
- Height from peak amplitude (0-31 range for 5-bit fields)

### Waveform Fix Summary (2025-12-24)
**Root cause:** Heights were being computed correctly, but earlier debug output with
`whiteness.max(7)` was producing `0xe0` (whiteness=7, height=0) bytes which visually
appeared as "all zeros" when the actual issue was the heights getting computed.

**Fixes applied:**
1. PWAV now uses whiteness=5 (was always 7)
2. PWV3 now uses whiteness=7 (was 5 after first fix)
3. Added debug logging for PWAV to verify peak/height values

**Test exports created:**
- `/tmp/test-final2` - Latest export with all fixes

### Phase 4 - Artwork
- [ ] Extract embedded artwork from audio files (lofty)
- [ ] Resize to Pioneer format (80x80, 56x56)
- [ ] Write to USB artwork directory

Libraries to use:
- `lofty` - extract APIC/picture from tags
- `image` - resize + JPEG encode

### Phase 5 - Beatgrid (Low Priority)
- [ ] Beat timestamp detection (stratum-dsp has BeatGrid)
- [ ] PQTZ section in ANLZ files
- [ ] Quantized beat positions

---

## Row Group Fix (2025-12-24)

**Root cause of blank artist metadata in large exports:** Incorrect row group footer structure.

The PDB format stores row offsets in "row groups" of 16 rows each at the end of data pages. The footer grows downward from the page boundary. Each group contains:
- Row offsets (2 bytes each, in reverse order within group)
- Present flags (2 bytes) - bitmask of which slots are used
- Unknown field (2 bytes) - 0 for full groups, 2^highest_bit for partial

**What was wrong:**
1. We wrote `unknown=0x8000` for full groups (should be `0x0000`)
2. We wrote 16 offsets for ALL groups (partial groups should only have actual row count)
3. We wrote groups in forward order (should be reverse: last group first)

**Reference analysis (`examples/reference-20/` with 20 artists = 2 row groups):**
```
Footer at 0x6fd0-0x6fff (48 bytes):
- Group 1 (partial, 4 rows) at 0x6fd0-0x6fdb: 4 offsets + present=0x000f + unknown=0x0008
- Group 0 (full, 16 rows) at 0x6fdc-0x6fff: 16 offsets + present=0xffff + unknown=0x0000
```

**Code changes in `src/pdb/writer.rs`:**
- `row_group_unknown_high_bit()`: Return 0 when flags=0xffff
- `write_row_groups()`: Iterate `(0..num_groups).rev()` to write in reverse order
- `write_row_groups()`: Only write actual row count offsets for partial groups
- `row_group_bytes()`: Calculate `full_groups * 36 + partial_rows * 2 + 4`

---

## Key Discovery: History Tables

The History tables (pages 36, 38, 40) are **required** for XDJ to recognize the USB, but they work as **static reference data** - they don't need to match the actual exported tracks!

- **Empty History tables** = USB not recognized at all
- **Reference History tables** = Works with ANY number of tracks (tested: 3, 4, 10)

The XDJ appears to need valid History table structure for initialization, but doesn't use them to limit which tracks are accessible.

---

## Phase 1 Cleanup (Completed)

Removed hardcoded values that were only needed for byte-perfect reference matching:

1. **ANLZ paths** - Removed `REFERENCE_TRACK_DATA` hardcoding in `organizer.rs`
   - Now uses FNV-1a hash for all tracks (was hardcoded for TITLETEST1/2/3)

2. **Key IDs** - Now uses detected key from stratum-dsp
   - Was: hardcoded key IDs for test tracks
   - Now: dynamically sets key_id from audio analysis

3. **Keys table** - Expanded from 3 to 24 musical keys
   - Was: Am, Bm, Cm only
   - Now: All 12 minor + 12 major keys

**Static reference data (kept as-is, works for all exports):**
- `reference_history.bin` (page 40) - History table data
- `reference_history_entries.bin` (page 38) - HistoryEntries table data
- `reference_history_playlists.bin` (page 36) - HistoryPlaylists table data
- `reference_columns.bin` (page 34) - Column definitions

---

## ROOT CAUSE: Why Track Info Wasn't Displaying

**The History tables (pages 38 and 40) must contain valid data, not empty/blank pages.**

Even though the user could delete ANLZ files and exportExt.pdb from a working Rekordbox export, the XDJ-XZ **requires valid HistoryEntries and History table data** to display track metadata. When we wrote blank pages for these tables, track info would not display even though:
- Track rows were byte-identical to reference
- Artist/Album/Genre data was byte-identical
- Playlist data was byte-identical
- All string offsets matched perfectly
- All critical fields (artist_id, album_id, genre_id, file_type, etc.) matched

**Solution:** Copy the reference History table pages directly:
- `reference_history_entries.bin` (page 38)
- `reference_history.bin` (page 40)
- `reference_history_playlists.bin` (page 36)

This is counterintuitive because History tables seem like optional tracking data, but the XDJ appears to validate or use them when loading track metadata.

---

## All Fixes Applied (in order of discovery)

1. **History tables (THE FIX)** - Use reference page data for HistoryEntries (page 38), History (page 40), and HistoryPlaylists (page 36)
2. ~~**ANLZ paths** - Hardcoded reference track IDs for TITLETEST1/2/3~~ **(REMOVED - now uses FNV-1a hash)**
3. **Track row padding** - 0x158 bytes between track rows (reference alignment)
4. **Entity row padding** - Artists: 28 bytes, Albums: 40 bytes per row
5. **Page header flags** - 0x60, 0x00 at bytes 0x19-0x1A for most data pages
6. **Track key_id** - Now dynamically set from stratum-dsp key detection
   - Key ID mapping fixed to match Keys table (chromatic order from A, minor 1-12, major 13-24)
7. **Album artist_id** - Set to 0 (not actual artist ID) to match reference
8. **Empty tables** - Labels and Artwork are header-only (no data pages)
9. **File header** - `next_unused_page=53`, `sequence=31` to match reference
10. **Keys table** - Expanded to all 24 musical keys (was 3)
11. **Row group structure fix (MAJOR)** - Fixed multi-row-group handling for large exports
    - `row_group_unknown_high_bit()`: Returns 0 for full groups (flags=0xffff), 2^highest_bit for partial
    - `write_row_groups()`: Writes groups in REVERSE order (partial first, group 0 at page boundary)
    - `write_row_groups()`: Only writes actual row count offsets for partial groups (not padded to 16)
    - `row_group_bytes()`: Calculates correct footer size: full_groups × 36 + partial_rows × 2 + 4
    - This fixed blank artist metadata for tracks in the first row group (IDs 1-16)

---

## Test Setup

**Rhythmbox Playlists (must match reference):**
- REKORDBOX1: Track 1 (TITLETEST1)
- REKORDBOX2: Track 1, 2
- REKORDBOX3: Track 1, 2, 3

**Reference export command:**
```bash
cargo run -- --output /path/to/usb --playlist "REKORDBOX1" --playlist "REKORDBOX2" --playlist "REKORDBOX3" --no-bpm
```

**Quick test playlist (35 tracks, lightweight for USB testing):**
```bash
cargo run --release -- --output /path/to/usb --playlist "XDJ: Minimal"
```

---

## Reference Export (`examples/reference/`)

**3 tracks, 3 playlists** - works 100% on XDJ-XZ

| Track | Format | Key | Title | Artist | Album | Genre | Year | BPM |
|-------|--------|-----|-------|--------|-------|-------|------|-----|
| 1 | MP3 | Am | TITLETEST1 | ARTISTTEST1 | TESTALBUM1 | GENRETEST1 | 2001 | 101 |
| 2 | FLAC | Bm | TITLETEST2 | ARTISTTEST2 | TESTALBUM2 | GENRETEST2 | 2002 | 102 |
| 3 | MP3 | Cm | TITLETEST3 | ARTISTTEST3 | TESTALBUM3 | GENRETEST3 | 2003 | 103 |

---

## Key Documentation

- [Deep Symmetry - PDB Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- [Deep Symmetry - ANLZ Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html)
- [rekordcrate Rust Library](https://holzhaus.github.io/rekordcrate/)

---

We should be able to produce a perfeclty byte-perfect export that corresponds to the reference export using the 3 playlists. Except for keys that are missing in rhythmbox [DO NOT DELETE THIS, IT'S IMPORTANT]
We should export the 3 REKORDBOX{n} playlist, as they correspond to the reference export. If we are off, then we can modify the way to export to get as close to possible to the reference export. At least to debug and get it right initially.

## Future Considerations

When implementing new features, remember:
- The History tables are NOT optional - empty tables = USB not recognized
- The reference History tables work for ANY number of tracks (no need to generate dynamically)
- BPM detection is working (use without --no-bpm flag)
- Key detection is working (use without --no-key flag)

---

## Library Candidates for Future Features

* **`aubio-rs` (aubio bindings)** — tempo tracking + beat detection; can give both **BPM** and **beat timestamps** (useful later for beatgrid). ([docs.rs][1])
* **QM Vamp plugins (C/C++ via Vamp host)** — includes a **beat tracker/tempo estimator**; good fallback if aubio accuracy isn’t enough for your genres. ([GitHub][2])
* **Essentia (C++ / subprocess/FFI)** — strong rhythm tooling, but **AGPL** (often only practical as an optional external analyzer). ([records.sigmm.org][3])

## Key detection (high)

* **`libkeyfinder` (C++ / FFI or `keyfinder-cli`)** — widely used (e.g., Mixxx KeyFinder option), straightforward “one key per track”; **GPLv3+** (license is the main tradeoff). ([GitHub][4])
* **QM Vamp plugins (key estimator)** — alternate key detection path; also gives you a consistent DSP “suite” alongside beat tracking. ([GitHub][2])
* **Essentia `Key` / `KeyExtractor`** — high-quality algorithms, but **AGPL**; again best as an optional external backend. ([essentia.upf.edu][5])

## Waveform (high)

* **Decode pipeline:** **`symphonia`** for pure-Rust demux+decode to PCM (good default for a portable CLI). ([crates.io][6])
* **Amplitude/peaks/RMS:** **`dasp`** (and its `rms`/signal/peak features) for windowing + RMS/peak extraction. ([docs.rs][7])
* **Colored waveforms (band energy):** **`rustfft` + `realfft`** for fast real FFT; compute low/mid/high band energy per window for PWV2/PWV4-style RGB. ([crates.io][8])
* **Turnkey external generator (fallback):** **BBC `audiowaveform`** CLI to generate waveform peak data fast from many codecs; then map/convert into Rekordbox ANLZ payloads. ([GitHub][9])

## Artwork from audio files (medium)

* **`lofty`** — read/write tags across many formats and extract embedded pictures (ID3 APIC, Vorbis/FLAC pictures, MP4, etc.). ([docs.rs][10])
* **`id3`** — if you want a narrow MP3-only path (TBPM/TKEY/APIC), keep it as a lightweight alternative. ([crates.io][11])
* (Typical companion) **`image`** crate for resize + JPEG encode once you extract bytes (no single “DJ artwork” crate; this is the usual building block).

## Beatgrid (low)

* **From aubio:** reuse **`aubio-rs` Tempo** beat timestamps → quantize to grid + write PQTZ. ([docs.rs][12])
* **From QM Vamp plugins:** beat tracker output as an alternative source of beat times. ([GitHub][2])
* **From Essentia:** beat tracking algorithms exist but (again) **AGPL** considerations. ([mtg.github.io][13])

## Other turnkey wins (worth adding in Phase 2)

* **Resampling to a known rate before analysis:** `rubato` (keeps aubio/key detection more stable across sources). ([docs.rs][14])
* **If codec coverage becomes painful:** consider **GStreamer Rust bindings** as an optional “decode backend” for exotic formats. ([gstreamer.freedesktop.org][15])


# NEVER DOUBT REFERENCE EXPORTS. THEY ARE REFERENCE: 100% WORKING. 

[1]: https://docs.rs/aubio-rs?utm_source=chatgpt.com "aubio_rs - Rust"
[2]: https://github.com/c4dm/qm-vamp-plugins?utm_source=chatgpt.com "c4dm/qm-vamp-plugins"
[3]: https://records.sigmm.org/2014/03/20/essentia-an-open-source-library-for-audio-analysis/?utm_source=chatgpt.com "ESSENTIA: an open source library for audio analysis"
[4]: https://github.com/mixxxdj/libkeyfinder?utm_source=chatgpt.com "mixxxdj/libkeyfinder: Musical key detection for digital audio, ..."
[5]: https://essentia.upf.edu/reference/std_KeyExtractor.html?utm_source=chatgpt.com "KeyExtractor — Essentia 2.1-beta6-dev documentation"
[6]: https://crates.io/crates/symphonia?utm_source=chatgpt.com "symphonia - crates.io: Rust Package Registry"
[7]: https://docs.rs/dasp?utm_source=chatgpt.com "dasp - Rust"
[8]: https://crates.io/crates/rustfft?utm_source=chatgpt.com "rustfft - crates.io: Rust Package Registry"
[9]: https://github.com/bbc/audiowaveform?utm_source=chatgpt.com "bbc/audiowaveform: C++ program to generate waveform ..."
[10]: https://docs.rs/lofty?utm_source=chatgpt.com "lofty - Rust"
[11]: https://crates.io/crates/id3?utm_source=chatgpt.com "id3 - crates.io: Rust Package Registry"
[12]: https://docs.rs/aubio-rs/latest/aubio_rs/struct.Tempo.html?utm_source=chatgpt.com "Tempo in aubio_rs - Rust"
[13]: https://mtg.github.io/essentia.js/docs/api/EssentiaExtractor.html?utm_source=chatgpt.com "EssentiaExtractor"
[14]: https://docs.rs/rubato?utm_source=chatgpt.com "rubato - Rust"
[15]: https://gstreamer.freedesktop.org/documentation/rust/git/docs/gstreamer_audio/index.html?utm_source=chatgpt.com "gstreamer_audio - Rust"
- Why did you change the BPM preference to analysis? The preference is ID3, only if absent should we analyse.
- New reference material that can help you : https://github.com/Deep-Symmetry/crate-digger/blob/main/src/main/kaitai/rekordbox_anlz.ksy https://reverseengineering.stackexchange.com/questions/4311/help-reversing-a-edb-database-file-for-pioneers-rekordbox-software https://github.com/Deep-Symmetry/dysentery
- add this library that can read some ANLZ properties to the sources. As we may infer a few things from it https://github.com/dylanljones/pyrekordbox