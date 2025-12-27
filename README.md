# Pioneer Exporter

Export Rhythmbox music libraries to Pioneer USB format (compatible with XDJ-XZ and similar devices).

## Status (2025-12-26)

| Feature | Status | Notes |
|---------|--------|-------|
| Multi-track exports | **Working** | 3+ tracks work on XDJ-XZ and Rekordbox 5 |
| Single-track exports | **Broken** | Corrupted in Rekordbox 5 |
| Large exports (88+) | **Working** | Dynamic page allocation |
| BPM/Key detection | **Working** | ~87%/~72% accuracy |
| Artwork | **Not Working** | See [ARTWORK.md](ARTWORK.md) |
| Waveforms | **Not Working** | See [WAVEFORMS.md](WAVEFORMS.md) |

### Verified Hardware
- XDJ-XZ: Playlists, tracks, metadata display properly
- Rekordbox 5: Multi-track exports import correctly

See [CLAUDE.md](CLAUDE.md) for implementation details.

## Quick Start

### Build

```bash
cargo build --release
```

### Export Playlists

```bash
# Export with full analysis (BPM + key detection)
cargo run --release -- --output /path/to/usb --playlist "MyPlaylist"

# Export multiple playlists (recommended - single playlist may corrupt)
cargo run --release -- --output /path/to/usb --playlist "Playlist1" --playlist "Playlist2"

# Fast export (skip audio analysis)
cargo run --release -- --output /path/to/usb --playlist "MyPlaylist" --no-bpm --no-key
```

### Options

```
-o, --output <DIR>        Target output directory (required)
    --playlist <NAME>     Playlists to export (can repeat)
    --no-bpm              Skip BPM detection
    --no-key              Skip key detection
    --cache-bpm           Write detected BPM to source files (FLAC only)
    --cache-key           Write detected key to source files (FLAC only)
    --min-bpm <BPM>       Minimum BPM range (default: 70)
    --max-bpm <BPM>       Maximum BPM range (default: 170)
    --max-parallel <N>    Limit parallel analyses (reduces RAM)
-v, --verbose             Enable verbose logging
```


Tips:
- Always use `--release` build
- Use `--no-bpm --no-key` for fast exports
- Use `--max-parallel 4` to limit memory usage

## Project Structure

```
src/
├── model/          # Track, Playlist, Library structures
├── rhythmbox/      # XML parsers
├── analysis/       # BPM/key detection, waveform generation
├── pdb/            # PDB database writer
├── anlz/           # ANLZ waveform file writer
├── export/         # Export pipeline
└── validation/     # rekordcrate validation
```

## Documentation

- [CLAUDE.md](CLAUDE.md) - Implementation guide and debugging
- [WAVEFORMS.md](WAVEFORMS.md) - Waveform generation
- [ARTWORK.md](ARTWORK.md) - Artwork (not working)
- [HISTORY.md](HISTORY.md) - Debug history and fixes

## Dependencies

**Core:** quick-xml, rekordcrate, anyhow, clap, log

**Audio:** symphonia (decoding), stratum-dsp (BPM/key), lofty (metadata), rayon (parallel)

## References

- [Deep Symmetry PDB Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- [Deep Symmetry ANLZ Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html)
- [rekordcrate](https://holzhaus.github.io/rekordcrate/)

## Testing

```bash
# Unit tests
cargo test --lib

# Validate existing export
cargo run --release -- --validate --output /path/to/export
```

**Always test on actual Pioneer hardware + rekordbox software** - rekordcrate validation is necessary but not sufficient. XDJ is stricter, Rekordbox is even stricter.

## Acknowledgments

- **Deep Symmetry** - Reverse-engineering the Rekordbox format
- **Jan Holthuis** - rekordcrate Rust library
