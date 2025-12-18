#!/usr/bin/env python3
"""
Check num_rows_small vs num_rows_large across all data pages in reference.
"""

import struct
from pathlib import Path

PAGE_SIZE = 4096

def read_u8(data, offset):
    return data[offset]

def read_u16(data, offset):
    return struct.unpack_from('<H', data, offset)[0]

def read_u32(data, offset):
    return struct.unpack_from('<I', data, offset)[0]

TABLE_NAMES = {
    0: "Tracks", 1: "Genres", 2: "Artists", 3: "Albums", 4: "Labels",
    5: "Keys", 6: "Colors", 7: "PlaylistTree", 8: "PlaylistEntries",
    13: "Artwork", 16: "Columns", 17: "HistoryPlaylists", 18: "HistoryEntries", 19: "History"
}

def analyze_rows(pdb_path):
    data = Path(pdb_path).read_bytes()
    num_pages = len(data) // PAGE_SIZE

    print(f"Analyzing {pdb_path}")
    print(f"Page  Type              rows_s  rows_l  used    free")
    print("-" * 60)

    for page_idx in range(num_pages):
        page_offset = page_idx * PAGE_SIZE
        table_type = read_u32(data, page_offset + 0x08)
        num_rows_small = read_u8(data, page_offset + 0x18)
        page_flags = read_u8(data, page_offset + 0x1b)
        free_size = read_u16(data, page_offset + 0x1c)
        used_size = read_u16(data, page_offset + 0x1e)
        num_rows_large = read_u16(data, page_offset + 0x22)

        # Skip header pages (page_flags & 0x40) and empty pages
        if page_flags & 0x40:
            continue
        if used_size == 0 and num_rows_small == 0:
            continue

        table_name = TABLE_NAMES.get(table_type, f"Type{table_type}")
        rows_match = "OK" if num_rows_small == num_rows_large else "DIFF"

        print(f"{page_idx:4d}  {table_name:16s}  {num_rows_small:5d}  {num_rows_large:5d}  {used_size:#06x}  {free_size:#06x}  {rows_match}")

if __name__ == "__main__":
    print("=== Reference Export ===")
    analyze_rows("/home/julien/Documents/Scripts/Pioneer/examples/PIONEER/rekordbox/export.pdb")
    print("\n=== Our Export ===")
    analyze_rows("/tmp/pioneer_test/PIONEER/rekordbox/export.pdb")
