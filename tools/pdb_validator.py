#!/usr/bin/env python3
"""
Comprehensive PDB Validator - Check DeviceSQL database integrity

This tool validates the internal consistency of Pioneer export.pdb files
and can compare against reference exports.
"""

import struct
import sys
from pathlib import Path
from dataclasses import dataclass, field
from typing import List, Dict, Optional, Tuple

PAGE_SIZE = 4096
HEAP_START = 0x28  # 40 bytes

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


@dataclass
class ValidationError:
    severity: str  # "ERROR", "WARNING", "INFO"
    category: str
    message: str
    page: Optional[int] = None

    def __str__(self):
        loc = f" (page {self.page})" if self.page is not None else ""
        return f"[{self.severity}] {self.category}{loc}: {self.message}"


@dataclass
class TablePointer:
    type_id: int
    name: str
    empty_candidate: int
    first_page: int
    last_page: int


@dataclass
class PageHeader:
    page_index: int
    table_type: int
    next_page: int
    unknown1: int
    unknown2: int
    num_rows_small: int
    unknown3: int
    unknown4: int
    page_flags: int
    free_size: int
    used_size: int
    unknown5: int
    num_rows_large: int
    unknown6: int
    unknown7: int


@dataclass
class RowGroup:
    offsets: List[int]
    flags: int
    unknown: int


@dataclass
class FileHeader:
    page_size: int
    num_tables: int
    next_unused_page: int
    unknown1: int
    sequence: int
    gap: int
    tables: List[TablePointer]


class PDBValidator:
    def __init__(self, data: bytes, filename: str = ""):
        self.data = data
        self.filename = filename
        self.errors: List[ValidationError] = []
        self.header: Optional[FileHeader] = None
        self.pages: Dict[int, PageHeader] = {}

    def error(self, category: str, message: str, page: Optional[int] = None):
        self.errors.append(ValidationError("ERROR", category, message, page))

    def warning(self, category: str, message: str, page: Optional[int] = None):
        self.errors.append(ValidationError("WARNING", category, message, page))

    def info(self, category: str, message: str, page: Optional[int] = None):
        self.errors.append(ValidationError("INFO", category, message, page))

    def parse_header(self) -> Optional[FileHeader]:
        """Parse file header (page 0)"""
        if len(self.data) < PAGE_SIZE:
            self.error("Header", "File too small for header page")
            return None

        page_size = struct.unpack_from('<I', self.data, 0x04)[0]
        num_tables = struct.unpack_from('<I', self.data, 0x08)[0]
        next_unused = struct.unpack_from('<I', self.data, 0x0c)[0]
        unknown1 = struct.unpack_from('<I', self.data, 0x10)[0]
        sequence = struct.unpack_from('<I', self.data, 0x14)[0]
        gap = struct.unpack_from('<I', self.data, 0x18)[0]

        tables = []
        for i in range(num_tables):
            offset = 0x1c + i * 16
            table_type = struct.unpack_from('<I', self.data, offset)[0]
            empty_candidate = struct.unpack_from('<I', self.data, offset + 4)[0]
            first_page = struct.unpack_from('<I', self.data, offset + 8)[0]
            last_page = struct.unpack_from('<I', self.data, offset + 12)[0]
            tables.append(TablePointer(
                type_id=table_type,
                name=TABLE_NAMES.get(table_type, f"Unknown({table_type})"),
                empty_candidate=empty_candidate,
                first_page=first_page,
                last_page=last_page,
            ))

        return FileHeader(
            page_size=page_size,
            num_tables=num_tables,
            next_unused_page=next_unused,
            unknown1=unknown1,
            sequence=sequence,
            gap=gap,
            tables=tables,
        )

    def parse_page_header(self, page_idx: int) -> Optional[PageHeader]:
        """Parse a page header"""
        offset = page_idx * PAGE_SIZE
        if offset + PAGE_SIZE > len(self.data):
            return None

        page = self.data[offset:offset + PAGE_SIZE]

        return PageHeader(
            page_index=struct.unpack_from('<I', page, 0x04)[0],
            table_type=struct.unpack_from('<I', page, 0x08)[0],
            next_page=struct.unpack_from('<I', page, 0x0c)[0],
            unknown1=struct.unpack_from('<I', page, 0x10)[0],
            unknown2=struct.unpack_from('<I', page, 0x14)[0],
            num_rows_small=page[0x18],
            unknown3=page[0x19],
            unknown4=page[0x1a],
            page_flags=page[0x1b],
            free_size=struct.unpack_from('<H', page, 0x1c)[0],
            used_size=struct.unpack_from('<H', page, 0x1e)[0],
            unknown5=struct.unpack_from('<H', page, 0x20)[0],
            num_rows_large=struct.unpack_from('<H', page, 0x22)[0],
            unknown6=struct.unpack_from('<H', page, 0x24)[0],
            unknown7=struct.unpack_from('<H', page, 0x26)[0],
        )

    def parse_row_groups(self, page_data: bytes, num_rows: int) -> List[RowGroup]:
        """Parse row groups from end of page"""
        if num_rows == 0:
            return []

        num_groups = (num_rows + 15) // 16
        groups = []

        for g in range(num_groups):
            rows_in_group = min(16, num_rows - g * 16)

            # Group size: rows_in_group * 2 (offsets) + 4 (flags + unknown)
            group_size = rows_in_group * 2 + 4
            group_offset = PAGE_SIZE - sum(min(16, num_rows - j * 16) * 2 + 4 for j in range(g + 1))

            if group_offset < 0 or group_offset + group_size > PAGE_SIZE:
                break

            group = page_data[group_offset:group_offset + group_size]

            # Offsets stored in reverse order within group
            offsets = []
            for slot in range(rows_in_group):
                off = struct.unpack_from('<H', group, (rows_in_group - 1 - slot) * 2)[0]
                offsets.append(off)

            flags = struct.unpack_from('<H', group, rows_in_group * 2)[0]
            unknown = struct.unpack_from('<H', group, rows_in_group * 2 + 2)[0]

            groups.append(RowGroup(offsets=offsets, flags=flags, unknown=unknown))

        return groups

    def validate_file_structure(self):
        """Validate basic file structure"""
        num_pages = len(self.data) // PAGE_SIZE

        # Check file size is page-aligned
        if len(self.data) % PAGE_SIZE != 0:
            self.error("FileStructure", f"File size {len(self.data)} not page-aligned (remainder {len(self.data) % PAGE_SIZE})")

        # Parse header
        self.header = self.parse_header()
        if not self.header:
            return

        # Validate page size
        if self.header.page_size != PAGE_SIZE:
            self.error("Header", f"Unexpected page_size: {self.header.page_size} (expected {PAGE_SIZE})")

        # Validate table count
        if self.header.num_tables != 20:
            self.warning("Header", f"num_tables={self.header.num_tables} (expected 20 for standard PDB)")

        # next_unused_page should be >= all table last_page values
        max_last_page = max(t.last_page for t in self.header.tables) if self.header.tables else 0
        # Note: next_unused may be less than file page count if empty_candidate pages exist

    def validate_table_chains(self):
        """Validate table page chains"""
        if not self.header:
            return

        num_pages = len(self.data) // PAGE_SIZE

        for table in self.header.tables:
            # Track visited pages to detect loops
            visited = set()
            current = table.first_page
            chain = []

            while current < num_pages:
                if current in visited:
                    self.error("TableChain", f"Loop detected in {table.name} chain at page {current}")
                    break

                visited.add(current)
                chain.append(current)

                ph = self.parse_page_header(current)
                if not ph:
                    self.error("TableChain", f"{table.name}: Cannot read page {current}")
                    break

                self.pages[current] = ph

                # Validate table type matches
                if ph.table_type != table.type_id:
                    self.error("TableChain", f"{table.name}: Page {current} has type {ph.table_type} (expected {table.type_id})")

                # Check if this is the last page
                if current == table.last_page:
                    # Last page should point to empty_candidate
                    if ph.next_page != table.empty_candidate:
                        self.warning("TableChain", f"{table.name}: Last page {current} next_page={ph.next_page} (expected empty_candidate={table.empty_candidate})")
                    break

                # Follow chain
                if ph.next_page >= num_pages and ph.next_page != table.empty_candidate:
                    # Check if it's within reserved range 41-52
                    if 41 <= ph.next_page <= 52:
                        # This is OK - empty candidate in reserved range
                        break
                    self.error("TableChain", f"{table.name}: Page {current} next_page={ph.next_page} out of range (file has {num_pages} pages)")
                    break

                current = ph.next_page

            # Verify chain ends at last_page
            if chain and chain[-1] != table.last_page:
                self.warning("TableChain", f"{table.name}: Chain ends at page {chain[-1]} (expected last_page={table.last_page})")

    def validate_page_headers(self):
        """Validate individual page headers"""
        if not self.header:
            return

        num_pages = len(self.data) // PAGE_SIZE

        for page_idx in range(1, num_pages):  # Skip page 0 (file header)
            ph = self.parse_page_header(page_idx)
            if not ph:
                continue

            # Skip truly empty pages (all zeros)
            if ph.page_index == 0 and ph.table_type == 0 and ph.next_page == 0 and ph.page_flags == 0:
                continue

            table_name = TABLE_NAMES.get(ph.table_type, f"Type{ph.table_type}")

            # Validate page_flags
            valid_flags = {0x24, 0x34, 0x44, 0x64}
            if ph.page_flags not in valid_flags:
                self.warning("PageHeader", f"Unusual page_flags 0x{ph.page_flags:02x}", page_idx)

            # Data pages (0x24, 0x34) should have valid row counts
            if ph.page_flags in {0x24, 0x34}:
                num_rows = ph.num_rows_small

                # num_rows_large relationship
                # For normal pages: num_rows_large = num_rows - 1 (or 0 when num_rows <= 1)
                # For pages with 8191: this is a special marker
                if ph.num_rows_large == 0x1fff:  # 8191
                    self.info("PageHeader", f"{table_name}: num_rows_large=8191 (special marker)", page_idx)
                elif num_rows > 1 and ph.num_rows_large != num_rows - 1:
                    self.warning("PageHeader", f"{table_name}: num_rows_large={ph.num_rows_large} (expected {num_rows - 1})", page_idx)

                # Validate free_size + used_size
                available_space = PAGE_SIZE - HEAP_START
                # Row groups take space at the end
                num_groups = (num_rows + 15) // 16
                row_group_space = 0
                for g in range(num_groups):
                    rows_in_group = min(16, num_rows - g * 16)
                    row_group_space += rows_in_group * 2 + 4

                usable_space = available_space - row_group_space

                if ph.free_size + ph.used_size != usable_space and num_rows > 0:
                    # Allow some slack for alignment
                    diff = abs((ph.free_size + ph.used_size) - usable_space)
                    if diff > 4:
                        self.warning("PageHeader", f"{table_name}: free_size({ph.free_size}) + used_size({ph.used_size}) = {ph.free_size + ph.used_size} (expected ~{usable_space})", page_idx)

    def validate_row_groups(self):
        """Validate row group structures"""
        if not self.header:
            return

        num_pages = len(self.data) // PAGE_SIZE

        for page_idx in range(1, num_pages):
            ph = self.parse_page_header(page_idx)
            if not ph or ph.page_flags not in {0x24, 0x34}:
                continue

            num_rows = ph.num_rows_small
            if num_rows == 0:
                continue

            offset = page_idx * PAGE_SIZE
            page_data = self.data[offset:offset + PAGE_SIZE]

            groups = self.parse_row_groups(page_data, num_rows)
            table_name = TABLE_NAMES.get(ph.table_type, f"Type{ph.table_type}")

            total_present = 0
            for g_idx, group in enumerate(groups):
                rows_in_group = len(group.offsets)

                # Count present bits
                present_count = bin(group.flags).count('1')
                total_present += present_count

                # Validate flags matches row count
                expected_flags = (1 << rows_in_group) - 1
                if group.flags != expected_flags:
                    self.warning("RowGroup", f"{table_name}: Group {g_idx} flags=0x{group.flags:04x} (expected 0x{expected_flags:04x} for {rows_in_group} rows)", page_idx)

                # Validate unknown field
                # Full groups (16 rows, flags=0xffff) have unknown=0
                # Partial groups have unknown = 2^(highest_set_bit)
                if group.flags == 0xffff:
                    expected_unknown = 0
                elif group.flags == 0:
                    expected_unknown = 0
                else:
                    highest_bit = group.flags.bit_length() - 1
                    expected_unknown = 1 << highest_bit

                if group.unknown != expected_unknown:
                    self.warning("RowGroup", f"{table_name}: Group {g_idx} unknown=0x{group.unknown:04x} (expected 0x{expected_unknown:04x})", page_idx)

                # Validate row offsets are within heap
                for slot, offset_val in enumerate(group.offsets):
                    if offset_val >= ph.used_size:
                        self.warning("RowGroup", f"{table_name}: Group {g_idx} slot {slot} offset 0x{offset_val:04x} >= used_size {ph.used_size}", page_idx)

            if total_present != num_rows:
                self.warning("RowGroup", f"{table_name}: Total present rows {total_present} != num_rows_small {num_rows}", page_idx)

    def validate_all(self):
        """Run all validations"""
        self.validate_file_structure()
        self.validate_table_chains()
        self.validate_page_headers()
        self.validate_row_groups()

    def print_summary(self):
        """Print validation summary"""
        print(f"\n{'='*60}")
        print(f"PDB Validation: {self.filename}")
        print(f"{'='*60}")

        if self.header:
            num_pages = len(self.data) // PAGE_SIZE
            print(f"File: {len(self.data)} bytes ({num_pages} pages)")
            print(f"Tables: {self.header.num_tables}")
            print(f"next_unused_page: {self.header.next_unused_page}")
            print(f"sequence: {self.header.sequence}")

        # Count by severity
        errors = [e for e in self.errors if e.severity == "ERROR"]
        warnings = [e for e in self.errors if e.severity == "WARNING"]
        infos = [e for e in self.errors if e.severity == "INFO"]

        print(f"\nResults: {len(errors)} errors, {len(warnings)} warnings, {len(infos)} info")

        if errors:
            print("\n--- ERRORS ---")
            for e in errors:
                print(f"  {e}")

        if warnings:
            print("\n--- WARNINGS ---")
            for e in warnings:
                print(f"  {e}")

        if infos and "-v" in sys.argv:
            print("\n--- INFO ---")
            for e in infos:
                print(f"  {e}")

        return len(errors) == 0


def compare_pdbs(file1: bytes, file2: bytes, name1: str, name2: str):
    """Compare two PDB files and report differences"""
    print(f"\n{'='*60}")
    print(f"Comparing: {name1} vs {name2}")
    print(f"{'='*60}")

    # Parse both headers
    def parse_header(data):
        page_size = struct.unpack_from('<I', data, 0x04)[0]
        num_tables = struct.unpack_from('<I', data, 0x08)[0]
        next_unused = struct.unpack_from('<I', data, 0x0c)[0]
        sequence = struct.unpack_from('<I', data, 0x14)[0]
        return page_size, num_tables, next_unused, sequence

    h1 = parse_header(file1)
    h2 = parse_header(file2)

    print(f"\n{name1}: {len(file1)} bytes ({len(file1)//PAGE_SIZE} pages), next_unused={h1[2]}, seq={h1[3]}")
    print(f"{name2}: {len(file2)} bytes ({len(file2)//PAGE_SIZE} pages), next_unused={h2[2]}, seq={h2[3]}")

    # Compare page by page
    num_pages = min(len(file1), len(file2)) // PAGE_SIZE
    different_pages = []

    for page_idx in range(num_pages):
        off = page_idx * PAGE_SIZE
        p1 = file1[off:off + PAGE_SIZE]
        p2 = file2[off:off + PAGE_SIZE]

        if p1 != p2:
            diff_count = sum(1 for a, b in zip(p1, p2) if a != b)
            different_pages.append((page_idx, diff_count))

    print(f"\nPage differences: {len(different_pages)} of {num_pages} pages differ")

    for page_idx, diff_count in different_pages[:20]:  # Limit output
        # Parse both page headers
        ph1 = struct.unpack_from('<IIII', file1, page_idx * PAGE_SIZE)
        ph2 = struct.unpack_from('<IIII', file2, page_idx * PAGE_SIZE)

        table_type = ph1[2] if ph1[2] else ph2[2]
        table_name = TABLE_NAMES.get(table_type, f"Type{table_type}")

        print(f"  Page {page_idx:2d} ({table_name:18s}): {diff_count} byte differences")

    if len(different_pages) > 20:
        print(f"  ... and {len(different_pages) - 20} more pages")


def main():
    if len(sys.argv) < 2:
        print("Usage: pdb_validator.py <file.pdb> [file2.pdb] [-v]")
        print("  -v: verbose (show INFO messages)")
        sys.exit(1)

    files = [arg for arg in sys.argv[1:] if not arg.startswith('-')]

    # Validate first file
    path1 = Path(files[0])
    data1 = path1.read_bytes()

    validator = PDBValidator(data1, str(path1))
    validator.validate_all()
    is_valid = validator.print_summary()

    # If second file provided, compare them
    if len(files) >= 2:
        path2 = Path(files[1])
        data2 = path2.read_bytes()

        validator2 = PDBValidator(data2, str(path2))
        validator2.validate_all()
        validator2.print_summary()

        compare_pdbs(data1, data2, str(path1), str(path2))

    sys.exit(0 if is_valid else 1)


if __name__ == '__main__':
    main()
