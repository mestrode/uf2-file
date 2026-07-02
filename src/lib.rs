#![cfg_attr(not(test), no_std)]

pub mod block;

pub use block::{
    BLOCK_SIZE, Checksum, Extension, ExtensionTag, Extensions, Flags,
    MAX_PAYLOAD_SIZE, MAGIC_NUMBER, Block, BlockError,
};
