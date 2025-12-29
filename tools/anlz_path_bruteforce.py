#!/usr/bin/env python3
"""
Brute-force tool to discover Pioneer's ANLZ path algorithm.

Target: Find what produces P04B/000154A5 for the waveform-1 reference track.
"""

import hashlib
import binascii
import struct
import sys

# Known reference data
REFERENCE = {
    "anlz_path": "P04B/000154A5",
    "p_value": 0x04B,  # 75 decimal
    "hash_value": 0x000154A5,  # 87205 decimal
    "file_path": "/Contents/BROOKLYN BOUNCE/The Theme (Of Progressive Attack)/This Is The Begining.mp3",
    "track_id": 1,
    "file_size": 34857,  # from reference
    "duration_ms": 3456,  # approximate from reference
    "sample_rate": 44100,
    "bitrate": 192,
}

# Hash algorithms to try
HASH_ALGOS = ['md5', 'sha1', 'sha256', 'sha512', 'sha3_256', 'blake2b', 'blake2s']

def try_hash(algo_name, data, encoding='utf-8'):
    """Try a hash algorithm and return various extractions."""
    if isinstance(data, str):
        data = data.encode(encoding)

    h = hashlib.new(algo_name)
    h.update(data)
    digest = h.hexdigest().upper()
    digest_bytes = h.digest()

    results = {
        'full': digest,
        'first_8': digest[:8],
        'first_4': digest[:4],
        'first_3': digest[:3],
        'last_8': digest[-8:],
        'last_4': digest[-4:],
        'last_3': digest[-3:],
    }

    # Extract numeric values
    if len(digest_bytes) >= 4:
        results['first_u32_le'] = struct.unpack('<I', digest_bytes[:4])[0]
        results['first_u32_be'] = struct.unpack('>I', digest_bytes[:4])[0]
        results['first_u16_le'] = struct.unpack('<H', digest_bytes[:2])[0]
        results['first_u16_be'] = struct.unpack('>H', digest_bytes[:2])[0]

    return results

def crc32_variants(data, encoding='utf-8'):
    """Try CRC32 with various tweaks."""
    if isinstance(data, str):
        data = data.encode(encoding)

    crc = binascii.crc32(data) & 0xFFFFFFFF
    return {
        'crc32': crc,
        'crc32_hex': f"{crc:08X}",
        'crc32_upper12': (crc >> 20) & 0xFFF,
        'crc32_lower24': crc & 0xFFFFFF,
        'crc32_upper16': (crc >> 16) & 0xFFFF,
        'crc32_lower16': crc & 0xFFFF,
    }

def adler32_variants(data, encoding='utf-8'):
    """Try Adler32."""
    import zlib
    if isinstance(data, str):
        data = data.encode(encoding)

    adler = zlib.adler32(data) & 0xFFFFFFFF
    return {
        'adler32': adler,
        'adler32_hex': f"{adler:08X}",
        'adler32_upper12': (adler >> 20) & 0xFFF,
        'adler32_lower24': adler & 0xFFFFFF,
    }

def check_match(p_value, hash_value, source_desc):
    """Check if values match reference."""
    p_match = p_value == REFERENCE['p_value']
    hash_match = hash_value == REFERENCE['hash_value']

    if p_match or hash_match:
        print(f"\n{'='*60}")
        print(f"PARTIAL MATCH: {source_desc}")
        print(f"  P value: {p_value:03X} (expected {REFERENCE['p_value']:03X}) {'MATCH!' if p_match else ''}")
        print(f"  Hash: {hash_value:08X} (expected {REFERENCE['hash_value']:08X}) {'MATCH!' if hash_match else ''}")
        print(f"{'='*60}")

    if p_match and hash_match:
        print("\n" + "!"*60)
        print("FULL MATCH FOUND!")
        print("!"*60)
        return True
    return False

def generate_inputs():
    """Generate all reasonable input variants."""
    track_id = REFERENCE['track_id']
    file_path = REFERENCE['file_path']
    file_size = REFERENCE['file_size']

    inputs = []

    # File path variants
    inputs.append(("file_path", file_path))
    inputs.append(("file_path_lower", file_path.lower()))
    inputs.append(("file_path_upper", file_path.upper()))
    inputs.append(("file_path_no_slash", file_path[1:]))  # Without leading /
    inputs.append(("file_path_backslash", file_path.replace('/', '\\')))

    # Just filename
    filename = file_path.split('/')[-1]
    inputs.append(("filename", filename))
    inputs.append(("filename_lower", filename.lower()))
    inputs.append(("filename_no_ext", filename.rsplit('.', 1)[0]))

    # Track ID based
    inputs.append(("track_id_str", str(track_id)))
    inputs.append(("track_id_padded", f"{track_id:08d}"))
    inputs.append(("track_{id}", f"track_{track_id}"))
    inputs.append(("TRACK_{id}", f"TRACK_{track_id}"))
    inputs.append(("{track_id}", f"{{{track_id}}}"))

    # Combined
    inputs.append(("id+path", f"{track_id}{file_path}"))
    inputs.append(("path+id", f"{file_path}{track_id}"))
    inputs.append(("id:path", f"{track_id}:{file_path}"))

    # File size based
    inputs.append(("file_size_str", str(file_size)))
    inputs.append(("size+path", f"{file_size}{file_path}"))
    inputs.append(("path+size", f"{file_path}{file_size}"))

    # Hex variants
    inputs.append(("track_id_hex", f"{track_id:08x}"))
    inputs.append(("track_id_hex_upper", f"{track_id:08X}"))

    # Pioneer specific guesses
    inputs.append(("rekordbox_{id}", f"rekordbox_{track_id}"))
    inputs.append(("rb_{id}", f"rb_{track_id}"))
    inputs.append(("ANLZ_{id}", f"ANLZ_{track_id}"))

    return inputs

def try_all_hashes():
    """Try all hash algorithms with all inputs."""
    inputs = generate_inputs()
    encodings = ['utf-8', 'utf-16le', 'utf-16be', 'ascii', 'latin-1']

    print("Testing hash algorithms...")
    print(f"Target: P{REFERENCE['p_value']:03X}/{REFERENCE['hash_value']:08X}")
    print(f"        = P04B/000154A5")
    print()

    for input_name, input_data in inputs:
        for encoding in encodings:
            try:
                # Skip non-ASCII compatible encodings for ASCII-only strings
                if encoding in ['utf-16le', 'utf-16be']:
                    data_bytes = input_data.encode(encoding)
                else:
                    data_bytes = input_data.encode(encoding)
            except:
                continue

            # Try CRC32
            crc_results = crc32_variants(input_data, encoding)

            # Check various extractions
            p_from_crc_upper = crc_results['crc32_upper12']
            hash_from_crc_lower = crc_results['crc32_lower24']
            check_match(p_from_crc_upper, hash_from_crc_lower,
                       f"CRC32({input_name}, {encoding}) upper12/lower24")

            # Also try lower bits for P value
            p_from_lower = crc_results['crc32_lower16'] & 0xFFF
            hash_from_upper = (crc_results['crc32'] >> 8) & 0xFFFFFF
            check_match(p_from_lower, hash_from_upper,
                       f"CRC32({input_name}, {encoding}) lower12/upper24")

            # Try Adler32
            adler_results = adler32_variants(input_data, encoding)
            p_from_adler = adler_results['adler32_upper12']
            hash_from_adler = adler_results['adler32_lower24']
            check_match(p_from_adler, hash_from_adler,
                       f"Adler32({input_name}, {encoding})")

            # Try MD5 and other hashes
            for algo in HASH_ALGOS:
                try:
                    results = try_hash(algo, input_data, encoding)

                    # Try extracting P value and hash from first bytes
                    if 'first_u32_le' in results:
                        u32 = results['first_u32_le']
                        p_val = (u32 >> 20) & 0xFFF
                        hash_val = u32 & 0xFFFFF  # 20 bits
                        # Extend to 24 bits for hash
                        hash_val_24 = u32 & 0xFFFFFF
                        check_match(p_val, hash_val_24,
                                   f"{algo.upper()}({input_name}, {encoding}) LE u32 split")

                        # Try different split
                        p_val2 = u32 & 0xFFF
                        hash_val2 = (u32 >> 12) & 0xFFFFFF
                        check_match(p_val2, hash_val2,
                                   f"{algo.upper()}({input_name}, {encoding}) LE u32 alt split")

                    if 'first_u32_be' in results:
                        u32 = results['first_u32_be']
                        p_val = (u32 >> 20) & 0xFFF
                        hash_val = u32 & 0xFFFFFF
                        check_match(p_val, hash_val,
                                   f"{algo.upper()}({input_name}, {encoding}) BE u32 split")

                    # Try hex string extractions
                    first_8 = results['first_8']
                    first_3 = results['first_3']
                    try:
                        p_from_first3 = int(first_3, 16)
                        hash_from_first8 = int(first_8, 16)
                        check_match(p_from_first3, hash_from_first8,
                                   f"{algo.upper()}({input_name}, {encoding}) hex first3/first8")
                    except:
                        pass

                except Exception as e:
                    pass

def try_simple_formulas():
    """Try simple numeric formulas."""
    track_id = REFERENCE['track_id']
    file_size = REFERENCE['file_size']
    target_p = REFERENCE['p_value']
    target_hash = REFERENCE['hash_value']

    print("\nTrying simple formulas...")

    # Direct relationships
    formulas = [
        ("track_id", track_id),
        ("track_id + 74", track_id + 74),  # 1 + 74 = 75 = 0x4B
        ("track_id * 75", track_id * 75),
        ("file_size", file_size),
        ("file_size % 0x1000", file_size % 0x1000),
        ("file_size & 0xFFF", file_size & 0xFFF),
    ]

    for name, p_val in formulas:
        if p_val == target_p:
            print(f"  P value formula candidate: {name} = {p_val:03X}")

    # Check if hash is related to track_id
    print(f"\n  Target hash: {target_hash} = 0x{target_hash:08X}")
    print(f"  Target P: {target_p} = 0x{target_p:03X}")

    # Is hash some function of track_id?
    if target_hash > 0:
        print(f"  hash / track_id = {target_hash / track_id}")
        print(f"  hash - track_id = {target_hash - track_id}")
        print(f"  hash ^ track_id = {target_hash ^ track_id}")

def try_rekordbox_internal_id():
    """
    Rekordbox may use internal database IDs, not export track IDs.
    The hash 0x154A5 = 87205 seems like it could be a database row ID.
    """
    print("\nAnalyzing hash value as potential database ID...")
    target_hash = REFERENCE['hash_value']

    print(f"  0x{target_hash:08X} = {target_hash} decimal")
    print(f"  Could be: internal rekordbox track ID from master database")
    print(f"  This would explain why same file produces different paths")
    print(f"  when exported by different Rekordbox instances")

def main():
    print("="*60)
    print("Pioneer ANLZ Path Algorithm Brute-Forcer")
    print("="*60)
    print()

    try_simple_formulas()
    try_rekordbox_internal_id()
    print()
    try_all_hashes()

    print("\n" + "="*60)
    print("Brute-force complete.")
    print()
    print("If no match found, the algorithm likely uses:")
    print("  1. Rekordbox's internal database track ID (not export ID)")
    print("  2. A proprietary hash we haven't identified")
    print("  3. A value stored elsewhere in the master rekordbox database")
    print("="*60)

if __name__ == "__main__":
    main()
