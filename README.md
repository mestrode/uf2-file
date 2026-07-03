# `uftwo`

[![Crate](https://img.shields.io/crates/v/uftwo.svg)](https://crates.io/crates/uftwo)
[![Docs](https://docs.rs/uftwo/badge.svg)](https://docs.rs/uftwo)

For working with the [UF2 file format](https://github.com/microsoft/uf2).

Why the name? `uf2` was already taken and appears to be abandoned.

## Features

- **Stream-based API**: Read and write UF2 data from/to any `Read`/`Write` implementor
- **No std support**: Core functionality works without std (no_std + alloc)
- **Flexible**: Works with files, memory buffers, network streams, etc.
- **Comprehensive**: Supports all UF2 features including extensions, checksums, and family IDs

### Feature Flags

- `std` - Enables writer functionality and std-dependent features (requires md5)
- `defmt` - Enables [defmt](https://github.com/knurling-rs/defmt) `Format` on relevant types
- `cli` - Enables the command-line interface

## Using the Library

### Basic Usage

```shell
cargo add uftwo
```

### Reading UF2 Files

```rust
use uftwo::{Uf2File, reader};
use std::fs::File;
use std::io::Read;

// Read from a file
let mut file = File::open("firmware.uf2").unwrap();
let uf2_file = reader::from_stream(&mut file).unwrap();

// Or read from bytes
let bytes = std::fs::read("firmware.uf2").unwrap();
let uf2_file = reader::from_bytes(&bytes).unwrap();

// Access blocks
for block in uf2_file.blocks() {
    println!("Block {}: {} bytes at 0x{:08X}", 
             block.block, block.data().len(), block.target_addr);
}

// Extract payload
if let Some(payload) = uf2_file.get_payload(None) {
    println!("Total payload: {} bytes", payload.len());
}
```

### Writing UF2 Files

```rust
use uftwo::{Uf2File, writer};
use std::fs::File;
use std::io::Write;

// Create a new UF2 file
let mut uf2_file = Uf2File::new();

// Add payload with optional family ID
uf2_file.add_payload(&firmware_bytes, Some(0x12345678)).unwrap();

// Write to a file
let mut file = File::create("output.uf2").unwrap();
uf2_file.to_writer(&mut file).unwrap();

// Or write to bytes
let bytes = uf2_file.to_bytes();
```

### Full Spec Compliance

```rust
use uftwo::{Uf2File, writer};

let mut uf2_file = Uf2File::new();

// Add binary with full UF2 spec compliance
uf2_file.add_binary(
    &binary_data,
    0x08000000,      // Target address
    Some(0x12345678), // Family ID
    2048,           // Page size
    "1.0.0"         // Semantic version
).unwrap();

// This creates blocks with:
// - Proper checksums (MD5)
// - Target page size extensions
// - Semantic version extensions
// - Full spec compliance
```

## Module Structure

- **`block`** - Core UF2 block types and parsing
- **`uf2file`** - UF2 file structure and manipulation
- **`reader`** - Stream-based reading functionality (no_std compatible)
- **`writer`** - Stream-based writing functionality (requires std feature)

## Using the CLI

```shell
cargo install uftwo --features="cli"
```

### Convert binary to UF2

```shell
uftwo convert input.bin output.uf2 --target-addr 0x08000000 --family-id 0x12345678
```

### Convert UF2 to binary

```shell
uftwo convert input.uf2 output.bin
```

## License

This project is licensed under the [MPL-2.0](LICENSE).
