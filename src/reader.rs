//! Stream-based UF2 reading functionality.
//!
//! This module provides APIs for reading and parsing UF2 data from streams
//! and byte buffers. It is designed to work without std (no_std + alloc).

extern crate alloc;
use alloc::vec::Vec;
use core::fmt;

use crate::block::{Block, BlockError, BLOCK_SIZE, MAX_PAYLOAD_SIZE};
use crate::uf2file::Uf2File;
use zerocopy::FromBytes;

/// Error type for UF2 reading operations.
#[derive(Debug, PartialEq, Eq)]
pub enum ReaderError {
    /// There was an issue with the input buffer size or alignment.
    InputBuffer,
    /// File size is not a multiple of the block size.
    FileSizeMismatch,
    /// One or more blocks in the file are corrupted.
    BlockCorruption(BlockError),
    /// Block order or indexing is inconsistent.
    BlockOrderMismatch,
    /// Address or payload size is not properly aligned.
    AlignmentError,
}

impl fmt::Display for ReaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputBuffer => write!(f, "Input buffer"),
            Self::FileSizeMismatch => write!(f, "File size mismatch"),
            Self::BlockCorruption(e) => {
                write!(f, "UF2 Block corruption: {}", e)
            }
            Self::BlockOrderMismatch => {
                write!(f, "UF2 Block index or order mismatch")
            }
            Self::AlignmentError => write!(f, "Alignment error"),
        }
    }
}

impl From<BlockError> for ReaderError {
    fn from(error: BlockError) -> Self {
        Self::BlockCorruption(error)
    }
}

/// Check if the given buffer is valid UF2 data.
///
/// Checks buffer size is a multiple of BLOCK_SIZE and first block is valid.
pub fn is_uf2_buffer(buf: &[u8]) -> bool {
    if !buf.len().is_multiple_of(BLOCK_SIZE) {
        return false;
    }

    let mut blocks = buf.chunks_exact(BLOCK_SIZE);
    let first_block = match blocks.next() {
        Some(b) => b,
        None => return false,
    };

    match <Block>::ref_from_bytes(first_block) {
        Ok(_) => (),
        Err(_) => return false,
    }

    true
}

/// Construct a [`Uf2File`] from a byte slice.
///
/// # Errors
/// - [`ReaderError::FileSizeMismatch`] if the buffer size is not a multiple of the block size.
/// - [`ReaderError::BlockCorruption`] if any block in the buffer is corrupted.
pub fn from_bytes(buf: &[u8]) -> Result<Uf2File, ReaderError> {
    if !buf.len().is_multiple_of(BLOCK_SIZE) {
        return Err(ReaderError::FileSizeMismatch);
    }

    let mut blocks = Vec::new();

    for chunk in buf.chunks_exact(BLOCK_SIZE) {
        let block = Block::from_bytes(chunk)?;
        blocks.push(block);
    }

    Ok(Uf2File::from_blocks(blocks))
}

/// Verify the integrity of the UF2 file.
///
/// # Errors
/// - [`ReaderError::BlockOrderMismatch`] if the block order is incorrect.
/// - [`ReaderError::BlockCorruption`] if any block is corrupted.
pub fn verify(uf2_file: &Uf2File) -> Result<(), ReaderError> {
    let mut prev_index = None;
    let mut prev_total_blocks = None;

    for block in uf2_file.blocks() {
        let index = block.block as usize;
        let total = block.total_blocks as usize;

        // block id must be < total_blocks
        if index >= total {
            return Err(ReaderError::BlockOrderMismatch);
        }

        // block id must be sequential, or reset to 0
        if let Some(prev_index) = prev_index {
            if index != prev_index + 1 && index != 0 {
                return Err(ReaderError::BlockOrderMismatch);
            }
        }

        // total must be consistent, unless index is 0
        if let Some(prev_total_blocks) = prev_total_blocks {
            if total != prev_total_blocks && index != 0 {
                return Err(ReaderError::BlockOrderMismatch);
            }
        }

        // verify data length
        if block.data().len() > MAX_PAYLOAD_SIZE {
            return Err(ReaderError::BlockCorruption(BlockError::PayloadSize));
        }

        prev_index = Some(index);
        prev_total_blocks = Some(total);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{Block, Flags};
    use zerocopy::IntoBytes;

    #[test]
    fn test_is_uf2_buffer_valid() {
        // Create a valid UF2 block
        let mut block = Block::default();
        block.data_len = 256;
        block.data[0..256].copy_from_slice(&[0xAA; 256]);

        let bytes = block.as_bytes();
        let buffer = [bytes; 1]; // Single block

        assert!(is_uf2_buffer(&buffer.concat()));
    }

    #[test]
    fn test_is_uf2_buffer_invalid_size() {
        // Buffer not multiple of BLOCK_SIZE
        let bytes = vec![0u8; 511];
        assert!(!is_uf2_buffer(&bytes));
    }

    #[test]
    fn test_is_uf2_buffer_valid_magic() {
        // Create a valid block and verify it's detected as UF2
        let block = Block::default();
        let bytes = block.as_bytes();

        assert!(is_uf2_buffer(&bytes));
    }

    #[test]
    fn test_is_uf2_buffer_empty() {
        // Empty buffer
        let bytes: &[u8] = &[];
        assert!(!is_uf2_buffer(bytes));
    }

    #[test]
    fn test_is_uf2_buffer_multiple_blocks() {
        // Create multiple valid blocks
        let mut block1 = Block::default();
        block1.block = 0;
        block1.total_blocks = 2;
        block1.data_len = 100;
        block1.data[0..100].copy_from_slice(&[0xAA; 100]);

        let mut block2 = Block::default();
        block2.block = 1;
        block2.total_blocks = 2;
        block2.data_len = 100;
        block2.data[0..100].copy_from_slice(&[0xBB; 100]);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(block1.as_bytes());
        bytes.extend_from_slice(block2.as_bytes());

        assert!(is_uf2_buffer(&bytes));
    }

    #[test]
    fn test_from_bytes_single_block() {
        let mut block = Block::default();
        block.block = 0;
        block.total_blocks = 1;
        block.data_len = 100;
        block.data[0..100].copy_from_slice(&[0xCC; 100]);

        let bytes = block.as_bytes();
        let uf2_file = from_bytes(&bytes).unwrap();

        assert_eq!(uf2_file.len(), 1);
        assert_eq!(uf2_file.blocks()[0].block, 0);
        assert_eq!(uf2_file.blocks()[0].data(), &[0xCC; 100]);
    }

    #[test]
    fn test_from_bytes_multiple_blocks() {
        let mut block1 = Block::default();
        block1.block = 0;
        block1.total_blocks = 2;
        block1.data_len = 100;
        block1.data[0..100].copy_from_slice(&[0xAA; 100]);

        let mut block2 = Block::default();
        block2.block = 1;
        block2.total_blocks = 2;
        block2.data_len = 50;
        block2.data[0..50].copy_from_slice(&[0xBB; 50]);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(block1.as_bytes());
        bytes.extend_from_slice(block2.as_bytes());

        let uf2_file = from_bytes(&bytes).unwrap();

        assert_eq!(uf2_file.len(), 2);
        assert_eq!(uf2_file.blocks()[0].data(), &[0xAA; 100]);
        assert_eq!(uf2_file.blocks()[1].data(), &[0xBB; 50]);
    }

    #[test]
    fn test_from_bytes_invalid_size() {
        let bytes = vec![0u8; 511]; // Not multiple of BLOCK_SIZE
        let result = from_bytes(&bytes);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ReaderError::FileSizeMismatch));
    }

    #[test]
    fn test_from_bytes_invalid_magic() {
        let mut bytes = vec![0u8; 512];
        // Set invalid magic numbers
        bytes[0..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

        let result = from_bytes(&bytes);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReaderError::BlockCorruption(_)
        ));
    }

    #[test]
    fn test_from_bytes_empty() {
        let bytes: &[u8] = &[];
        let result = from_bytes(bytes);

        // Empty buffer: 0 is a multiple of BLOCK_SIZE, but there are no blocks
        // This should succeed and return an empty Uf2File
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_verify_valid() {
        let mut uf2_file = Uf2File::new();

        // Add sequentially numbered blocks
        let block1 = Block::new(0, 2, &[0xAA; 100], 0);
        uf2_file.push_block(block1);

        let block2 = Block::new(1, 2, &[0xBB; 100], 0);
        uf2_file.push_block(block2);

        let result = verify(&uf2_file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_block_index_mismatch() {
        let mut uf2_file = Uf2File::new();

        // Add blocks with non-sequential indices
        let block1 = Block::new(0, 2, &[0xAA; 100], 0);
        uf2_file.push_block(block1);

        let block2 = Block::new(2, 2, &[0xBB; 100], 0); // Skip index 1
        uf2_file.push_block(block2);

        let result = verify(&uf2_file);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReaderError::BlockOrderMismatch
        ));
    }

    #[test]
    fn test_verify_total_blocks_mismatch() {
        let mut uf2_file = Uf2File::new();

        // Add blocks with inconsistent total_blocks
        let block1 = Block::new(0, 2, &[0xAA; 100], 0);
        uf2_file.push_block(block1);

        let block2 = Block::new(1, 3, &[0xBB; 100], 0); // Different total_blocks
        uf2_file.push_block(block2);

        let result = verify(&uf2_file);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReaderError::BlockOrderMismatch
        ));
    }

    #[test]
    fn test_verify_index_exceeds_total() {
        let mut uf2_file = Uf2File::new();

        // Create block manually to bypass Block::new() assertion
        let mut block = Block::default();
        block.block = 5;
        block.total_blocks = 3; // block=5 >= total=3
        block.data_len = 100;
        block.data[0..100].copy_from_slice(&[0xAA; 100]);

        uf2_file.push_block(block);

        let result = verify(&uf2_file);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReaderError::BlockOrderMismatch
        ));
    }

    #[test]
    fn test_verify_empty_file() {
        let uf2_file = Uf2File::new();
        let result = verify(&uf2_file);
        assert!(result.is_ok()); // Empty file is valid
    }

    #[test]
    fn test_verify_single_block() {
        let mut uf2_file = Uf2File::new();
        let block = Block::new(0, 1, &[0xAA; 100], 0);
        uf2_file.push_block(block);

        let result = verify(&uf2_file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_with_family_id() {
        let mut uf2_file = Uf2File::new();

        let mut block1 = Block::new(0, 2, &[0xAA; 100], 0);
        block1.flags |= Flags::FamilyId;
        block1.board_family_id_or_file_size = 0x12345678;
        uf2_file.push_block(block1);

        let mut block2 = Block::new(1, 2, &[0xBB; 100], 0);
        block2.flags |= Flags::FamilyId;
        block2.board_family_id_or_file_size = 0x12345678;
        uf2_file.push_block(block2);

        let result = verify(&uf2_file);
        assert!(result.is_ok()); // Family ID doesn't affect verification
    }

    #[test]
    fn test_reader_error_display() {
        let error = ReaderError::InputBuffer;
        assert_eq!(format!("{}", error), "Input buffer");

        let error = ReaderError::FileSizeMismatch;
        assert_eq!(format!("{}", error), "File size mismatch");

        let error = ReaderError::BlockOrderMismatch;
        assert_eq!(format!("{}", error), "UF2 Block index or order mismatch");

        let error = ReaderError::AlignmentError;
        assert_eq!(format!("{}", error), "Alignment error");

        let error = ReaderError::BlockCorruption(BlockError::MagicNumber);
        assert_eq!(
            format!("{}", error),
            "UF2 Block corruption: Magic number incorrect"
        );
    }

    #[test]
    fn test_reader_error_from_block_error() {
        let block_error = BlockError::PayloadSize;
        let reader_error: ReaderError = block_error.into();

        assert!(matches!(
            reader_error,
            ReaderError::BlockCorruption(BlockError::PayloadSize)
        ));
    }
}
