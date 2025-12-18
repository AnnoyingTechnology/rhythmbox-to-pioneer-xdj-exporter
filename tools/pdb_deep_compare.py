#!/usr/bin/env python3
"""
Deep PDB comparison - focus on row structure differences.
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

def parse_page_header(data, page_offset):
    return {
        'page_index': read_u32(data, page_offset + 0x04),
        'type': read_u32(data, page_offset + 0x08),
        'next_page': read_u32(data, page_offset + 0x0c),
        'unknown1': read_u32(data, page_offset + 0x10),
        'num_rows_small': read_u8(data, page_offset + 0x18),
        'unknown3': read_u8(data, page_offset + 0x19),
        'unknown4': read_u8(data, page_offset + 0x1a),
        'page_flags': read_u8(data, page_offset + 0x1b),
        'free_size': read_u16(data, page_offset + 0x1c),
        'used_size': read_u16(data, page_offset + 0x1e),
        'unknown5': read_u16(data, page_offset + 0x20),
        'num_rows_large': read_u16(data, page_offset + 0x22),
    }

def get_row_offsets(data, page_offset, num_rows):
    """Get all row offsets from the page"""
    if num_rows == 0:
        return []

    num_groups = (num_rows + 15) // 16
    group_area_start = page_offset + PAGE_SIZE - (num_groups * 36)

    offsets = []
    row_idx = 0
    for g in range(num_groups):
        group_offset = group_area_start + (g * 36)
        flags = read_u16(data, group_offset + 32)

        for i in range(16):
            if row_idx >= num_rows:
                break
            if flags & (1 << i):
                off = read_u16(data, group_offset + (15 - i) * 2)
                offsets.append(off)
            row_idx += 1

    return offsets

def hex_bytes(data, start, length):
    return ' '.join(f'{data[start + i]:02x}' for i in range(min(length, len(data) - start)))

def analyze_playlist_tree_rows(ref_path, test_path):
    """Compare PlaylistTree row structures"""
    ref_data = Path(ref_path).read_bytes()
    test_data = Path(test_path).read_bytes()

    # PlaylistTree data is on page 16
    page_offset = 16 * PAGE_SIZE

    ref_hdr = parse_page_header(ref_data, page_offset)
    test_hdr = parse_page_header(test_data, page_offset)

    print("=== PlaylistTree Row Analysis ===")
    print(f"Ref: {ref_hdr['num_rows_small']} rows, used={ref_hdr['used_size']:#x}")
    print(f"Test: {test_hdr['num_rows_small']} rows, used={test_hdr['used_size']:#x}")

    ref_offsets = get_row_offsets(ref_data, page_offset, ref_hdr['num_rows_small'])
    test_offsets = get_row_offsets(test_data, page_offset, test_hdr['num_rows_small'])

    print(f"\nRef row offsets: {[hex(o) for o in ref_offsets]}")
    print(f"Test row offsets: {[hex(o) for o in test_offsets]}")

    print("\n--- Reference Rows ---")
    for i, off in enumerate(ref_offsets):
        row_start = page_offset + HEAP_START + off
        print(f"Row {i} at offset {off:#x} (abs {row_start:#x}):")
        # PlaylistTree row: parent_id(4), unknown(4), sort_order(4), id(4), is_folder(4), name_string
        parent = read_u32(ref_data, row_start)
        unknown = read_u32(ref_data, row_start + 4)
        sort = read_u32(ref_data, row_start + 8)
        pid = read_u32(ref_data, row_start + 12)
        is_folder = read_u32(ref_data, row_start + 16)
        print(f"  parent={parent}, unknown={unknown}, sort={sort}, id={pid}, is_folder={is_folder}")
        print(f"  Raw: {hex_bytes(ref_data, row_start, 32)}")

    print("\n--- Test Rows ---")
    for i, off in enumerate(test_offsets):
        row_start = page_offset + HEAP_START + off
        print(f"Row {i} at offset {off:#x} (abs {row_start:#x}):")
        parent = read_u32(test_data, row_start)
        unknown = read_u32(test_data, row_start + 4)
        sort = read_u32(test_data, row_start + 8)
        pid = read_u32(test_data, row_start + 12)
        is_folder = read_u32(test_data, row_start + 16)
        print(f"  parent={parent}, unknown={unknown}, sort={sort}, id={pid}, is_folder={is_folder}")
        print(f"  Raw: {hex_bytes(test_data, row_start, 32)}")

def analyze_tracks_rows(ref_path, test_path):
    """Compare Track row structures"""
    ref_data = Path(ref_path).read_bytes()
    test_data = Path(test_path).read_bytes()

    # Tracks data is on page 2
    page_offset = 2 * PAGE_SIZE

    ref_hdr = parse_page_header(ref_data, page_offset)
    test_hdr = parse_page_header(test_data, page_offset)

    print("=== Track Row Analysis (Page 2) ===")
    print(f"Ref: {ref_hdr['num_rows_small']} rows, used={ref_hdr['used_size']:#x}")
    print(f"Test: {test_hdr['num_rows_small']} rows, used={test_hdr['used_size']:#x}")

    ref_offsets = get_row_offsets(ref_data, page_offset, ref_hdr['num_rows_small'])
    test_offsets = get_row_offsets(test_data, page_offset, test_hdr['num_rows_small'])

    print(f"\nRef row offsets: {[hex(o) for o in ref_offsets]}")
    print(f"Test row offsets: {[hex(o) for o in test_offsets]}")

    # Compare first track row structure
    if ref_offsets and test_offsets:
        print("\n--- First Track Row Comparison ---")
        ref_start = page_offset + HEAP_START + ref_offsets[0]
        test_start = page_offset + HEAP_START + test_offsets[0]

        # Track header is 94 bytes (0x5E)
        print("Header fields (first 94 bytes):")
        fields = [
            (0x00, 2, "subtype"),
            (0x02, 2, "index_shift"),
            (0x04, 4, "bitmask"),
            (0x08, 4, "sample_rate"),
            (0x0C, 4, "composer_id"),
            (0x10, 4, "file_size"),
            (0x14, 4, "u2"),
            (0x18, 2, "u3"),
            (0x1A, 2, "u4"),
            (0x1C, 4, "artwork_id"),
            (0x20, 4, "key_id"),
            (0x24, 4, "original_artist_id"),
            (0x28, 4, "label_id"),
            (0x2C, 4, "remixer_id"),
            (0x30, 4, "bitrate"),
            (0x34, 4, "track_number"),
            (0x38, 4, "tempo"),
            (0x3C, 4, "genre_id"),
            (0x40, 4, "album_id"),
            (0x44, 4, "artist_id"),
            (0x48, 4, "id"),
            (0x4C, 2, "disc_number"),
            (0x4E, 2, "play_count"),
            (0x50, 2, "year"),
            (0x52, 2, "sample_depth"),
            (0x54, 2, "duration"),
            (0x56, 2, "u5"),
            (0x58, 1, "color_id"),
            (0x59, 1, "rating"),
            (0x5A, 2, "file_type"),
            (0x5C, 2, "u7"),
        ]

        for off, size, name in fields:
            if size == 1:
                ref_val = read_u8(ref_data, ref_start + off)
                test_val = read_u8(test_data, test_start + off)
            elif size == 2:
                ref_val = read_u16(ref_data, ref_start + off)
                test_val = read_u16(test_data, test_start + off)
            else:
                ref_val = read_u32(ref_data, ref_start + off)
                test_val = read_u32(test_data, test_start + off)

            match = "OK" if ref_val == test_val else "DIFF"
            print(f"  {off:#04x} {name}: ref={ref_val:#x} test={test_val:#x} [{match}]")

def analyze_blank_pages(ref_path, test_path):
    """Check header page unknown1 values"""
    ref_data = Path(ref_path).read_bytes()
    test_data = Path(test_path).read_bytes()

    # Check all header pages (odd numbered from 1 onwards, type-specific)
    header_pages = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35, 37, 39]

    print("=== Header Page Unknown1 Values ===")
    for p in header_pages:
        page_offset = p * PAGE_SIZE
        ref_hdr = parse_page_header(ref_data, page_offset)
        test_hdr = parse_page_header(test_data, page_offset)

        if ref_hdr['unknown1'] != test_hdr['unknown1']:
            table_type = ref_hdr['type']
            print(f"Page {p} (type {table_type}): ref unknown1={ref_hdr['unknown1']:#x}, test={test_hdr['unknown1']:#x}")

def analyze_data_pages(ref_path, test_path):
    """Check data page header differences"""
    ref_data = Path(ref_path).read_bytes()
    test_data = Path(test_path).read_bytes()

    # Data pages
    data_pages = [2, 4, 6, 8, 10, 12, 14, 16, 18, 28, 34, 36, 38, 40, 51]

    print("=== Data Page Header Analysis ===")
    for p in data_pages:
        page_offset = p * PAGE_SIZE
        if page_offset >= len(ref_data) or page_offset >= len(test_data):
            continue

        ref_hdr = parse_page_header(ref_data, page_offset)
        test_hdr = parse_page_header(test_data, page_offset)

        diffs = []
        for key in ref_hdr:
            if ref_hdr[key] != test_hdr[key]:
                diffs.append(f"{key}: ref={ref_hdr[key]:#x} test={test_hdr[key]:#x}")

        if diffs:
            print(f"Page {p} (type {ref_hdr['type']}): {', '.join(diffs)}")

if __name__ == "__main__":
    import sys
    ref = sys.argv[1] if len(sys.argv) > 1 else "/home/julien/Documents/Scripts/Pioneer/examples/PIONEER/rekordbox/export.pdb"
    test = sys.argv[2] if len(sys.argv) > 2 else "/tmp/pioneer_test/PIONEER/rekordbox/export.pdb"

    analyze_playlist_tree_rows(ref, test)
    print("\n" + "="*60 + "\n")
    analyze_tracks_rows(ref, test)
    print("\n" + "="*60 + "\n")
    analyze_blank_pages(ref, test)
    print("\n" + "="*60 + "\n")
    analyze_data_pages(ref, test)
