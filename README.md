# Pioneer Exporter

Export Rhythmbox playlists to Pioneer USB format for XDJ/CDJ hardware.

## Features

- Full PDB database generation (tracks, artists, albums, genres, playlists)
- Waveform generation (preview + detail, mono + color)
- BPM detection (~87% accuracy)
- Key detection (~72% accuracy)
- Artwork extraction with deduplication
- Large library support (tested with 100+ tracks)

### Verified Hardware
- Pioneer XDJ-XZ
- Rekordbox 5

## Installation

```bash
cargo build --release
```

## Usage

```bash
# Export a playlist
./target/release/pioneer-exporter -o /media/usb --playlist "My Playlist"

# Export multiple playlists
./target/release/pioneer-exporter -o /media/usb --playlist "House" --playlist "Techno"

# Fast export (skip BPM/key analysis)
./target/release/pioneer-exporter -o /media/usb --playlist "My Playlist" --no-bpm --no-key
```

### Options

| Option | Description |
|--------|-------------|
| `-o, --output <DIR>` | Target USB/directory (required) |
| `--playlist <NAME>` | Playlist to export (repeatable) |
| `--no-bpm` | Skip BPM detection |
| `--no-key` | Skip key detection |
| `--cache-bpm` | Write BPM to source files (FLAC) |
| `--cache-key` | Write key to source files (FLAC) |
| `--min-bpm <N>` | Minimum BPM range (default: 70) |
| `--max-bpm <N>` | Maximum BPM range (default: 170) |
| `--max-parallel <N>` | Limit parallel analysis threads |
| `-v, --verbose` | Verbose logging |

## Limitations

- **Full export only** - No incremental exports yet; re-exports entire library each time
- **No analysis cache** - BPM/key/waveforms recomputed on each export

## Documentation

- [PIONEER.md](PIONEER.md) - Reverse-engineered format specs (community contribution)
- [CLAUDE.md](CLAUDE.md) - Implementation details

### PIONEER.md - Undocumented Discoveries

Previously undocumented specifications discovered through reverse engineering:

- **ANLZ Path Hash Algorithm** - How Pioneer computes waveform file paths (reverse-engineered from rekordbox binary)
- **PDB Page Layout** - Fixed page structure and reserved zones (pages 41-52)
- **Page Header Formulas** - Sequence and unk3 field calculations
- **Row Group Structure** - Footer layout with reverse-ordered groups
- **Track Row Structure** - Fixed fields and 21 string offset positions
- **History Tables** - Required for XDJ hardware recognition
- **Waveform Encoding** - PWAV, PWV2, PWV3, PWV4, PWV5 byte formats
- **Hardware Behavior** - Which waveform sections control which display

## Technical Details

The exporter generates:
- `PIONEER/rekordbox/export.pdb` - Track database
- `PIONEER/USBANLZ/P{XXX}/{HASH}/ANLZ0000.DAT` - Preview waveforms
- `PIONEER/USBANLZ/P{XXX}/{HASH}/ANLZ0000.EXT` - Detail waveforms + cues
- `Contents/{Artist}/{Album}/{Track}` - Audio files

ANLZ paths use Pioneer's proprietary hash algorithm (reverse-engineered from rekordbox binary).

## References

- [Deep Symmetry - PDB Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- [Deep Symmetry - ANLZ Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html)
- [rekordcrate](https://holzhaus.github.io/rekordcrate/)

## License

MIT
