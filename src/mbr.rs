extern crate byteorder;

use self::byteorder::{ReadBytesExt, WriteBytesExt, LittleEndian, BigEndian};
use std::io::{Result as IOResult, Read, Seek, SeekFrom, Write};
use std::{fmt, cmp};

/// A structure representing a Master Boot Record
pub struct MBR {
    bootloader: [u8; 446],
    partitions: [Option<PartitionEntry>; 4],
    boot_sig: u16
}

impl MBR {
    /// Initialize a new empty MBR
    pub fn new() -> MBR {
        MBR::default()
    }

    /// Load a MBR from stream
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

    /// Load a GPT from stream
    pub fn write_mbr<W: Write + Seek>(&self, write: &mut W) -> IOResult<()> {
        try!(write.seek(SeekFrom::Start(0)));
        try!(write.write(&self.bootloader));
        for p in &self.partitions {
            match p {
                &Some(ref part) => try!(part.write(write)),
                &None => try!(PartitionEntry::default().write(write))
            }
        }
        try!(write.write_u16::<BigEndian>(0x55AA));
        Ok(())
    }

    /// Get a list of all primary partitions
    pub fn partitions(&self) -> &[Option<PartitionEntry>] {
        &self.partitions
    }

    /// Count all primary partitions. May be changed in the future to also include logical
    /// partitions, once they are supported by this library
    pub fn partition_count(&self) -> u8 {
        self.partitions.iter().filter(|p| p.is_some()).count() as u8
    }

    /// Count all primary partitions. Does include the host partition for logical partitions
    pub fn primary_partition_count(&self) -> u8 {
        self.partition_count()
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
        fmt.debug_struct("MBR").field("bootloader", &"[446 Bytes]").field("partitions", &self.partitions).field("boot_sig", &self.boot_sig).finish()?;
        Ok(())
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct PartitionEntry {
    /// Is this partition marked as bootable
    pub bootable: bool,
    /// The partition type
    pub system_id: u8,
    /// the LBA at which the partition starts
    pub start_lba: u32,
    /// How long the partition is in blocks
    pub sector_count: u32,
}

impl PartitionEntry {
    fn load<R: Read + Seek>(read: &mut R) -> IOResult<Option<PartitionEntry>> {
        let boot = try!(read.read_u8()) == 0x80;
        try!(read.seek(SeekFrom::Current(3))); // Skip CHS
        let system_id = try!(read.read_u8());
        try!(read.seek(SeekFrom::Current(3))); // Skip CHS
        let start_lba = try!(read.read_u32::<LittleEndian>());
        let sector_count = try!(read.read_u32::<LittleEndian>());

        if system_id != 0 {
            Ok(Some(PartitionEntry {
                bootable: boot,
                system_id: system_id,
                start_lba: start_lba,
                sector_count: sector_count
            }))
        } else {
            Ok(None)
        }
    }
    
    fn write<W: Write + Seek>(&self, write: &mut W) -> IOResult<()> {

        if self.bootable {
            try!(write.write_u8(0x80));
        } else {
            try!(write.write_u8(0x00));
        }

        let mut chs = [0u8; 3];
        offset_to_chs(self.start_lba, &mut chs);
        try!(write.write(&chs));
        try!(write.write_u8(self.system_id));

        offset_to_chs(self.sector_count + self.start_lba - 1, &mut chs);
        try!(write.write(&chs));

        try!(write.write_u32::<LittleEndian>(self.start_lba));
        try!(write.write_u32::<LittleEndian>(self.sector_count));

        Ok(())

    }


}

fn offset_to_chs(offset: u32, buf: &mut [u8]) {

    let c = offset % 1024;
    let offset = offset / 1024;

    let h = offset % 256;
    let offset = offset / 256;

    let s = cmp::max(offset, 63);

    buf[0] = h as u8;
    buf[1] = (s | ((c & 0x0300) >> 2)) as u8;
    buf[2] = (c & 0xFF) as u8;
}
