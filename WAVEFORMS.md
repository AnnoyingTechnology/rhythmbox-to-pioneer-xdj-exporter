# Waveform Implementation

This document covers waveform generation for Pioneer USB exports.

## Current Status (2025-12-27)

**Status:** TESTING NEEDED on XDJ-XZ (Height normalization fixed)

### What Works
- Monochrome preview display in Rekordbox 5
- Main screen waveform displays when swapping EXT in reference export

### Recent Fixes (2025-12-27)

#### 1. Height Normalization Fix
**Root cause:** Heights were using raw peak values (0.0-1.0) instead of normalizing to the track's maximum peak.
- Before: Heights 1-11 (compressed, flat appearance)
- After: Heights 0-31 (full dynamic range)

**Solution:** All waveform generators now receive `overall_peak` parameter and normalize heights relative to it, ensuring the loudest part of the track reaches max height.

#### 2. PWV4 Color Encoding Fix
Fixed PWV4 (color preview) encoding for needle search waveform:
- Height range: Changed from 0-31 (5-bit) to 0-127 (full 8-bit)
- Color encoding: Low freq now uses HIGH color values (0xE0-0xFF), Mid/High use LOW values (0x01-0x30)

#### 3. PWV5 Encoding Fix
Fixed PWV5 byte order - height was in wrong byte position:
- Byte 0: `(blue_low3 << 5) | (height & 0x1f)` - height in LOW 5 bits
- Byte 1: `(red_3bits << 5) | (green_3bits << 2) | blue_high2`

#### 4. PWV5 Height Floor Fix
Rekordbox uses a minimum height floor of 12 for PWV5:
- Before: Heights 0-31 (our output had values below 12)
- After: Heights 12-31 (matching Rekordbox behavior)
- Formula: `height = 12 + (normalized_peak * 19)`

### Known Limitations
- PWV5 colors use crest-factor based coloring (may differ from Rekordbox)
- Heights normalized to our audio analysis (slight differences from Rekordbox)

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
- Bytes 0-1: Low frequency (height 0-127, color 0xE0-0xFF = bright)
- Bytes 2-3: Mid frequency (height 0-127, color 0x01-0x30 = dim)
- Bytes 4-5: High frequency (height 0-127, color 0x01-0x20 = dimmer)

Height uses FULL 8-bit range (0-127 typical), NOT 5-bit like PWAV/PWV3.

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

### PWV4 Format Fix (Fixed 2025-12-27)
**Root cause:** PWV4 was using wrong encoding - heights capped at 31 and all colors set to 0xF0+.
Reference analysis showed:
- Heights should be 0-127 (full 8-bit), not 0-31
- Low frequency color should be HIGH (0xE0-0xFF = bright)
- Mid/High frequency colors should be LOW (0x01-0x30 = dim)

### Height Normalization Fix (Fixed 2025-12-27)
**Root cause:** All waveform generators were using raw peak amplitude values (0.0-1.0) directly instead of normalizing to the track's maximum peak.

For the Fresh.mp3 test track:
- Decoded peak amplitude: 0.3725
- Before fix: heights 1-11 (0.3725 * 31 = 11)
- After fix: heights 0-31 (normalized so 0.3725 maps to 31)

**Analysis comparing reference vs ours:**
```
Reference PWV3 heights: 0-31 range (loudest parts hit 31)
Our PWV3 heights before: 1-11 range (compressed, flat)
Our PWV3 heights after: 0-31 range (full dynamic range)
```

**Fix:** All waveform generators now receive `overall_peak` parameter and calculate:
```rust
let normalized_peak = peak / overall_peak;
let height = (normalized_peak * MAX_HEIGHT as f32) as u8;
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
