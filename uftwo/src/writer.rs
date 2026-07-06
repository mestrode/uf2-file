//! Stream-based UF2 writing functionality.
//!
//! This module provides APIs for creating and writing UF2 files to streams.
//! It requires the `std` feature for I/O traits and MD5 checksums.

#[cfg(feature = "std")]
extern crate std;

use core::fmt;

use crate::MAGIC_NUMBER;
use crate::block::{
    Block, Flags, MAX_PAYLOAD_SIZE, MAX_PAYLOAD_SIZE_WITH_CHECKSUM,
};
use crate::file::Uf2File;

/// Error type for UF2 writing operations.
#[derive(Debug, PartialEq, Eq)]
pub enum WriterError {
    /// There was an issue with the input buffer.
    InputBuffer,
    /// Address or payload size is not properly aligned.
    AlignmentError,
    /// Block corruption occurred during writing.
    BlockCorruption,
}

impl fmt::Display for WriterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputBuffer => write!(f, "Input buffer error"),
            Self::AlignmentError => write!(f, "Alignment error"),
            Self::BlockCorruption => {
                write!(f, "Block corruption during writing")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for WriterError {}

impl Uf2File {
    /// Add payload to the UF2 file.
    ///
    /// # Note
    /// For full UF2 spec compliance including page-based alignment requirements,
    /// use `add_binary()` with a page_size parameter.
    ///
    /// This method validates that all target addresses are 4-byte aligned.
    /// The payload data length should be a multiple of 4 to maintain alignment
    /// across all blocks.
    ///
    /// # Errors
    /// Returns `Err` if target address is not 4-byte aligned.
    pub fn add_payload(
        &mut self,
        payload: &[u8],
        family_id: Option<u32>,
    ) -> Result<(), WriterError> {
        let mut offset = 0;
        let mut block_num = 0;
        let total_blocks =
            payload.len().div_ceil(MAX_PAYLOAD_SIZE_WITH_CHECKSUM);

        while offset < payload.len() {
            let chunk_size =
                core::cmp::min(MAX_PAYLOAD_SIZE, payload.len() - offset);

            // Validate 4-byte alignment of offset (target address)
            if !offset.is_multiple_of(4) {
                return Err(WriterError::AlignmentError);
            }

            // Also ensure chunk_size maintains alignment for next block
            if offset + chunk_size < payload.len()
                && !chunk_size.is_multiple_of(4)
            {
                return Err(WriterError::AlignmentError);
            }

            let mut block = Block {
                magic_start_0: MAGIC_NUMBER[0],
                magic_start_1: MAGIC_NUMBER[1],
                flags: Flags::default(),
                target_addr: offset as u32,
                data_len: chunk_size as u32,
                block: block_num as u32,
                total_blocks: total_blocks as u32,
                board_family_id_or_file_size: 0,
                data: [0; MAX_PAYLOAD_SIZE],
                magic_end: MAGIC_NUMBER[2],
            };

            // Copy data
            block.data[0..chunk_size]
                .copy_from_slice(&payload[offset..offset + chunk_size]);

            // Set family ID if provided
            if let Some(id) = family_id {
                block.flags |= Flags::FamilyId;
                block.board_family_id_or_file_size = id;
            }

            self.push_block(block);
            offset += chunk_size;
            block_num += 1;
        }

        Ok(())
    }

    /// Write the UF2 file to a writer.
    ///
    /// # Errors
    /// Returns `std::io::Error` if writing fails.
    /// Write the UF2 file to a writer.
    ///
    /// # Errors
    /// Returns `std::io::Error` if writing fails.
    #[cfg(feature = "std")]
    pub fn to_writer<W: std::io::Write>(
        &self,
        writer: &mut W,
    ) -> std::io::Result<()> {
        use zerocopy::IntoBytes;
        for block in self.blocks() {
            writer.write_all(block.as_bytes())?;
        }
        Ok(())
    }

    /// Write the UF2 file to a file path.
    ///
    /// # Errors
    /// Returns `WriterError::InputBuffer` if the file cannot be written.
    #[cfg(feature = "std")]
    pub fn to_file(&self, path: &std::path::Path) -> Result<(), WriterError> {
        let bytes = self.to_bytes();
        std::fs::write(path, &bytes).map_err(|_| WriterError::InputBuffer)
    }

    /// Create a UF2 file from binary data with extensions.
    ///
    /// Adds binary data to an existing UF2 file, converting it into UF2 blocks with checksums.
    /// Extensions (`TargetPageSize` and `SemVerString`) are added to the first block of each flash page.
    ///
    /// # Arguments
    /// * `binary` - Binary data to add to the UF2 file.
    /// * `target_addr` - Starting address for the first flash page.
    /// * `family_id` - Optional family ID for the UF2 file.
    /// * `page_size` - Size of the flash page for the target device.
    /// * `semver` - Semantic version string for the firmware.
    ///
    /// # Errors
    /// - [`WriterError::AlignmentError`] if target_addr or page_size are not properly aligned.
    #[cfg(feature = "std")]
    pub fn add_binary(
        &mut self,
        binary: &[u8],
        target_addr: u32,
        family_id: Option<u32>,
        page_size: usize,
        semver: &str,
    ) -> Result<(), WriterError> {
        use crate::ALIGN;
        use crate::block::{Checksum, Extension, ExtensionTag};
        use std::cmp;
        use std::string::ToString;
        use zerocopy::IntoBytes;
        // Validate that target_addr is 4-byte aligned
        if !target_addr.is_multiple_of(ALIGN as u32) {
            return Err(WriterError::AlignmentError);
        }

        let mut new_file = Uf2File::new();
        let mut block_no = 0;
        let mut target_offset = 0;

        while target_offset < binary.len() {
            // Determine the base address for this page
            let addr = target_addr as usize + target_offset;

            // Calculate the size of the current page
            let next_page_addr = addr.next_multiple_of(page_size);
            let mut this_page_size = next_page_addr - addr;
            if this_page_size == 0 {
                this_page_size = page_size;
            }

            // Calculate how much data fits in this page
            let remaining_data = binary.len() - target_offset;
            let this_page_size = this_page_size.min(remaining_data);

            let page = &binary[target_offset..target_offset + this_page_size];

            // Calculate checksum for the entire flash page
            let checksum = Checksum {
                start: addr as u32,
                length: page.len() as u32,
                checksum: *md5::compute(page),
            };

            // Create blocks for this page
            let mut page_offset = 0;
            while page_offset < page.len() {
                // Calculate max payload for this block
                let mut max_payload = MAX_PAYLOAD_SIZE_WITH_CHECKSUM;

                // For the first block in the page, reserve space for extensions
                let has_extensions = page_offset == 0;
                if has_extensions {
                    let page_size_str = page_size.to_string();
                    let target_page_size_ext = Extension::HEADER_SIZE
                        + page_size_str.len().next_multiple_of(ALIGN);
                    let semver_ext = Extension::HEADER_SIZE
                        + semver.len().next_multiple_of(ALIGN);
                    max_payload = max_payload
                        .saturating_sub(target_page_size_ext + semver_ext);
                }

                // Limit by remaining data in page
                let remaining_in_page = page.len() - page_offset;

                // Determine chunk size
                let chunk_size = if page_size <= MAX_PAYLOAD_SIZE {
                    // Try to use multiples of page_size where possible
                    let available = cmp::min(max_payload, remaining_in_page);
                    if available >= page_size {
                        // Use the largest multiple of page_size that fits
                        (available / page_size) * page_size
                    } else if available > 0 {
                        available
                    } else {
                        0
                    }
                } else {
                    // page_size > 476: any payload size is acceptable
                    cmp::min(max_payload, remaining_in_page)
                };

                // Must have at least 1 byte of payload
                if chunk_size == 0 {
                    break;
                }

                // create block
                let target_addr = addr + page_offset;
                let _start = chunk_size.next_multiple_of(ALIGN);
                let mut block = Block {
                    magic_start_0: MAGIC_NUMBER[0],
                    magic_start_1: MAGIC_NUMBER[1],
                    flags: Flags::default(),
                    target_addr: target_addr as u32,
                    data_len: chunk_size as u32,
                    block: block_no as u32,
                    total_blocks: u32::MAX, // Placeholder for total_blocks
                    board_family_id_or_file_size: 0,
                    data: [0; MAX_PAYLOAD_SIZE],
                    magic_end: MAGIC_NUMBER[2],
                };

                // Copy data
                block.data[0..chunk_size].copy_from_slice(
                    &page[page_offset..page_offset + chunk_size],
                );

                // Validate 4-byte alignment of target address (UF2 spec requirement)
                if !block.target_addr.is_multiple_of(ALIGN as u32) {
                    return Err(WriterError::AlignmentError);
                }

                // Add family ID if provided
                if let Some(id) = family_id {
                    block.flags |= Flags::FamilyId;
                    block.board_family_id_or_file_size = id;
                }

                // Set the checksum for all blocks in the page
                block.flags |= Flags::Checksum;
                let checksum_bytes = checksum.as_bytes();
                let end_index = MAX_PAYLOAD_SIZE;
                let start_index = end_index - 24;
                block.data[start_index..end_index]
                    .copy_from_slice(checksum_bytes);

                // Add extensions to the first block of each page
                if page_offset == 0 {
                    // Add TargetPageSize extension
                    let page_size_str = page_size.to_string();
                    let target_page_size_ext_len =
                        Extension::HEADER_SIZE + page_size_str.len();
                    let start = chunk_size.next_multiple_of(ALIGN);
                    let end = start + target_page_size_ext_len;

                    if end <= MAX_PAYLOAD_SIZE_WITH_CHECKSUM {
                        block.data[start] = target_page_size_ext_len as u8;
                        let tag_bytes = ExtensionTag::TargetPageSize.to_bytes();
                        block.data[start + 1..start + 4]
                            .copy_from_slice(&tag_bytes);
                        block.data[start + Extension::HEADER_SIZE..end]
                            .copy_from_slice(page_size_str.as_bytes());

                        block.flags |= Flags::ExtensionTags;
                    }

                    // Add SemverString extension
                    let semver_str = semver;
                    let semver_ext_len =
                        Extension::HEADER_SIZE + semver_str.len();
                    let start = (chunk_size + target_page_size_ext_len)
                        .next_multiple_of(ALIGN);
                    let end = start + semver_ext_len;

                    if end <= MAX_PAYLOAD_SIZE_WITH_CHECKSUM {
                        block.data[start] = semver_ext_len as u8;
                        let tag_bytes = ExtensionTag::SemverString.to_bytes();
                        block.data[start + 1..start + 4]
                            .copy_from_slice(&tag_bytes);
                        block.data[start + Extension::HEADER_SIZE..end]
                            .copy_from_slice(semver_str.as_bytes());

                        block.flags |= Flags::ExtensionTags;
                    }
                }

                page_offset += chunk_size;
                new_file.push_block(block);
                block_no += 1;
            }

            // Advance to next page
            target_offset += this_page_size;
        }

        // Update total_blocks for all blocks (existing + new)
        let total_blocks = new_file.blocks().len();
        for block in new_file.blocks_mut() {
            block.total_blocks = total_blocks as u32;
        }

        self.concat(&new_file);
        Ok(())
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::block::MAX_PAYLOAD_SIZE;
    use std::io::Cursor;

    #[test]
    fn test_add_payload_basic() {
        let mut uf2_file = Uf2File::new();
        let payload = [0xAA; 256];

        let result = uf2_file.add_payload(&payload, None);
        assert!(result.is_ok());
        assert_eq!(uf2_file.len(), 1); // 256 bytes fits in one block
    }

    #[test]
    fn test_add_payload_multiple_blocks() {
        let mut uf2_file = Uf2File::new();
        let payload = [0xBB; 512]; // Requires 2 blocks (476 + 36)

        let result = uf2_file.add_payload(&payload, None);
        assert!(result.is_ok());
        assert_eq!(uf2_file.len(), 2);
    }

    #[test]
    fn test_add_payload_with_family_id() {
        let mut uf2_file = Uf2File::new();
        let payload = [0xCC; 100];
        let family_id = 0x12345678;

        let result = uf2_file.add_payload(&payload, Some(family_id));
        assert!(result.is_ok());
        assert_eq!(uf2_file.len(), 1);

        // Verify family ID is set
        let block = uf2_file.blocks()[0];
        assert_eq!(block.board_family_id(), Some(family_id));
    }

    #[test]
    fn test_add_payload_various_sizes() {
        // Test with various payload sizes that should work
        let payloads = [
            vec![0xAA; 1],   // 1 byte
            vec![0xBB; 4],   // 4 bytes (aligned)
            vec![0xCC; 256], // 256 bytes
            vec![0xDD; 476], // MAX_PAYLOAD_SIZE
            vec![0xEE; 512], // More than one block
        ];

        for payload in payloads {
            let mut uf2_file = Uf2File::new();
            let result = uf2_file.add_payload(&payload, None);
            assert!(
                result.is_ok(),
                "Failed for payload size {}",
                payload.len()
            );
        }
    }

    #[test]
    fn test_to_writer() {
        let mut uf2_file = Uf2File::new();
        uf2_file.add_payload(&[0xAA; 256], None).unwrap();

        let mut cursor = Cursor::new(Vec::new());
        let result = uf2_file.to_writer(&mut cursor);

        assert!(result.is_ok());
        let bytes = cursor.into_inner();
        assert_eq!(bytes.len(), 512); // One block
    }

    #[test]
    fn test_to_writer_multiple_blocks() {
        let mut uf2_file = Uf2File::new();
        uf2_file.add_payload(&[0xBB; 512], None).unwrap();

        let mut cursor = Cursor::new(Vec::new());
        let result = uf2_file.to_writer(&mut cursor);

        assert!(result.is_ok());
        let bytes = cursor.into_inner();
        assert_eq!(bytes.len(), 1024); // Two blocks
    }

    #[test]
    fn test_to_bytes() {
        let mut uf2_file = Uf2File::new();
        uf2_file.add_payload(&[0xCC; 256], None).unwrap();

        let bytes = uf2_file.to_bytes();
        assert_eq!(bytes.len(), 512);
    }

    #[test]
    fn test_to_bytes_empty() {
        let uf2_file = Uf2File::new();
        let bytes = uf2_file.to_bytes();
        assert!(bytes.is_empty());
    }

    #[test]
    fn test_add_binary_basic() {
        let mut uf2_file = Uf2File::new();
        let binary = [0xAA; 256];

        let result = uf2_file.add_binary(
            &binary,
            0x08000000,
            Some(0x12345678),
            256,
            "1.0.0",
        );
        assert!(result.is_ok());
        assert!(uf2_file.len() > 0);
    }

    #[test]
    fn test_add_binary_alignment_error() {
        let mut uf2_file = Uf2File::new();
        let binary = [0xBB; 100];

        // Use unaligned target address
        let result = uf2_file.add_binary(
            &binary,
            0x08000001,
            Some(0x12345678),
            256,
            "1.0.0",
        );
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WriterError::AlignmentError));
    }

    #[test]
    fn test_add_binary_with_extensions() {
        let mut uf2_file = Uf2File::new();
        let binary = [0xCC; 256];

        let result = uf2_file.add_binary(
            &binary,
            0x08000000,
            Some(0x12345678),
            256,
            "1.0.0",
        );
        assert!(result.is_ok());

        // Verify extensions are added to first block
        let first_block = uf2_file.blocks()[0];
        assert!(first_block.has_extensions());
    }

    #[test]
    fn test_add_binary_checksum_flag() {
        let mut uf2_file = Uf2File::new();
        let binary = [0xDD; 256];

        let result = uf2_file.add_binary(
            &binary,
            0x08000000,
            Some(0x12345678),
            256,
            "1.0.0",
        );
        assert!(result.is_ok());

        // Verify checksum flag is set
        let first_block = uf2_file.blocks()[0];
        assert!(first_block.has_checksum());
    }

    #[test]
    fn test_set_target_addresses() {
        let mut uf2_file = Uf2File::new();
        uf2_file.add_payload(&[0xAA; 100], None).unwrap();
        uf2_file.add_payload(&[0xBB; 100], None).unwrap();

        uf2_file.set_target_addresses(0x08000000);

        assert_eq!(uf2_file.blocks()[0].target_addr, 0x08000000);
        assert_eq!(uf2_file.blocks()[1].target_addr, 0x08000000 + 100);
    }

    #[test]
    fn test_concat() {
        let mut uf2_file1 = Uf2File::new();
        uf2_file1.add_payload(&[0xAA; 100], None).unwrap();

        let mut uf2_file2 = Uf2File::new();
        uf2_file2.add_payload(&[0xBB; 100], None).unwrap();

        uf2_file1.concat(&uf2_file2);

        assert_eq!(uf2_file1.len(), 2);
        assert_eq!(uf2_file2.len(), 1); // Original unchanged
    }

    #[test]
    fn test_roundtrip_add_payload() {
        // Create a UF2 file with payload
        let mut original = Uf2File::new();
        let payload = [0xAA; 256];
        original.add_payload(&payload, Some(0x12345678)).unwrap();

        // Convert to bytes
        let bytes = original.to_bytes();

        // Parse back
        let restored = crate::reader::from_bytes(&bytes).unwrap();

        // Verify payload matches
        let restored_payload = restored.get_payload(Some(0x12345678)).unwrap();
        assert_eq!(payload, restored_payload.as_slice());
    }

    #[test]
    fn test_writer_error_display() {
        let error = WriterError::InputBuffer;
        assert_eq!(format!("{}", error), "Input buffer error");

        let error = WriterError::AlignmentError;
        assert_eq!(format!("{}", error), "Alignment error");

        let error = WriterError::BlockCorruption;
        assert_eq!(format!("{}", error), "Block corruption during writing");
    }

    #[test]
    fn test_add_payload_empty() {
        let mut uf2_file = Uf2File::new();
        let payload: &[u8] = &[];

        let result = uf2_file.add_payload(payload, None);
        assert!(result.is_ok());
        assert_eq!(uf2_file.len(), 0); // Empty payload results in no blocks
    }

    #[test]
    fn test_add_payload_max_size() {
        let mut uf2_file = Uf2File::new();
        let payload = [0xFF; MAX_PAYLOAD_SIZE * 2]; // Exactly 2 blocks worth

        let result = uf2_file.add_payload(&payload, None);
        assert!(result.is_ok());
        assert_eq!(uf2_file.len(), 2);
    }

    #[test]
    fn test_add_binary_page_alignment() {
        let mut uf2_file = Uf2File::new();
        let binary = [0xEE; 512];

        // Use page size that divides evenly
        let result = uf2_file.add_binary(
            &binary,
            0x08000000,
            Some(0x12345678),
            256,
            "1.0.0",
        );
        assert!(result.is_ok());

        // Verify blocks are created
        assert!(uf2_file.len() > 0);
    }
}
