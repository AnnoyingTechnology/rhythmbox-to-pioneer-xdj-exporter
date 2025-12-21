#!/usr/bin/env python3
"""Analyze track row string offsets and data."""

import sys
import struct

def decode_devicesql_string(data, offset):
    """Decode a DeviceSQL string at the given offset."""
    if offset >= len(data):
        return "(out of bounds)"

    header = data[offset]

    # Check if short ASCII (odd header byte)
    if header & 1:
        length = (header >> 1) - 1
        if length <= 0:
            return "(empty)"
        try:
            return data[offset + 1 : offset + 1 + length].decode('ascii', errors='replace')
        except:
            return f"(decode error, len={length})"
    else:
        # Long string format
        if offset + 4 > len(data):
            return "(long header out of bounds)"
        flags = data[offset]
        length_bytes = data[offset + 1 : offset + 4]
        length = int.from_bytes(length_bytes, 'little') - 4
        if flags == 0x40:
            # Long ASCII
            try:
                return data[offset + 4 : offset + 4 + length].decode('ascii', errors='replace')
            except:
                return f"(long ascii decode error)"
        elif flags == 0x90:
            # Long UTF-16LE
            try:
                return data[offset + 4 : offset + 4 + length].decode('utf-16-le', errors='replace')
            except:
                return f"(utf16 decode error)"
        else:
            return f"(unknown flags: {flags:#x})"

STRING_NAMES = [
    "isrc", "lyricist", "unknown2", "unknown3", "unknown4",
    "message", "publish_track_info", "autoload_hotcues", "unknown8", "unknown9",
    "date_added", "release_date", "mix_name", "unknown13", "analyze_path",
    "analyze_date", "comment", "title", "unknown18", "filename",
    "file_path"
]

def analyze_track_row(data, row_offset, label):
    """Analyze a single track row."""
    print(f"\n=== {label} Track Row ===")

    row = data[row_offset:]

    # Header fields (first 94 bytes)
    subtype = struct.unpack_from('<H', row, 0x00)[0]
    index_shift = struct.unpack_from('<H', row, 0x02)[0]
    bitmask = struct.unpack_from('<I', row, 0x04)[0]
    sample_rate = struct.unpack_from('<I', row, 0x08)[0]
    composer_id = struct.unpack_from('<I', row, 0x0c)[0]
    file_size = struct.unpack_from('<I', row, 0x10)[0]
    u2 = struct.unpack_from('<I', row, 0x14)[0]
    artwork_id = struct.unpack_from('<I', row, 0x1c)[0]
    key_id = struct.unpack_from('<I', row, 0x20)[0]
    bitrate = struct.unpack_from('<I', row, 0x30)[0]
    track_number = struct.unpack_from('<I', row, 0x34)[0]
    tempo = struct.unpack_from('<I', row, 0x38)[0]
    genre_id = struct.unpack_from('<I', row, 0x3c)[0]
    album_id = struct.unpack_from('<I', row, 0x40)[0]
    artist_id = struct.unpack_from('<I', row, 0x44)[0]
    track_id = struct.unpack_from('<I', row, 0x48)[0]
    duration = struct.unpack_from('<H', row, 0x54)[0]
    file_type = struct.unpack_from('<H', row, 0x5a)[0]

    print(f"  subtype: {subtype:#06x}")
    print(f"  bitmask: {bitmask:#010x}")
    print(f"  sample_rate: {sample_rate}")
    print(f"  file_size: {file_size}")
    print(f"  artwork_id: {artwork_id}")
    print(f"  tempo: {tempo} (BPM: {tempo/100:.2f})")
    print(f"  genre_id: {genre_id}, album_id: {album_id}, artist_id: {artist_id}")
    print(f"  track_id: {track_id}")
    print(f"  duration: {duration}s")
    print(f"  file_type: {file_type}")

    # String offsets (21 x u16 at offset 0x5e)
    print(f"\n  String Offsets (21 @ 0x5e):")
    string_offsets = []
    for i in range(21):
        offset = struct.unpack_from('<H', row, 0x5e + i * 2)[0]
        string_offsets.append(offset)

    # Decode each string
    for i, offset in enumerate(string_offsets):
        name = STRING_NAMES[i] if i < len(STRING_NAMES) else f"string{i}"
        value = decode_devicesql_string(row, offset)
        # Show all strings for debugging
        print(f"    [{i:2d}] {name:20s}: offset={offset:#06x} -> {value!r}")

def main():
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <reference.pdb> <our.pdb>")
        sys.exit(1)

    ref_path, our_path = sys.argv[1], sys.argv[2]

    with open(ref_path, 'rb') as f:
        ref_data = f.read()

    with open(our_path, 'rb') as f:
        our_data = f.read()

    # Track page 2 starts at offset 0x2000, row data at 0x28
    track_row_offset = 0x2000 + 0x28

    analyze_track_row(ref_data, track_row_offset, "Reference")
    analyze_track_row(our_data, track_row_offset, "Our Export")

if __name__ == "__main__":
    main()
