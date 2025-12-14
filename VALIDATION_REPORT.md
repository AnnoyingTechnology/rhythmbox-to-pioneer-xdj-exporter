# Pioneer Exporter - Hardware Validation Checklist

**Export Date:** 2025-12-14
**Test Export:** `/tmp/pioneer_test` (4 tracks, 1 playlist "Shower")

## Pre-Hardware Test Checklist

### ✅ Files Created
- [x] PDB file at `PIONEER/rekordbox/export.pdb` (24 KB)
- [x] 8 ANLZ files in `PIONEER/USBANLZ/` (4 tracks × 2 files)
- [x] 4 audio files in `Music/` directory (48.8 MB total)

### ✅ Directory Structure
```
/tmp/pioneer_test/
├── Music/
│   ├── 01 So Different.mp3
│   ├── 5-09 Ay No Corrida.mp3  
│   ├── 10 Only Child - Addicted.mp3
│   └── Harlem.mp3
└── PIONEER/
    ├── rekordbox/export.pdb
    └── USBANLZ/
        ├── ANLZ*.DAT (4 files)
        └── ANLZ*.EXT (4 files)
```

### ✅ File Validation
- [x] ANLZ files have valid PMAI headers
- [x] PDB has correct 4096-byte page structure
- [x] Audio files copied successfully
- [x] File paths in PDB match actual locations

## Hardware Test Procedure

### 1. USB Preparation
1. Copy `/tmp/pioneer_test/` contents to USB stick root
2. Safely eject USB stick
3. Insert into XDJ-XZ USB port

### 2. Hardware Tests

#### Test 1: USB Recognition
- [ ] XDJ-XZ recognizes USB stick
- [ ] No error messages displayed
- [ ] Library browser accessible

#### Test 2: Playlist Display
- [ ] "Shower" playlist appears in playlist list
- [ ] Playlist shows 4 tracks
- [ ] Track names displayed correctly

#### Test 3: Track Metadata
Check each track displays:
- [ ] Track 1: Kinky Foxx - So Different
- [ ] Track 2: Nicolas Skorsky - Harlem
- [ ] Track 3: Quincy Jones - Ay No Corrida
- [ ] Track 4: Only Child - Addicted

Metadata fields visible:
- [ ] Artist name
- [ ] Track title
- [ ] Album name
- [ ] Duration

#### Test 4: Track Playback
- [ ] Track 1 loads and plays
- [ ] Track 2 loads and plays
- [ ] Track 3 loads and plays
- [ ] Track 4 loads and plays
- [ ] Audio quality is normal (no corruption)

#### Test 5: Waveform Display
- [ ] Waveform shown (even if basic/stub)
- [ ] No error when accessing waveform view

## Expected Results

### Minimal Success (Phase 1 Goal)
- USB recognized
- Playlist appears
- Tracks load and play
- Basic metadata visible

### Known Limitations (Phase 1)
- ⚠️ No BPM displayed (stub analysis)
- ⚠️ No key displayed
- ⚠️ Waveform may be basic/empty
- ⚠️ No beatgrid markers

These are EXPECTED in Phase 1 and will be implemented in Phase 2.

## Troubleshooting

### If USB not recognized:
1. Check directory structure matches exactly
2. Verify PDB file is not corrupted
3. Try reformatting USB stick (FAT32)

### If tracks don't load:
1. Verify audio file paths in PDB
2. Check ANLZ file naming matches
3. Confirm audio files are not corrupted

### If metadata missing:
1. Check PDB Artist/Album tables
2. Verify string encoding in PDB
3. Review track table references

## Results

**Date Tested:** ______________
**Device:** XDJ-XZ
**USB Stick:** ______________

### Pass/Fail Summary
- USB Recognition: _______
- Playlist Display: _______
- Track Metadata: _______
- Track Playback: _______

### Notes
```


```

### Next Steps
- [ ] If tests pass → Proceed to Phase 2 (audio analysis)
- [ ] If tests fail → Debug based on error messages
- [ ] If partial success → Document what works/doesn't
