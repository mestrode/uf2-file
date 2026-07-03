#![cfg_attr(not(test), no_std)]

pub mod block;
pub mod reader;
pub mod uf2file;

pub use block::*;
pub use reader::{is_uf2_buffer, ReaderError};
pub use uf2file::Uf2File;
