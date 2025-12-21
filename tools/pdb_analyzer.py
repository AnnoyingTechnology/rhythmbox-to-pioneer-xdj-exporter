#!/usr/bin/env python3
"""
PDB File Analyzer - Compare and analyze Pioneer export.pdb files
"""

import struct
import sys
from pathlib import Path

PAGE_SIZE = 4096

TABLE_NAMES = {
    0: "Tracks",
    1: "Genres",
    2: "Artists",
    3: "Albums",
    4: "Labels",
    5: "Keys",
    6: "Colors",
    7: "PlaylistTree",
    8: "PlaylistEntries",
    9: "Unknown09",
    10: "Unknown0A",
    11: "Unknown0B",
    12: "Unknown0C",
    13: "Artwork",
    14: "Unknown0E",
    15: "Unknown0F",
    16: "Columns",
    17: "HistoryPlaylists",
    18: "HistoryEntries",
    19: "History",
}


def read_file(path):
    with open(path, "rb") as f:
        return f.read()


def parse_header(data):
    """Parse file header (page 0)"""
    if len(data) < PAGE_SIZE:
        return None

    header = data[:PAGE_SIZE]

    # File header structure (first 0x1c bytes)
    # 0x00: u32 unknown (always 0)
    # 0x04: u32 page_size
    # 0x08: u32 num_tables
    # 0x0c: u32 next_unused_page
    # 0x10: u32 unknown
    # 0x14: u32 sequence
    # 0x18: u32 gap

    page_size = struct.unpack_from('<I', header, 0x04)[0]
    num_tables = struct.unpack_from('<I', header, 0x08)[0]
    next_unused = struct.unpack_from('<I', header, 0x0c)[0]
    unknown1 = struct.unpack_from('<I', header, 0x10)[0]
    sequence = struct.unpack_from('<I', header, 0x14)[0]
    gap = struct.unpack_from('<I', header, 0x18)[0]

    # Table pointers start at 0x1c
    tables = []
    for i in range(num_tables):
        offset = 0x1c + i * 16
        table_type = struct.unpack_from('<I', header, offset)[0]
        empty_candidate = struct.unpack_from('<I', header, offset + 4)[0]
        first_page = struct.unpack_from('<I', header, offset + 8)[0]
        last_page = struct.unpack_from('<I', header, offset + 12)[0]
        tables.append({
            'type': table_type,
            'name': TABLE_NAMES.get(table_type, f"Unknown({table_type})"),
            'empty_candidate': empty_candidate,
            'first_page': first_page,
            'last_page': last_page,
        })

    return {
        'page_size': page_size,
        'num_tables': num_tables,
        'next_unused_page': next_unused,
        'unknown1': unknown1,
        'sequence': sequence,
        'gap': gap,
        'tables': tables,
    }


def parse_page_header(data, page_idx):
    """Parse a page header (40 bytes)"""
    offset = page_idx * PAGE_SIZE
    if offset + PAGE_SIZE > len(data):
        return None

    page = data[offset:offset + PAGE_SIZE]

    # Page header structure (0x28 = 40 bytes)
    # 0x00: u32 padding (0)
    # 0x04: u32 page_index
    # 0x08: u32 table_type
    # 0x0c: u32 next_page
    # 0x10: u32 unknown1
    # 0x14: u32 unknown2
    # 0x18: u8 num_rows_small
    # 0x19: u8 unknown3
    # 0x1a: u8 unknown4
    # 0x1b: u8 page_flags
    # 0x1c: u16 free_size
    # 0x1e: u16 used_size
    # 0x20: u16 unknown5
    # 0x22: u16 num_rows_large
    # 0x24: u16 unknown6
    # 0x26: u16 unknown7

    return {
        'padding': struct.unpack_from('<I', page, 0x00)[0],
        'page_index': struct.unpack_from('<I', page, 0x04)[0],
        'table_type': struct.unpack_from('<I', page, 0x08)[0],
        'next_page': struct.unpack_from('<I', page, 0x0c)[0],
        'unknown1': struct.unpack_from('<I', page, 0x10)[0],
        'unknown2': struct.unpack_from('<I', page, 0x14)[0],
        'num_rows_small': page[0x18],
        'unknown3': page[0x19],
        'unknown4': page[0x1a],
        'page_flags': page[0x1b],
        'free_size': struct.unpack_from('<H', page, 0x1c)[0],
        'used_size': struct.unpack_from('<H', page, 0x1e)[0],
        'unknown5': struct.unpack_from('<H', page, 0x20)[0],
        'num_rows_large': struct.unpack_from('<H', page, 0x22)[0],
        'unknown6': struct.unpack_from('<H', page, 0x24)[0],
        'unknown7': struct.unpack_from('<H', page, 0x26)[0],
        'raw_header': page[:0x28],
        'raw_page': page,
    }


def parse_row_groups(page_data, num_rows):
    """Parse row groups from end of page"""
    if num_rows == 0:
        return []

    num_groups = (num_rows + 15) // 16
    groups = []

    for g in range(num_groups):
        # Row groups are 36 bytes each, stored at end of page growing backwards
        group_offset = PAGE_SIZE - (g + 1) * 36
        group = page_data[group_offset:group_offset + 36]

        # Format: 16 offsets (32 bytes) + flags (2 bytes) + unknown (2 bytes)
        # Offsets stored slot 15 first, slot 0 last
        offsets = []
        for slot in range(16):
            # Slot 15 is first, slot 0 is last
            off = struct.unpack_from('<H', group, (15 - slot) * 2)[0]
            offsets.append(off)

        flags = struct.unpack_from('<H', group, 32)[0]
        unknown = struct.unpack_from('<H', group, 34)[0]

        groups.append({
            'offsets': offsets,
            'flags': flags,
            'unknown': unknown,
            'raw': group,
        })

    return groups


def analyze_track_row(page_data, row_offset):
    """Analyze a track row starting at given offset (relative to page start)"""
    # Actual offset in page data is row_offset + 0x28 (after page header)
    start = 0x28 + row_offset

    if start + 0x5E > len(page_data):
        return None

    row = page_data[start:]

    # Track header (94 bytes = 0x5E)
    result = {
        'subtype': struct.unpack_from('<H', row, 0x00)[0],
        'index_shift': struct.unpack_from('<H', row, 0x02)[0],
        'bitmask': struct.unpack_from('<I', row, 0x04)[0],
        'sample_rate': struct.unpack_from('<I', row, 0x08)[0],
        'composer_id': struct.unpack_from('<I', row, 0x0c)[0],
        'file_size': struct.unpack_from('<I', row, 0x10)[0],
        'u2': struct.unpack_from('<I', row, 0x14)[0],
        'u3': struct.unpack_from('<H', row, 0x18)[0],
        'u4': struct.unpack_from('<H', row, 0x1a)[0],
        'artwork_id': struct.unpack_from('<I', row, 0x1c)[0],
        'key_id': struct.unpack_from('<I', row, 0x20)[0],
        'orig_artist_id': struct.unpack_from('<I', row, 0x24)[0],
        'label_id': struct.unpack_from('<I', row, 0x28)[0],
        'remixer_id': struct.unpack_from('<I', row, 0x2c)[0],
        'bitrate': struct.unpack_from('<I', row, 0x30)[0],
        'track_number': struct.unpack_from('<I', row, 0x34)[0],
        'tempo': struct.unpack_from('<I', row, 0x38)[0],
        'genre_id': struct.unpack_from('<I', row, 0x3c)[0],
        'album_id': struct.unpack_from('<I', row, 0x40)[0],
        'artist_id': struct.unpack_from('<I', row, 0x44)[0],
        'track_id': struct.unpack_from('<I', row, 0x48)[0],
        'disc_number': struct.unpack_from('<H', row, 0x4c)[0],
        'play_count': struct.unpack_from('<H', row, 0x4e)[0],
        'year': struct.unpack_from('<H', row, 0x50)[0],
        'sample_depth': struct.unpack_from('<H', row, 0x52)[0],
        'duration': struct.unpack_from('<H', row, 0x54)[0],
        'u5': struct.unpack_from('<H', row, 0x56)[0],
        'color_id': row[0x58],
        'rating': row[0x59],
        'file_type': struct.unpack_from('<H', row, 0x5a)[0],
        'u7': struct.unpack_from('<H', row, 0x5c)[0],
    }

    # String offsets (21 x u16)
    string_offsets = []
    for i in range(21):
        off = struct.unpack_from('<H', row, 0x5e + i * 2)[0]
        string_offsets.append(off)

    result['string_offsets'] = string_offsets
    result['raw_header'] = row[:0x88]  # Header + string offsets

    return result


def print_file_info(path, data):
    """Print detailed file information"""
    header = parse_header(data)
    if not header:
        print(f"ERROR: Failed to parse header for {path}")
        return

    num_pages = len(data) // PAGE_SIZE

    print(f"\n{'='*60}")
    print(f"FILE: {path}")
    print(f"{'='*60}")
    print(f"Size: {len(data)} bytes ({num_pages} pages)")
    print(f"Page size: {header['page_size']}")
    print(f"Num tables: {header['num_tables']}")
    print(f"Next unused page: {header['next_unused_page']}")
    print(f"Sequence: {header['sequence']}")

    print(f"\n--- Table Pointers ---")
    for t in header['tables']:
        print(f"  {t['name']:20s}: type={t['type']:2d}, first={t['first_page']:2d}, last={t['last_page']:2d}, empty_cand={t['empty_candidate']:2d}")

    # Analyze key pages
    print(f"\n--- Page Details ---")
    for page_idx in range(min(num_pages, 55)):
        ph = parse_page_header(data, page_idx)
        if not ph:
            continue

        # Skip empty pages
        if ph['page_index'] == 0 and ph['table_type'] == 0 and ph['next_page'] == 0:
            continue

        table_name = TABLE_NAMES.get(ph['table_type'], f"Type{ph['table_type']}")
        row_info = f"rows_s={ph['num_rows_small']}, rows_l={ph['num_rows_large']}" if ph['page_flags'] in [0x24, 0x34] else ""

        print(f"  Page {page_idx:2d}: {table_name:18s} next={ph['next_page']:2d} flags=0x{ph['page_flags']:02x} free={ph['free_size']:4d} used={ph['used_size']:4d} {row_info}")

    return header


def analyze_tracks_page(data, page_idx):
    """Detailed analysis of a tracks data page"""
    ph = parse_page_header(data, page_idx)
    if not ph:
        return

    page_data = ph['raw_page']

    print(f"\n--- Tracks Page {page_idx} Analysis ---")
    print(f"Page header: next={ph['next_page']}, flags=0x{ph['page_flags']:02x}")
    print(f"Rows: small={ph['num_rows_small']}, large={ph['num_rows_large']}")
    print(f"Free: {ph['free_size']}, Used: {ph['used_size']}")

    # Parse row groups
    num_rows = max(ph['num_rows_small'], ph['num_rows_large'])
    if num_rows > 0:
        groups = parse_row_groups(page_data, num_rows)
        print(f"\nRow groups ({len(groups)}):")
        for g_idx, g in enumerate(groups):
            print(f"  Group {g_idx}: flags=0x{g['flags']:04x} unknown=0x{g['unknown']:04x}")
            print(f"    Offsets: {g['offsets'][:num_rows] if num_rows <= 16 else g['offsets']}")

            # Analyze each row
            for slot in range(min(num_rows, 16)):
                if g['flags'] & (1 << slot):
                    row_offset = g['offsets'][slot]
                    track = analyze_track_row(page_data, row_offset)
                    if track:
                        print(f"    Row {slot}: offset=0x{row_offset:04x} track_id={track['track_id']} artist_id={track['artist_id']} album_id={track['album_id']}")
                        print(f"            subtype=0x{track['subtype']:04x} index_shift=0x{track['index_shift']:04x} bitmask=0x{track['bitmask']:08x}")
                        print(f"            tempo={track['tempo']} duration={track['duration']} file_type={track['file_type']}")
                        print(f"            string_offsets={track['string_offsets']}")


def compare_pages(data1, data2, page_idx):
    """Compare a specific page between two files"""
    off = page_idx * PAGE_SIZE

    if off + PAGE_SIZE > len(data1) or off + PAGE_SIZE > len(data2):
        print(f"Page {page_idx}: One or both files don't have this page")
        return

    page1 = data1[off:off + PAGE_SIZE]
    page2 = data2[off:off + PAGE_SIZE]

    if page1 == page2:
        print(f"Page {page_idx}: IDENTICAL")
        return

    # Find differences
    diffs = []
    for i in range(PAGE_SIZE):
        if page1[i] != page2[i]:
            diffs.append((i, page1[i], page2[i]))

    print(f"Page {page_idx}: {len(diffs)} byte differences")

    # Group consecutive differences
    if len(diffs) <= 50:
        for offset, b1, b2 in diffs[:50]:
            print(f"  0x{offset:04x}: 0x{b1:02x} vs 0x{b2:02x}")


def main():
    if len(sys.argv) < 2:
        print("Usage: pdb_analyzer.py <file.pdb> [file2.pdb] [--compare-page N]")
        sys.exit(1)

    path1 = Path(sys.argv[1])
    data1 = read_file(path1)

    header1 = print_file_info(path1, data1)

    # Always analyze tracks page 2
    analyze_tracks_page(data1, 2)

    if len(sys.argv) >= 3 and not sys.argv[2].startswith('--'):
        path2 = Path(sys.argv[2])
        data2 = read_file(path2)
        header2 = print_file_info(path2, data2)

        # Compare specific pages
        if '--compare-page' in sys.argv:
            idx = sys.argv.index('--compare-page')
            page_num = int(sys.argv[idx + 1])
            compare_pages(data1, data2, page_num)
        else:
            # Compare all pages up to smaller file
            num_pages = min(len(data1), len(data2)) // PAGE_SIZE
            print(f"\n--- Comparing {num_pages} pages ---")
            for p in range(num_pages):
                compare_pages(data1, data2, p)


if __name__ == '__main__':
    main()
