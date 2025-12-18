#!/usr/bin/env python3
"""
PDB file comparison tool for Pioneer export debugging.
Compares our generated PDB against a reference Rekordbox export byte-by-byte.
"""

import sys
import struct
from pathlib import Path

PAGE_SIZE = 4096
HEAP_START = 0x28

# Table type names
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

def read_u8(data, offset):
    return data[offset]

def read_u16(data, offset):
    return struct.unpack_from('<H', data, offset)[0]

def read_u32(data, offset):
    return struct.unpack_from('<I', data, offset)[0]

def parse_file_header(data):
    """Parse PDB file header (page 0)"""
    return {
        'magic': read_u32(data, 0),
        'page_size': read_u32(data, 4),
        'num_tables': read_u32(data, 8),
        'next_unused_page': read_u32(data, 0x0c),
        'unknown': read_u32(data, 0x10),
        'sequence': read_u32(data, 0x14),
        'gap': read_u32(data, 0x18),
    }

def parse_table_pointers(data, num_tables):
    """Parse table pointer array starting at 0x1c"""
    pointers = []
    offset = 0x1c
    for i in range(num_tables):
        pointers.append({
            'type': read_u32(data, offset),
            'empty_candidate': read_u32(data, offset + 4),
            'first_page': read_u32(data, offset + 8),
            'last_page': read_u32(data, offset + 12),
        })
        offset += 16
    return pointers

def parse_page_header(data, page_offset):
    """Parse a page header (40 bytes)"""
    return {
        'padding': read_u32(data, page_offset + 0x00),
        'page_index': read_u32(data, page_offset + 0x04),
        'type': read_u32(data, page_offset + 0x08),
        'next_page': read_u32(data, page_offset + 0x0c),
        'unknown1': read_u32(data, page_offset + 0x10),
        'unknown2': read_u32(data, page_offset + 0x14),
        'num_rows_small': read_u8(data, page_offset + 0x18),
        'unknown3': read_u8(data, page_offset + 0x19),
        'unknown4': read_u8(data, page_offset + 0x1a),
        'page_flags': read_u8(data, page_offset + 0x1b),
        'free_size': read_u16(data, page_offset + 0x1c),
        'used_size': read_u16(data, page_offset + 0x1e),
        'unknown5': read_u16(data, page_offset + 0x20),
        'num_rows_large': read_u16(data, page_offset + 0x22),
        'unknown6': read_u16(data, page_offset + 0x24),
        'unknown7': read_u16(data, page_offset + 0x26),
    }

def parse_row_groups(data, page_offset, num_rows):
    """Parse row group index at end of page"""
    if num_rows == 0:
        return []

    num_groups = (num_rows + 15) // 16
    groups = []

    # Row groups are at the end of the page, each group is 36 bytes
    # Groups are stored in order (group 0 first)
    group_area_start = page_offset + PAGE_SIZE - (num_groups * 36)

    for g in range(num_groups):
        group_offset = group_area_start + (g * 36)
        # 16 row offsets (u16 each, stored in reverse order within group)
        offsets = []
        for i in range(16):
            off = read_u16(data, group_offset + (15 - i) * 2)
            offsets.append(off)
        flags = read_u16(data, group_offset + 32)
        unknown = read_u16(data, group_offset + 34)
        groups.append({
            'offsets': offsets,
            'flags': flags,
            'unknown': unknown,
        })

    return groups

def hex_dump(data, offset, length, prefix=""):
    """Print hex dump of data"""
    for i in range(0, length, 16):
        hex_part = ' '.join(f'{data[offset + i + j]:02x}' for j in range(min(16, length - i)))
        print(f"{prefix}{offset + i:04x}: {hex_part}")

def compare_files(ref_path, test_path):
    """Compare two PDB files"""
    ref_data = Path(ref_path).read_bytes()
    test_data = Path(test_path).read_bytes()

    print(f"Reference: {ref_path} ({len(ref_data)} bytes, {len(ref_data) // PAGE_SIZE} pages)")
    print(f"Test:      {test_path} ({len(test_data)} bytes, {len(test_data) // PAGE_SIZE} pages)")
    print()

    # Compare file headers
    ref_header = parse_file_header(ref_data)
    test_header = parse_file_header(test_data)

    print("=== FILE HEADER ===")
    for key in ref_header:
        ref_val = ref_header[key]
        test_val = test_header[key]
        match = "OK" if ref_val == test_val else "DIFF"
        print(f"  {key}: ref={ref_val:#x} test={test_val:#x} [{match}]")
    print()

    # Compare table pointers
    ref_pointers = parse_table_pointers(ref_data, ref_header['num_tables'])
    test_pointers = parse_table_pointers(test_data, test_header['num_tables'])

    print("=== TABLE POINTERS ===")
    for i, (ref_ptr, test_ptr) in enumerate(zip(ref_pointers, test_pointers)):
        table_name = TABLE_TYPES.get(ref_ptr['type'], f"Type{ref_ptr['type']}")
        diffs = []
        for key in ref_ptr:
            if ref_ptr[key] != test_ptr[key]:
                diffs.append(f"{key}: ref={ref_ptr[key]} test={test_ptr[key]}")
        if diffs:
            print(f"  [{i}] {table_name}: " + ", ".join(diffs))
        else:
            print(f"  [{i}] {table_name}: OK (first={ref_ptr['first_page']}, last={ref_ptr['last_page']}, empty={ref_ptr['empty_candidate']})")
    print()

    # Compare each page
    num_pages = min(len(ref_data), len(test_data)) // PAGE_SIZE

    print("=== PAGE COMPARISON ===")
    for page_idx in range(num_pages):
        page_offset = page_idx * PAGE_SIZE

        ref_page = ref_data[page_offset:page_offset + PAGE_SIZE]
        test_page = test_data[page_offset:page_offset + PAGE_SIZE]

        if ref_page == test_page:
            # Pages are identical
            if page_idx == 0:
                print(f"Page {page_idx}: FILE HEADER - identical")
            else:
                ref_hdr = parse_page_header(ref_data, page_offset)
                table_name = TABLE_TYPES.get(ref_hdr['type'], f"Type{ref_hdr['type']}")
                print(f"Page {page_idx}: {table_name} - identical")
            continue

        # Pages differ - analyze
        if page_idx == 0:
            print(f"Page {page_idx}: FILE HEADER - DIFFERS")
            # Already printed header diff above
            continue

        ref_hdr = parse_page_header(ref_data, page_offset)
        test_hdr = parse_page_header(test_data, page_offset)

        table_name = TABLE_TYPES.get(ref_hdr['type'], f"Type{ref_hdr['type']}")

        # Check if it's an empty page (all zeros in reference)
        if all(b == 0 for b in ref_page):
            if all(b == 0 for b in test_page):
                print(f"Page {page_idx}: EMPTY - identical (all zeros)")
            else:
                print(f"Page {page_idx}: EMPTY in ref, but test has data!")
            continue

        if all(b == 0 for b in test_page):
            print(f"Page {page_idx}: {table_name} in ref, but test is EMPTY!")
            continue

        print(f"\nPage {page_idx}: {table_name} - DIFFERS")

        # Compare headers
        header_diffs = []
        for key in ref_hdr:
            if ref_hdr[key] != test_hdr[key]:
                header_diffs.append(f"{key}: ref={ref_hdr[key]:#x} test={test_hdr[key]:#x}")

        if header_diffs:
            print(f"  Header diffs: {', '.join(header_diffs)}")
        else:
            print(f"  Header: OK (rows={ref_hdr['num_rows_small']}, used={ref_hdr['used_size']:#x}, free={ref_hdr['free_size']:#x})")

        # Compare heap data
        ref_heap = ref_data[page_offset + HEAP_START:page_offset + HEAP_START + ref_hdr['used_size']]
        test_heap = test_data[page_offset + HEAP_START:page_offset + HEAP_START + test_hdr['used_size']]

        if ref_heap != test_heap:
            print(f"  Heap data differs (ref={len(ref_heap)} bytes, test={len(test_heap)} bytes)")
            # Find first difference
            min_len = min(len(ref_heap), len(test_heap))
            for i in range(min_len):
                if ref_heap[i] != test_heap[i]:
                    print(f"    First diff at heap offset {i:#x} (page offset {HEAP_START + i:#x})")
                    print(f"    Ref bytes around diff:")
                    hex_dump(ref_data, page_offset + HEAP_START + max(0, i-8), min(32, len(ref_heap) - max(0, i-8)), "      ")
                    print(f"    Test bytes around diff:")
                    hex_dump(test_data, page_offset + HEAP_START + max(0, i-8), min(32, len(test_heap) - max(0, i-8)), "      ")
                    break

        # Compare row groups
        ref_groups = parse_row_groups(ref_data, page_offset, ref_hdr['num_rows_small'])
        test_groups = parse_row_groups(test_data, page_offset, test_hdr['num_rows_small'])

        if ref_groups != test_groups:
            print(f"  Row groups differ:")
            for g, (rg, tg) in enumerate(zip(ref_groups, test_groups)):
                if rg != tg:
                    print(f"    Group {g}:")
                    if rg['flags'] != tg['flags']:
                        print(f"      flags: ref={rg['flags']:#x} test={tg['flags']:#x}")
                    if rg['unknown'] != tg['unknown']:
                        print(f"      unknown: ref={rg['unknown']:#x} test={tg['unknown']:#x}")
                    for i, (ro, to) in enumerate(zip(rg['offsets'], tg['offsets'])):
                        if ro != to:
                            print(f"      offset[{i}]: ref={ro:#x} test={to:#x}")

def dump_page(pdb_path, page_idx):
    """Dump detailed info about a specific page"""
    data = Path(pdb_path).read_bytes()
    page_offset = page_idx * PAGE_SIZE

    if page_idx == 0:
        print("=== FILE HEADER (Page 0) ===")
        header = parse_file_header(data)
        for key, val in header.items():
            print(f"  {key}: {val:#x} ({val})")
        print()

        pointers = parse_table_pointers(data, header['num_tables'])
        print("Table Pointers:")
        for i, ptr in enumerate(pointers):
            table_name = TABLE_TYPES.get(ptr['type'], f"Type{ptr['type']}")
            print(f"  [{i}] {table_name}: type={ptr['type']}, empty={ptr['empty_candidate']}, first={ptr['first_page']}, last={ptr['last_page']}")
        return

    print(f"=== PAGE {page_idx} ===")
    hdr = parse_page_header(data, page_offset)
    table_name = TABLE_TYPES.get(hdr['type'], f"Type{hdr['type']}")

    print(f"Table: {table_name}")
    print("Header fields:")
    for key, val in hdr.items():
        print(f"  {key}: {val:#x} ({val})")

    print()
    print(f"Heap data ({hdr['used_size']} bytes):")
    hex_dump(data, page_offset + HEAP_START, min(hdr['used_size'], 256), "  ")
    if hdr['used_size'] > 256:
        print(f"  ... ({hdr['used_size'] - 256} more bytes)")

    print()
    print("Row groups:")
    groups = parse_row_groups(data, page_offset, hdr['num_rows_small'])
    for g, group in enumerate(groups):
        print(f"  Group {g}: flags={group['flags']:#06x} unknown={group['unknown']:#06x}")
        active_offsets = [(i, o) for i, o in enumerate(group['offsets']) if group['flags'] & (1 << i)]
        print(f"    Active offsets: {active_offsets}")

def main():
    if len(sys.argv) < 2:
        print("Usage:")
        print("  pdb_compare.py compare <reference.pdb> <test.pdb>")
        print("  pdb_compare.py dump <file.pdb> <page_index>")
        sys.exit(1)

    cmd = sys.argv[1]

    if cmd == "compare":
        if len(sys.argv) != 4:
            print("Usage: pdb_compare.py compare <reference.pdb> <test.pdb>")
            sys.exit(1)
        compare_files(sys.argv[2], sys.argv[3])

    elif cmd == "dump":
        if len(sys.argv) != 4:
            print("Usage: pdb_compare.py dump <file.pdb> <page_index>")
            sys.exit(1)
        dump_page(sys.argv[2], int(sys.argv[3]))

    else:
        print(f"Unknown command: {cmd}")
        sys.exit(1)

if __name__ == "__main__":
    main()
