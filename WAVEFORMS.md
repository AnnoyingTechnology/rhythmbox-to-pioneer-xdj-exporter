# Waveform Implementation

This document covers waveform generation for Pioneer USB exports.

## Current Status (2025-12-29)

**Status:** ROOT CAUSE IDENTIFIED - ANLZ Path Mismatch

The waveforms display correctly on **Rekordbox Desktop** but show as **FLATLINE on XDJ-XZ hardware**.

### Root Cause Discovery

**THE XDJ-XZ COMPUTES ITS OWN ANLZ PATH AND IGNORES THE `analyze_path` IN THE PDB!**

This was confirmed through the following tests:

| Test Variant | Result | Conclusion |
|--------------|--------|------------|
| Our export as-is | No waveforms | XDJ can't find ANLZ files |
| Reference export with exportExt.pdb deleted | Waveforms OK | XDJ doesn't need PDB for path |
| Reference export with OUR .EXT/.DAT files | Waveforms OK | Our ANLZ files are correct |
| Our export with reference USBANLZ folder copied | Waveforms OK | Path location is the issue |

**Key Evidence:**
- Reference path: `P04B/000154A5`
- Our computed path: `PDFB/00B97834`
- Both use the SAME audio file path: `/Contents/BROOKLYN BOUNCE/.../This Is The Begining.mp3`

The XDJ hardware looks for ANLZ files at a path computed from the audio file path, **NOT** from the `analyze_path` string stored in the track row.

---

## ANLZ Path Algorithm

### What We Know

The ANLZ path format is: `/PIONEER/USBANLZ/PXXX/YYYYYYYY/ANLZ0000.{DAT,EXT}`

Where:
- `XXX` = 3 hex digits (e.g., `04B`)
- `YYYYYYYY` = 8 hex digits (e.g., `000154A5`)

### What We've Ruled Out

| Algorithm | Our Result | Expected | Match? |
|-----------|------------|----------|--------|
| FNV-1a hash of file path | PDFB/00B97834 | P04B/000154A5 | No |
| CRC32 of UTF-8 path | 0xE85C7C53 | - | No |
| CRC32 of UTF-16LE path | 0xB68CE21F | - | No |

### What We Need

The exact algorithm Pioneer uses. Candidates to investigate:
1. Different hash algorithm (Adler-32, CRC-16, custom)
2. Input transformation (case folding, path normalization)
3. Rekordbox internal track ID from local database
4. Based on a field we haven't identified in the track row

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
