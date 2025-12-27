# Pioneer Exporter - Implementation Guide

## Current Status (2025-12-27)

| Export Type | Rekordbox 5 | XDJ-XZ | Notes |
|-------------|-------------|--------|-------|
| Small (1-10) | **WORKS** | **WORKS** | Validated |
| Medium (11-35) | **WORKS** | **WORKS** | Keys + overflow working |
| Large (88+) | **WORKS** | **WORKS** | Multi-overflow validated |

### Page Allocation (with Keys)

- **Keys**: page 12 (24 rows), empty=50
- **Tracks**: empty=51, overflow at 51, 53, 54... (skips 52)
- **PlaylistEntries**: empty=52

### What Works
- USB recognition on XDJ-XZ
- Playlists with correct structure
- Tracks playable with metadata
- Dynamic track paging
- Table pointers now match reference exactly
- Artwork extraction and display (see ARTWORK.md)

### What Doesn't Work
- Waveforms display (see WAVEFORMS.md)
- Track count display (uses static History data)

---

## PDB Page Layout

The PDB format uses a **fixed page layout** (4096 bytes per page):

```
Pages 0-40:  Fixed table structure
  Page 0:     File header
  Pages 1-2:  Tracks (header + data)
  Pages 3-4:  Genres
  Pages 5-6:  Artists
  Pages 7-8:  Albums
  Pages 9-10: Labels (header only)
  Pages 11:   Keys (header only)
  Pages 12:   Reserved for Keys growth
  Pages 13-14: Colors
  Pages 15-16: Playlists
  Pages 17-18: PlaylistEntries
  Pages 19-32: Unknown tables (mostly empty)
  Pages 33-34: Columns
  Pages 35-36: HistoryPlaylists
  Pages 37-38: HistoryEntries
  Pages 39-40: History

Pages 41-49: RESERVED (zeros)
  - ALL ZEROS in file
  - Never write page headers here

Pages 50+:   Track overflow data
  - Overflow pages start at 50 (NOT 53!)
  - Chain: 2 → 50 → 51 → ... → empty_candidate
  - empty_candidate = max(52, last_overflow + 1)
```

---

## Key Rules

### 1. Pages 41-52 Must Be Zeros
These are reserved for empty_candidate pointers. Never write page headers here.

### 2. Dynamic Values for Large Exports
```rust
// For large exports (>10 tracks):
actual_track_last_page = track_data_pages[last_chunk]
actual_track_empty_candidate = actual_track_last_page + 1
next_unused_page = actual_track_empty_candidate + 1

// Last track page's next_page points to empty_candidate
```

### 3. Track Row Padding
- Single track: pad to 332 bytes total
- Multiple tracks: pad each row to 344 bytes
- Row offsets: 0, 344, 692, 1040, ...

### 4. Row Group Footer Structure
```
[row_offsets...][present_flags:2][unknown:2]
- Groups written in REVERSE order (partial first)
- Partial groups: only actual row count offsets
- Full groups (16 rows): unknown=0x0000
- Partial groups: unknown=2^highest_bit
```

### 5. Static Reference Data Required
These files are copied from reference exports:
- `reference_history.bin` (page 40)
- `reference_history_entries.bin` (page 38)
- `reference_history_playlists.bin` (page 36)
- `reference_columns.bin` (page 34)

Empty History tables = USB not recognized by XDJ.

### 6. Sequence and unk3 Formulas (CRITICAL)

Page header fields must use dynamic formulas based on row count:

**Sequence (offset 0x10)**: `base + (rows - 1) * 5`
```
Table            | Base | Example: 10 rows
-----------------|------|------------------
Tracks           | 10   | 10 + 9*5 = 55 (0x37)
Genres           | 8    | 8 + 9*5 = 53 (0x35)
Artists          | 7    | 7 + 9*5 = 52 (0x34)
Albums           | 9    | 9 + 9*5 = 54 (0x36)
Playlists        | 6    | Always 6 (usually 1 playlist)
PlaylistEntries  | 11   | 11 + 9*5 = 56 (0x38)
History          | 10   | Same as Tracks
```

**unk3 (byte 0x19)**: `(rows % 8) * 0x20`
```
Rows  | unk3
------|-------
1     | 0x20
2     | 0x40
3     | 0x60
...   | ...
7     | 0xe0
8     | 0x00
9     | 0x20 (cycle repeats)
```

**History Header (page 39) Special Values**:
```
Field           | 1 track    | 2+ tracks
----------------|------------|------------
unk5 (0x20)     | 0x0001     | 0x1fff
num_rows_large  | 0x0000     | 0x1fff
unk6 (0x24)     | 0x03ec     | 0x03ec
unk7 (0x26)     | 0x0001     | 0x0001
```

Using wrong values causes corruption in Rekordbox 5.

---

## Reference Exports

### reference-1 (Single Track)
- Playlist: REKORDBOX4
- Track: Fresh.mp3
- Use for: Single-track debugging

### reference (3 Tracks)
- Playlists: REKORDBOX1, REKORDBOX2, REKORDBOX3
- Tracks: TITLETEST1, TITLETEST2, TITLETEST3
- Use for: Multi-track debugging

### reference-84 (Large Export)
- 84 tracks with artwork
- Use for: Large export and artwork debugging

---

## Test Commands

```bash
# Match reference-1 (single track)
cargo run --release -- --output /tmp/test --playlist "REKORDBOX4" --no-bpm --no-key

# Match reference (3 tracks)
cargo run --release -- --output /tmp/test --playlist "REKORDBOX1" --playlist "REKORDBOX2" --playlist "REKORDBOX3" --no-bpm --no-key

# Large export
cargo run --release -- --output /tmp/test --playlist "XDJ: Minimal" --no-bpm --no-key
```

---

## Debugging Tools

### Compare with Reference
```bash
# Count byte differences
cmp -l our.pdb reference.pdb | wc -l

# Show differences
diff <(xxd our.pdb) <(xxd reference.pdb) | head -50

# Check specific page
xxd -s $((PAGE * 4096)) -l 128 export.pdb
```

### Analyze PDB Structure
```bash
python3 -c "
import struct
with open('export.pdb', 'rb') as f:
    header = f.read(512)
    _, page_size, num_tables, next_unused, _, sequence = struct.unpack('<IIIIII', header[:24])
    print(f'next_unused={next_unused}, sequence={sequence}')
"
```

---

## Resolved Issues

### 1-10 Track Corruption (FIXED 2025-12-27)

**Root Cause**: Page header fields `sequence` and `unk3` were using hardcoded values
from 3-track reference instead of dynamic formulas.

**Fix Applied**: Implemented formulas for all entity tables:
- Sequence: `base + (rows - 1) * 5` per table
- unk3: `(rows % 8) * 0x20` (cyclic pattern)
- History header: Special values for 1-track vs 2+ tracks
- History data page: Patch sequence after copying reference data

---

## Related Documentation

- **HISTORY.md** - Detailed debugging sessions and fixes
- **WAVEFORMS.md** - Waveform generation details
- **ARTWORK.md** - Artwork implementation (not working)

---

## External References

- [Deep Symmetry - PDB Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- [Deep Symmetry - ANLZ Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html)
- [rekordcrate Library](https://holzhaus.github.io/rekordcrate/)
- [Kaitai PDB Spec](https://github.com/Deep-Symmetry/crate-digger/blob/main/src/main/kaitai/rekordbox_pdb.ksy)
- [pyrekordbox](https://github.com/dylanljones/pyrekordbox)

---

## Golden Rule

**NEVER DOUBT REFERENCE EXPORTS. THEY ARE 100% WORKING.**

When debugging, always compare against reference exports byte-by-byte. The reference is always correct.
