# Session Notes - 2025-12-17 (continued)

## CRITICAL FINDING
**The issue is in the PDB file, NOT the ANLZ files.**
- Reference export works on XDJ even with USBANLZ folder deleted
- Our export fails on XDJ even with USBANLZ folder deleted
- rekordcrate parses both PDBs successfully
- XDJ firmware checks something that rekordcrate doesn't validate

## Latest Changes
- **Columns table bug found**: `write_columns_table()` was writing correctly, but the row index format at the end of page differed from reference
- **Temporary fix**: Changed Columns table to write as empty stub (like Labels/Keys/Colors) to avoid rekordcrate parse errors
- **rekordcrate validates successfully** with empty Columns table
- **Ready for hardware testing** at `/tmp/pioneer_test/`

## Key Findings from Byte Comparison

### Table Pointer Structure
Our export writes tables sequentially with empty_candidate pages immediately after data. Reference has empty_candidate pages grouped at end of file. Both approaches should work.

### Page 50 (Columns Data) Issue
- Our page had correct data (27 rows) written by `write_columns_table()`
- Row index format at end of page differed from reference:
  - Reference: ends with `ff ff ff ff` sentinel
  - Ours: ends with `ff 07 00 00`
- rekordcrate failed to parse our row index format, reading past file end
- **Fix**: Write empty Columns table for now; TODO fix row index format later

### Row Count Comparison (ref vs ours)
- Tracks: 10 vs 10 ✓
- Artists: 9 vs 9 ✓
- Albums: 7 vs 8 (minor diff)
- Genres: 9 vs 9 ✓
- PlaylistTree: 2 vs 3 (minor diff)
- PlaylistEntries: 13 vs 13 ✓
- **Colors: 8 vs 0** - Reference has preset colors
- **Keys: 7 vs 0** - Reference has detected keys
- **Labels: 1 vs 0** - Reference has record label
- **Columns: 27 vs 0** - Now empty (was 27, but broken format)

## Previous Status
- **Hardware test still failing**: XDJ shows empty USB browser (no error message)
- **Rhythmbox playlists aligned**: REKORDBOX1 and REKORDBOX2 now match reference export exactly
- **Ready for byte-level comparison**: Same tracks in same order enables direct binary diff

## Rhythmbox Playlist Configuration (matches reference)

**REKORDBOX1** (10 tracks, in order):
1. You're the Kind of Girl I Like - Kwick
2. What's Luv? - Fat Joe feat. Ashanti
3. Turn da Music Up (Milano radio edit) - 2 Brothers on the 4th Floor
4. Omen III (single edit) - Magic Affair
5. I'm Not Alone (deadmau5 remix) - Calvin Harris
6. Déjà Vu - Eminem
7. Love Will Save The Day (The Underground Mix) - Whitney Houston
8. What Is Love (7'' Mix) - Haddaway
9. I Am the Black God of the Sun - Rotary Connection
10. Playgirl (Felix da Housecat Glitz Clubhead mix) - Ladytron

**REKORDBOX2** (3 tracks, in order):
1. Turn da Music Up (Milano radio edit) - 2 Brothers on the 4th Floor
2. Omen III (single edit) - Magic Affair
3. Love Will Save The Day (The Underground Mix) - Whitney Houston

## Tools Created
- `examples/list_tracks.rs` - Lists tracks from a PDB using rekordcrate
- `examples/list_playlists.rs` - Lists playlist tree and entries from a PDB

## Next Steps
1. Rebuild export with aligned playlists
2. Byte-by-byte comparison of our export vs reference (same tracks now)
3. Identify remaining differences causing hardware rejection

---

# Session Notes - 2025-12-17 (earlier)

## What Happened
- **Empty candidate pages now properly zeroed**: Reference exports have completely zeroed empty candidate pages (all 4096 bytes = 0x00). Fixed `write_empty_candidate_page()` function.
- **Track row field fixes**:
  - `index_shift`: Now increments by 0x20 per row (was all zeros)
  - `u3`: Fixed to 0xe5b6 (was 0xe556)
  - `u4`: Fixed to 0x6a76 (was 0x6a2e)
- **Page chain structure verified**: Each table has header → data → empty_candidate chain.
- **rekordcrate validation passes**: PDB header, album pages, track pages all parse correctly.

## Result After Fixes
- rekordcrate parses the generated PDB successfully
- Hardware still shows empty USB browser
- Confirmed issue is in PDB (not ANLZ) by testing with reference USBANLZ

## Files Modified This Session
- `src/pdb/writer.rs`:
  - Fixed `write_empty_candidate_page()` to write all zeros
  - Fixed `index_shift` in track rows to increment by 0x20
  - Fixed `u3` constant from 0xe556 to 0xe5b6
  - Fixed `u4` constant from 0x6a2e to 0x6a76

## Test Export Location
`/tmp/pioneer_test/` - ready to copy to USB for XDJ testing

# Session Notes - 2025-12-16

## What Happened
- Pulled the new valid Rekordbox export (`examples/PIONEER/rekordbox/export.pdb`, playlists REKORDBOX1/2) and inspected page headers/rows.
- Updated PDB header pages to use reference defaults (0x1fff sentinels, unknown6=0x03ec, track header unknown1=0x3e/unknown7=1; others unknown1=1, history unknown1=0x12).
- Set data-page defaults closer to the example: track/artist/album/playlist/columns unknown1/3/4/5 values, header metadata (next_unused_page=final page, unknown=5, sequence=0x44).
- Track rows now mirror reference constants: bitmask=0x0700, u2=track_id+6, u3=0xe556, u4=0x6a2e, file-type-aware bitrate, genre IDs populated; album/artist index_shift now match reference (0x00c0/0x0100).
- Columns table expanded to the 27 UTF-16 annotated entries from the valid export; genres table is now written from track metadata.
- Row offsets and row-group layout fixed (offsets relative to heap, flags + unknown16 per group). Latest export `/tmp/pioneer_test` (REKORDBOX1/2 only) passes rekordcrate (2 track pages: header + 10 rows).

## Current Result
- rekordcrate parses the generated PDB.
- Hardware still shows an empty USB browser (no error message) with the latest export (`/tmp/pioneer_test`).

## Next Focus
- Add proper empty-candidate pages and match next_page chains to the valid export.
- Implement multi-page support for large tables (tracks/playlists/columns) and tighten data-page unknown fields/free-space values to match the example.
- Cross-check row offsets and page headers against `examples/PIONEER/rekordbox/export.pdb` before the next hardware test.

# Session Notes - 2025-12-14

## What Happened Today

### Hardware Test #1 Result
- **Error:** "Database not found" on XDJ-XZ
- **Root Cause:** Page numbering bug in PDB writer

### Bugs Found and Fixed

#### Bug #1: Page Numbering Started at 0
**Location:** `src/pdb/writer.rs` line 60

**Problem:**
```rust
let mut current_page = 0;  // WRONG - page 0 is the header!
```

This caused:
- `artists_page = 0` → pointed to header page
- `albums_page = 1`
- `tracks_page = 2`
- etc.

**Fix:**
```rust
let mut current_page = 1;  // Page 0 is header, data starts at page 1
```

Now:
- `artists_page = 1` ✅
- `albums_page = 2` ✅
- `tracks_page = 3` ✅
- etc.

#### Bug #2: Page Headers Had Wrong Indices
**Location:** `src/pdb/writer.rs` lines 256, 336, 417, 629, 704

**Problem:** Hardcoded page indices didn't match table pointers

**Fix:** Updated all `write_page_header()` calls:
```rust
// Artists table
write_page_header(file, 1, TableType::Artists as u32, ...)  // was 0

// Albums table
write_page_header(file, 2, TableType::Albums as u32, ...)   // was 1

// Tracks table
write_page_header(file, 3, TableType::Tracks as u32, ...)   // was 2

// PlaylistTree table
write_page_header(file, 4, TableType::PlaylistTree as u32, ...)  // was 3

// PlaylistEntries table
write_page_header(file, 5, TableType::PlaylistEntries as u32, ...)  // was 4
```

### Current Status

#### What Works ✅
- PDB file header parses successfully
- Table pointers correctly reference pages 1-5
- Albums table validates with rekordcrate (4 rows)
- Export creates complete directory structure
- ANLZ stub files generated
- Audio files copied to USB

#### What's Broken ❌
- Track table structure doesn't match rekordcrate expectations
  - rekordcrate expects Track rows to start with DeviceSQLString `unknown_string1`
  - Our implementation starts with u16 subtype field
  - **May not affect XDJ hardware** (different parser than rekordcrate)

### Files Ready for Testing
```
/tmp/pioneer_test/
├── Music/
│   ├── 01 So Different.mp3
│   ├── 10 Only Child - Addicted.mp3
│   ├── 5-09 Ay No Corrida.mp3
│   └── Harlem.mp3
└── PIONEER/
    ├── rekordbox/export.pdb (24 KB)
    └── USBANLZ/
        ├── ANLZ242ee464.DAT/EXT (4 pairs)
        └── ...
```

## Next Steps

### Immediate: Hardware Test #2
Copy `/tmp/pioneer_test/` to USB and test on XDJ-XZ

**Expected outcomes:**
1. **Best case:** Database loads, tracks play → Proceed to Phase 2
2. **Likely:** Database loads, but tracks don't load → Fix Track row structure
3. **Worst case:** "Database not found" again → Deeper debugging needed

### If Track Structure Needs Fixing

The rekordcrate error shows:
```
Error parsing field 'unknown_string1' in Track at offset 0x3028
- Tried ShortASCII: header & 0b1 == 1 failed
- Tried Long formats: no valid flags found
```

This means rekordcrate expects the first field in Track row to be a DeviceSQLString, but we're writing a u16 subtype.

**To fix:**
1. Read rekordcrate source for Track row structure
2. Examine real PDB Track rows byte-by-byte
3. Implement correct Track row layout
4. Re-validate and re-test

### If Hardware Test Succeeds

1. Test with larger playlists (10+ tracks)
2. Test full library export (all 9298 tracks)
3. Plan Phase 2: Real audio analysis
   - BPM detection
   - Musical key detection
   - Waveform generation
   - Beatgrid creation

## Key Learnings

1. **Always check page numbering in multi-page formats**
   - Page 0 is often header/metadata
   - Data pages typically start at page 1

2. **Consistency is critical**
   - Table pointers must match actual page indices
   - Page headers must have correct page_index field
   - One wrong offset breaks the entire chain

3. **Hardware testing is essential**
   - rekordcrate validation != XDJ compatibility
   - The hardware may be more forgiving than the library
   - Can't know for sure without testing on actual device

## Files Modified

1. `src/pdb/writer.rs` - Page numbering fixes (6 lines changed)
2. `CLAUDE.md` - Updated status and documented bug fixes
3. `README.md` - Updated status section
4. `HARDWARE_TEST_2.md` - Created new test documentation
5. `SESSION_NOTES.md` - This file

## Build Commands

```bash
# Build
cargo build --release

# Generate test export
cargo run --release -- --output /tmp/pioneer_test --playlist "Shower"

# Validate with rekordcrate
cargo run --release -- --output /tmp/pioneer_test --playlist "Shower" --validate
```

## Validation Output

```
✅ PDB Header: OK
✅ Table 0: Artists (Page 1)
✅ Table 1: Albums (Page 2) - 4 rows parsed
✅ Table 2: Tracks (Page 3)
✅ Table 3: PlaylistTree (Page 4)
✅ Table 4: PlaylistEntries (Page 5)

❌ Albums: Parsed successfully
❌ Tracks: Structure mismatch at offset 0x3028
```

## Summary

We found and fixed the critical bug causing "Database not found" on the XDJ. The page numbering was off by 1, causing all table pointers to reference the wrong pages. The fix was simple but had a major impact.

The export is now ready for hardware testing again. There's still a Track row structure issue that shows up in rekordcrate validation, but it may not affect the XDJ hardware. Hardware testing will determine if we need to fix the Track structure or if we can proceed to Phase 2.

## Update 2025-12-15

- Implemented Rekordbox-style 20-table layout with two pages per table (header + data), page flags/unknown fields aligned to the reference, columns table added (UTF-16 annotated names), and empty_candidate pointers set to data pages.
- Current PDB validates with rekordcrate, but XDJ reports **"library is corrupted"** and shows an empty browser.
- Remaining suspected mismatches vs reference: large empty_candidate values and pagination span from the 10k-track example, plus header-page fields possibly still differing in ways the hardware cares about.
- Latest export for hardware testing: `/tmp/pioneer_test`.
