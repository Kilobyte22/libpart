extern crate byteorder;

use self::byteorder::{ReadBytesExt, LittleEndian};
use std::io::{Result as IOResult, Read, Seek, SeekFrom};
use std::fmt;

pub struct MBR {
    bootloader: [u8; 446],
    partitions: [Option<PartitionEntry>; 4],
    boot_sig: u16
}

impl MBR {
    fn new() -> MBR {
        MBR::default()
    }

    pub fn load<R: Read + Seek>(read: &mut R) -> IOResult<MBR> {
        try!(read.seek(SeekFrom::Start(0)));
        let mut stage0 = [0u8; 446];
        try!(read.read(&mut stage0));
        let mut parts = [None; 4];
        for i in 0..4 {
            parts[i] = try!(PartitionEntry::load(read));
        }
        let sig = try!(read.read_u16::<LittleEndian>());

        Ok(MBR {
            bootloader: stage0,
            partitions: parts,
            boot_sig: sig
        })
    }

    pub fn partitions(&self) -> &[Option<PartitionEntry>] {
        &self.partitions
    }

    pub fn partition_count(&self) -> u8 {
        self.partitions.iter().filter(|p| p.is_some()).count() as u8
    }
}

impl Default for MBR {
    fn default() -> MBR {
        MBR {
            bootloader: [0u8; 446],
            partitions: [None; 4],
            boot_sig: 0
        }
    }
}

impl fmt::Debug for MBR {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.debug_struct("MBR").field("bootloader", &"[446 Bytes]").field("partitions", &self.partitions).field("boot_sig", &self.boot_sig).finish();
        Ok(())
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct PartitionEntry {
    pub bootable: bool,
    head_start: u8,
    sector_start: u8,
    cylinder_start: u16,
    pub system_id: u8,
    head_end: u8,
    sector_end: u8,
    cylinder_end: u16,
    pub start_lba: u32,
    pub sector_count: u32
}

impl PartitionEntry {
    fn load<R: Read + Seek>(read: &mut R) -> IOResult<Option<PartitionEntry>> {
        let boot = try!(read.read_u8()) == 0x80;
        let head_start = try!(read.read_u8());
        let mut sector_start = try!(read.read_u8());
        let mut cylinder_start = try!(read.read_u8()) as u16;
        cylinder_start |= (sector_start as u16 & 0x00C0) << 2;
        sector_start &= 0x3F;
        let system_id = try!(read.read_u8());
        let head_end = try!(read.read_u8());
        let mut sector_end = try!(read.read_u8());
        let mut cylinder_end = try!(read.read_u8()) as u16;
        cylinder_end |= (sector_end as u16 & 0x00C0) << 2;
        sector_end &= 0x3F;
        let start_lba = try!(read.read_u32::<LittleEndian>());
        let sector_count = try!(read.read_u32::<LittleEndian>());

        if system_id != 0 {
            Ok(Some(PartitionEntry {
                bootable: boot,
                head_start: head_start,
                sector_start: sector_start,
                cylinder_start: cylinder_start,
                system_id: system_id,
                head_end: head_end,
                sector_end: sector_end,
                cylinder_end: cylinder_end,
                start_lba: start_lba,
                sector_count: sector_count
            }))
        } else {
            Ok(None)
        }
    }
}
