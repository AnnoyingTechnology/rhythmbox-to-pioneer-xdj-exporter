# Waveform Implementation

This document covers waveform generation for Pioneer USB exports.

## Current Status (2025-12-26)

**Status:** NOT working on XDJ-XZ

### What Works
- Some monochrome preview display in Rekordbox 5 (when the export is not corrupted).
- When swapping with our ANLZ000.EXT in a reference export, we have ONE waveform displayed on the XDJ

### Known Limitations
- PWV5 colors are all white (no frequency-based coloring)
- Heights may be lower than Rekordbox-generated waveforms

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
```

### PWV4 (Color Preview - 3 Frequency Bands)
Each 6-byte entry has 3 columns for frequency bands (low/mid/high):
- Bytes 0-1: Low frequency (height 0-31, whiteness 0xF0-0xFF)
- Bytes 2-3: Mid frequency (height, whiteness)
- Bytes 4-5: High frequency (height, whiteness)

### PWV5 (Color Detail)
```
RGB (3 bits each) | height (5 bits)
```

---

## Implementation Details

### Audio Processing
- Uses `symphonia` to decode audio to mono samples
- Calculates RMS and peak per time window
- Height from peak amplitude (0-31 range for 5-bit fields)

### Key Files
- `src/analysis/waveform.rs` - Waveform generation
- `src/anlz/writer.rs` - ANLZ file writing (contains waveform sections)

---

## Historical Issues (Resolved)

### PWV4 Generation Bug (Fixed 2025-12-25)
**Root cause:** `generate_pwv4()` function existed but was never called. The code returned an empty Vec.

```rust
// OLD CODE - BROKEN:
let color_preview = Vec::new(); // Returns empty

// NEW CODE - FIXED:
let color_preview = generate_pwv4(&samples, sample_rate);
```

### StubAnalyzer Bug (Fixed)
When using `--no-bpm --no-key`, the StubAnalyzer was returning `WaveformData::minimal_stub()` (empty vectors) instead of calling `generate_waveforms()`.

### Whiteness/Height Fix
- PWAV now uses whiteness=5 (was 7)
- PWV3 now uses whiteness=7 (was 5)

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
4. PWV4 (color preview)
5. PWV5 (color detail)

---

## Test Results (2025-12-25)

| Test | Result | Conclusion |
|------|--------|------------|
| Remove `exportExt.pdb` | Works | exportExt.pdb NOT required |
| Remove `ANLZ0000.EXT` | Broken | EXT file is CRITICAL |
| Remove `ANLZ0000.DAT` | Works | DAT is secondary/optional |
| Swap DAT with ours | Works | Our DAT is valid or not relevant |
| Swap EXT with ours | Partial | Main screen waveform works, needle search waveform missing |

---

## Future Improvements

- [ ] PWV5 frequency-based coloring (lows=red, highs=blue)
- [ ] Height scaling to match Rekordbox ranges
- [ ] PWV6/PWV7 3-band waveforms (CDJ-3000)
- [ ] Log scaling for visual style

---

## References

- [Deep Symmetry - ANLZ Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html)
- [rekordbox_anlz.ksy](https://github.com/Deep-Symmetry/crate-digger/blob/main/src/main/kaitai/rekordbox_anlz.ksy)
