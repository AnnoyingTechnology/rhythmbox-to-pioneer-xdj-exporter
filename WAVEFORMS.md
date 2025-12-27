# Waveform Implementation

This document covers waveform generation for Pioneer USB exports.

## Current Status (2025-12-27)

**Status:** UNSOLVED - Waveforms display as FLATLINE on XDJ-XZ

The ANLZ files are structurally correct and byte-for-byte identical to reference when injected.
The problem is in the **PDB file** - some field tells the XDJ to display flatline.

---

## XDJ-XZ Hardware Test Results

| Test Variant | Needle Search | Jogwheel | Main Screen | Notes |
|--------------|---------------|----------|-------------|-------|
| pwv5 only | Nothing | Nothing | Nothing | PWV5 alone insufficient |
| pwv4 only | Nothing | Nothing | Nothing | PWV4 alone insufficient |
| pwv3 only | **WORKS** | **WORKS** | Nothing | Shows FLATLINE |
| all-ext | Nothing | Nothing | Nothing | Multiple replacements broke it |
| all-waveforms | **WORKS** | **WORKS** | Nothing | Shows FLATLINE |
| ref-anlz | **WORKS** | **WORKS** | Nothing | Shows FLATLINE |
| Complete ref export | **WORKS** | **WORKS** | **WORKS** | Proper dynamic waveform |

**Key Findings:**
1. **PWV3 controls needle search + jogwheel** on XDJ-XZ (not PWV4 or PWV5)
2. **PWV5 controls main screen waveform** - doesn't work in any hybrid export
3. **FLATLINE issue**: Even with reference PWV3 data, XDJ shows flatline instead of proper waveform
4. Complete reference export works fully - issue is in our **PDB file**, not ANLZ files

---

## Investigation Summary

### What Has Been Verified

1. **ANLZ File Structure** - IDENTICAL to reference
   - Same section order, sizes, headers
   - PWV3 at offset 0x82 with correct entry count
   - Section headers match byte-for-byte

2. **Waveform Data** - IDENTICAL when injected
   ```
   Reference PWV3 @ 0x9a: e0e0 e0a0 80a0 c060 a0e0 e0e1 e1e2...
   Injected PWV3 @ 0x9a: e0e0 e0a0 80a0 c060 a0e0 e0e1 e1e2... (SAME)
   ```

3. **Track Row Waveform Fields** - IDENTICAL
   | Offset | Field | Ours | Reference |
   |--------|-------|------|-----------|
   | 0x08 | sample_rate | 44100 | 44100 |
   | 0x52 | sample_depth | 16 | 16 |
   | 0x54 | duration_secs | 1 | 1 |

4. **String Offsets** - All 21 offsets IDENTICAL

5. **analyze_path** - Both point to valid ANLZ locations
   ```
   Ours: /PIONEER/USBANLZ/P9CC/00C30E0D/ANLZ0000.DAT
   Ref:  /PIONEER/USBANLZ/P05C/0001D2C0/ANLZ0000.DAT
   ```

### Known Differences (May or May Not Affect Waveforms)

| Offset | Field | Ours | Reference | Notes |
|--------|-------|------|-----------|-------|
| 0x10 | file_size | 30491 | 34857 | Different audio files |
| 0x14 | u2 | 21 | 44 | track_id + 20 |
| 0x20 | key_id | 0 | 1 | We don't detect key |
| 0x30 | bitrate | 320 | 192 | Hardcoded value |

### Root Cause Hypothesis

The flatline with reference ANLZ data means our PDB is causing the XDJ to misinterpret the waveform.

Possible causes:
- Some unknown field indicates "waveform not analyzed"
- A field affecting waveform interpretation we haven't identified
- Something in the track row structure beyond the fields we've compared

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

## Next Steps for Expert Consultation

See `help/ask.md` for comprehensive context to share with experts.

Key questions:
1. What PDB field could cause FLATLINE despite valid ANLZ data?
2. Is there an "analyzed" flag in the track row?
3. Do any unknown fields affect waveform display?

---

## Historical Issues (Resolved)

### PWV4 Generation Bug (Fixed 2025-12-25)
**Root cause:** `generate_pwv4()` function existed but was never called. The code returned an empty Vec.

### StubAnalyzer Bug (Fixed)
When using `--no-bpm --no-key`, the StubAnalyzer was returning `WaveformData::minimal_stub()` (empty vectors) instead of calling `generate_waveforms()`.

### Height Normalization Fix (Fixed 2025-12-27)
All waveform generators now receive `overall_peak` parameter and normalize heights to full 0-31 range.

### Height Floor Assumption (INCORRECT - Reverted 2025-12-27)
Previous changes added height floors (.max(1)) based on incomplete analysis. Reference data includes height 0, so floors were removed.

---

## References

- [Deep Symmetry - ANLZ Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html)
- [rekordbox_anlz.ksy](https://github.com/Deep-Symmetry/crate-digger/blob/main/src/main/kaitai/rekordbox_anlz.ksy)
