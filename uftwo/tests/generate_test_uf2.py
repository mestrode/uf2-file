#!/usr/bin/env python3
"""
Generate test UF2 files according to the UF2 specification.
Reference: https://github.com/microsoft/uf2
"""

import struct
import hashlib
import os

# UF2 constants
BLOCK_SIZE = 512
MAX_PAYLOAD_SIZE = 476
MAGIC_START0 = 0x0A324655  # "UF2\n"
MAGIC_START1 = 0x9E5D5157
MAGIC_END = 0x0AB16F30

# Flag constants
FLAG_NOFLASH = 0x00000001
FLAG_FILECONTAINER = 0x00001000
FLAG_FAMILY_ID = 0x00002000
FLAG_MD5_CHKSUM = 0x00004000
FLAG_EXTENSION_TAGS = 0x00008000

# Extension tags
TAG_SEMVER = 0x9FC7BC
TAG_DESCRIPTION = 0x650D9D
TAG_PAGE_SIZE = 0x0BE9F7
TAG_SHA2_CHKSUM = 0xB46DB0
TAG_DEVICE_TYPE = 0xC8A729


def create_uf2_block(
    target_addr: int,
    payload: bytes,
    block_no: int,
    total_blocks: int,
    flags: int = 0,
    family_id: int = 0,
    file_size: int = 0,
    checksum: bytes = None,
    extensions: list = None,
) -> bytes:
    """Create a single UF2 block."""
    # Validate payload size
    if len(payload) > MAX_PAYLOAD_SIZE:
        raise ValueError(f"Payload too large: {len(payload)} > {MAX_PAYLOAD_SIZE}")

    # Create data array (476 bytes, padded with zeros)
    data = bytearray(MAX_PAYLOAD_SIZE)
    data[: len(payload)] = payload

    # Add extensions if present
    if extensions:
        payload_size = len(payload)
        # Align to 4-byte boundary
        pos = (payload_size + 3) & ~3
        for tag_type, tag_data in extensions:
            # Tag format: [length (1 byte), tag (3 bytes), data (variable)]
            # Length includes the 4-byte header
            tag_bytes = tag_type.to_bytes(3, "little")
            tag_len = 1 + 3 + len(tag_data)  # length byte + tag + data
            # Pad to 4-byte boundary
            padded_len = (tag_len + 3) & ~3

            if pos + padded_len > MAX_PAYLOAD_SIZE:
                raise ValueError("Extensions exceed payload size")

            # Write tag
            data[pos] = tag_len
            data[pos + 1 : pos + 4] = tag_bytes
            data[pos + 4 : pos + 4 + len(tag_data)] = tag_data
            pos += padded_len

    # Add checksum if present
    if checksum:
        if len(checksum) != 24:
            raise ValueError("Checksum must be 24 bytes")
        checksum_start = MAX_PAYLOAD_SIZE - 24
        data[checksum_start : checksum_start + 24] = checksum

    # Create block
    block = struct.pack(
        "<IIIIIIII",
        MAGIC_START0,
        MAGIC_START1,
        flags,
        target_addr,
        len(payload),
        block_no,
        total_blocks,
        family_id if flags & FLAG_FAMILY_ID else file_size,
    )
    block += bytes(data)
    block += struct.pack("<I", MAGIC_END)

    if len(block) != BLOCK_SIZE:
        raise ValueError(f"Block size mismatch: {len(block)} != {BLOCK_SIZE}")

    return block


def compute_md5(data: bytes, start_addr: int, length: int) -> bytes:
    """Compute MD5 checksum for UF2 block."""
    md5_hash = hashlib.md5(data).digest()
    # UF2 checksum format: start_addr (4 bytes), length (4 bytes), checksum (16 bytes)
    return struct.pack("<II16s", start_addr, length, md5_hash)


def generate_simple_uf2(filename: str, data: bytes, target_addr: int = 0x2000):
    """Generate a simple UF2 file with 256-byte blocks."""
    blocks = []
    chunk_size = 256

    for i, chunk in enumerate(
        [data[j : j + chunk_size] for j in range(0, len(data), chunk_size)]
    ):
        block = create_uf2_block(
            target_addr=target_addr + i * chunk_size,
            payload=chunk,
            block_no=i,
            total_blocks=(len(data) + chunk_size - 1) // chunk_size,
        )
        blocks.append(block)

    with open(filename, "wb") as f:
        for block in blocks:
            f.write(block)

    print(f"Generated {filename}: {len(blocks)} blocks, {len(data)} bytes")


def generate_uf2_with_family_id(
    filename: str, data: bytes, family_id: int, target_addr: int = 0x2000
):
    """Generate a UF2 file with family ID."""
    blocks = []
    chunk_size = 256

    for i, chunk in enumerate(
        [data[j : j + chunk_size] for j in range(0, len(data), chunk_size)]
    ):
        block = create_uf2_block(
            target_addr=target_addr + i * chunk_size,
            payload=chunk,
            block_no=i,
            total_blocks=(len(data) + chunk_size - 1) // chunk_size,
            flags=FLAG_FAMILY_ID,
            family_id=family_id,
        )
        blocks.append(block)

    with open(filename, "wb") as f:
        for block in blocks:
            f.write(block)

    print(
        f"Generated {filename}: {len(blocks)} blocks, {len(data)} bytes, family_id=0x{family_id:08x}"
    )


def generate_uf2_with_checksum(
    filename: str, data: bytes, target_addr: int = 0x2000, page_size: int = 256
):
    """Generate a UF2 file with MD5 checksums for each page."""
    blocks = []
    chunk_size = 256

    for i, chunk in enumerate(
        [data[j : j + chunk_size] for j in range(0, len(data), chunk_size)]
    ):
        # Compute checksum for the entire page (just this chunk for simplicity)
        checksum = compute_md5(chunk, target_addr + i * chunk_size, len(chunk))

        block = create_uf2_block(
            target_addr=target_addr + i * chunk_size,
            payload=chunk,
            block_no=i,
            total_blocks=(len(data) + chunk_size - 1) // chunk_size,
            flags=FLAG_MD5_CHKSUM,
            checksum=checksum,
        )
        blocks.append(block)

    with open(filename, "wb") as f:
        for block in blocks:
            f.write(block)

    print(
        f"Generated {filename}: {len(blocks)} blocks, {len(data)} bytes, with MD5 checksums"
    )


def generate_uf2_with_extensions(
    filename: str,
    data: bytes,
    target_addr: int = 0x2000,
    semver: str = "1.0.0",
    description: str = "Test Device",
    page_size: int = 256,
):
    """Generate a UF2 file with extensions in the first block."""
    blocks = []
    chunk_size = 256
    total_blocks = (len(data) + chunk_size - 1) // chunk_size

    # First block with extensions
    extensions = [
        (TAG_SEMVER, semver.encode("utf-8")),
        (TAG_DESCRIPTION, description.encode("utf-8")),
        (TAG_PAGE_SIZE, struct.pack("<I", page_size)),
    ]

    first_chunk = data[:chunk_size]
    block = create_uf2_block(
        target_addr=target_addr,
        payload=first_chunk,
        block_no=0,
        total_blocks=total_blocks,
        flags=FLAG_EXTENSION_TAGS,
        extensions=extensions,
    )
    blocks.append(block)

    # Remaining blocks
    for i, chunk in enumerate(
        [data[j : j + chunk_size] for j in range(chunk_size, len(data), chunk_size)]
    ):
        block = create_uf2_block(
            target_addr=target_addr + (i + 1) * chunk_size,
            payload=chunk,
            block_no=i + 1,
            total_blocks=total_blocks,
        )
        blocks.append(block)

    with open(filename, "wb") as f:
        for block in blocks:
            f.write(block)

    print(
        f"Generated {filename}: {len(blocks)} blocks, {len(data)} bytes, with extensions"
    )


def generate_uf2_full_spec(
    filename: str,
    data: bytes,
    target_addr: int = 0x2000,
    family_id: int = 0x12345678,
    page_size: int = 256,
    semver: str = "1.0.0",
    description: str = "Test Device",
):
    """Generate a fully spec-compliant UF2 file with family ID, checksums, and extensions."""
    blocks = []
    chunk_size = 256
    total_blocks = (len(data) + chunk_size - 1) // chunk_size

    # First block with family ID, checksum, and extensions
    first_chunk = data[:chunk_size]

    # Compute checksum for the entire page
    checksum = compute_md5(first_chunk, target_addr, len(first_chunk))

    # Extensions
    extensions = [
        (TAG_SEMVER, semver.encode("utf-8")),
        (TAG_DESCRIPTION, description.encode("utf-8")),
        (TAG_PAGE_SIZE, struct.pack("<I", page_size)),
    ]

    block = create_uf2_block(
        target_addr=target_addr,
        payload=first_chunk,
        block_no=0,
        total_blocks=total_blocks,
        flags=FLAG_FAMILY_ID | FLAG_MD5_CHKSUM | FLAG_EXTENSION_TAGS,
        family_id=family_id,
        checksum=checksum,
        extensions=extensions,
    )
    blocks.append(block)

    # Remaining blocks with checksums
    for i, chunk in enumerate(
        [data[j : j + chunk_size] for j in range(chunk_size, len(data), chunk_size)]
    ):
        checksum = compute_md5(chunk, target_addr + (i + 1) * chunk_size, len(chunk))

        block = create_uf2_block(
            target_addr=target_addr + (i + 1) * chunk_size,
            payload=chunk,
            block_no=i + 1,
            total_blocks=total_blocks,
            flags=FLAG_FAMILY_ID | FLAG_MD5_CHKSUM,
            family_id=family_id,
            checksum=checksum,
        )
        blocks.append(block)

    with open(filename, "wb") as f:
        for block in blocks:
            f.write(block)

    print(f"Generated {filename}: {len(blocks)} blocks, {len(data)} bytes, full spec")


def generate_minimal_uf2(filename: str):
    """Generate a minimal UF2 file with a single block."""
    data = b"\x00" * 256  # 256 bytes of zeros
    block = create_uf2_block(
        target_addr=0x2000, payload=data, block_no=0, total_blocks=1
    )

    with open(filename, "wb") as f:
        f.write(block)

    print(f"Generated {filename}: 1 block, 256 bytes, minimal")


def generate_edge_case_uf2(filename: str):
    """Generate a UF2 file with edge cases: maximum payload, various flags."""
    # Test 1: Maximum payload size (476 bytes)
    data = b"\xaa" * MAX_PAYLOAD_SIZE
    block = create_uf2_block(
        target_addr=0x2000, payload=data, block_no=0, total_blocks=1
    )

    with open(filename, "wb") as f:
        f.write(block)

    print(f"Generated {filename}: 1 block, {MAX_PAYLOAD_SIZE} bytes, max payload")


def main():
    os.makedirs("test-files", exist_ok=True)

    # Generate test data
    test_data_256 = b"\x01" * 256
    test_data_512 = b"\x02" * 512
    test_data_1024 = b"\x03" * 1024
    test_data_476 = b"\x04" * 476

    print("Generating test UF2 files...")
    print("=" * 60)

    # Simple UF2 files
    generate_simple_uf2("test-files/simple_256.uf2", test_data_256)
    generate_simple_uf2("test-files/simple_512.uf2", test_data_512)
    generate_simple_uf2("test-files/simple_1024.uf2", test_data_1024)

    print()

    # UF2 with family ID
    generate_uf2_with_family_id("test-files/family_id.uf2", test_data_256, 0x12345678)
    generate_uf2_with_family_id("test-files/family_id_2.uf2", test_data_512, 0x87654321)

    print()

    # UF2 with checksums
    generate_uf2_with_checksum("test-files/checksum_256.uf2", test_data_256)
    generate_uf2_with_checksum("test-files/checksum_512.uf2", test_data_512)

    print()

    # UF2 with extensions
    generate_uf2_with_extensions("test-files/extensions.uf2", test_data_256)
    generate_uf2_with_extensions(
        "test-files/extensions_512.uf2",
        test_data_512,
        semver="2.0.0",
        description="Another Test Device",
    )

    print()

    # Full spec UF2
    generate_uf2_full_spec(
        "test-files/full_spec.uf2",
        test_data_512,
        family_id=0x12345678,
        page_size=256,
        semver="1.0.0",
        description="Full Spec Test",
    )

    print()

    # Edge cases
    generate_minimal_uf2("test-files/minimal.uf2")
    generate_edge_case_uf2("test-files/max_payload.uf2")

    print()
    print("=" * 60)
    print("All test UF2 files generated successfully!")

    # List generated files
    print("\nGenerated files:")
    for filename in sorted(os.listdir("test-files")):
        filepath = os.path.join("test-files", filename)
        size = os.path.getsize(filepath)
        print(f"  {filename}: {size} bytes")


if __name__ == "__main__":
    main()
