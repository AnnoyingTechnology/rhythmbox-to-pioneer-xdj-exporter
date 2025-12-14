Below is a **spec (not an implementation)** for a Linux-native “Rhythmbox → Pioneer USB (XDJ-XZ)” exporter that **keeps waveforms** and **computes musical key**, without Mixxx.

---

## Target outcome

Given a Rhythmbox library + playlists, produce a USB stick that the XDJ-XZ reads exactly like a Rekordbox-exported device:

* `/PIONEER/rekordbox/export.pdb` containing track + playlist metadata. ([DJ Link Ecosystem Analysis][1])
* `/PIONEER/USBANLZ/.../ANLZxxxx.DAT` and `.EXT` analysis files containing **beatgrid + waveforms** (and optionally cues/song structure). Analysis files are where Rekordbox stores the “too big for DB” data; later hardware adds `.EXT` for more detailed/colored waveforms. ([DJ Link Ecosystem Analysis][2])
* Audio files copied onto the stick, plus consistent paths referenced by the PDB rows.

> Pioneer staff themselves describe `USBANLZ` as holding waveform/beatgrid/cue data. ([Pioneer DJ Forums][3])

---

## Inputs

1. **Rhythmbox DB**

* `~/.local/share/rhythmbox/rhythmdb.xml` (tracks + tags) ([GNOME Wiki][4])
* `~/.local/share/rhythmbox/playlists.xml` (playlist membership/order) (commonly used; also widely referenced in practice) ([Ask Ubuntu][5])

2. **Audio files** referenced by Rhythmbox locations (file:// URIs).

3. **Export configuration**

* USB mount point
* “Copy audio vs. reference existing” (for Pioneer USB exports, **copy is the normal model**)
* Analysis fidelity targets (see below)
* Target player class: XDJ-XZ (assume needs `.DAT` + `.EXT`; `.2EX` likely not required unless you want CDJ-3000 3-band waveforms) ([DJ Link Ecosystem Analysis][2])

---

## Outputs and acceptance criteria

### A) Database export

Create `export.pdb` with at minimum:

* **tracks** table rows with metadata (title/artist/album/genre/length/file path/etc.) ([DJ Link Ecosystem Analysis][1])
* **keys** table + track key references (so the player can filter/match keys) ([DJ Link Ecosystem Analysis][1])
* **playlist tree** + **playlist entries** (names + ordering) (option to export a single playlist for testing, or all)
* Track row **analyze_path** string (string index 14 in Deep Symmetry doc) pointing to the matching ANLZ file path on USB. ([DJ Link Ecosystem Analysis][1])

**Acceptance:** a parser like `rekordcrate` can read the PDB and enumerate tracks + playlists. ([Holzhaus][6])

### B) Analysis files (non-negotiable per your requirement)

For each exported track create:

* `.DAT` with at least:

  * Beat grid section
  * Waveform preview sections (monochrome)
  * Waveform detail section (if required by XDJ-XZ display mode) 
* `.EXT` with:

  * Colored waveform preview + detail (or whatever subset XDJ-XZ expects) ([DJ Link Ecosystem Analysis][2])

**Acceptance:** `rekordcrate` can parse the generated `.DAT/.EXT` and extract waveform/beatgrid structures without error; the XDJ-XZ shows waveforms and beatgrid on load. ([Holzhaus][7])

---

## Audio analysis spec (what you must compute)

### 1) Decode + normalize

* Decode to PCM (44.1kHz or 48kHz as input demands; keep consistent internal rate).
* Normalize to consistent peak/RMS target (so waveform quantization matches expected dynamic range).

**Suggested libraries**

* FFmpeg/libav (decode)
* Rust: symphonia / ffmpeg-next (if writing in Rust)

### 2) Beatgrid (tempo + beat positions + downbeat)

* Estimate tempo (BPM)
* Track beat timestamps (beat phase consistency)
* Determine bar/downbeat if possible (nice-to-have; depends on device expectations)

**Suggested libraries**

* **Essentia** (broad MIR toolkit; includes rhythm-related algorithms) ([essentia.upf.edu][8])
* **librosa** beat tracker (Python) ([Librosa][9])
* **aubio** (tempo/beat/onset) ([aubio.org][10])

### 3) Musical key detection (must)

* Output: one global key estimate (e.g., “A minor”) + confidence
* Map to Rekordbox “key” table conventions used in `export.pdb` (Deep Symmetry describes keys table existence; exact encoding should be validated against known-good exports). ([DJ Link Ecosystem Analysis][1])

**Suggested libraries**

* **Essentia Key** algorithm (HPCP→key estimation) ([essentia.upf.edu][11])
* **libkeyfinder** (purpose-built DJ key detection) ([GitHub][12])

### 4) Waveform generation (must)

Generate the specific waveform representations expected by Pioneer analysis tags:

* **Waveform Preview** (fixed-width overview)
* **Tiny Waveform Preview**
* **Waveform Detail**
* **Waveform Color Preview**
* **Waveform Color Detail** 

You’re not inventing these: the tag structure and sections are documented/reversed. ([DJ Link Ecosystem Analysis][2])

**Core task:** replicate the *format* (binary tag layout + quantization), not Rekordbox’s exact DSP. If the player renders the waveform correctly, you’re done.

---

## File-format writing requirements

### PDB writer requirements

Implement writing, not just parsing.

Minimum:

* Fixed-size pages + row index structures + string offset arrays (the PDB is a custom paged DB). ([DJ Link Ecosystem Analysis][1])
* Correct table pointers + row presence flags behavior. ([DJ Link Ecosystem Analysis][1])
* Stable IDs and referential integrity between tables (tracks ↔ artists/albums/keys ↔ playlists).

**Useful references**

* Deep Symmetry “Database Exports” (structure, table types, track strings including analyze_path). ([DJ Link Ecosystem Analysis][1])
* `rekordcrate::pdb` (parser + type definitions you can mirror into a writer). ([Holzhaus][6])

### ANLZ writer requirements

* ANLZ files are **tagged sections** under a `PMAI` header; `.DAT/.EXT/.2EX` share structure, different tag sets. ([DJ Link Ecosystem Analysis][2])
* Write only the tag types needed for XDJ-XZ waveforms/beatgrid (start minimal, expand).

**Useful references**

* Deep Symmetry “Analysis Files” page + PDF sections listing beatgrid + waveform tags. ([DJ Link Ecosystem Analysis][2])
* `rekordcrate::anlz` module (structs/enums for beatgrid, waveforms, cues). ([Holzhaus][7])

---

## Validation plan (don’t skip)

1. **Golden stick diffing**

* Export one track with Rekordbox 5 (your existing working stick).
* Parse its PDB + ANLZ with `rekordcrate`, record:

  * required tags present
  * section sizes
  * waveform byte ranges

2. **Round-trip parse**

* Your generated stick must parse cleanly with `rekordcrate` for:

  * PDB tables
  * ANLZ sections ([Docs.rs][13])

3. **Hardware test**

* XDJ-XZ: confirm

  * playlists appear
  * tracks load
  * waveforms visible
  * key displayed and sortable/filterable

---

## Reality check (so you spec the right effort)

* “No waveforms” is easy; **“with waveforms + beatgrid + key”** means you’re effectively building a large chunk of Rekordbox’s export pipeline, *plus* two proprietary binary writers (PDB + ANLZ). The reverse-engineering docs make it feasible, but it’s not a weekend script. 
* The good news: for RB5-era device exports, the analysis tags you care about are documented and (for many tags) already modeled in `rekordcrate`. ([Holzhaus][7])

---

[1]: https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html "Database Exports :: DJ Link Ecosystem Analysis"
[2]: https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html "Analysis Files :: DJ Link Ecosystem Analysis"
[3]: https://forums.pioneerdj.com/hc/en-us/community/posts/360038839651-USBANLZ-is-12gb-help?utm_source=chatgpt.com "USBANLZ is 12gb - help!"
[4]: https://wiki.gnome.org/Apps%282f%29Rhythmbox%282f%29InternalDesign.html "Apps/Rhythmbox/InternalDesign – GNOME Wiki Archive"
[5]: https://askubuntu.com/questions/1555905/restore-rhythmbox-library?utm_source=chatgpt.com "24.04 - Restore rhythmbox library"
[6]: https://holzhaus.github.io/rekordcrate/rekordcrate/pdb/index.html "rekordcrate::pdb - Rust"
[7]: https://holzhaus.github.io/rekordcrate/rekordcrate/anlz/index.html "rekordcrate::anlz - Rust"
[8]: https://essentia.upf.edu/?utm_source=chatgpt.com "Homepage — Essentia 2.1-beta6-dev documentation"
[9]: https://librosa.org/doc/main/generated/librosa.beat.beat_track.html?utm_source=chatgpt.com "librosa.beat.beat_track — librosa 0.11.0 documentation"
[10]: https://aubio.org/?utm_source=chatgpt.com "aubio, a library for audio labelling"
[11]: https://essentia.upf.edu/reference/streaming_Key.html?utm_source=chatgpt.com "Key — Essentia 2.1-beta6-dev documentation"
[12]: https://github.com/mixxxdj/libkeyfinder?utm_source=chatgpt.com "mixxxdj/libkeyfinder: Musical key detection for digital audio, ..."
[13]: https://docs.rs/rekordcrate?utm_source=chatgpt.com "rekordcrate - Rust"
