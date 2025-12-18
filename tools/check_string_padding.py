#!/usr/bin/env python3
"""
Check how reference PDB pads strings/rows.
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

def check_genres_padding(pdb_path):
    """Check Genre row padding"""
    data = Path(pdb_path).read_bytes()

    page_offset = 4 * PAGE_SIZE
    num_rows = data[page_offset + 0x18]

    # Get row offsets
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

    print(f"=== Genre Rows ({num_rows} rows) ===")
    for i, off in enumerate(offsets[:num_rows]):
        row_start = page_offset + HEAP_START + off
        # Genre row: u32 id, then string
        genre_id = read_u32(data, row_start)
        string_start = row_start + 4
        header = data[string_start]

        if header & 0x01:  # ShortASCII
            length = (header >> 1) - 1
            content = data[string_start + 1:string_start + 1 + length].decode('ascii', errors='replace')
            raw_string_len = 1 + length  # header + content (no null included in ShortASCII)
        else:
            content = "<long>"
            raw_string_len = 0

        # Calculate total row bytes to next row
        if i + 1 < len(offsets):
            next_off = offsets[i + 1]
            row_size = next_off - off
        else:
            row_size = "last"

        # Check padding bytes after string
        if row_size != "last":
            content_end = 4 + raw_string_len  # id + string
            padding = row_size - content_end
            padding_bytes = hex_bytes(data, row_start + content_end, min(padding, 8)) if padding > 0 else "none"
        else:
            padding = "n/a"
            padding_bytes = "n/a"

        print(f"Row {i}: id={genre_id}, name='{content}' ({len(content)} chars)")
        print(f"  Raw: {hex_bytes(data, row_start, min(24, row_size if row_size != 'last' else 24))}")
        print(f"  String header={header:#x}, raw_len={raw_string_len}, row_size={row_size}, padding={padding}")

def check_colors_padding(pdb_path):
    """Check Color row padding"""
    data = Path(pdb_path).read_bytes()

    page_offset = 14 * PAGE_SIZE
    num_rows = data[page_offset + 0x18]

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

    print(f"\n=== Color Rows ({num_rows} rows) ===")
    for i, off in enumerate(offsets[:num_rows]):
        row_start = page_offset + HEAP_START + off
        # Color row: u32+u8+u8+u16 = 8 bytes, then string
        string_start = row_start + 8
        header = data[string_start]

        if header & 0x01:
            length = (header >> 1) - 1
            content = data[string_start + 1:string_start + 1 + length].decode('ascii', errors='replace')
            raw_string_len = 1 + length
        else:
            content = "<long>"
            raw_string_len = 0

        if i + 1 < len(offsets):
            next_off = offsets[i + 1]
            row_size = next_off - off
        else:
            row_size = "last"

        content_size = 8 + raw_string_len
        padding = row_size - content_size if row_size != "last" else "n/a"

        print(f"Row {i}: name='{content}' ({len(content)} chars)")
        print(f"  Raw: {hex_bytes(data, row_start, min(20, row_size if row_size != 'last' else 20))}")
        print(f"  content_size={content_size}, row_size={row_size}, padding={padding}")

if __name__ == "__main__":
    import sys
    pdb = sys.argv[1] if len(sys.argv) > 1 else "/home/julien/Documents/Scripts/Pioneer/examples/PIONEER/rekordbox/export.pdb"
    check_genres_padding(pdb)
    check_colors_padding(pdb)
