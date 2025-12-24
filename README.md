# Pioneer Exporter

Export Rhythmbox music libraries to Pioneer USB format (compatible with XDJ-XZ and similar devices).

## Status: Phase 2 Complete - Audio Analysis Working!

**Successfully tested on XDJ-XZ hardware (2025-12-24)**

### What Works
- Playlists display and are navigable
- Tracks load and play correctly
- Artist/Album/Title/Genre metadata displays properly
- **BPM detection** (~87% accuracy, stratum-dsp)
- **Key detection** (~72% accuracy, stratum-dsp)
- **Beatgrid generation** (constant tempo)
- Accented characters (UTF-16) display correctly
- Multi-page track support (tested with 84+ tracks)
- Parallel audio analysis (uses all CPU cores)
- rekordcrate validation passes

### Current Limitations
- No waveform display (Phase 3)
- No album artwork (Phase 4)
- No variable tempo beatgrid (Phase 5)

See [CLAUDE.md](CLAUDE.md) for detailed implementation notes and roadmap.

## Quick Start

### Build

```bash
cargo build --release
```

### Export Rhythmbox Library

**Export specific playlist(s) with full analysis:**
```bash
cargo run --release -- --output /path/to/usb --playlist "MyPlaylist"
```

**Export multiple playlists:**
```bash
cargo run --release -- --output /path/to/usb --playlist "Playlist1" --playlist "Playlist2"
```

**Skip BPM/key detection (faster export):**
```bash
cargo run --release -- --output /path/to/usb --playlist "MyPlaylist" --no-bpm --no-key
```

**Cache detected BPM/key to source files (FLAC only):**
```bash
cargo run --release -- --output /path/to/usb --playlist "MyPlaylist" --cache-bpm --cache-key
```

**Custom BPM range (for genre-specific tuning):**
```bash
cargo run --release -- --output /path/to/usb --playlist "MyPlaylist" --min-bpm 100 --max-bpm 180
```

### Options

```
-d, --database <FILE>     Path to rhythmdb.xml (default: ~/.local/share/rhythmbox/rhythmdb.xml)
-p, --playlists <FILE>    Path to playlists.xml (default: ~/.local/share/rhythmbox/playlists.xml)
-o, --output <DIR>        Target output directory (required)
    --playlist <NAME>     Export only specific playlists (can be used multiple times)
-v, --verbose             Enable verbose logging
    --validate            Validate existing export without creating new one
    --no-bpm              Skip BPM detection
    --no-key              Skip key detection
    --cache-bpm           Write detected BPM to source file tags (FLAC only)
    --cache-key           Write detected key to source file tags (FLAC only)
    --min-bpm <BPM>       Minimum BPM for detection range (default: 70)
    --max-bpm <BPM>       Maximum BPM for detection range (default: 170)
    --max-parallel <N>    Limit parallel track analysis (reduces memory usage)
```

### Example Output

```
[INFO] Pioneer Exporter - Audio Analysis Enabled
[INFO] Loading Rhythmbox library...
[INFO] Library loaded: 9298 tracks, 36 playlists
[INFO] Filtering to 1 playlist(s): ["My DJ Set"]
[INFO] Exporting 45 tracks, 1 playlists
[INFO] Analyzing audio (BPM + Key detection)...
[INFO] Analyzed 45/45 tracks
[INFO] Processing tracks...
[INFO] Export completed successfully!
[INFO] Validation passed!
```

## Performance

Audio analysis uses parallel processing with rayon (cores - 1 threads):

| Tracks | Time (release build) | Notes |
|--------|---------------------|-------|
| 8 | ~3 seconds | 32-core system |
| 45 | ~10 seconds | 32-core system |
| 84 | ~10 minutes | High RAM usage (~30GB) |

**Tips:**
- Always use `--release` for best performance
- Use `--no-bpm --no-key` for fast exports without analysis
- Pre-cache BPM/key with `--cache-bpm --cache-key` to speed up future exports
- Use `--max-parallel 4` to limit memory usage on large exports

## Architecture

### Current Implementation

- **Rhythmbox parsing**: XML parsing for library and playlists
- **Audio analysis**: stratum-dsp for BPM + key detection in a single pass
- **PDB writing**: Full database with all 20 table types
- **ANLZ writing**: PPTH (path), PVBR (VBR index), PQTZ (beatgrid)
- **Parallel processing**: rayon for multi-threaded analysis

### Project Structure

```
src/
├── model/          # Data structures (Track, Playlist, Library)
├── rhythmbox/      # Rhythmbox XML parsers
├── analysis/       # Audio analysis
│   ├── traits.rs   # AudioAnalyzer trait
│   ├── stratum.rs  # stratum-dsp BPM + key detection
│   ├── real.rs     # Full analyzer implementation
│   └── stub.rs     # Fast stub (no analysis)
├── pdb/            # PDB (database) file writer
│   ├── writer.rs   # Main PDB writer
│   ├── strings.rs  # DeviceSQL string encoding
│   └── types.rs    # Table types and constants
├── anlz/           # ANLZ (analysis) file writer
├── export/         # Export pipeline and USB organization
└── validation/     # Round-trip validation with rekordcrate
```

## Dependencies

### Core
- `quick-xml` - Rhythmbox XML parsing
- `rekordcrate` - PDB/ANLZ format reference and validation
- `anyhow` / `thiserror` - Error handling
- `clap` - CLI argument parsing
- `log` / `env_logger` - Logging

### Audio Analysis
- `symphonia` - Audio decoding (MP3, FLAC, AAC, WAV, OGG)
- `stratum-dsp` - BPM + key detection (pure Rust)
- `lofty` - Metadata read/write for caching
- `rayon` - Parallel processing

## Roadmap

### Phase 2.1: Quick Wins (Complete)
- [x] Rhythmbox rating → PDB rating
- [x] FAT32 path improvements (sanitization, truncation)
- [x] Memory optimization (`--max-parallel` option)

### Phase 3: Waveforms
- [ ] Monochrome preview waveform (PWAV - 400 bytes)
- [ ] Monochrome detail waveform (PWV3)
- [ ] Color preview waveform (PWV4)
- [ ] Color detail waveform (PWV5)

### Phase 4: Artwork
- [ ] Extract embedded artwork from audio files
- [ ] Resize to Pioneer format (80×80, 56×56)
- [ ] Write to USB artwork directory

### Phase 5: Advanced Beatgrid
- [ ] Beat detection for phase alignment
- [ ] Variable tempo support

## File Format References

- **PDB Format:** https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html
- **ANLZ Format:** https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html
- **rekordcrate:** https://holzhaus.github.io/rekordcrate/

## Testing

```bash
# Unit tests
cargo test --lib

# Validate existing export
cargo run --release -- --validate --output /path/to/existing/export
```

**Always test on actual Pioneer hardware** - rekordcrate validation is necessary but not sufficient.

## Acknowledgments

- **Deep Symmetry** - Reverse-engineering the Rekordbox format
- **Jan Holthuis** - rekordcrate Rust library
- **Anthropic Claude** - Code implementation assistance
