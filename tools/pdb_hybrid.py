#!/usr/bin/env python3
"""
PDB Hybrid Creator - Create hybrid PDBs by swapping pages between files

Usage:
  pdb_hybrid.py <base.pdb> <donor.pdb> <output.pdb> <page_nums...>

Example:
  # Take base file, replace pages 2,4,6 from donor
  pdb_hybrid.py reference.pdb ours.pdb hybrid.pdb 2 4 6
"""

import sys
from pathlib import Path

PAGE_SIZE = 4096


def create_hybrid(base_path, donor_path, output_path, pages_to_swap):
    """Create a hybrid PDB by swapping specific pages"""
    base_data = bytearray(Path(base_path).read_bytes())
    donor_data = Path(donor_path).read_bytes()

    print(f"Base: {base_path} ({len(base_data)} bytes)")
    print(f"Donor: {donor_path} ({len(donor_data)} bytes)")
    print(f"Swapping pages: {pages_to_swap}")

    for page_num in pages_to_swap:
        base_offset = page_num * PAGE_SIZE
        donor_offset = page_num * PAGE_SIZE

        if base_offset + PAGE_SIZE > len(base_data):
            print(f"  Page {page_num}: Extending base file")
            base_data.extend(b'\x00' * (base_offset + PAGE_SIZE - len(base_data)))

        if donor_offset + PAGE_SIZE > len(donor_data):
            print(f"  Page {page_num}: SKIP (donor doesn't have this page)")
            continue

        donor_page = donor_data[donor_offset:donor_offset + PAGE_SIZE]
        base_data[base_offset:base_offset + PAGE_SIZE] = donor_page
        print(f"  Page {page_num}: Swapped")

    Path(output_path).write_bytes(base_data)
    print(f"Output: {output_path} ({len(base_data)} bytes)")


def main():
    if len(sys.argv) < 5:
        print(__doc__)
        sys.exit(1)

    base_path = sys.argv[1]
    donor_path = sys.argv[2]
    output_path = sys.argv[3]
    pages = [int(x) for x in sys.argv[4:]]

    create_hybrid(base_path, donor_path, output_path, pages)


if __name__ == '__main__':
    main()
