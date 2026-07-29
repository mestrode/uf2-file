//! UF2 file structure and utilities.

extern crate alloc;
use alloc::vec::Vec;

use crate::block::Block;
use zerocopy::IntoBytes;

/// UF2 file structure containing a collection of blocks.
#[derive(Debug, Default)]
pub struct Uf2File {
    blocks: Vec<Block>,
}

impl Uf2File {
    /// Create a new empty UF2 file.
    pub fn new() -> Self {
        Self { blocks: Vec::new() }
    }

    /// Create a UF2 file from a slice of blocks.
    pub fn from_blocks(blocks: &[Block]) -> Self {
        Self {
            blocks: blocks.to_vec(),
        }
    }

    /// Get the number of blocks in the UF2 file.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Check if the UF2 file is empty.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Get a slice of all blocks in the file.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Get a mutable slice of all blocks in the file.
    pub fn blocks_mut(&mut self) -> &mut [Block] {
        &mut self.blocks
    }

    /// Add a block to the file.
    pub fn push_block(&mut self, block: Block) {
        self.blocks.push(block);
    }

    /// Concatenate another [`Uf2File`] to this one.
    pub fn concat(&mut self, other: &Self) {
        self.blocks.extend(other.blocks.iter().cloned());
    }

    /// Set the target address for each block starting from the given address.
    ///
    /// This updates the target address of each block sequentially based on the data length.
    pub fn set_target_addresses(&mut self, start_addr: u32) {
        let mut offset = start_addr as usize;
        for block in &mut self.blocks {
            block.target_addr = offset as u32;
            offset += block.data_len as usize;
        }
    }

    /// Get payload of the UF2 file with the specified family ID.
    ///
    /// # Returns
    /// - `Some(Vec<u8>)` if the payload is not empty.
    /// - `None` if the payload is empty.
    pub fn get_payload(&self, family_id: Option<u32>) -> Option<Vec<u8>> {
        let mut payload = Vec::new();

        for block in &self.blocks {
            if let Some(id) = family_id
                && block.board_family_id() != Some(id)
            {
                continue;
            }
            payload.extend_from_slice(block.data());
        }
        if payload.is_empty() {
            None
        } else {
            Some(payload)
        }
    }

    /// List all family IDs in the UF2 file.
    ///
    /// Returns a vector of family IDs.
    /// If there are duplicate family IDs, they are removed.
    /// If there are no family IDs, returns an empty vector.
    pub fn list_family_ids(&self) -> Vec<u32> {
        let mut family_ids = Vec::new();
        for block in &self.blocks {
            if let Some(id) = block.board_family_id()
                && !family_ids.contains(&id)
            {
                family_ids.push(id);
            }
        }
        family_ids
    }

    /// Convert the UF2 file to a byte vector.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.blocks
            .iter()
            .flat_map(|block| block.as_bytes().to_vec())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{Block, Flags};

    #[test]
    fn test_new() {
        let uf2_file = Uf2File::new();
        assert_eq!(uf2_file.len(), 0);
        assert!(uf2_file.is_empty());
        assert_eq!(uf2_file.blocks().len(), 0);
    }

    #[test]
    fn test_from_blocks() {
        let block1 = Block::default();
        let block2 = Block::default();
        let blocks = vec![block1, block2];

        let uf2_file = Uf2File::from_blocks(&blocks);
        assert_eq!(uf2_file.len(), 2);
        assert!(!uf2_file.is_empty());
        assert_eq!(uf2_file.blocks().len(), 2);
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut uf2_file = Uf2File::new();
        assert_eq!(uf2_file.len(), 0);
        assert!(uf2_file.is_empty());

        uf2_file.push_block(Block::default());
        assert_eq!(uf2_file.len(), 1);
        assert!(!uf2_file.is_empty());

        uf2_file.push_block(Block::default());
        assert_eq!(uf2_file.len(), 2);
        assert!(!uf2_file.is_empty());
    }

    #[test]
    fn test_blocks_and_blocks_mut() {
        let mut uf2_file = Uf2File::new();

        // Test empty blocks
        assert_eq!(uf2_file.blocks().len(), 0);

        // Add some blocks
        let block1 = Block::new(0, 1, &[0xAA; 100], 0x08000000);
        let block2 = Block::new(1, 2, &[0xBB; 100], 0x08000064);

        uf2_file.push_block(block1);
        uf2_file.push_block(block2);

        // Test blocks()
        let blocks = uf2_file.blocks();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block, 0);
        assert_eq!(blocks[1].block, 1);

        // Test blocks_mut()
        let blocks_mut = uf2_file.blocks_mut();
        assert_eq!(blocks_mut.len(), 2);
        blocks_mut[0].block = 10;
        blocks_mut[1].block = 11;

        // Verify mutation worked
        assert_eq!(uf2_file.blocks()[0].block, 10);
        assert_eq!(uf2_file.blocks()[1].block, 11);
    }

    #[test]
    fn test_push_block() {
        let mut uf2_file = Uf2File::new();
        assert_eq!(uf2_file.len(), 0);

        let block1 = Block::new(0, 1, &[0xCC; 50], 0);
        uf2_file.push_block(block1);
        assert_eq!(uf2_file.len(), 1);

        let block2 = Block::new(1, 2, &[0xDD; 50], 0);
        uf2_file.push_block(block2);
        assert_eq!(uf2_file.len(), 2);

        // Verify blocks are stored correctly
        assert_eq!(uf2_file.blocks()[0].block, 0);
        assert_eq!(uf2_file.blocks()[1].block, 1);
    }

    #[test]
    fn test_concat() {
        let mut uf2_file1 = Uf2File::new();
        uf2_file1.push_block(Block::new(0, 1, &[0xAA; 50], 0));
        uf2_file1.push_block(Block::new(1, 2, &[0xBB; 50], 0));

        let mut uf2_file2 = Uf2File::new();
        uf2_file2.push_block(Block::new(0, 1, &[0xCC; 50], 0));

        // Concat file2 into file1
        uf2_file1.concat(&uf2_file2);

        assert_eq!(uf2_file1.len(), 3);
        assert_eq!(uf2_file2.len(), 1); // Original unchanged

        // Verify blocks are in correct order
        assert_eq!(uf2_file1.blocks()[0].data()[0], 0xAA);
        assert_eq!(uf2_file1.blocks()[1].data()[0], 0xBB);
        assert_eq!(uf2_file1.blocks()[2].data()[0], 0xCC);
    }

    #[test]
    fn test_set_target_addresses() {
        let mut uf2_file = Uf2File::new();

        // Add blocks with some data
        uf2_file.push_block(Block::new(0, 2, &[0xAA; 100], 0));
        uf2_file.push_block(Block::new(1, 2, &[0xBB; 50], 0));

        // Set target addresses starting from 0x08000000
        uf2_file.set_target_addresses(0x08000000);

        // Verify addresses are set correctly
        assert_eq!(uf2_file.blocks()[0].target_addr, 0x08000000);
        assert_eq!(uf2_file.blocks()[1].target_addr, 0x08000000 + 100); // 0x08000064
    }

    #[test]
    fn test_get_payload() {
        let mut uf2_file = Uf2File::new();

        // Add blocks with different family IDs
        let mut block1 = Block::new(0, 2, &[0xAA; 100], 0);
        block1.flags |= Flags::FamilyId;
        block1.board_family_id_or_file_size = 0x12345678;
        uf2_file.push_block(block1);

        let mut block2 = Block::new(1, 2, &[0xBB; 50], 0);
        block2.flags |= Flags::FamilyId;
        block2.board_family_id_or_file_size = 0x87654321;
        uf2_file.push_block(block2);

        let block3 = Block::new(2, 3, &[0xCC; 75], 0);
        // No family ID set
        uf2_file.push_block(block3);

        // Get payload for family ID 0x12345678
        let payload1 = uf2_file.get_payload(Some(0x12345678)).unwrap();
        assert_eq!(payload1.len(), 100);
        assert_eq!(payload1[0], 0xAA);

        // Get payload for family ID 0x87654321
        let payload2 = uf2_file.get_payload(Some(0x87654321)).unwrap();
        assert_eq!(payload2.len(), 50);
        assert_eq!(payload2[0], 0xBB);

        // Get payload for no family ID (should include block3)
        let payload_none = uf2_file.get_payload(None).unwrap();
        assert_eq!(payload_none.len(), 225); // 100 + 50 + 75
    }

    #[test]
    fn test_get_payload_empty() {
        let uf2_file = Uf2File::new();
        assert_eq!(uf2_file.get_payload(None), None);
        assert_eq!(uf2_file.get_payload(Some(0x1234)), None);
    }

    #[test]
    fn test_list_family_ids() {
        let mut uf2_file = Uf2File::new();

        // Add blocks with different family IDs
        let mut block1 = Block::new(0, 1, &[0xAA; 50], 0);
        block1.flags |= Flags::FamilyId;
        block1.board_family_id_or_file_size = 0x12345678;
        uf2_file.push_block(block1);

        let mut block2 = Block::new(1, 2, &[0xBB; 50], 0);
        block2.flags |= Flags::FamilyId;
        block2.board_family_id_or_file_size = 0x87654321;
        uf2_file.push_block(block2);

        // Add block without family ID
        uf2_file.push_block(Block::new(2, 3, &[0xCC; 50], 0));

        // Add another block with same family ID as block1
        let mut block4 = Block::new(3, 4, &[0xDD; 50], 0);
        block4.flags |= Flags::FamilyId;
        block4.board_family_id_or_file_size = 0x12345678; // Duplicate
        uf2_file.push_block(block4);

        let family_ids = uf2_file.list_family_ids();
        assert_eq!(family_ids.len(), 2); // Duplicates removed
        assert!(family_ids.contains(&0x12345678));
        assert!(family_ids.contains(&0x87654321));
    }

    #[test]
    fn test_list_family_ids_empty() {
        let uf2_file = Uf2File::new();
        let family_ids = uf2_file.list_family_ids();
        assert!(family_ids.is_empty());
    }

    #[test]
    fn test_to_bytes() {
        let mut uf2_file = Uf2File::new();

        // Add some blocks
        uf2_file.push_block(Block::new(0, 2, &[0xAA; 100], 0));
        uf2_file.push_block(Block::new(1, 2, &[0xBB; 50], 0));

        let bytes = uf2_file.to_bytes();

        // Should be exactly 2 blocks * 512 bytes each
        assert_eq!(bytes.len(), 2 * 512);

        // Verify we can parse the first block back
        let first_block = Block::from_bytes(&bytes[0..512]).unwrap();
        assert_eq!(first_block.block, 0);
        assert_eq!(first_block.data()[0], 0xAA);
    }

    #[test]
    fn test_to_bytes_empty() {
        let uf2_file = Uf2File::new();
        let bytes = uf2_file.to_bytes();
        assert!(bytes.is_empty());
    }

    #[test]
    fn test_roundtrip() {
        // Create a UF2 file with some blocks
        let mut original = Uf2File::new();
        original.push_block(Block::new(0, 3, &[0xAA; 100], 0x08000000));
        original.push_block(Block::new(1, 3, &[0xBB; 100], 0x08000064));
        original.push_block(Block::new(2, 3, &[0xCC; 100], 0x080000C8));

        // Convert to bytes
        let bytes = original.to_bytes();

        // Create a new UF2 file from bytes
        let mut restored = Uf2File::new();
        for chunk in bytes.chunks_exact(512) {
            let block = Block::from_bytes(chunk).unwrap();
            restored.push_block(block);
        }

        // Verify they match
        assert_eq!(original.len(), restored.len());
        assert_eq!(original.blocks()[0].data(), restored.blocks()[0].data());
        assert_eq!(original.blocks()[1].data(), restored.blocks()[1].data());
        assert_eq!(original.blocks()[2].data(), restored.blocks()[2].data());
    }
}
