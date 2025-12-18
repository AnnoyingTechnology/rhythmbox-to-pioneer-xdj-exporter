#!/usr/bin/env python3
"""
Create a hybrid PDB that uses our header structure with reference data pages.
This helps isolate whether the issue is in headers vs data content.
"""

import shutil
from pathlib import Path

PAGE_SIZE = 4096

def create_hybrid(ref_path, test_path, output_path):
    """
    Create a hybrid PDB:
    - Use test file's page 0 (file header with table pointers)
    - Use test file's header pages (1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35, 37, 39)
    - Use reference file's data pages (2, 4, 6, 8, 10, 12, 14, 16, 18, 28, 34, 36, 38, 40, 51)
    - Use test file's empty candidate pages
    """
    ref_data = bytearray(Path(ref_path).read_bytes())
    test_data = bytearray(Path(test_path).read_bytes())

    # Start with a copy of test (to get correct file size and empty pages)
    output_data = bytearray(test_data)

    # Copy data pages from reference
    data_pages = [2, 4, 6, 8, 10, 12, 14, 16, 18, 28, 34, 36, 38, 40, 51]

    for page_idx in data_pages:
        ref_start = page_idx * PAGE_SIZE
        ref_end = ref_start + PAGE_SIZE

        if ref_end <= len(ref_data):
            output_data[ref_start:ref_end] = ref_data[ref_start:ref_end]
            print(f"Copied page {page_idx} from reference")

    Path(output_path).write_bytes(output_data)
    print(f"\nCreated hybrid PDB at {output_path}")
    print(f"File size: {len(output_data)} bytes")

def create_reference_with_our_headers(ref_path, test_path, output_path):
    """
    Alternative: Start with reference, replace header pages with ours.
    This tests if our header page content is correct.
    """
    ref_data = bytearray(Path(ref_path).read_bytes())
    test_data = bytearray(Path(test_path).read_bytes())

    # Start with reference
    output_data = bytearray(ref_data)

    # Copy header pages from test
    header_pages = [0, 1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35, 37, 39]

    for page_idx in header_pages:
        test_start = page_idx * PAGE_SIZE
        test_end = test_start + PAGE_SIZE

        if test_end <= len(test_data) and test_end <= len(output_data):
            output_data[test_start:test_end] = test_data[test_start:test_end]
            print(f"Copied page {page_idx} (header) from test")

    Path(output_path).write_bytes(output_data)
    print(f"\nCreated hybrid PDB at {output_path}")
    print(f"File size: {len(output_data)} bytes")

if __name__ == "__main__":
    import sys

    ref = "/home/julien/Documents/Scripts/Pioneer/examples/PIONEER/rekordbox/export.pdb"
    test = "/tmp/pioneer_test/PIONEER/rekordbox/export.pdb"

    if len(sys.argv) > 1 and sys.argv[1] == "headers":
        # Test our headers with reference data
        output = "/tmp/hybrid_our_headers.pdb"
        create_reference_with_our_headers(ref, test, output)
    else:
        # Test reference data with our structure
        output = "/tmp/hybrid_ref_data.pdb"
        create_hybrid(ref, test, output)
