#!/usr/bin/env python3
"""Compare PDB headers between reference and our export."""

import sys
import struct

def parse_header(data):
    """Parse the PDB header and return table info."""
    # Header structure
    # 0x00-0x03: unknown (always 0)
    # 0x04-0x07: page_size (4096 = 0x1000)
    # 0x08-0x0b: num_tables (20 = 0x14)
    # 0x0c-0x0f: next_unused_page
    # 0x10-0x13: unknown
    # 0x14-0x17: sequence
    # 0x18-0x1b: gap (always 0)
    # 0x1c+: table entries (16 bytes each)

    page_size = struct.unpack_from('<I', data, 0x04)[0]
    num_tables = struct.unpack_from('<I', data, 0x08)[0]
    next_unused = struct.unpack_from('<I', data, 0x0c)[0]
    sequence = struct.unpack_from('<I', data, 0x14)[0]

    print(f"  Page size: {page_size}")
    print(f"  Num tables: {num_tables}")
    print(f"  Next unused page: {next_unused}")
    print(f"  Sequence: {sequence}")

    tables = []
    offset = 0x1c
    for i in range(num_tables):
        empty_candidate = struct.unpack_from('<I', data, offset)[0]
        first_page = struct.unpack_from('<I', data, offset + 4)[0]
        last_page = struct.unpack_from('<I', data, offset + 8)[0]
        table_type = struct.unpack_from('<I', data, offset + 12)[0]
        tables.append({
            'type': table_type,
            'first': first_page,
            'last': last_page,
            'empty_candidate': empty_candidate
        })
        offset += 16

    return tables

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
    9: "Unknown9",
    10: "Unknown10",
    11: "Unknown11",
    12: "Unknown12",
    13: "Artwork",
    14: "Unknown14",
    15: "Unknown15",
    16: "Columns",
    17: "HistoryPlaylists",
    18: "HistoryEntries",
    19: "History",
}

def main():
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <reference.pdb> <our.pdb>")
        sys.exit(1)

    ref_path, our_path = sys.argv[1], sys.argv[2]

    with open(ref_path, 'rb') as f:
        ref_data = f.read(4096)

    with open(our_path, 'rb') as f:
        our_data = f.read(4096)

    print("=== Reference PDB ===")
    ref_tables = parse_header(ref_data)

    print("\n=== Our PDB ===")
    our_tables = parse_header(our_data)

    print("\n=== Table Comparison ===")
    print(f"{'Table':<20} {'Ref First':>10} {'Ref Last':>10} {'Our First':>10} {'Our Last':>10} {'Match':>8}")
    print("-" * 70)

    for i in range(20):
        name = TABLE_NAMES.get(i, f"Unknown({i})")
        ref = ref_tables[i]
        our = our_tables[i]
        match = "✓" if ref['first'] == our['first'] and ref['last'] == our['last'] else "✗"
        print(f"{name:<20} {ref['first']:>10} {ref['last']:>10} {our['first']:>10} {our['last']:>10} {match:>8}")

if __name__ == "__main__":
    main()
