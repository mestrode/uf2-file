#![cfg_attr(not(test), no_std)]

pub mod block;
pub mod reader;
pub mod uf2file;
pub mod writer;

pub use block::*;
pub use reader::{is_uf2_buffer, ReaderError};
pub use uf2file::Uf2File;
pub use writer::WriterError;

// Implement core::error::Error for error types
impl core::error::Error for ReaderError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            ReaderError::BlockCorruption(e) => Some(e),
            _ => None,
        }
    }
}
