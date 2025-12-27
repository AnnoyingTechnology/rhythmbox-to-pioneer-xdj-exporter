#!/usr/bin/env python3
"""
Detailed PDB page comparison - shows byte-by-byte differences with context
"""

import struct
import sys
from pathlib import Path

PAGE_SIZE = 4096

TABLE_NAMES = {
    0: "Tracks",
    1: "Genres",
    2: "Artists",
    3: "Albums",
    4: "Labels",
    5: "Keys",
    6: "Colors",
    7: "PlaylistTree",
    8: "PlaylistEntries",
    9: "Unknown09",
    10: "Unknown0A",
    11: "Unknown0B",
    12: "Unknown0C",
    13: "Artwork",
    14: "Unknown0E",
    15: "Unknown0F",
    16: "Columns",
    17: "HistoryPlaylists",
    18: "HistoryEntries",
    19: "History",
}


def describe_offset(offset, page_idx):
    """Try to describe what a page offset represents"""
    if offset < 0x28:
        # Page header
        fields = {
            0x00: "padding",
            0x04: "page_index",
            0x08: "table_type",
            0x0c: "next_page",
            0x10: "unknown1",
            0x14: "unknown2",
            0x18: "num_rows_small",
            0x19: "unknown3",
            0x1a: "unknown4",
            0x1b: "page_flags",
            0x1c: "free_size",
            0x1e: "used_size",
            0x20: "unknown5",
            0x22: "num_rows_large",
            0x24: "unknown6",
            0x26: "unknown7",
        }
        for field_off, name in sorted(fields.items(), reverse=True):
            if offset >= field_off:
                return f"header.{name}+{offset - field_off}"

    return f"heap+0x{offset - 0x28:03x}"


def compare_pages_detailed(data1, data2, page_idx):
    """Compare a specific page in detail"""
    off = page_idx * PAGE_SIZE

    if off + PAGE_SIZE > len(data1) or off + PAGE_SIZE > len(data2):
        print(f"Page {page_idx}: One or both files don't have this page")
        return

    page1 = data1[off:off + PAGE_SIZE]
    page2 = data2[off:off + PAGE_SIZE]

    if page1 == page2:
        print(f"Page {page_idx}: IDENTICAL")
        return

    # Get table type for context
    table_type = struct.unpack_from('<I', page1, 0x08)[0]
    table_name = TABLE_NAMES.get(table_type, f"Type{table_type}")

    # Find all differences
    diffs = []
    for i in range(PAGE_SIZE):
        if page1[i] != page2[i]:
            diffs.append((i, page1[i], page2[i]))

    print(f"\n{'='*70}")
    print(f"Page {page_idx} ({table_name}): {len(diffs)} byte differences")
    print(f"{'='*70}")

    # Group consecutive differences
    groups = []
    current_group = None

    for offset, b1, b2 in diffs:
        if current_group is None or offset > current_group[-1][0] + 4:
            current_group = []
            groups.append(current_group)
        current_group.append((offset, b1, b2))

    for group in groups:
        start_offset = group[0][0]
        end_offset = group[-1][0]

        # Show context (a few bytes before and after)
        ctx_start = max(0, start_offset - 8)
        ctx_end = min(PAGE_SIZE, end_offset + 8)

        desc = describe_offset(start_offset, page_idx)
        print(f"\n  Offset 0x{start_offset:04x} ({desc}):")

        # Show hex dump with differences highlighted
        for chunk_start in range(ctx_start, ctx_end, 16):
            chunk_end = min(chunk_start + 16, ctx_end)

            hex1 = ""
            hex2 = ""
            diff_markers = ""

            for i in range(chunk_start, chunk_end):
                b1 = page1[i]
                b2 = page2[i]

                if b1 != b2:
                    hex1 += f" [{b1:02x}]"
                    hex2 += f" [{b2:02x}]"
                else:
                    hex1 += f"  {b1:02x} "
                    hex2 += f"  {b2:02x} "

            if any(page1[i] != page2[i] for i in range(chunk_start, chunk_end)):
                print(f"    0x{chunk_start:04x}: File1: {hex1}")
                print(f"    0x{chunk_start:04x}: File2: {hex2}")


def main():
    if len(sys.argv) < 3:
        print("Usage: pdb_diff_detail.py <file1.pdb> <file2.pdb> [page_num...]")
        sys.exit(1)

    path1 = Path(sys.argv[1])
    path2 = Path(sys.argv[2])

    data1 = path1.read_bytes()
    data2 = path2.read_bytes()

    # Specific pages to compare, or find all differing pages
    if len(sys.argv) > 3:
        pages = [int(arg) for arg in sys.argv[3:]]
    else:
        num_pages = min(len(data1), len(data2)) // PAGE_SIZE
        pages = []
        for p in range(num_pages):
            off = p * PAGE_SIZE
            if data1[off:off + PAGE_SIZE] != data2[off:off + PAGE_SIZE]:
                pages.append(p)

    print(f"Comparing: {path1} vs {path2}")
    print(f"File sizes: {len(data1)} vs {len(data2)} bytes")
    print(f"Pages with differences: {pages}")

    for page_idx in pages:
        compare_pages_detailed(data1, data2, page_idx)


if __name__ == '__main__':
    main()
