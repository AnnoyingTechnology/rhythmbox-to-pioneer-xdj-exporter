#!/usr/bin/env python3
"""
Create an export by patching the reference PDB file.
This is the most reliable way to create a valid export since we start
from a known-working file and only change what's necessary.
"""

import struct
import sys
import os
import shutil
from pathlib import Path

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
    if isinstance(s, str):
        b = s.encode('utf-8')
    else:
        b = s
    if len(b) > 126:
        raise ValueError(f"String too long for short format: {len(b)}")
    header = ((len(b) + 1) << 1) | 1
    return bytes([header]) + b

def patch_string(data, page_num, heap_offset, new_str, max_len=None):
    """
    Patch a string in place. The new string must fit in the same space.
    Returns True if successful.
    """
    file_offset = page_num * PAGE_SIZE + HEAP_START + heap_offset
    new_encoded = encode_short_string(new_str)

    if max_len and len(new_encoded) > max_len:
        print(f"Error: New string too long ({len(new_encoded)} > {max_len})")
        return False

    data[file_offset:file_offset + len(new_encoded)] = new_encoded
    return True

def main():
    ref_dir = Path("/home/julien/Documents/Scripts/Pioneer/examples/single-playlist-single-track")
    out_dir = Path("/tmp/patched_export")

    # Clean and copy reference
    if out_dir.exists():
        shutil.rmtree(out_dir)
    shutil.copytree(ref_dir, out_dir)

    print(f"Copied reference export to {out_dir}")

    # The reference export uses these paths:
    # File: /Contents/Rihanna/Unapologetic/06_Rihanna_-_Jump.flac
    # ANLZ: /PIONEER/USBANLZ/P03F/0000E2D5/ANLZ0000.DAT

    # These match the actual files in the reference, so no patching needed!
    # The reference should work as-is.

    pdb_path = out_dir / "PIONEER" / "rekordbox" / "export.pdb"
    print(f"\nPDB file: {pdb_path}")
    print(f"Size: {pdb_path.stat().st_size} bytes")

    # List all files
    print("\nExport contents:")
    for root, dirs, files in os.walk(out_dir):
        level = root.replace(str(out_dir), '').count(os.sep)
        indent = ' ' * 2 * level
        print(f"{indent}{os.path.basename(root)}/")
        sub_indent = ' ' * 2 * (level + 1)
        for f in files:
            fpath = Path(root) / f
            size = fpath.stat().st_size
            print(f"{sub_indent}{f} ({size:,} bytes)")

    print(f"\n=== COPY TO USB ===")
    print(f"Copy the contents of {out_dir} to a USB drive to test.")
    print(f"The directory structure should be:")
    print(f"  USB_ROOT/")
    print(f"    Contents/")
    print(f"      Rihanna/Unapologetic/06_Rihanna_-_Jump.flac")
    print(f"    PIONEER/")
    print(f"      rekordbox/export.pdb")
    print(f"      USBANLZ/P03F/0000E2D5/ANLZ0000.DAT")
    print(f"      Artwork/00001/a1.jpg, a1_m.jpg")

if __name__ == '__main__':
    main()
