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

    pub fn write_mbr<W: Write + Seek>(&self, read: &mut W) -> IOResult<()> {
        try!(write.seek(SeekFrom::Start(0)));
        try!(write.write(self.bootloader));
        for part in &self.partitions {
            try!(part.write(write));
        }
        try!(write.write_u16::<BigEndian>(0x55AA));
    }

    pub fn partitions(&self) -> &[Option<PartitionEntry>] {
        &self.partitions
    }

    pub fn partition_count(&self) -> u8 {
        self.partitions.iter().filter(|p| p.is_some()).count() as u8
    }

    pub fn has_gpt(&self) -> bool {
        self.partitions.iter().any(|p| p.system_id == 0xFF)
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
    pub system_id: u8,
    pub start_lba: u32,
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
    
    fn write<W: Write + Seek>(&self, write: &mut W) -> IOResult<()> {

        if self.bootable {
            try!(write.write_u8(0x80));
        } else {
            try!(write.write_u8(0x00));
        }

        let mut chs = [3; 0u8];
        offset_to_chs(self.start_lba, &mut chs);
        try!(write.write(&chs));
        try!(write.write_u8(self.system_id));

        offset_to_chs(self.sector_count + self.start_lba - 1, &mut chs);
        try!(write.write(&chs));

        try!(write.write_u32::<LittleEndian>(self.start_lba));
        try!(write.write_u32::<LittleEndian>(self.sector_count));

    }

    fn offset_to_chs(offset: u32, buf: &mut [u8]) {

        let c = offset % 1024;
        let offset = offset / 1024;

        let h = offset % 256;
        let offset = offset / 256;

        let s = cmp::max(offset, 63);

        buf[0] = h;
        buf[1] = s | ((c & 0x0300) >> 2);
        buf[2] = (c & 0xFF) as u8;
    }

}
