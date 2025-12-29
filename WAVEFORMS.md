# Waveform Implementation

This document covers waveform generation for Pioneer USB exports.

## Current Status (2025-12-29)

**Status:** ALGORITHM DISCOVERED AND IMPLEMENTED

Waveforms now work on XDJ-XZ hardware. The ANLZ path hash algorithm was reverse-engineered from the rekordbox binary.

---

## ANLZ Path Algorithm (SOLVED)

### Discovery

The XDJ-XZ computes its own ANLZ path from the audio file path, ignoring the `analyze_path` in the PDB. The algorithm was reverse-engineered from `CreateAnlzFileFolderPath()` in the rekordbox macOS binary using radare2.

### Path Format

`/PIONEER/USBANLZ/P{XXX}/{YYYYYYYY}/ANLZ0000.{DAT,EXT}`

Where:
- `XXX` = 3 hex digits (P value, scattered bits from hash)
- `YYYYYYYY` = 8 hex digits (hash result modulo 200003)

### Algorithm

```rust
fn compute_anlz_path_hash(file_path: &str) -> (u16, u32) {
    let mut hash: u32 = 0;

    // Process path as UTF-16 code units
    for c in file_path.chars() {
        let code_unit = (c as u32) & 0xFFFF;

        // Pioneer's rolling hash
        let temp = hash.wrapping_mul(0x5bc9).wrapping_add(code_unit);
        hash = temp.wrapping_mul(0x93b5).wrapping_add(code_unit);
    }

    // Modulo 200003 (0x30d43)
    let hash_result = hash % 0x30d43;

    // Extract P value from scattered bits
    let mut p_value: u16 = 0;
    p_value |= ((hash_result >> 0) & 1) as u16;     // bit 0 -> bit 0
    p_value |= ((hash_result >> 1) & 2) as u16;     // bit 2 -> bit 1
    p_value |= ((hash_result >> 4) & 4) as u16;     // bit 6 -> bit 2
    p_value |= ((hash_result >> 4) & 8) as u16;     // bit 7 -> bit 3
    p_value |= ((hash_result >> 5) & 0x10) as u16;  // bit 9 -> bit 4
    p_value |= ((hash_result >> 8) & 0x20) as u16;  // bit 13 -> bit 5
    p_value |= ((hash_result >> 10) & 0x40) as u16; // bit 16 -> bit 6

    (p_value, hash_result)
}
```

### Verified Test Cases

| File Path | Expected P | Expected Hash | Result |
|-----------|------------|---------------|--------|
| /Contents/ARTISTTEST1/ALBUMTEST1/TITLETEST1.mp3 | 051 | 0001D603 | MATCH |
| /Contents/ARTISTTEST2/ALBUMTEST2/TITLETEST2.mp3 | 03C | 0000A6CA | MATCH |
| /Contents/ARTISTTEST3/ALBUMTEST3/TITLETEST3.mp3 | 045 | 0001096B | MATCH |
| /Contents/BROOKLYN BOUNCE/.../This Is The Begining.mp3 | 04B | 000154A5 | MATCH |

### Implementation

The algorithm is implemented in `src/export/organizer.rs` in the `compute_anlz_path_hash()` function.

---

## Historical Research (Archived)
| CRC32 | file path (all encodings) | No match |
| Adler32 | file path, track ID | No match |
| FNV-1a | file path | No match |
| Character sum | file path chars | No match |

**Input variations tested:**
- File path: original, lowercase, uppercase, no leading slash, backslashes
- Filename only, without extension
- Track ID: string, padded, hex
- Combinations: id+path, path+id, size+path

### Critical Discovery: Paths are CONSISTENT Across Exports

**SAME audio file = SAME ANLZ path in ALL exports:**

| Audio File | ANLZ Path | Exports |
|------------|-----------|---------|
| TITLETEST1.mp3 | P051/0001D603 | Same in 10 exports |
| TITLETEST2.mp3 | P03C/0000A6CA | Same in 9 exports |
| TITLETEST3.mp3 | P045/0001096B | Same in 8 exports |

This proves:
1. The path IS deterministic (not random, not export-time ID)
2. There IS a computable algorithm
3. We just haven't found it yet

### What's NOT the source

**Critical Test: TITLETEST1/2/3 have IDENTICAL audio content but DIFFERENT ANLZ paths!**

| File | Audio MD5 | ANLZ Path | Conclusion |
|------|-----------|-----------|------------|
| TITLETEST1.mp3 | 0x2F... | P051/... | Same audio |
| TITLETEST2.mp3 | 0x2F... | P03C/... | Different path |
| TITLETEST3.mp3 | 0x2F... | P045/... | Different path |

This **disproves** audio content hash - the TITLETEST{n} files are empty/silent tracks with identical audio content but different file paths, and they get DIFFERENT ANLZ paths.

**The path MUST be derived from the file path** - we just haven't found the correct algorithm yet.

Tested and ruled out:

| Source | Result | Match? |
|--------|--------|--------|
| MD5 of full file | Different per file | No |
| MD5 of ID3 tag | Different per file | No |
| MD5 of audio data (no ID3) | SAME for test files | No (paths differ!) |
| SHA1/SHA256 of audio | SAME for test files | No |
| File path hash (any algo) | Various | No |
| CRC32, Adler32, FNV-1a | Various | No |

### Key Constraint: XDJ-XZ Computes Path From USB Data Only

The XDJ-XZ can (and does) compute the ANLZ path using ONLY what's on the USB drive, not the database:
- Audio files (path, content, metadata)
- PDB track row data
- No access to rekordbox.db or any external source

**This means the algorithm MUST use inputs available on the USB.**

Analysis of reference-84 (174 ANLZ files):
- **Hash values range: 218 - 198,945**
- **P values range: 0x000 - 0x07A** (93 unique values)
- Same P value appears for completely different file paths

### What Remains To Test

Since it's not file path, not audio content, the algorithm likely uses:
1. **Something in the PDB track row** (unknown field we haven't examined)
2. **File metadata** (creation date, file size, specific ID3 fields)
3. **A specific byte range** in the audio file we haven't tested
4. **Combination of multiple inputs**

### Next Step: Binary Reverse Engineering

Rekordbox binaries available for analysis:
- `rekordbox/Install_rekordbox_x64_5_8_7.exe` (Windows)
- `rekordbox/rekordbox.app` (macOS)

Approach:
1. Disassemble the binary
2. Search for USBANLZ string references
3. Find the path generation function
4. Identify exact input values and algorithm

---

## Why This Matters

1. **Our ANLZ files are correct** - they work when placed at the right path
2. **Our waveform data is valid** - displays correctly in Rekordbox Desktop
3. **PDB structure is correct** - Rekordbox 5 reads our exports fine
4. **Only XDJ hardware fails** - because it computes its own path

---

## Temporary Workaround

Until the path algorithm is reverse-engineered:

```bash
# Copy reference USBANLZ folder structure over our export
cp -r reference-export/PIONEER/USBANLZ/* our-export/PIONEER/USBANLZ/
```

This makes waveforms work because XDJ finds files at the expected paths.

---

## Waveform Types

| Type | File | Size | Description |
|------|------|------|-------------|
| PWAV | .DAT | 400 bytes | Monochrome preview waveform |
| PWV2 | .DAT | 100 bytes | Tiny preview (needle display) |
| PWV3 | .EXT | 150 entries/sec | Monochrome detail waveform |
| PWV4 | .EXT | 1200 entries x 6 bytes | Color preview (3 frequency bands) |
| PWV5 | .EXT | 150 entries/sec x 2 bytes | Color detail waveform |

---

## Encoding Format

### PWAV (Monochrome Preview)
```
height (5 low bits) | whiteness (3 high bits)
whiteness = 5 (like reference)
```

### PWV2 (Tiny Preview)
```
height (4 bits) - simple peak amplitude
```

### PWV3 (Monochrome Detail)
```
height (5 low bits) | whiteness (3 high bits)
whiteness = 7 (like reference)
Entry count = duration_secs * 150
```

### PWV4 (Color Preview - 3 Frequency Bands)
Each 6-byte entry has 3 columns for frequency bands (low/mid/high):
- Bytes 0-1: Low frequency (height 0-127, color 0xE0-0xFF = bright)
- Bytes 2-3: Mid frequency (height 0-127, color 0x01-0x30 = dim)
- Bytes 4-5: High frequency (height 0-127, color 0x01-0x20 = dimmer)

Height uses FULL 8-bit range (0-127 typical), NOT 5-bit like PWAV/PWV3.

### PWV5 (Color Detail)
```
RGB (3 bits each) | height (5 bits)
```

---

## ANLZ File Structure

### .DAT File Sections
1. PMAI (header)
2. PPTH (path to audio file)
3. PVBR (VBR timing index)
4. PQTZ (beatgrid)
5. PWAV (preview waveform)
6. PWV2 (tiny preview)

### .EXT File Sections
1. PMAI (header)
2. PPTH (path)
3. PWV3 (detail waveform)
4. PCOB x2 (cue bank)
5. PCO2 x2 (cue list)
6. PWV5 (color detail)
7. PWV4 (color preview)

---

## XDJ-XZ Hardware Test Results

| Test Variant | Needle Search | Jogwheel | Main Screen | Notes |
|--------------|---------------|----------|-------------|-------|
| Our export as-is | FLATLINE | FLATLINE | Nothing | Wrong ANLZ path |
| Reference ANLZ folder | **WORKS** | **WORKS** | **WORKS** | Correct path |
| Our ANLZ in ref location | **WORKS** | **WORKS** | **WORKS** | Data is valid |

---

## Next Steps

1. **Reverse-engineer the Pioneer ANLZ path algorithm**
   - Analyze more reference exports to find patterns
   - Check pyrekordbox/rekordcrate source for clues
   - Try different hash algorithms on various inputs

2. **Alternative approaches**
   - Store both our path AND compute Pioneer's expected path
   - Use a lookup table for known tracks

---

## Related Documentation

- **CLAUDE.md** - Project implementation guide
- **HISTORY.md** - Debugging session history

## References

- [Deep Symmetry - ANLZ Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html)
- [rekordcrate Library](https://holzhaus.github.io/rekordcrate/)
- [pyrekordbox](https://github.com/dylanljones/pyrekordbox)
- [Traktor-Bridge Issue #4](https://github.com/bsm3d/Traktor-Bridge/issues/4) - Same ANLZ path problem, confirms MD5 doesn't work
