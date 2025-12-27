#!/usr/bin/env python3
"""
Track Row Analyzer - Parse and compare track rows in PDB files
"""

import struct
import sys
from pathlib import Path

PAGE_SIZE = 4096
HEAP_START = 0x28

# Track row field definitions (offset from row start, size, name)
TRACK_FIELDS = [
    (0x00, 2, "subtype"),
    (0x02, 2, "index_shift"),
    (0x04, 4, "bitmask"),
    (0x08, 4, "sample_rate"),
    (0x0c, 4, "composer_id"),
    (0x10, 4, "file_size"),
    (0x14, 4, "u2"),
    (0x18, 2, "u3"),
    (0x1a, 2, "u4"),
    (0x1c, 4, "artwork_id"),
    (0x20, 4, "key_id"),
    (0x24, 4, "orig_artist_id"),
    (0x28, 4, "label_id"),
    (0x2c, 4, "remixer_id"),
    (0x30, 4, "bitrate"),
    (0x34, 4, "track_number"),
    (0x38, 4, "tempo"),
    (0x3c, 4, "genre_id"),
    (0x40, 4, "album_id"),
    (0x44, 4, "artist_id"),
    (0x48, 4, "track_id"),
    (0x4c, 2, "disc_number"),
    (0x4e, 2, "play_count"),
    (0x50, 2, "year"),
    (0x52, 2, "sample_depth"),
    (0x54, 2, "duration"),
    (0x56, 2, "u5"),
    (0x58, 1, "color_id"),
    (0x59, 1, "rating"),
    (0x5a, 2, "file_type"),
    (0x5c, 2, "u7"),
]


def parse_track_row(data, offset):
    """Parse a track row and return field values"""
    result = {}
    for field_offset, size, name in TRACK_FIELDS:
        pos = offset + field_offset
        if size == 1:
            result[name] = data[pos]
        elif size == 2:
            result[name] = struct.unpack_from('<H', data, pos)[0]
        elif size == 4:
            result[name] = struct.unpack_from('<I', data, pos)[0]
    return result


def compare_track_rows(data1, data2, page_idx, row_offset):
    """Compare track rows and show differences"""
    offset1 = page_idx * PAGE_SIZE + HEAP_START + row_offset
    offset2 = page_idx * PAGE_SIZE + HEAP_START + row_offset

    fields1 = parse_track_row(data1, offset1)
    fields2 = parse_track_row(data2, offset2)

    print(f"\n{'Field':<20} {'File1':<15} {'File2':<15} {'Diff?'}")
    print("=" * 60)

    differences = []
    for field_offset, size, name in TRACK_FIELDS:
        v1 = fields1[name]
        v2 = fields2[name]
        diff = "***" if v1 != v2 else ""
        if v1 != v2:
            differences.append((name, v1, v2, field_offset))

        if size == 4:
            print(f"{name:<20} 0x{v1:08x}     0x{v2:08x}     {diff}")
        elif size == 2:
            print(f"{name:<20} 0x{v1:04x}         0x{v2:04x}         {diff}")
        else:
            print(f"{name:<20} 0x{v1:02x}           0x{v2:02x}           {diff}")

    if differences:
        print(f"\n{len(differences)} field(s) differ:")
        for name, v1, v2, off in differences:
            print(f"  {name} (row offset 0x{off:02x}): {v1} vs {v2}")

    return differences


def main():
    if len(sys.argv) < 3:
        print("Usage: track_row_analyzer.py <file1.pdb> <file2.pdb> [page] [row_offset]")
        sys.exit(1)

    path1 = Path(sys.argv[1])
    path2 = Path(sys.argv[2])
    page_idx = int(sys.argv[3]) if len(sys.argv) > 3 else 2
    row_offset = int(sys.argv[4], 0) if len(sys.argv) > 4 else 0

    data1 = path1.read_bytes()
    data2 = path2.read_bytes()

    print(f"Comparing track row at page {page_idx}, row offset 0x{row_offset:04x}")
    print(f"File 1: {path1}")
    print(f"File 2: {path2}")

    compare_track_rows(data1, data2, page_idx, row_offset)


if __name__ == '__main__':
    main()
