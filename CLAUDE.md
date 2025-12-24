# Pioneer Exporter Implementation Strategy

## Current Status (2025-12-24)

**Phase:** Phase 1 cleanup complete - preparing for dynamic exports
**Status:** Removed hardcoded test track values. Export still works on XDJ-XZ.

---

## Phase 1 Cleanup (Completed)

Removed hardcoded values that were only needed for byte-perfect reference matching:

1. **ANLZ paths** - Removed `REFERENCE_TRACK_DATA` hardcoding in `organizer.rs`
   - Now uses FNV-1a hash for all tracks (was hardcoded for TITLETEST1/2/3)

2. **Key IDs** - Removed track title matching in `pdb/writer.rs`
   - Now sets `key_id=0` for all tracks (no key assigned)
   - TODO: Add key detection from audio analysis

3. **Keys table** - Expanded from 3 to 24 musical keys
   - Was: Am, Bm, Cm only
   - Now: All 12 minor + 12 major keys

**Remaining hardcoded values (Phase 2 - required for track info display):**
- `reference_history.bin` (page 40) - History table data
- `reference_history_entries.bin` (page 38) - HistoryEntries table data
- `reference_history_playlists.bin` (page 36) - HistoryPlaylists table data

These History tables still reference track IDs 1-3 from the reference export. To export arbitrary playlists, we need to generate these tables dynamically.

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
6. ~~**Track key_id** - Correct key IDs (1=Am, 2=Bm, 3=Cm) for test tracks~~ **(REMOVED - now 0 for all)**
7. **Album artist_id** - Set to 0 (not actual artist ID) to match reference
8. **Empty tables** - Labels and Artwork are header-only (no data pages)
9. **File header** - `next_unused_page=53`, `sequence=31` to match reference
10. **Keys table** - Expanded to all 24 musical keys (was 3)

---

## Test Setup

**Rhythmbox Playlists (must match reference):**
- REKORDBOX1: Track 1 (TITLETEST1)
- REKORDBOX2: Track 1, 2
- REKORDBOX3: Track 1, 2, 3

**Export command:**
```bash
cargo run -- --output /path/to/usb --playlist "REKORDBOX1" --playlist "REKORDBOX2" --playlist "REKORDBOX3" --no-bpm
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
- The History tables are NOT optional for track info display
- When adding tracks dynamically, the History tables will need to be generated properly (not just copied from reference)
- The HistoryEntries table appears to link tracks to playlist history
- The History table appears to track playback/import history
