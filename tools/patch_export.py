#!/usr/bin/env python3
"""
Patch a reference PDB export with new track data.
This tool takes a known-working reference export and patches only the
dynamic parts (strings, IDs) while preserving the exact binary structure.
"""

import struct
import sys
import os
import shutil
from pathlib import Path

# Constants
PAGE_SIZE = 4096
HEAP_START = 0x28

def read_file(path):
    with open(path, "rb") as f:
        return bytearray(f.read())

def write_file(path, data):
    with open(path, "wb") as f:
        f.write(data)

def encode_short_string(s):
    """Encode a short ASCII string in DeviceSQL format"""
    b = s.encode('utf-8') if isinstance(s, str) else s
    if len(b) > 126:
        raise ValueError(f"String too long for short format: {len(b)}")
    header = ((len(b) + 1) << 1) | 1
    return bytes([header]) + b

def find_string_in_page(page_data, search_str):
    """Find a DeviceSQL-encoded string in page data"""
    encoded = encode_short_string(search_str)
    pos = page_data.find(encoded)
    return pos

def replace_string_at(data, offset, old_str, new_str):
    """Replace a string at a specific offset, must be same length"""
    old_encoded = encode_short_string(old_str)
    new_encoded = encode_short_string(new_str)

    if len(old_encoded) != len(new_encoded):
        raise ValueError(f"String length mismatch: old={len(old_encoded)}, new={len(new_encoded)}")

    # Verify old string is at expected position
    actual = data[offset:offset + len(old_encoded)]
    if actual != old_encoded:
        print(f"Warning: Expected {old_encoded.hex()} at offset {offset}, found {actual.hex()}")
        return False

    data[offset:offset + len(new_encoded)] = new_encoded
    return True

def patch_pdb_strings(pdb_data, old_values, new_values):
    """
    Patch specific strings in the PDB.
    old_values and new_values are dicts with keys like 'anlz_path', 'file_path', etc.
    """
    # Page 2 is the tracks data page
    page2_start = 2 * PAGE_SIZE
    page2 = pdb_data[page2_start:page2_start + PAGE_SIZE]

    changes = []

    for key in old_values:
        old_val = old_values[key]
        new_val = new_values.get(key, old_val)

        if old_val == new_val:
            continue

        # Find the string in page 2
        encoded_old = encode_short_string(old_val)
        pos = page2.find(encoded_old)

        if pos == -1:
            print(f"Warning: Could not find '{key}' string: {old_val[:50]}...")
            continue

        # The string appears twice in the reference (once for each row slot)
        # Find all occurrences
        offset = 0
        while True:
            pos = page2.find(encoded_old, offset)
            if pos == -1:
                break

            file_offset = page2_start + pos
            changes.append((file_offset, old_val, new_val))
            offset = pos + 1

    # Apply changes
    for file_offset, old_val, new_val in changes:
        if len(old_val) != len(new_val):
            print(f"Error: Cannot replace '{old_val[:30]}' with '{new_val[:30]}' - different lengths")
            continue

        old_encoded = encode_short_string(old_val)
        new_encoded = encode_short_string(new_val)

        print(f"Patching at offset {file_offset}: {old_val[:40]}... -> {new_val[:40]}...")
        pdb_data[file_offset:file_offset + len(new_encoded)] = new_encoded

    return pdb_data

def main():
    if len(sys.argv) < 3:
        print("Usage: patch_export.py <reference_dir> <output_dir> [new_track_path]")
        print("")
        print("This tool copies a reference export and optionally patches paths")
        print("to point to a different audio file location.")
        sys.exit(1)

    ref_dir = Path(sys.argv[1])
    out_dir = Path(sys.argv[2])

    # Copy the entire reference structure
    if out_dir.exists():
        shutil.rmtree(out_dir)
    shutil.copytree(ref_dir, out_dir)

    print(f"Copied reference export to {out_dir}")

    # If no track path specified, just copy without patching
    if len(sys.argv) < 4:
        print("No patching requested - using reference as-is")
        return

    new_track_path = sys.argv[3]

    # The reference export paths (from analyzing the reference)
    old_values = {
        'file_path': '/Contents/Rihanna/Unapologetic/06_Rihanna_-_Jump.flac',
        'filename': '/06_Rihanna_-_Jump.flac',
        'anlz_path': '/PIONEER/USBANLZ/P03F/0000E2D5/ANLZ0000.DAT',
    }

    # New values would need to match the new track
    # For now, just demonstrate the patching capability
    print("Reference paths in PDB:")
    for k, v in old_values.items():
        print(f"  {k}: {v}")

    # Read and patch PDB
    pdb_path = out_dir / "PIONEER" / "rekordbox" / "export.pdb"
    pdb_data = read_file(pdb_path)

    # Example: just verify the strings are there
    page2_start = 2 * PAGE_SIZE
    page2 = pdb_data[page2_start:page2_start + PAGE_SIZE]

    for key, val in old_values.items():
        encoded = encode_short_string(val)
        count = page2.count(encoded)
        print(f"Found '{key}' string {count} time(s) in page 2")

    print(f"\nOutput at: {out_dir}")
    print("Copy this directory to a USB drive to test on XDJ")

if __name__ == '__main__':
    main()
