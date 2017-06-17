use std::ops::{Add, Sub};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Block(pub u64);

impl Block {
    /// Gets the byte offset of a block
    pub fn to_bytes(&self, sector_size: u16) -> u64 {
        let sector_size = sector_size as u64;
        self.0 * sector_size
    }

    /// Gets the Block from a byte offset
    /// Returns none if not at valid byte offset
    pub fn from_bytes(bytes: u64, sector_size: u16) -> Option<Block> {
        let sector_size = sector_size as u64;
        if bytes % sector_size == 0 {
            Some(Block(bytes / 512))
        } else {
            None
        }
    }

    /// Gets the Block as well as the offset within the block of a given offset
    pub fn from_bytes_offset(bytes: u64, sector_size: u16) -> (Block, u16) {
        let sector_size = sector_size as u64;
        (Block(bytes / sector_size), (bytes % sector_size) as u16)
    }
}

impl Add for Block {
    type Output = Block;

    fn add(self, other: Block) -> Block {
        Block(self.0 + other.0)
    }
}

impl Sub for Block {
    type Output = Block;

    fn sub(self, other: Block) -> Block {
        Block(self.0 - other.0)
    }
}
