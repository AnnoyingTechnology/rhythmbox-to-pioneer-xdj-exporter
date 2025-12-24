# Pioneer Exporter Implementation Strategy

## Current Status (2025-12-24)

**Phase:** Audio Analysis COMPLETE
**Status:** Full export pipeline with parallel BPM + key detection. Works on XDJ-XZ.

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
- **FAT32 filename issues** - Need to slugify and truncate filenames
  - FAT32 is case-insensitive: "Album Name" vs "album name" → same folder
  - Long filenames can cause issues
  - Special characters may not be supported
  - **TODO:** Implement proper slugification and truncation for all paths
- **Performance is poor for large exports** - ~10 minutes for 84 tracks
  - 30GB RAM usage during analysis
  - All CPU cores maxed (31 threads on 32-core system)
  - stratum-dsp analysis is the bottleneck
  - Consider: limiting concurrent analyses, streaming decode, or caching analyzed results

### Phase 2.1
- [ ] Rhythmbox track rating (stars) to PDB rating

### Phase 3 - Waveforms (Next)
- [ ] Waveform preview (PWV3 - monochrome, ~400 samples)
- [ ] Waveform detail (PWV5 - monochrome, higher resolution)
- [ ] Color waveform preview (PWV4 - RGB frequency bands)
- [ ] Color waveform detail (PWV6 - RGB high-res)

Libraries to use:
- `rustfft` / `realfft` - FFT for frequency analysis
- `dasp` - RMS/peak extraction per window

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

If you tell me your license constraints (GPL/AGPL acceptable or not), I can narrow these to the “safe-to-bundle” shortlist immediately.

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