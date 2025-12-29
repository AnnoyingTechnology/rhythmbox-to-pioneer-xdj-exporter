# Pioneer DeviceSQL Format - Undocumented Discoveries

This document contains technical discoveries about Pioneer's proprietary DeviceSQL format (export.pdb) and ANLZ waveform files that were **NOT previously documented** in any public source. These findings were obtained through extensive reverse engineering, binary analysis, and hardware testing.

**Contributions to the DJ community** - Feel free to use this information in your own projects.

---

## Table of Contents

1. [ANLZ Path Hash Algorithm](#anlz-path-hash-algorithm) - **WORLD FIRST**
2. [PDB Page Layout](#pdb-page-layout)
3. [Page Header Formulas](#page-header-formulas)
4. [Row Group Structure](#row-group-structure)
5. [Track Row Structure](#track-row-structure)
6. [History Tables](#history-tables)
7. [Waveform Encoding](#waveform-encoding)
8. [Hardware Behavior](#hardware-behavior)

---

## ANLZ Path Hash Algorithm

**Status:** Reverse-engineered from rekordbox binary (December 2025)

This algorithm was discovered by disassembling the `CreateAnlzFileFolderPath()` function in the rekordbox macOS binary using radare2. **This information was not publicly documented anywhere.**

### The Problem

Pioneer CDJs and XDJ hardware compute their own ANLZ file paths from the audio file path. They **ignore** the `analyze_path` field stored in the PDB database. If your ANLZ files are not at the exact path the hardware expects, waveforms will not display.

### Path Format

```
/PIONEER/USBANLZ/P{XXX}/{YYYYYYYY}/ANLZ0000.{DAT,EXT}
```

Where:
- `XXX` = 3 hex digits (P value)
- `YYYYYYYY` = 8 hex digits (hash value)

### Algorithm (Pseudocode)

```python
def compute_anlz_path(file_path: str) -> tuple[int, int]:
    """
    Compute Pioneer ANLZ path from audio file path.

    Args:
        file_path: Path relative to USB root, e.g. "/Contents/Artist/Album/Track.mp3"

    Returns:
        (p_value, hash_value) for path format P{p_value:03X}/{hash_value:08X}
    """
    hash_val = 0

    # Process as UTF-16 code units
    for char in file_path:
        c = ord(char) & 0xFFFF

        # Pioneer's rolling hash - two multiplications per character
        temp = (hash_val * 0x5BC9 + c) & 0xFFFFFFFF
        hash_val = (temp * 0x93B5 + c) & 0xFFFFFFFF

    # Apply modulo 200003
    hash_result = hash_val % 200003  # 0x30D43

    # Extract P value from scattered bits of hash
    p_value = 0
    p_value |= (hash_result >> 0) & 0x01   # bit 0  -> bit 0
    p_value |= (hash_result >> 1) & 0x02   # bit 2  -> bit 1
    p_value |= (hash_result >> 4) & 0x04   # bit 6  -> bit 2
    p_value |= (hash_result >> 4) & 0x08   # bit 7  -> bit 3
    p_value |= (hash_result >> 5) & 0x10   # bit 9  -> bit 4
    p_value |= (hash_result >> 8) & 0x20   # bit 13 -> bit 5
    p_value |= (hash_result >> 10) & 0x40  # bit 16 -> bit 6

    return (p_value, hash_result)
```

### Implementation (Rust)

```rust
fn compute_anlz_path_hash(file_path: &str) -> (u16, u32) {
    let mut hash: u32 = 0;

    for c in file_path.chars() {
        let code_unit = (c as u32) & 0xFFFF;
        let temp = hash.wrapping_mul(0x5bc9).wrapping_add(code_unit);
        hash = temp.wrapping_mul(0x93b5).wrapping_add(code_unit);
    }

    let hash_result = hash % 0x30d43;

    let mut p_value: u16 = 0;
    p_value |= ((hash_result >> 0) & 1) as u16;
    p_value |= ((hash_result >> 1) & 2) as u16;
    p_value |= ((hash_result >> 4) & 4) as u16;
    p_value |= ((hash_result >> 4) & 8) as u16;
    p_value |= ((hash_result >> 5) & 0x10) as u16;
    p_value |= ((hash_result >> 8) & 0x20) as u16;
    p_value |= ((hash_result >> 10) & 0x40) as u16;

    (p_value, hash_result)
}
```

### Verified Test Cases

| Input Path | P Value | Hash Value |
|------------|---------|------------|
| `/Contents/ARTISTTEST1/ALBUMTEST1/TITLETEST1.mp3` | 0x051 | 0x0001D603 |
| `/Contents/ARTISTTEST2/ALBUMTEST2/TITLETEST2.mp3` | 0x03C | 0x0000A6CA |
| `/Contents/ARTISTTEST3/ALBUMTEST3/TITLETEST3.mp3` | 0x045 | 0x0001096B |
| `/Contents/BROOKLYN BOUNCE/The Theme (Of Progressive Attack)/This Is The Begining.mp3` | 0x04B | 0x000154A5 |

### Key Insights

1. **Input is the file path** relative to USB root (starting with `/Contents/...`)
2. **UTF-16 processing** - each character is treated as a 16-bit code unit
3. **Hash constants**: 0x5BC9 and 0x93B5 (proprietary multipliers)
4. **Modulo 200003** (prime number) for hash distribution
5. **P value bit scrambling** - bits are extracted from non-contiguous positions
6. **Deterministic** - same path always produces same result across all exports

---

## PDB Page Layout

Pioneer's DeviceSQL format uses fixed 4096-byte pages with a specific layout:

```
Page 0:      File header (512 bytes used)
Pages 1-2:   Tracks (header + first data page)
Pages 3-4:   Genres
Pages 5-6:   Artists
Pages 7-8:   Albums
Pages 9-10:  Labels (header only, no data)
Pages 11-12: Keys
Pages 13-14: Colors
Pages 15-16: Playlists
Pages 17-18: PlaylistEntries
Pages 19-32: Unknown/Reserved tables
Pages 33-34: Columns
Pages 35-36: HistoryPlaylists
Pages 37-38: HistoryEntries
Pages 39-40: History

Pages 41-52: RESERVED ZONE - MUST BE ALL ZEROS
             These pages are pointed to by empty_candidate fields
             Never write page headers or data here

Pages 53+:   Overflow data for large exports
             Track overflow chains: 2 -> 53 -> 54 -> ...
```

### Critical Discovery: Pages 41-52

**Pages 41-52 must be completely zeroed.** Writing any data to these pages causes corruption. These pages serve as "empty candidate" targets for various tables:

| Page | Purpose |
|------|---------|
| 41-49 | Reserved for future table growth |
| 50 | Keys.empty_candidate |
| 51 | Tracks.empty_candidate (small exports) |
| 52 | PlaylistEntries.empty_candidate |

---

## Page Header Formulas

Each data page has a 40-byte header. Two fields use **undocumented formulas** based on row count:

### Sequence Field (offset 0x10, 4 bytes)

```
sequence = base + (total_rows - 1) * 5
```

| Table | Base Value |
|-------|------------|
| Tracks | 10 |
| Genres | 8 |
| Artists | 7 |
| Albums | 9 |
| Playlists | 6 |
| PlaylistEntries | 11 |
| History | 10 |

**For multi-page tables**: Sequence is cumulative across pages.
```
Page 1: base + (rows_page1 - 1) * 5
Page 2: page1_seq + rows_page2 * 5
Page 3: page2_seq + rows_page3 * 5
```

### unk3 Field (offset 0x19, 1 byte)

```
unk3 = (rows % 8) * 0x20
```

| Rows mod 8 | unk3 |
|------------|------|
| 1 | 0x20 |
| 2 | 0x40 |
| 3 | 0x60 |
| 4 | 0x80 |
| 5 | 0xA0 |
| 6 | 0xC0 |
| 7 | 0xE0 |
| 0 (8, 16, ...) | 0x00 |

**Using incorrect values causes silent corruption in Rekordbox 5.**

---

## Row Group Structure

PDB stores row offsets in "row groups" at the end of each data page. **This structure was incorrectly documented elsewhere.**

### Layout

Row groups are stored in **reverse order** (last group first), growing downward from the page boundary:

```
Page data area: [row data...]
                    |
                    v
Footer area:    [Group N-1][Group N-2]...[Group 1][Group 0]
                                                          ^ Page end (0xFFF)
```

### Group Structure (variable size)

```
For a full group (16 rows):
  [offset_15][offset_14]...[offset_0][present_flags:2][unknown:2]
  = 16 * 2 + 2 + 2 = 36 bytes

For a partial group (N rows, N < 16):
  [offset_N-1][offset_N-2]...[offset_0][present_flags:2][unknown:2]
  = N * 2 + 4 bytes
```

### Fields

| Field | Size | Description |
|-------|------|-------------|
| offsets | 2 bytes each | Row start offsets within page, in reverse order |
| present_flags | 2 bytes | Bitmask of used slots (0xFFFF for full group) |
| unknown | 2 bytes | 0x0000 for full groups, `1 << (N-1)` for partial groups |

### Example: 20 rows (Group 0 = 16 rows, Group 1 = 4 rows)

```
Footer layout (48 bytes total):
  0x6FD0: [off_3][off_2][off_1][off_0][0x000F][0x0008]  <- Group 1 (partial, 4 rows)
  0x6FDC: [off_15]...[off_0][0xFFFF][0x0000]            <- Group 0 (full, 16 rows)
```

---

## Track Row Structure

Track rows use a complex variable-length structure with string pointers.

### Fixed Fields

| Offset | Size | Field | Notes |
|--------|------|-------|-------|
| 0x00 | 2 | unknown | Always 0x0024 |
| 0x02 | 2 | index_shift | Always 0x0003 |
| 0x04 | 4 | bitmask | 0x1FF803DE for standard track |
| 0x08 | 4 | sample_rate | e.g., 44100 |
| 0x0C | 4 | composer_id | Usually 0 |
| 0x10 | 4 | file_size | Audio file size in bytes |
| 0x14 | 4 | unknown_2 | Usually track_id + 20 |
| ... | ... | ... | ... |
| 0x52 | 2 | sample_depth | e.g., 16 |
| 0x54 | 2 | duration | Duration in seconds |

### String Offsets

Track rows contain 21 string offset fields (2 bytes each) pointing to DeviceSQL-encoded strings:

1. isrc
2. texter
3. unknown_string_2
4. unknown_string_3
5. unknown_string_4
6. message
7. kuvo_public
8. autoload_hotcues
9. unknown_string_5
10. unknown_string_6
11. date_added
12. release_date
13. mix_name
14. unknown_string_7
15. analyze_path
16. analyze_date
17. comment
18. title
19. unknown_string_8
20. filename
21. file_path

### Row Padding

- **Single track exports**: Pad to 332 bytes total
- **Multi-track exports**: Pad each row to 344 bytes

---

## History Tables

**Critical discovery**: History tables are **required** for XDJ hardware to recognize the USB, but their content doesn't need to match the exported tracks.

### Behavior

| History Tables | USB Recognition |
|---------------|-----------------|
| Empty (no rows) | **NOT RECOGNIZED** |
| Any valid data | Works |

### Solution

Copy reference History table pages directly from a working export:
- Page 36: HistoryPlaylists header
- Page 38: HistoryEntries data
- Page 40: History data

### History Header Special Values (Page 39)

| Field | 1 Track | 2+ Tracks |
|-------|---------|-----------|
| unk5 (0x20) | 0x0001 | 0x1FFF |
| num_rows_large | 0x0000 | 0x1FFF |
| unk6 (0x24) | 0x03EC | 0x03EC |
| unk7 (0x26) | 0x0001 | 0x0001 |

---

## Waveform Encoding

### PWAV - Monochrome Preview (400 bytes)

```
Each byte: [whiteness:3][height:5]
- height: 0-31 (amplitude)
- whiteness: 5 for preview (controls brightness)
```

### PWV2 - Tiny Preview (100 bytes)

```
Each byte: [unused:4][height:4]
- height: 0-15 (simple peak amplitude)
```

### PWV3 - Monochrome Detail (variable size)

```
Each byte: [whiteness:3][height:5]
- whiteness: 7 for detail waveform
- Entry count: duration_seconds * 150
```

### PWV4 - Color Preview (7200 bytes = 1200 entries x 6 bytes)

Three frequency bands per entry:

| Bytes | Band | Height Range | Color Range |
|-------|------|--------------|-------------|
| 0-1 | Low | 0-127 | 0xE0-0xFF (bright) |
| 2-3 | Mid | 0-127 | 0x01-0x30 (dim) |
| 4-5 | High | 0-127 | 0x01-0x20 (dimmer) |

**Note**: PWV4 uses full 8-bit height (0-127), not 5-bit like PWAV/PWV3.

### PWV5 - Color Detail (variable size)

```
Each 2-byte entry: [blue:3][green:3][red:3][height:5]
- Entry count: duration_seconds * 150
```

---

## Hardware Behavior

### XDJ-XZ Waveform Loading

1. Hardware receives track selection
2. Computes ANLZ path using hash algorithm (ignores PDB `analyze_path`)
3. Loads `/PIONEER/USBANLZ/P{XXX}/{YYYYYYYY}/ANLZ0000.EXT`
4. Falls back to `.DAT` if `.EXT` missing

### Which Waveform Sections Control What

| Section | Needle Search | Jogwheel | Main Screen |
|---------|---------------|----------|-------------|
| PWV3 | **Primary** | **Primary** | No |
| PWV4 | No | No | Color preview |
| PWV5 | No | No | Color detail |
| PWAV | Overview | No | No |

**Key finding**: PWV3 (monochrome detail) controls both needle search and jogwheel display on XDJ-XZ, not PWV4/PWV5 as one might expect.

### File Requirements

| File | Required? | Purpose |
|------|-----------|---------|
| export.pdb | Yes | Track database |
| exportExt.pdb | No | Extended database (can be deleted) |
| ANLZ0000.DAT | Optional | Basic waveforms |
| ANLZ0000.EXT | **Yes** | Detailed waveforms + cues |

---

## References

### Existing Documentation (starting points)

- [Deep Symmetry - PDB Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- [Deep Symmetry - ANLZ Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html)
- [rekordcrate Library](https://holzhaus.github.io/rekordcrate/)
- [Kaitai PDB Spec](https://github.com/Deep-Symmetry/crate-digger/blob/main/src/main/kaitai/rekordbox_pdb.ksy)

### Tools Used for Reverse Engineering

- radare2 - Binary disassembly
- xxd - Hex dumps
- Python struct - Binary parsing
- XDJ-XZ hardware - Live testing
- Rekordbox 5 (MacOS) - Live testing

---

## License

This documentation is released into the public domain. Use it however you want.

If you find this useful, consider contributing back any additional discoveries.

---

*Last updated: December 29, 2025*
