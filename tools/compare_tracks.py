#!/usr/bin/env python3
"""
Deep comparison of track rows between two PDB files.
"""

import struct
import sys
from pathlib import Path

PAGE_SIZE = 4096
HEAP_START = 0x28

TRACK_FIELDS = [
    (0x00, 2, 'subtype'),
    (0x02, 2, 'index_shift'),
    (0x04, 4, 'bitmask'),
    (0x08, 4, 'sample_rate'),
    (0x0C, 4, 'composer_id'),
    (0x10, 4, 'file_size'),
    (0x14, 4, 'u2'),
    (0x18, 2, 'u3'),
    (0x1A, 2, 'u4'),
    (0x1C, 4, 'artwork_id'),
    (0x20, 4, 'key_id'),
    (0x24, 4, 'orig_artist_id'),
    (0x28, 4, 'label_id'),
    (0x2C, 4, 'remixer_id'),
    (0x30, 4, 'bitrate'),
    (0x34, 4, 'track_number'),
    (0x38, 4, 'tempo'),
    (0x3C, 4, 'genre_id'),
    (0x40, 4, 'album_id'),
    (0x44, 4, 'artist_id'),
    (0x48, 4, 'track_id'),
    (0x4C, 2, 'disc_number'),
    (0x4E, 2, 'play_count'),
    (0x50, 2, 'year'),
    (0x52, 2, 'sample_depth'),
    (0x54, 2, 'duration'),
    (0x56, 2, 'u5'),
    (0x58, 1, 'color_id'),
    (0x59, 1, 'rating'),
    (0x5A, 2, 'file_type'),
    (0x5C, 2, 'u7'),
]

def read_file(path):
    with open(path, "rb") as f:
        return f.read()

def get_page(data, page_num):
    start = page_num * PAGE_SIZE
    return data[start:start + PAGE_SIZE]

def parse_row_groups(page_data):
    group = page_data[-36:]
    offsets = []
    for i in range(16):
        off = struct.unpack_from('<H', group, (15-i)*2)[0]
        offsets.append(off)
    flags = struct.unpack_from('<H', group, 32)[0]
    unknown = struct.unpack_from('<H', group, 34)[0]
    return offsets, flags, unknown

def parse_track_row(page_data, heap_offset):
    row_start = HEAP_START + heap_offset
    row = page_data[row_start:]
    result = {}
    for offset, size, name in TRACK_FIELDS:
        if size == 1:
            result[name] = row[offset]
        elif size == 2:
            result[name] = struct.unpack_from('<H', row, offset)[0]
        elif size == 4:
            result[name] = struct.unpack_from('<I', row, offset)[0]
    result['string_offsets'] = []
    for i in range(21):
        off = struct.unpack_from('<H', row, 0x5E + i*2)[0]
        result['string_offsets'].append(off)
    return result, row[:0x88]

def compare_pages(page1, page2, label1="REF", label2="GEN"):
    print(f"\n{'='*70}")
    print(f"PAGE HEADER COMPARISON")
    print(f"{'='*70}")

    header_fields = [
        (0x00, 4, 'padding'),
        (0x04, 4, 'page_index'),
        (0x08, 4, 'table_type'),
        (0x0C, 4, 'next_page'),
        (0x10, 4, 'unknown1'),
        (0x14, 4, 'unknown2'),
        (0x18, 1, 'num_rows_small'),
        (0x19, 1, 'unknown3'),
        (0x1A, 1, 'unknown4'),
        (0x1B, 1, 'page_flags'),
        (0x1C, 2, 'free_size'),
        (0x1E, 2, 'used_size'),
        (0x20, 2, 'unknown5'),
        (0x22, 2, 'num_rows_large'),
        (0x24, 2, 'unknown6'),
        (0x26, 2, 'unknown7'),
    ]

    for offset, size, name in header_fields:
        if size == 1:
            v1 = page1[offset]
            v2 = page2[offset]
        elif size == 2:
            v1 = struct.unpack_from('<H', page1, offset)[0]
            v2 = struct.unpack_from('<H', page2, offset)[0]
        elif size == 4:
            v1 = struct.unpack_from('<I', page1, offset)[0]
            v2 = struct.unpack_from('<I', page2, offset)[0]
        match = "==" if v1 == v2 else "!="
        print(f"  {name:18s}: {label1}=0x{v1:08x}  {match}  {label2}=0x{v2:08x}")

    print(f"\n{'='*70}")
    print(f"ROW GROUPS")
    print(f"{'='*70}")

    off1, flags1, unk1 = parse_row_groups(page1)
    off2, flags2, unk2 = parse_row_groups(page2)

    print(f"  {label1}: flags=0x{flags1:04x}, unknown=0x{unk1:04x}, offsets={[o for o in off1 if o != 0]}")
    print(f"  {label2}: flags=0x{flags2:04x}, unknown=0x{unk2:04x}, offsets={[o for o in off2 if o != 0]}")

    print(f"\n{'='*70}")
    print(f"TRACK ROWS")
    print(f"{'='*70}")

    for slot in range(16):
        has1 = (flags1 & (1 << slot)) != 0
        has2 = (flags2 & (1 << slot)) != 0
        if not has1 and not has2:
            continue
        print(f"\n  --- Slot {slot} ---")
        if has1:
            row1, raw1 = parse_track_row(page1, off1[slot])
            print(f"  {label1}: offset={off1[slot]}")
        else:
            row1 = None
            print(f"  {label1}: <empty>")
        if has2:
            row2, raw2 = parse_track_row(page2, off2[slot])
            print(f"  {label2}: offset={off2[slot]}")
        else:
            row2 = None
            print(f"  {label2}: <empty>")

        if has1 and has2:
            for offset, size, name in TRACK_FIELDS:
                v1 = row1[name]
                v2 = row2[name]
                if v1 != v2:
                    print(f"    {name:18s}: 0x{v1:08x} != 0x{v2:08x}")
        elif has1:
            print(f"    Track data: subtype=0x{row1['subtype']:04x}, track_id={row1['track_id']}")
        elif has2:
            print(f"    Track data: subtype=0x{row2['subtype']:04x}, track_id={row2['track_id']}")

if len(sys.argv) < 3:
    print("Usage: compare_tracks.py <ref.pdb> <gen.pdb>")
    sys.exit(1)

ref_data = read_file(sys.argv[1])
gen_data = read_file(sys.argv[2])

ref_page2 = get_page(ref_data, 2)
gen_page2 = get_page(gen_data, 2)

compare_pages(ref_page2, gen_page2, "REF", "GEN")
