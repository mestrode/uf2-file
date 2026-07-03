#![cfg_attr(not(test), no_std)]

#[cfg(feature = "std")]
extern crate std;

pub mod block;
pub mod reader;
pub mod uf2file;

#[cfg(feature = "std")]
pub mod writer;

pub use block::*;
pub use reader::{is_uf2_buffer, ReaderError};
pub use uf2file::Uf2File;

#[cfg(feature = "std")]
pub use writer::WriterError;

// Implement std::error::Error for error types when std is available
#[cfg(feature = "std")]
mod std_error_impl {
    use super::*;

    impl std::error::Error for ReaderError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                ReaderError::BlockCorruption(e) => Some(e),
                _ => None,
            }
        }
    }

    impl std::error::Error for WriterError {}
}
