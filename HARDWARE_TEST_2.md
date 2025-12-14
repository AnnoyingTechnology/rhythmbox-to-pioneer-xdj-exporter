# Hardware Test #2 - After Page Numbering Fix

**Date:** 2025-12-14
**Export Location:** `/tmp/pioneer_test/`
**Test Playlist:** "Shower" (4 tracks)

## Changes Since Test #1

### Bugs Fixed
1. **Page numbering bug** - `current_page` was starting at 0 instead of 1
   - This caused table pointers to point to page 0 (header page)
   - XDJ couldn't find table data, resulted in "Database not found"

2. **Page header indices** - Were hardcoded as 0,1,2,3,4 instead of 1,2,3,4,5
   - Now correctly match the table pointers

### Code Changes
**File:** `src/pdb/writer.rs`

**Change 1 (line 61):**
```rust
// Before:
let mut current_page = 0;

// After:
let mut current_page = 1;  // Page 0 is header, data starts at page 1
```

**Change 2 (lines 256, 336, 417, 629, 704):**
```rust
// Before:
write_page_header(file, 0, TableType::Artists as u32, ...)
write_page_header(file, 1, TableType::Albums as u32, ...)
write_page_header(file, 2, TableType::Tracks as u32, ...)
write_page_header(file, 3, TableType::PlaylistTree as u32, ...)
write_page_header(file, 4, TableType::PlaylistEntries as u32, ...)

// After:
write_page_header(file, 1, TableType::Artists as u32, ...)
write_page_header(file, 2, TableType::Albums as u32, ...)
write_page_header(file, 3, TableType::Tracks as u32, ...)
write_page_header(file, 4, TableType::PlaylistTree as u32, ...)
write_page_header(file, 5, TableType::PlaylistEntries as u32, ...)
```

## Validation Results

### rekordcrate Validation
```
✅ PDB Header: Parsed successfully
✅ Page size: 4096 bytes
✅ Sequence: 1
✅ Tables: 5

✅ Table Pointers:
  - Artists: Page 1
  - Albums: Page 2
  - Tracks: Page 3
  - PlaylistTree: Page 4
  - PlaylistEntries: Page 5

✅ Albums Table: Parsed successfully (4 rows)
❌ Tracks Table: Structure mismatch
```

### Track Table Issue
- rekordcrate expects Track rows to start with `unknown_string1` (DeviceSQLString)
- Our implementation starts with u16 subtype field
- **This may not affect XDJ hardware** - Pioneer's parser might differ from rekordcrate
- Hardware testing required to determine if this is critical

## Files Ready for Testing

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
        ├── ANLZ242ee464.DAT
        ├── ANLZ242ee464.EXT
        ├── ANLZ4f008eaa.DAT
        ├── ANLZ4f008eaa.EXT
        ├── ANLZ7116a0d7.DAT
        ├── ANLZ7116a0d7.EXT
        ├── ANLZ80939d47.DAT
        └── ANLZ80939d47.EXT
```

## Expected Results

### Minimal Success Criteria
- [ ] XDJ recognizes USB stick
- [ ] No "Database not found" error
- [ ] Playlist "Shower" appears in playlist list
- [ ] 4 tracks visible in playlist

### Track Metadata Display
- [ ] Track titles shown
- [ ] Artist names shown
- [ ] Album names shown
- [ ] Duration shown

### Playback
- [ ] Tracks load successfully
- [ ] Audio plays without corruption
- [ ] Track navigation works

### Known Limitations (Expected)
- ⚠️ No BPM displayed (stub analysis)
- ⚠️ No musical key shown
- ⚠️ Basic/empty waveforms
- ⚠️ No beatgrid markers

## Possible Outcomes

### Scenario 1: Full Success ✅
- Database recognized
- Tracks load and play
- Metadata displays correctly
→ **Action:** Proceed to Phase 2 (audio analysis)

### Scenario 2: Database Recognized, Tracks Don't Load ⚠️
- Playlist appears
- But tracks fail to load or show errors
→ **Action:** Fix Track row structure to match rekordcrate expectations

### Scenario 3: "Database not found" again ❌
- XDJ doesn't recognize the database
→ **Action:** Debug deeper - examine table structure byte-by-byte against working PDB

## Debug Information

### Generate Fresh Export
```bash
cargo run --release -- --output /tmp/pioneer_test --playlist "Shower"
```

### Validate with rekordcrate
```bash
cargo run --release -- --output /tmp/pioneer_test --playlist "Shower" --validate
```

### Examine PDB Structure
```bash
# Header (first 128 bytes)
od -A x -t x1z -N 128 /tmp/pioneer_test/PIONEER/rekordbox/export.pdb

# Artists page (page 1, offset 0x1000)
od -A x -t x1z -N 256 -j 4096 /tmp/pioneer_test/PIONEER/rekordbox/export.pdb

# Albums page (page 2, offset 0x2000)
od -A x -t x1z -N 256 -j 8192 /tmp/pioneer_test/PIONEER/rekordbox/export.pdb

# Tracks page (page 3, offset 0x3000)
od -A x -t x1z -N 256 -j 12288 /tmp/pioneer_test/PIONEER/rekordbox/export.pdb
```

## Next Steps After Hardware Test

### If Successful
1. Test with larger playlists (10+ tracks)
2. Test full library export
3. Begin Phase 2 planning (real audio analysis)

### If Track Structure Needs Fixing
1. Examine rekordcrate Track row definition in detail
2. Compare byte-by-byte with real PDB Track rows
3. Implement correct Track row structure
4. Re-test validation and hardware

### If Other Issues Found
1. Document exact error messages from XDJ
2. Compare behavior with known-working PDB from examples/
3. Identify specific fields causing issues
4. Iterate on fixes
