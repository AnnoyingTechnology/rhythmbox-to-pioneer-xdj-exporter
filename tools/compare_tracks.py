#!/usr/bin/env python3
"""
Compare track data between reference and test PDB files.
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

def read_device_sql_string(data, offset):
    """Read a DeviceSQL string at the given offset"""
    if offset == 0 or offset >= len(data):
        return ""
    header = data[offset]
    if header & 0x01:  # ShortASCII
        length = (header >> 1) - 1
        if length <= 0:
            return ""
        return data[offset + 1:offset + 1 + length].decode('ascii', errors='replace')
    elif header == 0x40:  # Long ASCII
        length = read_u16(data, offset + 1)
        return data[offset + 4:offset + length].decode('ascii', errors='replace')
    elif header == 0x90:  # UTF-16
        length = read_u16(data, offset + 1)
        return data[offset + 4:offset + length].decode('utf-16-le', errors='replace')
    return ""

def parse_track_row(data, row_start):
    """Parse a track row and return key fields"""
    # Track header structure (94 bytes = 0x5E)
    track = {
        'subtype': read_u16(data, row_start + 0x00),
        'index_shift': read_u16(data, row_start + 0x02),
        'bitmask': read_u32(data, row_start + 0x04),
        'sample_rate': read_u32(data, row_start + 0x08),
        'file_size': read_u32(data, row_start + 0x10),
        'u2': read_u32(data, row_start + 0x14),
        'artwork_id': read_u32(data, row_start + 0x1c),
        'key_id': read_u32(data, row_start + 0x20),
        'bitrate': read_u32(data, row_start + 0x30),
        'track_number': read_u32(data, row_start + 0x34),
        'tempo': read_u32(data, row_start + 0x38),
        'genre_id': read_u32(data, row_start + 0x3c),
        'album_id': read_u32(data, row_start + 0x40),
        'artist_id': read_u32(data, row_start + 0x44),
        'id': read_u32(data, row_start + 0x48),
        'disc_number': read_u16(data, row_start + 0x4c),
        'year': read_u16(data, row_start + 0x50),
        'duration': read_u16(data, row_start + 0x54),
        'color_id': read_u8(data, row_start + 0x58),
        'file_type': read_u16(data, row_start + 0x5a),
    }

    # String offsets at 0x5E (21 x u16)
    string_offsets = []
    for i in range(21):
        off = read_u16(data, row_start + 0x5E + i * 2)
        string_offsets.append(off)

    # Read key strings
    def get_string(idx):
        off = string_offsets[idx]
        if off == 0:
            return ""
        return read_device_sql_string(data, row_start + off)

    track['title'] = get_string(17)
    track['filename'] = get_string(19)
    track['file_path'] = get_string(20)
    track['analyze_path'] = get_string(14)
    track['date_added'] = get_string(10)
    track['analyze_date'] = get_string(15)
    track['autoload_hotcues'] = get_string(7)

    return track

def compare_tracks(ref_path, test_path):
    """Compare tracks between reference and test PDB"""
    ref_data = Path(ref_path).read_bytes()
    test_data = Path(test_path).read_bytes()

    # Track data pages: 2 and 51
    track_pages = [2, 51]

    print("=== Reference Tracks ===")
    ref_tracks = []
    for page_idx in track_pages:
        page_offset = page_idx * PAGE_SIZE
        if page_offset >= len(ref_data):
            continue
        num_rows = ref_data[page_offset + 0x18]
        if num_rows == 0:
            continue
        offsets = get_row_offsets(ref_data, page_offset, num_rows)
        for off in offsets:
            row_start = page_offset + HEAP_START + off
            track = parse_track_row(ref_data, row_start)
            ref_tracks.append(track)
            print(f"  ID={track['id']:2d} '{track['title'][:40]:<40}' file_size={track['file_size']:>10}")

    print(f"\n=== Test Tracks ===")
    test_tracks = []
    for page_idx in track_pages:
        page_offset = page_idx * PAGE_SIZE
        if page_offset >= len(test_data):
            continue
        num_rows = test_data[page_offset + 0x18]
        if num_rows == 0:
            continue
        offsets = get_row_offsets(test_data, page_offset, num_rows)
        for off in offsets:
            row_start = page_offset + HEAP_START + off
            track = parse_track_row(test_data, row_start)
            test_tracks.append(track)
            print(f"  ID={track['id']:2d} '{track['title'][:40]:<40}' file_size={track['file_size']:>10}")

    print(f"\n=== Comparison ===")
    print(f"Reference: {len(ref_tracks)} tracks")
    print(f"Test: {len(test_tracks)} tracks")

    # Match by title
    print(f"\n=== Field Differences (matching by title) ===")
    for ref in ref_tracks:
        for test in test_tracks:
            if ref['title'] == test['title']:
                diffs = []
                for key in ['id', 'file_size', 'key_id', 'tempo', 'year', 'genre_id', 'album_id', 'artist_id', 'file_path']:
                    if ref[key] != test[key]:
                        diffs.append(f"{key}: ref={ref[key]} test={test[key]}")
                if diffs:
                    print(f"\n'{ref['title'][:50]}':")
                    for d in diffs:
                        print(f"  {d}")
                break

if __name__ == "__main__":
    compare_tracks(
        "/home/julien/Documents/Scripts/Pioneer/examples/PIONEER/rekordbox/export.pdb",
        "/tmp/pioneer_test/PIONEER/rekordbox/export.pdb"
    )
