#!/usr/bin/env python3
"""
Check row group flags vs num_rows_large to find the pattern.
"""

import struct
from pathlib import Path

PAGE_SIZE = 4096
HEAP_START = 0x28

def read_u8(data, offset):
    return data[offset]

def read_u16(data, offset):
    return struct.unpack_from('<H', data, offset)[0]

def read_u32(data, offset):
    return struct.unpack_from('<I', data, offset)[0]

def get_row_group_info(data, page_offset, num_rows):
    """Get row group flags and count present rows"""
    if num_rows == 0:
        return 0, 0, []

    num_groups = (num_rows + 15) // 16
    group_area_start = page_offset + PAGE_SIZE - (num_groups * 36)

    total_present = 0
    group_info = []

    for g in range(num_groups):
        group_offset = group_area_start + (g * 36)
        flags = read_u16(data, group_offset + 32)
        unknown = read_u16(data, group_offset + 34)
        present = bin(flags).count('1')
        total_present += present
        group_info.append((flags, unknown, present))

    return num_groups, total_present, group_info

def analyze(pdb_path):
    data = Path(pdb_path).read_bytes()

    # Data pages to check
    pages = [
        (2, "Tracks"),
        (4, "Genres"),
        (6, "Artists"),
        (8, "Albums"),
        (10, "Labels"),
        (12, "Keys"),
        (14, "Colors"),
        (16, "PlaylistTree"),
        (18, "PlaylistEntries"),
        (28, "Artwork"),
        (34, "Columns"),
        (36, "HistoryPlaylists"),
        (38, "HistoryEntries"),
        (40, "History"),
        (51, "Tracks2"),
    ]

    print(f"Analyzing {pdb_path}")
    print(f"Page  Table             rows_s  rows_l  groups  present  flags")
    print("-" * 75)

    for page_idx, name in pages:
        page_offset = page_idx * PAGE_SIZE
        if page_offset >= len(data):
            continue

        num_rows_small = read_u8(data, page_offset + 0x18)
        num_rows_large = read_u16(data, page_offset + 0x22)

        if num_rows_small == 0:
            continue

        num_groups, total_present, group_info = get_row_group_info(data, page_offset, num_rows_small)

        flags_str = ', '.join(f'{g[0]:#06x}' for g in group_info)

        print(f"{page_idx:4d}  {name:16s}  {num_rows_small:5d}  {num_rows_large:5d}  {num_groups:6d}  {total_present:7d}  {flags_str}")

if __name__ == "__main__":
    print("=== Reference Export ===")
    analyze("/home/julien/Documents/Scripts/Pioneer/examples/PIONEER/rekordbox/export.pdb")
