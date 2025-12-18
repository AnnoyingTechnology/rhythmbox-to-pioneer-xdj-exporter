#!/usr/bin/env python3
"""
Analyze data page differences to identify what needs to be fixed.
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

def hex_bytes(data, start, length):
    return ' '.join(f'{data[start + i]:02x}' for i in range(min(length, len(data) - start)))

def compare_page_headers(ref_data, test_data, page_idx, name):
    """Compare page headers and identify differences"""
    page_offset = page_idx * PAGE_SIZE

    fields = [
        (0x04, 4, "page_index"),
        (0x08, 4, "type"),
        (0x0c, 4, "next_page"),
        (0x10, 4, "unknown1"),
        (0x18, 1, "num_rows_small"),
        (0x19, 1, "unknown3"),
        (0x1a, 1, "unknown4"),
        (0x1b, 1, "page_flags"),
        (0x1c, 2, "free_size"),
        (0x1e, 2, "used_size"),
        (0x20, 2, "unknown5"),
        (0x22, 2, "num_rows_large"),
        (0x24, 2, "unknown6"),
        (0x26, 2, "unknown7"),
    ]

    print(f"\n=== Page {page_idx}: {name} ===")
    diffs = []
    for off, size, field in fields:
        if size == 1:
            ref_val = read_u8(ref_data, page_offset + off)
            test_val = read_u8(test_data, page_offset + off)
        elif size == 2:
            ref_val = read_u16(ref_data, page_offset + off)
            test_val = read_u16(test_data, page_offset + off)
        else:
            ref_val = read_u32(ref_data, page_offset + off)
            test_val = read_u32(test_data, page_offset + off)

        if ref_val != test_val:
            diffs.append(f"{field}: ref={ref_val:#x} test={test_val:#x}")

    if diffs:
        print("Header differences:")
        for d in diffs:
            print(f"  {d}")
    else:
        print("Header: IDENTICAL")

    return len(diffs) == 0

def analyze_track_first_row(ref_data, test_data):
    """Compare first track row structure in detail"""
    page_offset = 2 * PAGE_SIZE + HEAP_START

    print("\n=== First Track Row Analysis ===")

    # Track header fields
    fields = [
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
        (0x24, 4, "original_artist_id"),
        (0x28, 4, "label_id"),
        (0x2c, 4, "remixer_id"),
        (0x30, 4, "bitrate"),
        (0x34, 4, "track_number"),
        (0x38, 4, "tempo"),
        (0x3c, 4, "genre_id"),
        (0x40, 4, "album_id"),
        (0x44, 4, "artist_id"),
        (0x48, 4, "id"),
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

    print("Field comparison (first track):")
    for off, size, field in fields:
        if size == 1:
            ref_val = read_u8(ref_data, page_offset + off)
            test_val = read_u8(test_data, page_offset + off)
        elif size == 2:
            ref_val = read_u16(ref_data, page_offset + off)
            test_val = read_u16(test_data, page_offset + off)
        else:
            ref_val = read_u32(ref_data, page_offset + off)
            test_val = read_u32(test_data, page_offset + off)

        match = "OK" if ref_val == test_val else "DIFF"
        if match == "DIFF":
            print(f"  {off:#04x} {field}: ref={ref_val:#x} ({ref_val}) test={test_val:#x} ({test_val}) [{match}]")

def analyze_string_offsets(ref_data, test_data):
    """Compare string offset arrays"""
    page_offset = 2 * PAGE_SIZE + HEAP_START

    print("\n=== String Offsets (first track) ===")
    print("Idx  Ref    Test   Diff")
    for i in range(21):
        ref_off = read_u16(ref_data, page_offset + 0x5e + i*2)
        test_off = read_u16(test_data, page_offset + 0x5e + i*2)
        diff = "DIFF" if ref_off != test_off else ""
        if ref_off != 0 or test_off != 0:
            print(f"{i:3d}  {ref_off:#06x} {test_off:#06x} {diff}")

def main():
    ref_path = "/home/julien/Documents/Scripts/Pioneer/examples/PIONEER/rekordbox/export.pdb"
    test_path = "/tmp/pioneer_test/PIONEER/rekordbox/export.pdb"

    ref_data = Path(ref_path).read_bytes()
    test_data = Path(test_path).read_bytes()

    print("Comparing data pages between reference and our export")
    print("=" * 60)

    # Compare key data page headers
    data_pages = [
        (2, "Tracks (page 2)"),
        (4, "Genres"),
        (6, "Artists"),
        (8, "Albums"),
        (14, "Colors"),
        (16, "PlaylistTree"),
        (18, "PlaylistEntries"),
        (51, "Tracks (page 51)"),
    ]

    for page_idx, name in data_pages:
        if page_idx * PAGE_SIZE < len(ref_data) and page_idx * PAGE_SIZE < len(test_data):
            compare_page_headers(ref_data, test_data, page_idx, name)

    # Detailed track analysis
    analyze_track_first_row(ref_data, test_data)
    analyze_string_offsets(ref_data, test_data)

if __name__ == "__main__":
    main()
