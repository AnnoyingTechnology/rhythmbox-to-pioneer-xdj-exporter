# PDB Exporter Review - Systematic Analysis

## Overview

This document contains findings from a systematic review of the Pioneer PDB exporter, comparing our generated exports against reference Rekordbox exports of varying sizes (1, 3, 20, 84 tracks).

---

## Key Findings Summary

### CRITICAL ISSUES

1. **Keys Table Over-Population** (HIGH PRIORITY)
   - We write all 24 musical keys to the Keys table
   - Reference exports only include keys that are actually referenced by tracks
   - Impact: Page 12 has 140+ byte differences vs reference
   - Fix needed: Only write keys that are used by exported tracks

2. **Row Count Field Interpretation** (MEDIUM PRIORITY)
   - Bytes 0x18-0x1A in page header are packed non-byte-aligned:
     - 13 bits: `num_row_offsets` (total slots ever allocated)
     - 11 bits: `num_rows` (current valid rows)
   - Our code treats them as separate byte/u16 fields
   - Impact: Works for small exports but may cause issues at scale

### MODERATE ISSUES

3. **Page Header `unknown1` Field** (Byte 0x10)
   - Values vary significantly between our exports and reference
   - Examples:
     - PlaylistTree: 0x08 (ours) vs 0x06 (ref)
     - PlaylistEntries: 0x1d (ours) vs 0x0c (ref)
     - History header: 0x12 (ours) vs 0x0b (ref)
   - Hypothesis: May be a running counter or checksum
   - Impact: Unknown, but non-matching reference

4. **Page Header `unknown3` Field** (Byte 0x19)
   - We use 0x60 for multi-content pages, 0x20 for single
   - Reference uses 0x20 in more cases
   - Examples:
     - PlaylistTree 1-playlist: ours=0x60, ref=0x20
     - PlaylistEntries 1-entry: ours=0xc0, ref=0x20
   - Impact: Unknown, but non-matching reference

### MINOR ISSUES

5. **Dynamic Content Differences** (EXPECTED)
   - Date strings (date_added, analyze_date): Current date vs export date
   - ANLZ file paths: Different FNV-1a hashes based on source paths
   - Track metadata: tempo, duration, key_id may vary
   - Impact: None - these are content-dependent

6. **Sequence Number Calculation**
   - Our calculation: `14 + (total_entities * 3)`
   - Reference values vary: 14 (1-track), 31 (3-track), 108 (20-track), 520 (84-track)
   - Our 3-track: 59 vs reference: 31
   - Impact: Unknown, but likely cosmetic

---

## Reference Export Analysis

### File Structure Patterns

| Export | Pages | Tracks | next_unused | sequence | Track empty_cand |
|--------|-------|--------|-------------|----------|------------------|
| ref-1  | 41    | 1      | 53          | 14       | 51               |
| ref-3  | 41    | 3      | 53          | 31       | 51               |
| ref-20 | 51    | 20     | 53          | 108      | 52               |
| ref-84 | 64    | 84     | 65          | 520      | 64               |

### Key Observations

1. **Pages 41-52 are reserved** for empty_candidate pointers
   - Never contain actual page headers in small exports
   - File size stays at 41 pages until more space needed
   - Large exports skip this range for data pages

2. **Dynamic Table Pointers**: For large exports:
   - Tracks table gets new empty_candidate (51 → 52 → 64)
   - Other tables can also grow (Labels, Artwork in ref-84)
   - next_unused_page increases beyond 53

3. **num_rows_large = 8191 (0x1fff)**: Special marker
   - Appears in Track pages of ref-84
   - Indicates extended row counting mode
   - Pages 2, 58, 59, 60, 61 all have this

4. **History Table Anomalies**:
   - Present flags don't match num_rows_small
   - Has deleted/hidden rows
   - Static reference data works (copied from real export)

---

## Row Group Footer Analysis

### Observed Patterns

For most tables:
- Full groups (16 rows): `flags=0xffff`, `unknown=0x0000`
- Partial groups: `flags=(1<<N)-1`, `unknown=1<<(N-1)`

For special tables (Colors, Columns, HistoryPlaylists, HistoryEntries):
- Different `unknown` patterns that don't follow the formula
- These use static reference data, so pattern is preserved

### Our Implementation

```rust
fn row_group_unknown_high_bit(flags: u16) -> u16 {
    if flags == 0 || flags == 0xffff {
        0
    } else {
        let leading = flags.leading_zeros() as u16;
        let idx = 15u16.saturating_sub(leading);
        1u16 << idx
    }
}
```

This matches the expected pattern for Tracks, Artists, Albums, Genres.

---

## Track Row Layout

### Reference Row Offsets

| Export | Row 0 | Row 1 | Row 2 | Spacing |
|--------|-------|-------|-------|---------|
| ref-1  | 0x000 | -     | -     | 332 total |
| ref-3  | 0x000 | 0x158 | 0x2B4 | 344 bytes |
| ref-20 | 0x000 | 0x160 | 0x2C0 | 352 bytes |

### Our Implementation

- Single track: Pad to 332 bytes
- Multiple tracks: Pad to 344 bytes between rows
- Uses reference offsets [0x000, 0x158, 0x2B4, 0x410] for first 4 rows

### Potential Issues

- Row spacing varies by export (344 vs 352)
- May need dynamic calculation based on actual row size

---

## Recommendations

### Immediate Fixes

1. **Keys Table**: Only write keys used by exported tracks
   - Collect unique key_ids from track metadata
   - Write only those keys to Keys table
   - Update key_id references in tracks

2. **Verify Row Count Fields**: Review packed bit interpretation
   - Bytes 0x18-0x1A may need proper 13+11 bit packing
   - Test with larger exports (>255 rows per page)

### Future Improvements

3. **Page Header Unknown Fields**:
   - Reverse-engineer `unknown1` pattern
   - May be based on content hash or row count

4. **Sequence Number**:
   - Study larger dataset of reference exports
   - Determine actual calculation formula

5. **Dynamic Row Spacing**:
   - Calculate based on actual string lengths
   - Don't rely on fixed padding values

---

## Validation Checklist

For any export to be valid:

- [ ] File size is page-aligned (multiple of 4096)
- [ ] All table chains are connected (first → last via next_page)
- [ ] Last page of each chain points to empty_candidate
- [ ] Pages 41-52 are zeros (or don't exist) for small exports
- [ ] num_rows_small matches row group present bits
- [ ] Row offsets point within used_size bounds
- [ ] free_size + used_size ≈ available heap space
- [ ] All referenced IDs exist in their tables

---

## Test Commands

```bash
# Single track (matches reference-1)
cargo run --release -- --output /tmp/test-1 --playlist "REKORDBOX4" --no-bpm --no-key

# Three tracks (matches reference)
cargo run --release -- --output /tmp/test-3 --playlist "REKORDBOX1" --playlist "REKORDBOX2" --playlist "REKORDBOX3" --no-bpm --no-key

# Large export
cargo run --release -- --output /tmp/test-large --playlist "XDJ: Funk" --no-bpm --no-key

# Validate and compare
python3 tools/pdb_validator.py /tmp/test-1/PIONEER/rekordbox/export.pdb examples/reference-1/PIONEER/rekordbox/export.pdb

# Detailed diff
python3 tools/pdb_diff_detail.py /tmp/test-1/PIONEER/rekordbox/export.pdb examples/reference-1/PIONEER/rekordbox/export.pdb
```

---

## Appendix: Page Header Structure

```
Offset  Size  Name            Description
0x00    4     padding         Always 0
0x04    4     page_index      Index within table chain
0x08    4     table_type      Table ID (0=Tracks, 1=Genres, ...)
0x0C    4     next_page       Next page in chain, or empty_candidate
0x10    4     unknown1        Varies by content (counter? checksum?)
0x14    4     unknown2        Usually 0
0x18    1     num_rows_small  Row count (low byte of packed value)
0x19    1     unknown3        Varies (0x20, 0x60, 0xc0, ...)
0x1A    1     unknown4        Usually 0x00 or 0x01
0x1B    1     page_flags      0x24=data, 0x34=extended, 0x64=header
0x1C    2     free_size       Unused bytes in heap
0x1E    2     used_size       Used bytes in heap
0x20    2     unknown5        Usually 0x0001
0x22    2     num_rows_large  High bits or special value (0x1fff)
0x24    2     unknown6        Usually 0x0000
0x26    2     unknown7        Usually 0x0000 (0x0001 for History)
```

---

## Conclusions

### What's Working Correctly

1. **Core Database Structure**: File header, table pointers, page chains all validate correctly
2. **Row Group Footers**: Our formula for `unknown` field matches expected pattern
3. **Table Linking**: All table chains (first → last → empty_candidate) are intact
4. **Track Data Pages**: Tracks write correctly with proper row offsets
5. **Entity Tables**: Artists, Albums, Genres all structure correctly
6. **Dynamic Page Allocation**: Large exports correctly skip pages 41-52

### Root Cause of Current Issues

The current "single-track corruption" issue in CLAUDE.md is likely NOT a structural problem. Based on this analysis:

1. **Our exports pass structural validation** with 0 errors
2. **Differences from reference are content-based**, not structural:
   - Keys table (all 24 vs only used keys)
   - Date strings (current date vs export date)
   - ANLZ paths (different hashes)
   - Page header unknown fields (cosmetic)

3. **Reference exports have their own anomalies**:
   - reference-84 has 46 warnings (deleted rows, mismatched flags)
   - History tables have massive hidden content
   - These are artifacts of real Rekordbox usage over time

### Hypothesis for Single-Track Issue

If single-track exports are corrupted but 3+ track exports work, the issue may be:

1. **Track Row Size**: Single track uses 332-byte padding vs 344 for multi-track
2. **Row Group Footer**: Single row has different `unknown` value pattern
3. **Some field value**: A specific field value that's only present/different in single-track case

### Recommended Next Steps

1. ~~**Binary diff single-track export**~~ DONE - Only 16 bytes differ, all content-based
2. **Test on actual hardware** - Use hybrid tests below to isolate issue
3. ~~**Create hybrid PDB**~~ DONE - Multiple hybrids created
4. **Keys table fix**: Only include used keys (LIKELY CAUSE - see below)

---

## Deep Analysis Results (Updated)

### Page Header Comparison: 1-Track

Our export vs reference-1:
- **Track page 2 header**: IDENTICAL (40 bytes match perfectly)
- **Row group footer**: IDENTICAL (offset=0, flags=0x0001, unknown=0x0001)
- **used_size**: Both 332 bytes
- **Structure**: Perfect match

### Content-Only Differences (Track Row)

| Offset | Field | Ours | Reference | Reason |
|--------|-------|------|-----------|--------|
| 0x0038 | file_size | 30491 | 34857 | Different audio file |
| 0x003c | u2 | 21 | 44 | Unknown field |
| 0x0048 | key_id | 0 | 1 | --no-key vs F#m |
| 0x0058 | bitrate | 320 | 192 | MP3 default |
| 0x00c8 | date | 26 | 24 | Current vs export date |
| 0x00df | ANLZ path | P9CC/... | P05C/... | Different hash |

### Potential Corruption Cause: Keys Table

**Critical Finding**: Our Keys page has 24 rows, reference has 1.

| Metric | Ours | Reference |
|--------|------|-----------|
| num_rows_small | 24 | 1 |
| num_rows_large | 23 | 0 |
| used_size | 288 | 12 |

If Rekordbox validates that all Keys table entries are referenced by tracks,
having 23 unreferenced keys could trigger a corruption flag.

### Hybrid Test Files Created

Located in `/tmp/hybrid-tests/`:

1. **ours-with-ref-keys.pdb** - Our export + reference Keys pages
   - Tests if Keys table is the issue

2. **ref-with-our-keys.pdb** - Reference + our Keys pages
   - Tests if our Keys table corrupts a working export

3. **ours-with-ref-tracks.pdb** - Our export + reference Track page
   - Tests if track data content matters

4. **ref-with-our-tracks.pdb** - Reference + our Track page
   - Tests if our track data content breaks things

### Test Procedure

1. Copy each hybrid PDB to USB as `PIONEER/rekordbox/export.pdb`
2. Test in Rekordbox 5 - which ones are corrupted?
3. Report back which hybrids work/fail

If **ours-with-ref-keys.pdb** works but original doesn't:
→ Keys table is the issue. Fix: Only write used keys.

If **ref-with-our-tracks.pdb** fails but reference works:
→ Track content is the issue. Investigate u2 field or key_id=0.

---

## Additional Deep Investigation Findings

### Finding 1: u2 Field is NOT track_id + 20

Analysis across all references shows u2 values are:
- reference-1 (1 track): u2=44 (track_id=1, diff=43)
- reference (3 tracks): u2=21,22,23 (track_id=1,2,3, diff=20 consistently)
- reference-20: Varies wildly (35, 32, 29, 39, 36...) with no pattern
- reference-84: Completely inconsistent (54, 112, 66, 111, 93...)

**Conclusion**: u2 is likely an internal Rekordbox ID, not export-generated. Our formula `track_id + 20` works by coincidence for some exports. This is likely NOT causing corruption.

### Finding 2: unknown3 (byte 0x19) Mismatch

Our single-track export has wrong unknown3 values:
- Page 16 (PlaylistTree): ours=0x60, should be 0x20
- Page 18 (PlaylistEntries): ours=0xc0, should be 0x20

Pattern observed in references:
- 1 row: unknown3=0x20
- 3+ rows: unknown3=0x60
- 6+ rows: unknown3=0xc0

**Created fix**: `/tmp/hybrid-tests/ours-fixed-unknown3.pdb`

### Finding 3: Single-Row Padding Differs

For some tables, single-row pages have different row sizes than multi-row:

| Table | Single-Row Size | Multi-Row Size | Difference |
|-------|-----------------|----------------|------------|
| Albums | 44 bytes | 40 bytes | +4 |
| Tracks | 332 bytes | 344 bytes | -12 |

Our code uses fixed sizes regardless of row count. This may cause issues.

### Finding 4: Large Export Scaling Failure

**258-track export fails with page overflow:**
```
Error: Page overflow: heap 6236 rows 209 exceed page capacity
```

**Root Cause**: Artists table has 209 rows but single page can only hold ~118.

**Maximum entities per single page:**
| Table | Max Rows | Reason |
|-------|----------|--------|
| Artists | ~118 | 30 bytes avg/row |
| Albums | ~78 | 45 bytes avg/row |
| Genres | ~177 | 20 bytes avg/row |
| Keys | ~295 | 12 bytes avg/row |

**Impact**: Any export with 100+ unique artists will fail.

**Required Fix**: Implement multi-page support for Artists, Albums, and Genres tables (like Tracks already has).

---

## Summary of Issues by Priority

### P0 - Blocking (Prevents Export)
1. **Multi-page support for entity tables** - Exports with many artists/albums fail

### P1 - Likely Causing Corruption
2. **unknown3 field miscalculation** - Single-row pages have wrong value
3. **Single-row padding differences** - Albums single-row should be 44, not 40

### P2 - Non-Matching but May Work
4. **Keys table has all 24 keys** - Reference only has used keys
5. **unknown1 field differences** - Values don't match reference

### P3 - Cosmetic/Unknown Impact
6. **u2 field values** - Using track_id + 20 instead of unknown source
7. **Sequence number calculation** - Different from reference

---

## Hybrid Test Files Summary

All located in `/tmp/hybrid-tests/`:

| File | Description | Tests | Result |
|------|-------------|-------|--------|
| ours-with-ref-keys.pdb | Our export + reference Keys pages | Keys table issue | **CORRUPTED** |
| ref-with-our-keys.pdb | Reference + our Keys pages | If 24 keys break it | **WORKS** |
| ours-with-ref-tracks.pdb | Our export + reference Track page | Track content | **CORRUPTED** |
| ref-with-our-tracks.pdb | Reference + our Track page | Our track content | **WORKS** |
| ours-with-ref-playlists.pdb | Our export + reference Playlist pages | Playlist structure | **CORRUPTED** |
| ours-fixed-unknown3.pdb | Our export with fixed unknown3/unknown1 | unknown3 hypothesis | **CORRUPTED** |

---

## CRITICAL FINDING (2025-12-26 Hardware Test)

### Test Results Summary

**WORKS (2 files):**
- ref-with-our-keys.pdb (Reference base + our Keys)
- ref-with-our-tracks.pdb (Reference base + our Track data)

**CORRUPTED (4 files):**
- ours-with-ref-keys.pdb (Our base + reference Keys)
- ours-with-ref-tracks.pdb (Our base + reference Track)
- ours-with-ref-playlists.pdb (Our base + reference Playlists)
- ours-fixed-unknown3.pdb (Our base with patched values)

### Key Insight

**Pattern: Reference as BASE → WORKS. Our export as BASE → CORRUPTED.**

This definitively proves:
1. ✅ Our Keys table (24 keys) is NOT the problem
2. ✅ Our Track row content is NOT the problem
3. ✅ Our Playlist pages are NOT the problem
4. ✅ The unknown3 field is NOT the problem

**The corruption is in our BASE STRUCTURE** - something in pages we haven't swapped:
- Page 0 (file header)
- Page 1 (Tracks header page)
- Pages 3-4 (Genres)
- Pages 5-6 (Artists)
- Pages 7-8 (Albums)
- Pages 9-10 (Labels)
- Pages 13-14 (Colors)
- Other structural pages

### Next Investigation Steps

1. Compare Page 0 (file header) byte-by-byte
2. Compare all header pages (odd-numbered: 1, 3, 5, 7, 9, 11, 13...)
3. Compare entity data pages (Genres, Artists, Albums)
4. Create more hybrids swapping these pages to isolate the exact cause

---

## Session 2025-12-27: Formula Discovery

### Breakthrough: Sequence and unk3 Formulas

Analyzed reference exports with exact track counts (1,2,3,4,5,10,15,20) and discovered:

**Sequence Formula**: `base + (rows - 1) * 5`
- Tracks base: 10
- Genres base: 8
- Artists base: 7
- Albums base: 9
- Playlists base: 6
- PlaylistEntries base: 11

**unk3 Formula**: `(rows % 8) * 0x20`
- Creates cyclic pattern: 0x20, 0x40, 0x60, 0x80, 0xa0, 0xc0, 0xe0, 0x00, repeat

**History Header Special Values**:
- 1 track: unk5=0x0001, num_rows_large=0x0000
- 2+ tracks: unk5=0x1fff, num_rows_large=0x1fff
- Always: unk6=0x03ec, unk7=0x0001

### Current Status

After implementing formulas:
- **1-10 tracks**: WORKS in Rekordbox 5
- **11+ tracks**: CORRUPTED - investigating

The 11-track boundary suggests something changes when:
- Track page gets more than 10 rows (row group behavior?)
- Some table exceeds a threshold
- Multi-page handling triggers

---

*Generated: 2025-12-26*
*Updated: 2025-12-27 with formula discovery*
*Analysis by: Claude Opus 4.5*
