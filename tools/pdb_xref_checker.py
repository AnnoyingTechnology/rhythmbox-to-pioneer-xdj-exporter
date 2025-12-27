#!/usr/bin/env python3
"""
PDB Cross-Reference Integrity Checker

Validates that all IDs referenced in tracks exist in their respective tables.
"""

import struct
import sys
from pathlib import Path

PAGE_SIZE = 4096
HEAP_START = 0x28


def get_row_offsets(page_data, num_rows):
    """Get row offsets from page footer"""
    if num_rows == 0:
        return []

    num_groups = (num_rows + 15) // 16
    footer_size = 0
    for g in range(num_groups):
        rows_in_group = min(16, num_rows - g * 16)
        footer_size += rows_in_group * 2 + 4

    footer_start = PAGE_SIZE - footer_size
    footer = page_data[footer_start:]

    offsets = []
    pos = 0
    for g in range(num_groups - 1, -1, -1):
        rows_in_group = min(16, num_rows - g * 16)
        for slot in range(rows_in_group - 1, -1, -1):
            off = struct.unpack('<H', footer[pos:pos+2])[0]
            offsets.append((g * 16 + slot, off))
            pos += 2
        pos += 4

    offsets.sort()
    return [off for _, off in offsets]


def read_simple_table_ids(data, first_page, last_page, id_offset=0):
    """Read IDs from a simple table (Genres, Artists, Albums, Keys)"""
    ids = set()
    current_page = first_page

    while current_page <= last_page:
        page_start = current_page * PAGE_SIZE
        page_data = data[page_start:page_start + PAGE_SIZE]

        # Check page flags
        page_flags = page_data[0x1b]
        if page_flags == 0x64:  # Header page
            current_page = struct.unpack('<I', page_data[0x0c:0x10])[0]
            continue

        num_rows = page_data[0x18]
        if num_rows == 0:
            break

        offsets = get_row_offsets(page_data, num_rows)

        for off in offsets:
            row_start = HEAP_START + off
            # For simple tables, ID is at the start of the row
            row_id = struct.unpack('<I', page_data[row_start + id_offset:row_start + id_offset + 4])[0]
            ids.add(row_id)

        next_page = struct.unpack('<I', page_data[0x0c:0x10])[0]
        if next_page == current_page or next_page >= len(data) // PAGE_SIZE:
            break
        current_page = next_page

    return ids


def read_track_refs(data, first_page, last_page):
    """Read all ID references from tracks"""
    refs = {
        'artist_ids': set(),
        'album_ids': set(),
        'genre_ids': set(),
        'key_ids': set(),
        'label_ids': set(),
        'artwork_ids': set(),
    }

    current_page = first_page

    while current_page <= last_page:
        page_start = current_page * PAGE_SIZE
        page_data = data[page_start:page_start + PAGE_SIZE]

        page_flags = page_data[0x1b]
        if page_flags == 0x64:
            current_page = struct.unpack('<I', page_data[0x0c:0x10])[0]
            continue

        num_rows = page_data[0x18]
        if num_rows == 0:
            break

        offsets = get_row_offsets(page_data, num_rows)

        for off in offsets:
            row = page_data[HEAP_START + off:]

            refs['artist_ids'].add(struct.unpack('<I', row[0x44:0x48])[0])
            refs['album_ids'].add(struct.unpack('<I', row[0x40:0x44])[0])
            refs['genre_ids'].add(struct.unpack('<I', row[0x3c:0x40])[0])
            refs['key_ids'].add(struct.unpack('<I', row[0x20:0x24])[0])
            refs['label_ids'].add(struct.unpack('<I', row[0x28:0x2c])[0])
            refs['artwork_ids'].add(struct.unpack('<I', row[0x1c:0x20])[0])

        next_page = struct.unpack('<I', page_data[0x0c:0x10])[0]
        if next_page == current_page or next_page >= len(data) // PAGE_SIZE:
            break
        current_page = next_page

    return refs


def check_xrefs(path):
    """Check cross-reference integrity"""
    data = Path(path).read_bytes()

    print(f"Checking: {path}")
    print(f"Size: {len(data)} bytes ({len(data) // PAGE_SIZE} pages)")
    print()

    # Read table pointers
    def get_table_info(table_idx):
        offset = 0x1c + table_idx * 16
        return {
            'first': struct.unpack('<I', data[offset + 8:offset + 12])[0],
            'last': struct.unpack('<I', data[offset + 12:offset + 16])[0],
        }

    tracks = get_table_info(0)
    genres = get_table_info(1)
    artists = get_table_info(2)
    albums = get_table_info(3)
    labels = get_table_info(4)
    keys = get_table_info(5)
    artwork = get_table_info(13)

    # Read IDs from tables
    genre_ids = read_simple_table_ids(data, genres['first'], genres['last'])
    artist_ids = read_simple_table_ids(data, artists['first'], artists['last'])
    album_ids = read_simple_table_ids(data, albums['first'], albums['last'])
    key_ids = read_simple_table_ids(data, keys['first'], keys['last'])
    label_ids = read_simple_table_ids(data, labels['first'], labels['last'])
    artwork_ids = read_simple_table_ids(data, artwork['first'], artwork['last'])

    print(f"Table contents:")
    print(f"  Genres: {len(genre_ids)} rows, IDs: {sorted(genre_ids)[:10]}{'...' if len(genre_ids) > 10 else ''}")
    print(f"  Artists: {len(artist_ids)} rows, IDs: {sorted(artist_ids)[:10]}{'...' if len(artist_ids) > 10 else ''}")
    print(f"  Albums: {len(album_ids)} rows, IDs: {sorted(album_ids)[:10]}{'...' if len(album_ids) > 10 else ''}")
    print(f"  Keys: {len(key_ids)} rows, IDs: {sorted(key_ids)[:10]}{'...' if len(key_ids) > 10 else ''}")
    print(f"  Labels: {len(label_ids)} rows")
    print(f"  Artwork: {len(artwork_ids)} rows")
    print()

    # Read track references
    refs = read_track_refs(data, tracks['first'], tracks['last'])

    # Remove 0 (null reference)
    for key in refs:
        refs[key].discard(0)

    print(f"Track references (excluding 0):")
    print(f"  artist_ids: {sorted(refs['artist_ids'])}")
    print(f"  album_ids: {sorted(refs['album_ids'])}")
    print(f"  genre_ids: {sorted(refs['genre_ids'])}")
    print(f"  key_ids: {sorted(refs['key_ids'])}")
    print(f"  label_ids: {sorted(refs['label_ids'])}")
    print(f"  artwork_ids: {sorted(refs['artwork_ids'])}")
    print()

    # Check for missing references
    issues = []

    missing_artists = refs['artist_ids'] - artist_ids
    if missing_artists:
        issues.append(f"Missing artist IDs: {missing_artists}")

    missing_albums = refs['album_ids'] - album_ids
    if missing_albums:
        issues.append(f"Missing album IDs: {missing_albums}")

    missing_genres = refs['genre_ids'] - genre_ids
    if missing_genres:
        issues.append(f"Missing genre IDs: {missing_genres}")

    missing_keys = refs['key_ids'] - key_ids
    if missing_keys:
        issues.append(f"Missing key IDs: {missing_keys}")

    missing_labels = refs['label_ids'] - label_ids
    if missing_labels:
        issues.append(f"Missing label IDs: {missing_labels}")

    missing_artwork = refs['artwork_ids'] - artwork_ids
    if missing_artwork:
        issues.append(f"Missing artwork IDs: {missing_artwork}")

    # Check for unreferenced entries (orphans)
    unreferenced_genres = genre_ids - refs['genre_ids']
    if unreferenced_genres:
        issues.append(f"Unreferenced genre IDs: {unreferenced_genres}")

    unreferenced_artists = artist_ids - refs['artist_ids']
    if unreferenced_artists:
        issues.append(f"Unreferenced artist IDs: {unreferenced_artists}")

    unreferenced_albums = album_ids - refs['album_ids']
    if unreferenced_albums:
        issues.append(f"Unreferenced album IDs: {unreferenced_albums}")

    unreferenced_keys = key_ids - refs['key_ids']
    if unreferenced_keys:
        issues.append(f"Unreferenced key IDs: {sorted(unreferenced_keys)}")

    if issues:
        print("ISSUES FOUND:")
        for issue in issues:
            print(f"  - {issue}")
    else:
        print("No cross-reference issues found.")

    return len(issues) == 0


def main():
    if len(sys.argv) < 2:
        print("Usage: pdb_xref_checker.py <file.pdb> [file2.pdb ...]")
        sys.exit(1)

    all_ok = True
    for path in sys.argv[1:]:
        ok = check_xrefs(path)
        if not ok:
            all_ok = False
        print()
        print("=" * 60)
        print()

    sys.exit(0 if all_ok else 1)


if __name__ == '__main__':
    main()
