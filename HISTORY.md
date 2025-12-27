# Pioneer Exporter - Development History

This file contains resolved issues and historical debugging context. See CLAUDE.md for current status.

---

## Session 2025-12-27: Entity Overflow and Page Conflict Fix

### Problem
Large exports (100+ artists, 80+ albums) failed with "Page overflow" errors. After adding entity chunking, exports passed validation but were corrupted in Rekordbox.

### Root Cause
Page conflict between `Tracks.empty_candidate` and artist overflow pages:
- Track data ended at page 72
- `next_alloc_page` was 73 after track allocation
- Artist overflow started at page 73
- But `actual_track_empty_candidate` was ALSO 73!

Both track empty_candidate pointer and artist data pointed to the same page.

### Fix Applied (src/pdb/writer.rs)

1. **Calculate track empty_candidate FIRST** before entity overflow allocation
2. **Start entity overflow AFTER empty_candidate**, not at `next_alloc_page`

```rust
// Calculate track empty_candidate FIRST
let actual_track_empty_candidate = if track_chunks.len() > 1 {
    (actual_track_last_page + 1).max(53)
} else {
    51u32
};

// Artist overflow starts AFTER track empty_candidate
let artist_start_page = if track_chunks.len() > 1 {
    actual_track_empty_candidate + 1  // Skip track's empty_candidate
} else {
    next_alloc_page.max(53)
};

// Album overflow starts AFTER artist empty_candidate
let album_start_page = if artist_chunks.len() > 1 {
    actual_artist_empty_candidate + 1
} else {
    next_alloc_page.max(53)
};
```

### Result
- Track data: pages 2, 51, 53-69, empty_candidate=70
- Artist overflow: pages 71-73, empty_candidate=74
- Album data: page 8 (no overflow needed)
- No page conflicts ✅
- 149 tracks, 183 artists validated ✅

---

## Session 2025-12-27: Keys Table and Page Reservation Fix

### Problem 1: Missing Keys
After making Keys header-only, key detection was lost.

### Problem 2: Page Conflicts
Exports with 11-20 tracks corrupted. Track overflow conflicted with reserved pages.

### Root Cause
Page allocation must account for ALL reserved empty_candidate pages:
- Page 50: Keys.empty_candidate
- Page 51: Tracks.empty_candidate (base)
- Page 52: PlaylistEntries.empty_candidate

### Fix Applied (src/pdb/writer.rs)

1. **Restored Keys table** with data page 12 (24 musical keys)
2. **Updated table pointers**:
   - Keys: data=[12], empty=50, last=12
   - Tracks: empty=51
   - PlaylistEntries: empty=52
3. **Track overflow starts at 51, skips 52**:
```rust
let mut next_alloc_page = 51u32;
while track_data_pages.len() < track_chunks.len() {
    track_data_pages.push(next_alloc_page);
    next_alloc_page += 1;
    if next_alloc_page == 52 {  // Skip PlaylistEntries.empty
        next_alloc_page = 53;
    }
}
```

### Result
- Keys table has 24 rows on page 12 ✅
- Track chain: 2 → 51 → 53 → 54 → ... (skips 52) ✅
- 35-track export validated ✅

---

## Session 2025-12-27: Table Pointer Discovery (CRITICAL FIX)

### Problem
All exports with 10+ tracks (requiring overflow pages) were corrupted in Rekordbox 5.

### Root Cause
**Incorrect table pointer values in file header** at offset 0x1c.

Each table has a 16-byte pointer: `[type:u32][empty_candidate:u32][first:u32][last:u32]`

Our TABLE_LAYOUTS had three critical errors:

| Table | Field | WRONG | CORRECT |
|-------|-------|-------|---------|
| Tracks | empty_candidate | 51 | 50 |
| Keys | data_pages | [12] | [] (header-only!) |
| Keys | empty_candidate | 50 | 12 |
| Keys | last_page | 12 | 11 |
| PlaylistEntries | empty_candidate | 52 | 51 |

### Discovery Method
Byte-by-byte comparison of table pointers (0x1c-0x15c) revealed mismatches.

### Fixes Applied (src/pdb/writer.rs)

1. **TABLE_LAYOUTS corrections:**
   ```rust
   // Tracks: empty_candidate 51 → 50
   TableLayout { table: TableType::Tracks, ..., empty_candidate: 50, ... }

   // Keys: header-only (no data page)
   TableLayout { table: TableType::Keys, ..., data_pages: &[], empty_candidate: 12, last_page: 11 }

   // PlaylistEntries: empty_candidate 52 → 51
   TableLayout { table: TableType::PlaylistEntries, ..., empty_candidate: 51, ... }
   ```

2. **Keys moved to empty tables handler** - no longer tries to write data page

3. **Dynamic overflow allocation** - pages 50+ for track overflow, not 53+

4. **Dynamic empty_candidate** - `max(52, last_overflow + 1)`

5. **Dynamic next_unused** - `max(52, empty_candidate + 1)`

### Result
- 3-track: Table pointers now match reference **exactly**
- 10-track: Correct overflow structure (2→50→52)
- 88-track: Correct multi-overflow (2→50→51→52→53)
- All pass validation

---

## Session 2025-12-27: Dynamic Track Paging and Multi-Page Fix

### Problem
After fixing sequence/unk3 formulas, exports with 11+ tracks still failed:
- 11 tracks: page overflow (fixed TRACKS_PER_PAGE=10 was wrong)
- 28+ tracks: page overflow (tracks with long metadata exceeded 11/page)
- Multi-page exports: wrong sequence calculation (per-page instead of cumulative)

### Root Causes

1. **Fixed TRACKS_PER_PAGE=11** didn't account for variable track row sizes
   - Reference exports use ~345 bytes/row with short metadata
   - Our exports with long file paths could use 400+ bytes/row
   - 11 tracks * 400 bytes = 4400 bytes > page capacity (4000)

2. **Sequence calculation was per-page, not cumulative**
   - First page: `base + (rows-1)*5` - correct
   - Subsequent pages: was using `base + (rows-1)*5` per page
   - Should be: `prev_page_seq + rows*5`

### Solution

1. **Dynamic track paging**: Added `estimate_track_row_size()` function to calculate
   approximate row size based on string lengths. Tracks are now packed into pages
   until cumulative size would exceed PAGE_DATA_CAPACITY (4000 bytes).

2. **Cumulative sequence**: Track `cumulative_sequence` across pages. For subsequent
   pages, use `cumulative_sequence + rows*5` instead of base formula.

### Verification

35-track export with 4 pages:
```
Page 2:  9 rows, seq=0x32 (50)   = 10 + (9-1)*5 = 50 ✓
Page 53: 9 rows, seq=0x5f (95)   = 50 + 9*5 = 95 ✓
Page 54: 10 rows, seq=0x91 (145) = 95 + 10*5 = 145 ✓
Page 55: 7 rows, seq=0xb4 (180)  = 145 + 7*5 = 180 ✓
```

---

## Session 2025-12-27: Sequence and unk3 Formula Discovery

### Problem
Exports were corrupted in Rekordbox 5:
- 1 track: CORRUPTED
- 2-9 tracks: WORKS
- 10+ tracks: CORRUPTED

The code was using hardcoded values for sequence and unk3 fields, which only worked
for specific sizes that happened to match the reference exports.

### Analysis

Used new reference exports (`examples/exact-tracks-count/1,2,3,4,5,10,15,20/`) to
discover patterns:

**Sequence (offset 0x10)** - Linear formula per table:
```
sequence = base + (rows - 1) * 5

Table bases:
- Tracks: 10
- Genres: 8
- Artists: 7
- Albums: 9
- Playlists: 6
- PlaylistEntries: 11
- History: 10 (same as Tracks)
```

**unk3 (byte 0x19)** - Cyclic formula:
```
unk3 = (rows % 8) * 0x20

Pattern: 0x20, 0x40, 0x60, 0x80, 0xa0, 0xc0, 0xe0, 0x00, 0x20, ...
```

**History Header (page 39)** - Special values for 1-track:
```
1 track: unk5=0x0001, num_rows_large=0x0000
2+ tracks: unk5=0x1fff, num_rows_large=0x1fff
Always: unk6=0x03ec, unk7=0x0001
```

### Fixes Applied

1. Added `calculate_unk3(rows)` helper function
2. Fixed all entity tables to use sequence and unk3 formulas
3. Fixed History header byte layout (was setting wrong fields)
4. Added patch to fix History data page sequence after copying reference

### Result

- 1-10 tracks: **WORKS** in Rekordbox 5
- 11+ tracks: Still **CORRUPTED** - under investigation

---

## Session 2025-12-25: Large Export Fix

### Root Causes of Large Export Corruption

1. **Pages 41-52 had headers** - We were writing page headers to these reserved pages, but they should be ALL ZEROS
2. **Last track page pointed to wrong place** - Was pointing to static empty_candidate (51), should point to dynamic value (last_page + 1)
3. **next_unused_page was hardcoded** - Was always 53, should be dynamic for large exports

### The Fixes

```rust
// 1. Pages 41-52: Don't write anything - leave as zeros
// (File is zero-filled by set_len(), so just skip writing)

// 2. Last track page next pointer: Point to empty_candidate
let next_page = if is_last_chunk {
    actual_track_last_page + 1  // NOT the static 51
} else {
    next_track_page
};

// 3. Dynamic header values
let actual_track_empty_candidate = actual_track_last_page + 1;
let next_unused_page = actual_track_empty_candidate + 1;
```

---

## Session 2025-12-25: Waveform Investigation

### Expert Analysis Summary

Consulted 4 experts on the waveform display issue. Key insights:

**PWV4 Structure (6 bytes per entry, 1200 entries):**
```
Channel 0: Unknown (affects blue waveform whiteness)
Channel 1: Luminance boost
Channel 2: Inverse intensity for blue
Channel 3: Red component (0-127)
Channel 4: Green component (0-127)
Channel 5: Blue component + height (0-127)
```

### Diagnostic Tests Performed

| # | Test | Result | Conclusion |
|---|------|--------|------------|
| A1 | Reference Injection: Copy reference ANLZ files to our path | NO WAVEFORMS | Problem is AT LEAST in PDB |
| Reverse | Reference PDB + Our ANLZ | NO WAVEFORMS | Problem is ALSO in ANLZ |

### CRITICAL USER TESTS - Breakthrough!

User performed systematic tests on reference-1 export:

| Test | Result | Conclusion |
|------|--------|------------|
| Remove `exportExt.pdb` | ✅ Works | **exportExt.pdb NOT required** |
| Remove `ANLZ0000.EXT` | ❌ Broken | **EXT file is CRITICAL** for waveforms |
| Remove `ANLZ0000.DAT` | ✅ Works | DAT file is secondary/optional |
| Swap DAT with ours | ✅ Works | **Our DAT is valid** |
| Swap EXT with ours | ⚠️ PARTIAL | **Our EXT is partially broken** |

### Root Cause: PWV4 in EXT file

```rust
// OLD CODE - BROKEN:
let color_preview = Vec::new(); // Returns empty, written as all zeros

// NEW CODE - FIXED:
let color_preview = generate_pwv4(&samples, sample_rate); // Actually generates waveform data
```

### PWV4 Format (corrected)

Each 6-byte entry has 3 columns for frequency bands (low/mid/high):
- Bytes 0-1: Low frequency (height 0-31, whiteness 0xF0-0xFF)
- Bytes 2-3: Mid frequency (height, whiteness)
- Bytes 4-5: High frequency (height, whiteness)

---

## Session 2025-12-24: Row Group Fix

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

**Solution:** Copy reference History table pages directly:
- `reference_history_entries.bin` (page 38)
- `reference_history.bin` (page 40)
- `reference_history_playlists.bin` (page 36)

---

## Phase 1 Cleanup (Completed)

Removed hardcoded values that were only needed for byte-perfect reference matching:

1. **ANLZ paths** - Removed `REFERENCE_TRACK_DATA` hardcoding in `organizer.rs`
   - Now uses FNV-1a hash for all tracks

2. **Key IDs** - Now uses detected key from stratum-dsp
   - Dynamically sets key_id from audio analysis

3. **Keys table** - Expanded from 3 to 24 musical keys

---

## All Fixes Applied (chronological order)

1. **History tables** - Use reference page data for HistoryEntries (page 38), History (page 40), and HistoryPlaylists (page 36)
2. **Track row padding** - 0x158 bytes between track rows (reference alignment)
3. **Entity row padding** - Artists: 28 bytes, Albums: 40 bytes per row
4. **Page header flags** - 0x60, 0x00 at bytes 0x19-0x1A for most data pages
5. **Track key_id** - Now dynamically set from stratum-dsp key detection
   - Key ID mapping fixed to match Keys table (chromatic order from A, minor 1-12, major 13-24)
6. **Album artist_id** - Set to 0 (not actual artist ID) to match reference
7. **Empty tables** - Labels and Artwork are header-only (no data pages)
8. **File header** - `next_unused_page=53`, `sequence=31` to match reference
9. **Keys table** - Expanded to all 24 musical keys (was 3)
10. **Row group structure fix (MAJOR)** - Fixed multi-row-group handling for large exports
11. **Large export fix** - Dynamic page allocation for 41-52 reserved range
12. **PWV4 generation** - Fixed waveform preview generation (was returning empty Vec)

---

## Completed Phases

### Phase 2 - Audio Analysis (Complete)
- [x] BPM detection with range normalization
- [x] Key detection with correct Rekordbox ID mapping
- [x] Parallel track analysis (31 threads, ~5 tracks/sec)
- [x] Smart/automatic playlist support (genre, duration, artist filters)
- [x] Metadata caching (FLAC only, MP3 TODO)
- [x] Key ID fix (chromatic order from A: minor 1-12, major 13-24)
- [x] Filename sanitization for FAT32 (quotes, colons, etc. → underscore)

### Phase 2.1 - Ratings & Sanitization (Complete)
- [x] Rhythmbox track rating (stars) to PDB rating
  - Reads rating from ID3 POPM (Popularimeter) frames
  - Converts ID3 rating (0-255) to stars (1-5)
  - Falls back to Rhythmbox XML rating if POPM not present
- [x] FAT32 filename sanitization (illegal chars, truncation to 250 chars)
- [x] FAT32 path component sanitization (truncation to 200 chars)
- [x] `--max-parallel` CLI option to limit concurrent analyses

### Phase 3 - Waveforms (Mostly Complete)
- [x] PWAV waveform preview (400 bytes, monochrome) - .DAT file
- [x] PWV2 tiny preview (100 bytes) - .DAT file
- [x] PWV3 waveform detail (150 entries/sec, monochrome) - .EXT file
- [x] PWV5 color waveform detail (150 entries/sec, 2 bytes/entry) - .EXT file
- [x] PWV4 color preview - Fixed (was returning empty Vec)
- [x] StubAnalyzer fix - now generates actual waveforms instead of empty stubs
- [x] Whiteness/height encoding fix - PWAV uses whiteness=5, PWV3 uses whiteness=7

---

## Waveform Encoding Reference

- PWAV: height (5 low bits) | whiteness (3 high bits) - whiteness=5 like reference
- PWV2: height (4 bits) - simple peak amplitude
- PWV3: height (5 low bits) | whiteness (3 high bits) - whiteness=7 like reference
- PWV5: RGB (3 bits each) | height (5 bits)
- PWV4: 1200 entries × 6 bytes (3 frequency bands)

Implementation:
- Uses symphonia to decode audio to mono samples
- Calculates RMS and peak per time window
- Height from peak amplitude (0-31 range for 5-bit fields)
