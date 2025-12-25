#!/usr/bin/env python3
"""
PDB track row comparison tool.
Compares track row data between two export.pdb files.
"""

import sys
import struct
from pathlib import Path

def find_track_rows(data: bytes) -> list:
    """Find all track rows by searching for subtype 0x0024"""
    rows = []
    # Track rows start with subtype 0x0024 (little-endian)
    marker = b'\x24\x00'

    offset = 0
    while True:
        pos = data.find(marker, offset)
        if pos == -1:
            break
        # Basic validation: check if this looks like a track row
        # Track rows are at least 0x9e bytes long
        if pos + 0x9e <= len(data):
            rows.append(pos)
        offset = pos + 1

    return rows

def decode_track_row(data: bytes, offset: int) -> dict:
    """Decode track row fields"""
    row = data[offset:offset + 0x200]  # Read up to 512 bytes

    result = {
        'offset': hex(offset),
        'subtype': row[0:2].hex(),
        'index_shift': struct.unpack('<H', row[2:4])[0],
        'bitmask': row[4:8].hex(),
        'sample_rate': struct.unpack('<I', row[8:12])[0],
        'composer_id': struct.unpack('<I', row[12:16])[0],
        'file_size': struct.unpack('<I', row[16:20])[0],
        'u2': struct.unpack('<I', row[20:24])[0],
        'u3': struct.unpack('<H', row[24:26])[0],
        'u4': struct.unpack('<H', row[26:28])[0],
        'artwork_id': struct.unpack('<I', row[28:32])[0],
        'key_id': struct.unpack('<I', row[32:36])[0],
        'orig_artist_id': struct.unpack('<I', row[36:40])[0],
        'label_id': struct.unpack('<I', row[40:44])[0],
        'remixer_id': struct.unpack('<I', row[44:48])[0],
        'bitrate': struct.unpack('<I', row[48:52])[0],
        'track_number': struct.unpack('<I', row[52:56])[0],
        'tempo': struct.unpack('<I', row[56:60])[0],  # BPM * 100
        'genre_id': struct.unpack('<I', row[60:64])[0],
        'album_id': struct.unpack('<I', row[64:68])[0],
        'artist_id': struct.unpack('<I', row[68:72])[0],
        'id': struct.unpack('<I', row[72:76])[0],
        'disc_number': struct.unpack('<H', row[76:78])[0],
        'play_count': struct.unpack('<H', row[78:80])[0],
        'year': struct.unpack('<H', row[80:82])[0],
        'sample_depth': struct.unpack('<H', row[82:84])[0],
        'duration': struct.unpack('<H', row[84:86])[0],
        'u5': struct.unpack('<H', row[86:88])[0],
        'color_id': row[88],
        'rating': row[89],
        'u6': struct.unpack('<H', row[90:92])[0],
        'u7': struct.unpack('<H', row[92:94])[0],
        # String offsets start at 0x5e (94)
    }

    return result

def main():
    if len(sys.argv) < 2:
        print("Usage: pdb_track_compare.py <pdb_file1> [pdb_file2]")
        sys.exit(1)

    path1 = Path(sys.argv[1])
    data1 = path1.read_bytes()

    rows1 = find_track_rows(data1)
    print(f"File: {path1}")
    print(f"Found {len(rows1)} potential track rows")

    for i, offset in enumerate(rows1[:5]):  # First 5 rows
        print(f"\n=== Track Row {i} ===")
        row = decode_track_row(data1, offset)
        for key, value in row.items():
            print(f"  {key}: {value}")

    if len(sys.argv) >= 3:
        path2 = Path(sys.argv[2])
        data2 = path2.read_bytes()
        rows2 = find_track_rows(data2)

        print(f"\n\nFile: {path2}")
        print(f"Found {len(rows2)} potential track rows")

        for i, offset in enumerate(rows2[:5]):
            print(f"\n=== Track Row {i} ===")
            row = decode_track_row(data2, offset)
            for key, value in row.items():
                print(f"  {key}: {value}")

        if rows1 and rows2:
            print("\n\n=== BITMASK COMPARISON ===")
            row1 = decode_track_row(data1, rows1[0])
            row2 = decode_track_row(data2, rows2[0])
            print(f"Reference bitmask: {row1['bitmask']}")
            print(f"Our bitmask:       {row2['bitmask']}")

            bm1 = int(row1['bitmask'], 16)
            bm2 = int(row2['bitmask'], 16)
            if bm1 != bm2:
                print(f"DIFFERENCE: {bin(bm1)} vs {bin(bm2)}")
                print(f"Reference bits: {bin(bm1)}")
                print(f"Our bits:       {bin(bm2)}")

if __name__ == '__main__':
    main()
