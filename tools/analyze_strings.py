#!/usr/bin/env python3
"""
Analyze DeviceSQL string encoding in PDB files.
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

def parse_device_sql_string(data, offset):
    """Parse a DeviceSQL string and return (decoded_string, total_bytes)"""
    header = data[offset]

    if header & 0x01:  # ShortASCII
        length = (header >> 1) - 1  # Length without null
        content = data[offset + 1:offset + 1 + length].decode('ascii', errors='replace')
        # Check for trailing null (sometimes present, sometimes not)
        total = 1 + length
        if offset + total < len(data) and data[offset + total] == 0:
            total += 1  # Include trailing null in total
        return content, total, "ShortASCII"

    elif header in (0x40, 0x90):  # Long ASCII or UTF-16
        length = read_u16(data, offset + 1)  # Length includes 4-byte header
        if header == 0x40:  # ASCII
            content = data[offset + 4:offset + length].decode('ascii', errors='replace')
            return content, length, "LongASCII"
        else:  # UTF-16
            content = data[offset + 4:offset + length].decode('utf-16-le', errors='replace')
            return content, length, "UTF-16"

    return f"<unknown header {header:#x}>", 1, "Unknown"

def analyze_playlist_strings(pdb_path, label):
    """Analyze string encoding in PlaylistTree rows"""
    data = Path(pdb_path).read_bytes()

    page_offset = 16 * PAGE_SIZE
    num_rows = data[page_offset + 0x18]

    print(f"\n=== {label} PlaylistTree Strings ===")

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

    for i, off in enumerate(offsets[:num_rows]):
        row_start = page_offset + HEAP_START + off
        # Skip 20 bytes of fixed fields to get to string
        string_offset = row_start + 20
        content, total_bytes, encoding = parse_device_sql_string(data, string_offset)
        print(f"Row {i}: name='{content}' ({encoding}, {total_bytes} bytes)")
        print(f"  Row bytes: {hex_bytes(data, row_start, 32)}")
        # Calculate next row start
        if i + 1 < len(offsets):
            next_off = offsets[i + 1]
            row_size = next_off - off
            print(f"  Row size: {row_size} bytes (offset {off:#x} to {next_off:#x})")

def analyze_color_strings(pdb_path, label):
    """Analyze Color row structure"""
    data = Path(pdb_path).read_bytes()

    page_offset = 14 * PAGE_SIZE
    num_rows = data[page_offset + 0x18]

    print(f"\n=== {label} Color Rows ===")

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

    print(f"Num rows: {num_rows}, offsets: {[hex(o) for o in offsets[:num_rows]]}")

    for i, off in enumerate(offsets[:num_rows]):
        row_start = page_offset + HEAP_START + off
        print(f"Row {i} at offset {off:#x}: {hex_bytes(data, row_start, 20)}")
        # Color row: u32 unknown1, u8 unknown2, u8 color_index, u16 unknown3, then string
        u1 = read_u32(data, row_start)
        u2 = read_u8(data, row_start + 4)
        color = read_u8(data, row_start + 5)
        u3 = read_u16(data, row_start + 6)
        content, total_bytes, encoding = parse_device_sql_string(data, row_start + 8)
        print(f"  u1={u1:#x}, u2={u2:#x}, color={color}, u3={u3:#x}, name='{content}' ({encoding})")

def analyze_column_strings(pdb_path, label):
    """Analyze Column row structure"""
    data = Path(pdb_path).read_bytes()

    page_offset = 34 * PAGE_SIZE
    num_rows = data[page_offset + 0x18]

    print(f"\n=== {label} Column Rows (first 5) ===")

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

    print(f"Num rows: {num_rows}, first offsets: {[hex(o) for o in offsets[:5]]}")

    for i, off in enumerate(offsets[:min(5, num_rows)]):
        row_start = page_offset + HEAP_START + off
        print(f"Row {i} at offset {off:#x}: {hex_bytes(data, row_start, 30)}")
        # Column row: u16 id, u16 flags, then UTF-16 string
        col_id = read_u16(data, row_start)
        col_flags = read_u16(data, row_start + 2)
        content, total_bytes, encoding = parse_device_sql_string(data, row_start + 4)
        print(f"  id={col_id}, flags={col_flags:#x}, name='{content}' ({encoding})")

if __name__ == "__main__":
    ref = "/home/julien/Documents/Scripts/Pioneer/examples/PIONEER/rekordbox/export.pdb"
    test = "/tmp/pioneer_test/PIONEER/rekordbox/export.pdb"

    analyze_playlist_strings(ref, "Reference")
    analyze_playlist_strings(test, "Test")

    analyze_color_strings(ref, "Reference")
    analyze_color_strings(test, "Test")

    analyze_column_strings(ref, "Reference")
    analyze_column_strings(test, "Test")
