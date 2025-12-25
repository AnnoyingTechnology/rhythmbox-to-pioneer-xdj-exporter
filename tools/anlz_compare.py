#!/usr/bin/env python3
"""
ANLZ file comparison tool for debugging waveform issues.
Parses and compares DAT and EXT files between two exports.
"""

import sys
import struct
from pathlib import Path
from dataclasses import dataclass
from typing import List, Optional, Tuple, Dict

@dataclass
class Section:
    """Represents an ANLZ section"""
    tag: str
    header_len: int
    total_len: int
    offset: int
    data: bytes

    @property
    def payload(self) -> bytes:
        return self.data[self.header_len:]

def parse_anlz_file(path: Path) -> List[Section]:
    """Parse an ANLZ file (DAT or EXT) into sections"""
    data = path.read_bytes()
    sections = []
    offset = 0

    # First, parse the PMAI file header
    if len(data) < 28:
        return sections

    tag = data[0:4].decode('ascii', errors='replace')
    if tag != 'PMAI':
        print(f"Warning: Expected PMAI header, got {tag}")
        return sections

    pmai_header_len = struct.unpack('>I', data[4:8])[0]
    pmai_total_len = struct.unpack('>I', data[8:12])[0]

    # Add PMAI as first section (but it's really a file header)
    sections.append(Section('PMAI', pmai_header_len, pmai_total_len, 0, data[0:pmai_header_len]))

    # Now parse sections starting after PMAI header
    offset = pmai_header_len

    while offset < len(data):
        if offset + 12 > len(data):
            break

        tag = data[offset:offset+4].decode('ascii', errors='replace')
        header_len = struct.unpack('>I', data[offset+4:offset+8])[0]
        total_len = struct.unpack('>I', data[offset+8:offset+12])[0]

        if total_len == 0 or offset + total_len > len(data):
            break

        section_data = data[offset:offset+total_len]
        sections.append(Section(tag, header_len, total_len, offset, section_data))
        offset += total_len

    return sections

def format_bytes(data: bytes, max_bytes: int = 64) -> str:
    """Format bytes as hex string"""
    if len(data) <= max_bytes:
        return data.hex()
    return data[:max_bytes//2].hex() + "..." + data[-max_bytes//2:].hex()

def parse_pmai(section: Section) -> dict:
    """Parse PMAI file header"""
    data = section.data
    return {
        'header_len': section.header_len,
        'total_len': section.total_len,
        'unknown1': struct.unpack('>I', data[12:16])[0],
        'unknown2': struct.unpack('>I', data[16:20])[0],
        'unknown3': struct.unpack('>I', data[20:24])[0],
        'unknown4': struct.unpack('>I', data[24:28])[0] if len(data) >= 28 else None,
    }

def parse_ppth(section: Section) -> dict:
    """Parse PPTH path section"""
    data = section.data
    path_len = struct.unpack('>I', data[12:16])[0]
    path_bytes = data[16:16+path_len]
    # UTF-16BE path
    try:
        path = path_bytes.decode('utf-16-be').rstrip('\x00')
    except:
        path = path_bytes.hex()
    return {
        'header_len': section.header_len,
        'total_len': section.total_len,
        'path_len': path_len,
        'path': path,
    }

def parse_pvbr(section: Section) -> dict:
    """Parse PVBR variable bitrate info"""
    data = section.data
    return {
        'header_len': section.header_len,
        'total_len': section.total_len,
        'data_preview': format_bytes(data[12:], 32),
    }

def parse_pwav(section: Section) -> dict:
    """Parse PWAV waveform preview (400 bytes)"""
    data = section.data
    entry_count = struct.unpack('>I', data[12:16])[0]
    return {
        'header_len': section.header_len,
        'total_len': section.total_len,
        'entry_count': entry_count,
        'payload_size': len(section.payload),
        'first_16': section.payload[:16].hex() if len(section.payload) >= 16 else section.payload.hex(),
        'last_16': section.payload[-16:].hex() if len(section.payload) >= 16 else '',
    }

def parse_pwv2(section: Section) -> dict:
    """Parse PWV2 tiny preview (100 bytes)"""
    data = section.data
    entry_count = struct.unpack('>I', data[12:16])[0]
    return {
        'header_len': section.header_len,
        'total_len': section.total_len,
        'entry_count': entry_count,
        'payload_size': len(section.payload),
        'first_16': section.payload[:16].hex() if len(section.payload) >= 16 else section.payload.hex(),
    }

def parse_pwv3(section: Section) -> dict:
    """Parse PWV3 monochrome waveform detail"""
    data = section.data
    # Header: tag(4) + header_len(4) + total_len(4) + unknown(4) + entry_count(4) + unknown2(4)
    entry_count = struct.unpack('>I', data[16:20])[0]
    unknown1 = struct.unpack('>I', data[12:16])[0]
    unknown2 = struct.unpack('>H', data[20:22])[0]
    return {
        'header_len': section.header_len,
        'total_len': section.total_len,
        'unknown1': hex(unknown1),
        'entry_count': entry_count,
        'unknown2': hex(unknown2),
        'payload_size': len(section.payload),
        'first_32': section.payload[:32].hex() if len(section.payload) >= 32 else section.payload.hex(),
        'last_16': section.payload[-16:].hex() if len(section.payload) >= 16 else '',
    }

def parse_pwv5(section: Section) -> dict:
    """Parse PWV5 color waveform detail (2 bytes per entry)"""
    data = section.data
    # Header: tag(4) + header_len(4) + total_len(4) + unknown(4) + entry_count(4) + unknown2(4)
    entry_count = struct.unpack('>I', data[16:20])[0]
    unknown1 = struct.unpack('>I', data[12:16])[0]
    unknown2 = struct.unpack('>H', data[20:22])[0]
    payload = section.payload
    return {
        'header_len': section.header_len,
        'total_len': section.total_len,
        'unknown1': hex(unknown1),
        'entry_count': entry_count,
        'unknown2': hex(unknown2),
        'payload_size': len(payload),
        'first_32': payload[:32].hex() if len(payload) >= 32 else payload.hex(),
        'last_16': payload[-16:].hex() if len(payload) >= 16 else '',
    }

def parse_pwv4(section: Section) -> dict:
    """Parse PWV4 color preview (1200 x 6 bytes)"""
    data = section.data
    entry_count = struct.unpack('>I', data[16:20])[0]
    unknown1 = struct.unpack('>I', data[12:16])[0]
    return {
        'header_len': section.header_len,
        'total_len': section.total_len,
        'unknown1': hex(unknown1),
        'entry_count': entry_count,
        'payload_size': len(section.payload),
        'non_zero_bytes': sum(1 for b in section.payload if b != 0),
    }

def parse_pqtz(section: Section) -> dict:
    """Parse PQTZ beat grid"""
    data = section.data
    # Header: tag(4) + header_len(4) + total_len(4) + unknown(4) + unknown(4) + entry_count(4)
    entry_count = struct.unpack('>I', data[20:24])[0] if len(data) >= 24 else 0
    return {
        'header_len': section.header_len,
        'total_len': section.total_len,
        'entry_count': entry_count,
        'payload_size': len(section.payload),
    }

def parse_pcob(section: Section) -> dict:
    """Parse PCOB cue/loop section"""
    data = section.data
    entry_count = struct.unpack('>I', data[16:20])[0] if len(data) >= 20 else 0
    memory_count = struct.unpack('>I', data[20:24])[0] if len(data) >= 24 else 0
    return {
        'header_len': section.header_len,
        'total_len': section.total_len,
        'entry_count': entry_count,
        'memory_count': memory_count,
    }

def parse_pco2(section: Section) -> dict:
    """Parse PCO2 extended cue section"""
    data = section.data
    entry_count = struct.unpack('>I', data[16:20])[0] if len(data) >= 20 else 0
    return {
        'header_len': section.header_len,
        'total_len': section.total_len,
        'entry_count': entry_count,
    }

PARSERS = {
    'PMAI': parse_pmai,
    'PPTH': parse_ppth,
    'PVBR': parse_pvbr,
    'PWAV': parse_pwav,
    'PWV2': parse_pwv2,
    'PWV3': parse_pwv3,
    'PWV5': parse_pwv5,
    'PWV4': parse_pwv4,
    'PQTZ': parse_pqtz,
    'PCOB': parse_pcob,
    'PCO2': parse_pco2,
}

def analyze_file(path: Path) -> None:
    """Analyze a single ANLZ file"""
    print(f"\n{'='*60}")
    print(f"File: {path}")
    print(f"Size: {path.stat().st_size} bytes")
    print(f"{'='*60}")

    sections = parse_anlz_file(path)

    for section in sections:
        print(f"\n[{section.tag}] @ offset {hex(section.offset)}")
        print(f"  header_len: {section.header_len}, total_len: {section.total_len}")

        parser = PARSERS.get(section.tag)
        if parser:
            info = parser(section)
            for k, v in info.items():
                if k not in ('header_len', 'total_len'):
                    print(f"  {k}: {v}")
        else:
            print(f"  raw data: {format_bytes(section.data, 32)}")

def compare_sections(sec1: Section, sec2: Section, name1: str, name2: str) -> List[str]:
    """Compare two sections and return differences"""
    diffs = []

    if sec1.header_len != sec2.header_len:
        diffs.append(f"  header_len: {sec1.header_len} vs {sec2.header_len}")
    if sec1.total_len != sec2.total_len:
        diffs.append(f"  total_len: {sec1.total_len} vs {sec2.total_len}")

    # Compare payloads byte by byte
    p1, p2 = sec1.payload, sec2.payload
    if len(p1) != len(p2):
        diffs.append(f"  payload size: {len(p1)} vs {len(p2)}")

    min_len = min(len(p1), len(p2))
    diff_count = sum(1 for i in range(min_len) if p1[i] != p2[i])
    if diff_count > 0:
        diffs.append(f"  differing bytes: {diff_count}/{min_len}")
        # Find first difference
        for i in range(min_len):
            if p1[i] != p2[i]:
                diffs.append(f"  first diff at offset {i}: {hex(p1[i])} vs {hex(p2[i])}")
                break

    return diffs

def compare_files(path1: Path, path2: Path) -> None:
    """Compare two ANLZ files"""
    print(f"\n{'='*60}")
    print(f"Comparing:")
    print(f"  A: {path1}")
    print(f"  B: {path2}")
    print(f"{'='*60}")

    sections1 = parse_anlz_file(path1)
    sections2 = parse_anlz_file(path2)

    tags1 = [s.tag for s in sections1]
    tags2 = [s.tag for s in sections2]

    print(f"\nSection order A: {' -> '.join(tags1)}")
    print(f"Section order B: {' -> '.join(tags2)}")

    if tags1 != tags2:
        print("\n*** SECTION ORDER DIFFERS! ***")

    # Compare each section
    sections2_by_tag = {s.tag: s for s in sections2}

    for sec1 in sections1:
        sec2 = sections2_by_tag.get(sec1.tag)
        if not sec2:
            print(f"\n[{sec1.tag}] - MISSING in B")
            continue

        diffs = compare_sections(sec1, sec2, "A", "B")
        if diffs:
            print(f"\n[{sec1.tag}] - DIFFERS:")
            for d in diffs:
                print(d)
        else:
            print(f"\n[{sec1.tag}] - IDENTICAL")

    # Check for sections only in B
    tags1_set = set(tags1)
    for sec2 in sections2:
        if sec2.tag not in tags1_set:
            print(f"\n[{sec2.tag}] - ONLY in B")

def main():
    if len(sys.argv) < 2:
        print("Usage:")
        print("  anlz_compare.py <file>              - Analyze single file")
        print("  anlz_compare.py <file1> <file2>     - Compare two files")
        print("  anlz_compare.py <dir1> <dir2>       - Compare two ANLZ directories")
        sys.exit(1)

    path1 = Path(sys.argv[1])

    if len(sys.argv) == 2:
        # Single file analysis
        if path1.is_file():
            analyze_file(path1)
        elif path1.is_dir():
            for f in sorted(path1.glob('ANLZ*.*')):
                analyze_file(f)
        else:
            print(f"Error: {path1} not found")
            sys.exit(1)
    else:
        # Comparison mode
        path2 = Path(sys.argv[2])

        if path1.is_file() and path2.is_file():
            compare_files(path1, path2)
        elif path1.is_dir() and path2.is_dir():
            # Find ANLZ files in both
            files1 = {f.name: f for f in path1.glob('ANLZ*.*')}
            files2 = {f.name: f for f in path2.glob('ANLZ*.*')}

            for name in sorted(set(files1.keys()) | set(files2.keys())):
                if name in files1 and name in files2:
                    compare_files(files1[name], files2[name])
                elif name in files1:
                    print(f"\n{name}: ONLY in A")
                else:
                    print(f"\n{name}: ONLY in B")
        else:
            print("Error: Both arguments must be files or both must be directories")
            sys.exit(1)

if __name__ == '__main__':
    main()
