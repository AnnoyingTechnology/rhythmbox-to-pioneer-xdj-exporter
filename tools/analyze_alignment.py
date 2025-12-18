#!/usr/bin/env python3
"""
Analyze row alignment patterns in reference PDB.
"""

import struct
from pathlib import Path

PAGE_SIZE = 4096
HEAP_START = 0x28

TABLE_TYPES = {
    0: "Tracks",
    1: "Genres",
    2: "Artists",
    3: "Albums",
    4: "Labels",
    5: "Keys",
    6: "Colors",
    7: "PlaylistTree",
    8: "PlaylistEntries",
    13: "Artwork",
    16: "Columns",
}

def read_u8(data, offset):
    return data[offset]

def read_u16(data, offset):
    return struct.unpack_from('<H', data, offset)[0]

def read_u32(data, offset):
    return struct.unpack_from('<I', data, offset)[0]

def parse_page_header(data, page_offset):
    return {
        'type': read_u32(data, page_offset + 0x08),
        'num_rows_small': read_u8(data, page_offset + 0x18),
        'used_size': read_u16(data, page_offset + 0x1e),
    }

def get_row_offsets(data, page_offset, num_rows):
    if num_rows == 0:
        return []

    num_groups = (num_rows + 15) // 16
    group_area_start = page_offset + PAGE_SIZE - (num_groups * 36)

    offsets = []
    for g in range(num_groups):
        group_offset = group_area_start + (g * 36)
        flags = read_u16(data, group_offset + 32)
        for i in range(16):
            if flags & (1 << i):
                off = read_u16(data, group_offset + (15 - i) * 2)
                offsets.append(off)

    return offsets[:num_rows]

def analyze_alignment(pdb_path):
    """Analyze row alignment for each table type"""
    data = Path(pdb_path).read_bytes()

    # Data pages to analyze
    data_pages = {
        2: "Tracks",
        4: "Genres",
        6: "Artists",
        8: "Albums",
        10: "Labels",
        12: "Keys",
        14: "Colors",
        16: "PlaylistTree",
        18: "PlaylistEntries",
        28: "Artwork",
        34: "Columns",
        51: "Tracks (pg 51)",
    }

    for page_idx, name in data_pages.items():
        page_offset = page_idx * PAGE_SIZE
        if page_offset >= len(data):
            continue

        hdr = parse_page_header(data, page_offset)
        num_rows = hdr['num_rows_small']

        if num_rows == 0:
            continue

        offsets = get_row_offsets(data, page_offset, num_rows)

        print(f"\n=== Page {page_idx}: {name} ({num_rows} rows) ===")
        print(f"Row offsets: {[hex(o) for o in offsets]}")

        # Calculate row sizes and check alignment
        row_sizes = []
        for i in range(len(offsets) - 1):
            row_sizes.append(offsets[i+1] - offsets[i])

        if row_sizes:
            print(f"Row sizes: {row_sizes}")

            # Check alignment
            alignments = set()
            for off in offsets:
                for align in [4, 8, 12, 16]:
                    if off % align == 0:
                        alignments.add(align)

            # Find the common alignment
            for align in [16, 8, 4]:
                if all(off % align == 0 for off in offsets):
                    print(f"Alignment: {align}-byte aligned")
                    break
            else:
                # Check if sizes follow a pattern
                if len(set(row_sizes)) == 1:
                    print(f"Fixed row size: {row_sizes[0]} bytes")
                else:
                    print(f"Variable row sizes, no strict alignment")

if __name__ == "__main__":
    import sys
    pdb = sys.argv[1] if len(sys.argv) > 1 else "/home/julien/Documents/Scripts/Pioneer/examples/PIONEER/rekordbox/export.pdb"
    analyze_alignment(pdb)
