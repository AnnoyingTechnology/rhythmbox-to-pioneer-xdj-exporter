#!/usr/bin/env python3
"""
Analyze header page content after the 40-byte page header.
"""

import struct
from pathlib import Path

PAGE_SIZE = 4096
HEADER_SIZE = 0x28

def read_u8(data, offset):
    return data[offset]

def read_u16(data, offset):
    return struct.unpack_from('<H', data, offset)[0]

def read_u32(data, offset):
    return struct.unpack_from('<I', data, offset)[0]

def hex_bytes(data, start, length):
    return ' '.join(f'{data[start + i]:02x}' for i in range(min(length, len(data) - start)))

def analyze_header_pages(pdb_path):
    """Check header pages for extra content"""
    data = Path(pdb_path).read_bytes()

    # Header pages (based on TABLE_LAYOUTS)
    header_pages = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35, 37, 39]

    table_names = {
        0: "Tracks", 1: "Genres", 2: "Artists", 3: "Albums", 4: "Labels",
        5: "Keys", 6: "Colors", 7: "PlaylistTree", 8: "PlaylistEntries",
        9: "Unknown09", 10: "Unknown0A", 11: "Unknown0B", 12: "Unknown0C",
        13: "Artwork", 14: "Unknown0E", 15: "Unknown0F", 16: "Columns",
        17: "HistoryPlaylists", 18: "HistoryEntries", 19: "History"
    }

    for p in header_pages:
        page_offset = p * PAGE_SIZE
        if page_offset >= len(data):
            continue

        page_type = read_u32(data, page_offset + 0x08)
        table_name = table_names.get(page_type, f"Type{page_type}")

        # Check if there's non-zero content after header
        post_header = data[page_offset + HEADER_SIZE:page_offset + HEADER_SIZE + 32]
        if any(b != 0 for b in post_header):
            print(f"Page {p} ({table_name}): has content after header")
            print(f"  Header+0x00: {hex_bytes(data, page_offset + HEADER_SIZE, 16)}")
            print(f"  Header+0x10: {hex_bytes(data, page_offset + HEADER_SIZE + 16, 16)}")

            # Try to interpret the structure
            # It looks like: u32, u32, and then some pattern
            v1 = read_u32(data, page_offset + HEADER_SIZE)
            v2 = read_u32(data, page_offset + HEADER_SIZE + 4)
            v3 = read_u32(data, page_offset + HEADER_SIZE + 8)
            v4 = read_u32(data, page_offset + HEADER_SIZE + 12)
            print(f"  Values: {v1:#x}, {v2:#x}, {v3:#x}, {v4:#x}")
        else:
            print(f"Page {p} ({table_name}): header page is empty after header")

if __name__ == "__main__":
    import sys

    print("=== Reference Export ===")
    analyze_header_pages("/home/julien/Documents/Scripts/Pioneer/examples/PIONEER/rekordbox/export.pdb")

    print("\n=== Our Export ===")
    analyze_header_pages("/tmp/pioneer_test/PIONEER/rekordbox/export.pdb")
