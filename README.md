# Pioneer Exporter

Export Rhythmbox music libraries to Pioneer USB format (compatible with XDJ-XZ and similar devices).

## Status: Phase 1 Complete!

**Successfully tested on XDJ-XZ hardware (2025-12-18)**

- Playlists display and are navigable
- Tracks load and play correctly
- Artist/Album/Title metadata displays properly
- Accented characters (UTF-16) display correctly

### What Works
- Full PDB database generation with all 20 table types
- Playlist export with track metadata
- USB directory structure creation
- Audio file copying
- rekordcrate validation passes

### Current Limitations
- No BPM detection (shows 0)
- No waveform display (Phase 2 feature)
- No beatgrid (Phase 2 feature)
- No key detection (Phase 2 feature)

See [CLAUDE.md](CLAUDE.md) for detailed implementation notes and debugging history.

## Quick Start

### Build

```bash
cargo build --release
```

### Export Rhythmbox Library

**Export specific playlist(s):**
```bash
cargo run --release -- --output /path/to/usb --playlist "MyPlaylist" --playlist "Another Playlist"
```

**Full export:**
```bash
cargo run --release -- --output /path/to/usb
```

**With verbose logging:**
```bash
cargo run --release -- --output /path/to/usb --verbose
```

### Copy to USB

After export, copy the generated files to your USB drive:
```bash
cp -r /path/to/output/* /media/usb/
```

Or export directly to the USB mount point.

### Options

```
-d, --database <FILE>         Path to rhythmdb.xml (default: ~/.local/share/rhythmbox/rhythmdb.xml)
-p, --playlists <FILE>        Path to playlists.xml (default: ~/.local/share/rhythmbox/playlists.xml)
-o, --output <DIR>            Target output directory (required)
    --playlist <NAME>         Export only specific playlists (can be used multiple times)
-v, --verbose                 Enable verbose logging
    --validate                Validate existing export without creating new one
```

### Example Output

```
[INFO] Pioneer Exporter - Phase 1 (Stub Analysis)
[INFO] Loading Rhythmbox library...
[INFO] Library loaded: 9298 tracks, 34 playlists
[INFO] Filtering to 2 playlist(s): ["REKORDBOX1", "REKORDBOX2"]
[INFO] Exporting 10 tracks, 2 playlists
[INFO] Processing tracks...
[INFO] Export completed successfully!
[INFO] Validation passed!
```

## Architecture

### Two-Phase Design

**Phase 1 (Complete):** Core export system with stub analysis
- Rhythmbox XML parsing
- PDB database writing (all 20 table types)
- ANLZ stub file generation
- USB file organization
- Audio file copying
- UTF-16 encoding for international characters

**Phase 2 (Future):** Real audio analysis
- BPM detection
- Musical key detection
- Waveform generation
- Beatgrid creation

### Project Structure

```
src/
├── model/          # Data structures (Track, Playlist, Library)
├── rhythmbox/      # Rhythmbox XML parsers
├── analysis/       # Audio analysis (trait-based, extensible)
│   ├── traits.rs   # AudioAnalyzer trait
│   └── stub.rs     # Phase 1 stub implementation
├── pdb/            # PDB (database) file writer
│   ├── writer.rs   # Main PDB writer
│   ├── strings.rs  # DeviceSQL string encoding
│   ├── types.rs    # Table types and constants
│   └── *.bin       # Reference binary data for sensitive tables
├── anlz/           # ANLZ (analysis) file writer
├── export/         # Export pipeline and USB organization
└── validation/     # Round-trip validation with rekordcrate
```

## File Format References

This project implements the Rekordbox USB export format based on reverse-engineering documentation:

- **PDB Format:** https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html
- **ANLZ Format:** https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html
- **rekordcrate:** https://holzhaus.github.io/rekordcrate/ (Rust parser used for validation)

## Key Implementation Details

### String Encoding
The PDB format uses "DeviceSQL" string encoding:
- **Short ASCII:** For strings ≤126 chars containing only ASCII
- **Long ASCII:** For longer ASCII-only strings
- **Long UTF-16LE:** For strings with non-ASCII characters (accents, unicode)

Strings with accented characters (é, ü, ñ, etc.) are automatically encoded as UTF-16LE.

### Reference Binary Data
Some tables (Columns, HistoryPlaylists) use byte-perfect copies from a reference Rekordbox export because the XDJ hardware is extremely sensitive to their exact structure:
- `src/pdb/reference_columns.bin` - 27 column definitions
- `src/pdb/reference_history_playlists.bin` - History playlist entries

## Testing

### Unit Tests
```bash
cargo test --lib
```

### Validation
```bash
cargo run --release -- --validate --output /path/to/existing/export
```

## Development

### Adding Phase 2 Analysis

The architecture is designed for easy extension:

1. Implement real analyzers in `src/analysis/`:
   ```rust
   pub struct FullAnalyzer { /* ... */ }

   impl AudioAnalyzer for FullAnalyzer {
       fn analyze(&self, audio_path: &Path) -> Result<AnalysisResult> {
           // Real BPM detection, key detection, waveform generation
       }
   }
   ```

2. Swap analyzer in `main.rs`:
   ```rust
   // Phase 1
   let analyzer = StubAnalyzer::new();

   // Phase 2
   let analyzer = FullAnalyzer::new();
   ```

3. Export pipeline automatically uses the new analyzer - no changes needed!

### Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` to check for warnings

## Dependencies

### Core
- `quick-xml` - Rhythmbox XML parsing
- `rekordcrate` - PDB/ANLZ format reference and validation
- `anyhow` - Error handling
- `clap` - CLI argument parsing
- `log` / `env_logger` - Logging

### Phase 2 (Future)
- `symphonia` or `ffmpeg-next` - Audio decoding
- Essentia or aubio - Beat/tempo detection
- libkeyfinder - Key detection

## Documentation

- [CLAUDE.md](CLAUDE.md) - Implementation strategy and debugging history

## Acknowledgments

- **Deep Symmetry** - Reverse-engineering the Rekordbox format
- **Jan Holthuis** - rekordcrate Rust library
- **Anthropic Claude** - Code implementation assistance
