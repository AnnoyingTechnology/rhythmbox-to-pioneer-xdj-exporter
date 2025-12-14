# Pioneer Exporter

Export Rhythmbox music libraries to Pioneer USB format (compatible with XDJ-XZ and similar devices).

## Status: Phase 1 - Hardware Testing Round 2

✅ **Fixed:** Critical page numbering bugs (test #1 failure)
✅ **Working:** PDB header, table pointers, Albums table
⚠️ **Known Issue:** Track row structure (may not affect XDJ hardware)
❓ **Next Step:** Hardware testing on XDJ-XZ required

See [HARDWARE_TEST_2.md](HARDWARE_TEST_2.md) for current test status and [CLAUDE.md](CLAUDE.md) for detailed bug fixes.

## Quick Start

### Build

```bash
cargo build --release
```

### Run Tests

```bash
cargo test
```

### Export Rhythmbox Library

**Full export:**
```bash
cargo run --release -- --output /path/to/usb
```

**Export specific playlist(s):**
```bash
cargo run --release -- --output /path/to/usb --playlist "Shower" --playlist "Party Mix"
```

**With verbose logging:**
```bash
cargo run --release -- --output /path/to/usb --verbose
```

### Options

```
-d, --database <FILE>         Path to rhythmdb.xml (default: ~/.local/share/rhythmbox/rhythmdb.xml)
-p, --playlists <FILE>        Path to playlists.xml (default: ~/.local/share/rhythmbox/playlists.xml)
-o, --output <DIR>            Target USB mount point (required)
    --playlist <NAME>         Export only specific playlists (can be used multiple times)
-v, --verbose                 Enable verbose logging
    --validate                Validate existing export without creating new one
```

### Example Output

```
[INFO] Pioneer Exporter - Phase 1 (Stub Analysis)
[INFO] Loading Rhythmbox library...
[INFO] Library loaded: 9298 tracks, 34 playlists
[INFO] Filtering to 1 playlist(s): ["Shower"]
[INFO] Exporting 4 tracks, 1 playlists
[INFO] Processing tracks...
[INFO] [1/4] Processing: Kinky Foxx - So Different
[INFO] [2/4] Processing: Nicolas Skorsky - Harlem
[INFO] [3/4] Processing: Quincy Jones - Ay No Corrida
[INFO] [4/4] Processing: Only Child - Addicted
[INFO] Export completed successfully!
[INFO] USB stick ready at: /tmp/pioneer_test
```

## Architecture

### Two-Phase Design

**Phase 1 (Current):** Core export system with stub analysis
- Rhythmbox parsing
- PDB database writing
- ANLZ file generation (stubs)
- USB file organization
- Audio file copying

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
├── anlz/           # ANLZ (analysis) file writer
├── export/         # Export pipeline and USB organization
└── validation/     # Round-trip validation with rekordcrate
```

## File Format References

This project implements the Rekordbox USB export format based on reverse-engineering documentation:

- **PDB Format:** https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html
- **ANLZ Format:** https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html
- **rekordcrate:** https://holzhaus.github.io/rekordcrate/ (Rust parser used for validation)

## Current Limitations

### Phase 1 Incomplete
1. **Track rows missing critical fields:**
   - `file_path` - XDJ-XZ needs this to find audio files
   - `analyze_path` - Links to waveform/beatgrid data

2. **Playlist parser incomplete** - Doesn't extract track entries yet

3. **ANLZ files empty** - Only headers, no waveform/beatgrid data

### Expected Behavior
- ✅ Generates valid file structure
- ✅ Creates PDB database
- ✅ Copies audio files
- ⚠️ XDJ-XZ may not recognize tracks without path fields
- ⚠️ No waveforms or beatgrids displayed

## Testing

### Unit Tests
```bash
cargo test --lib
```

Tests string encoding, data model, library management.

### Integration Tests
```bash
cargo test --test basic_export
```

Tests complete export pipeline with mock data.

### Expected Output
```
running 8 tests
test model::library::tests::test_add_playlist ... ok
test model::library::tests::test_library_creation ... ok
test model::library::tests::test_add_track ... ok
test pdb::strings::tests::test_empty_string ... ok
test pdb::strings::tests::test_short_string ... ok
test test_stub_analyzer ... ok
test test_library_creation ... ok
test test_export_creates_directory_structure ... ok

✓ Export completed successfully
  PDB file: /tmp/.../PIONEER/rekordbox/export.pdb
  PDB size: 20588 bytes
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
- All public APIs should have documentation

## Dependencies

### Core
- `quick-xml` - Rhythmbox XML parsing
- `rekordcrate` - PDB/ANLZ format reference
- `anyhow` - Error handling
- `clap` - CLI argument parsing

### Phase 2 (Future)
- `symphonia` or `ffmpeg-next` - Audio decoding
- Essentia or aubio - Beat/tempo detection
- libkeyfinder - Key detection

## Documentation

- [OBJECTIVE.md](OBJECTIVE.md) - Original specification
- [CLAUDE.md](CLAUDE.md) - Implementation strategy
- [STATUS.md](STATUS.md) - Current status and TODOs
- [VALIDATION_REPORT.md](VALIDATION_REPORT.md) - Test results and validation

## Contributing

This is a personal project, but contributions are welcome. Key areas:

1. **Complete track row format** - Add file_path and analyze_path fields
2. **Complete playlist parser** - Extract track entries from playlists.xml
3. **Validate with rekordcrate** - Ensure PDB parses correctly
4. **Test on hardware** - XDJ-XZ compatibility testing
5. **Phase 2 analysis** - Real BPM/key detection, waveform generation

## License

[Add license here]

## Acknowledgments

- **Deep Symmetry** - Reverse-engineering the Rekordbox format
- **Jan Holthuis** - rekordcrate Rust library
- **Anthropic Claude** - Code implementation assistance
