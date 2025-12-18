#!/usr/bin/env python3
"""
Create hybrid PDBs to isolate which data page is causing the XDJ to fail.

Strategy: Start with reference, replace one table's data page at a time with ours.
When it breaks, we've found the culprit.
"""

from pathlib import Path
import shutil

PAGE_SIZE = 4096

def create_hybrid(ref_path, test_path, output_path, pages_from_test, description):
    """Create hybrid with specific pages from test, rest from reference."""
    ref_data = bytearray(Path(ref_path).read_bytes())
    test_data = bytearray(Path(test_path).read_bytes())

    # Start with reference
    output_data = bytearray(ref_data)

    # Copy specified pages from test
    for page_idx in pages_from_test:
        start = page_idx * PAGE_SIZE
        end = start + PAGE_SIZE
        if end <= len(test_data) and end <= len(output_data):
            output_data[start:end] = test_data[start:end]

    Path(output_path).write_bytes(output_data)
    print(f"Created: {output_path}")
    print(f"  {description}")
    print(f"  Pages from test: {pages_from_test}")

def main():
    ref = "/home/julien/Documents/Scripts/Pioneer/examples/PIONEER/rekordbox/export.pdb"
    test = "/tmp/pioneer_test/PIONEER/rekordbox/export.pdb"
    out_dir = Path("/tmp/pioneer_isolation")
    out_dir.mkdir(exist_ok=True)

    # Test 1: Reference with our Tracks data pages (2, 51)
    create_hybrid(ref, test, out_dir / "test1_our_tracks.pdb",
                  [2, 51], "Our Tracks data pages only")

    # Test 2: Reference with our Genres data page (4)
    create_hybrid(ref, test, out_dir / "test2_our_genres.pdb",
                  [4], "Our Genres data page only")

    # Test 3: Reference with our Artists data page (6)
    create_hybrid(ref, test, out_dir / "test3_our_artists.pdb",
                  [6], "Our Artists data page only")

    # Test 4: Reference with our Albums data page (8)
    create_hybrid(ref, test, out_dir / "test4_our_albums.pdb",
                  [8], "Our Albums data page only")

    # Test 5: Reference with our Colors data page (14)
    create_hybrid(ref, test, out_dir / "test5_our_colors.pdb",
                  [14], "Our Colors data page only")

    # Test 6: Reference with our PlaylistTree data page (16)
    create_hybrid(ref, test, out_dir / "test6_our_playlist_tree.pdb",
                  [16], "Our PlaylistTree data page only")

    # Test 7: Reference with our PlaylistEntries data page (18)
    create_hybrid(ref, test, out_dir / "test7_our_playlist_entries.pdb",
                  [18], "Our PlaylistEntries data page only")

    # Test 8: Reference with our Columns data page (34)
    create_hybrid(ref, test, out_dir / "test8_our_columns.pdb",
                  [34], "Our Columns data page only")

    # Test 9: All simple tables (Genres, Artists, Albums, Colors)
    create_hybrid(ref, test, out_dir / "test9_simple_tables.pdb",
                  [4, 6, 8, 14], "Our simple entity tables")

    # Test 10: All playlist-related tables
    create_hybrid(ref, test, out_dir / "test10_playlist_tables.pdb",
                  [16, 18], "Our playlist tables")

    print(f"\nCreated {10} test files in {out_dir}")
    print("\nTest order recommendation:")
    print("1. test9_simple_tables.pdb - if fails, test 2-5 individually")
    print("2. test10_playlist_tables.pdb - if fails, test 6-7 individually")
    print("3. test1_our_tracks.pdb - tracks are complex")
    print("4. test8_our_columns.pdb - columns table")

if __name__ == "__main__":
    main()
