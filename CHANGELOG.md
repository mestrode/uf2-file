# Changelog

# [Unreleased]

- Moved into `block.rs` - Core UF2 block types (`Block`, `Checksum`, `Flags`, `Extensions`, `ExtensionTag`)
- Fixed `Block::new()` was not setting the `data_len` field, causing `data()` method to return empty slices
- Added `uf2file.rs` - UF2 file structure (`Uf2File`) and utilities
- Added `reader.rs` - Stream-based reading functionality (no_std compatible)
- Added `writer.rs` - Stream-based writing functionality (requires std feature)
- Added comprehensive tests covering nearly all functionality
- Added Integration tests using synthetic UF2 test files
- Added Roundtrip tests to verify data integrity

# v0.2.0

- Updated defmt to v1.0
- Updated zerocopy to v0.8
- Added `board_family_id` getter to `Block`

# v0.1.0

- Initial release
