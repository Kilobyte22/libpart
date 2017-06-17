extern crate checksum;
extern crate uuid;
extern crate byteorder;

use super::util::Block;
use std::cmp;
use self::checksum::crc32::Crc32 as CRC32;
use self::uuid::{Uuid as UUID, ParseError as UUIDError};
use self::byteorder::{WriteBytesExt, ReadBytesExt, LittleEndian, ByteOrder};
use std::io::{Result as IOResult, Write, Read, Error as IOError, Seek, SeekFrom, Cursor};
use std::error::Error;
use std::fmt;

const GPT_MAGIC: [u8; 8] = [0x45, 0x46, 0x49, 0x20, 0x50, 0x41, 0x52, 0x54];

/// Options for parsing GPT
pub struct GPTOptions {
    /// The block size to use. Defaults to 512
    pub block_size: u16,
    /// Make checksum errors non-fatal.
    pub ignore_csum: bool,
    /// Make UTF-16 error non-fatal
    pub ignore_utf16_errors: bool
}

impl Default for GPTOptions {
    fn default() -> GPTOptions {
        GPTOptions {
            block_size: 512,
            ignore_csum: false,
            ignore_utf16_errors: false
        }        
    }
}

#[derive(Debug)]
pub struct GPTTable {
    primary_gpt: Block,
    backup_gpt: Block,
    first_usable: Block,
    last_usable: Block,
    gpt_uuid: UUID,
    partitions: Vec<Option<PartitionEntry>>,
    checksum: u32
}

#[derive(Debug)]
pub struct PartitionEntry {
    /// The type UUID of the partition
    pub part_type: UUID,
    /// The PARTUUID of the partition
    pub part_id: UUID,
    /// The first block of the partition
    pub start: Block,
    /// The last block of the partition
    pub end: Block,
    /// The flags of the partition
    pub flags: u64,
    /// The human readable name of the partition. At most 19 chars.
    pub name: String
}

impl PartitionEntry {
    /// Creates a new empty partition entry
    fn empty() -> PartitionEntry {
        PartitionEntry {
            part_type: UUID::nil(),
            part_id: UUID::nil(),
            start: Block(0),
            end: Block(0),
            flags: 0,
            name: String::new()
        }
    }
}

#[derive(Debug)]
pub enum ErrorType {
    /// There was no GPT found
    NoTable,
    /// The checksum of both the primary and the backup GPT was incorrect
    ChecksumError,
    /// The Version of the GPT is incompatible with this implementation
    InvalidVersion,
    /// The header in itself is invalid
    InvalidHeader,
    /// During parsing or writing a GPT an IO Error occured
    IOError(IOError),
    /// One of the UUIDs is not valid
    UUIDError(UUIDError),
    /// One of the strings is invalid UTF-16
    UTF16Error,
    InvalidID
}

#[derive(Debug)]
pub struct GPTError {
    error_type: ErrorType,
    desc: String
}

impl GPTError {
    fn new(t: ErrorType) -> GPTError {
        let desc = String::from(match &t {
            &ErrorType::NoTable => String::from("No GPT found"),
            &ErrorType::ChecksumError => String::from("GPT corrupt"),
            &ErrorType::InvalidVersion => String::from("Invalid GPT Version"),
            &ErrorType::InvalidHeader => String::from("Invalid GPT Header"),
            &ErrorType::UTF16Error => String::from("Encoding Error in GPT: Invalid UTF-16"),
            &ErrorType::InvalidID => String::from("Invalid ID"),
            &ErrorType::IOError(ref e) => format!("IO Error while processing GPT: {}", e.description()),
            &ErrorType::UUIDError(ref e) => format!("Invalid UUID: {}", e.description())
        });
        GPTError {
            error_type: t,
            desc: desc
        }
    }
}

impl From<IOError> for GPTError {
    fn from(err: IOError) -> GPTError {
        GPTError::new(ErrorType::IOError(err))
    }
}

impl From<UUIDError> for GPTError {
    fn from(err: UUIDError) -> GPTError {
        GPTError::new(ErrorType::UUIDError(err))
    }
}

impl Error for GPTError {
    fn description(&self) -> &str {
        &self.desc
    }

    fn cause(&self) -> Option<&Error> {
        match &self.error_type {
            &ErrorType::IOError(ref e) => Some(e),
            &ErrorType::UUIDError(ref e) => Some(e),
            _ => None
        }
    }
}

impl fmt::Display for GPTError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl GPTTable {


    pub fn exists<T: Read + Seek>(read: &mut T, options: &GPTOptions) -> Result<bool, IOError> {
        let block_size = options.block_size;

        read.seek(SeekFrom::Start(block_size as u64))?;

        let mut buf = [0u8; 8];
        try!(read.read(&mut buf));

        Ok(buf == GPT_MAGIC)
    }

    /// Load a GPT from file or stream
    pub fn load<T: Read + Seek>(read: &mut T, options: &GPTOptions) -> Result<GPTTable, GPTError> {

        let block_size = options.block_size;

        // Actually go to the start of the GPT
        try!(read.seek(SeekFrom::Start(block_size as u64)));

        let mut buf = [0u8; 8];
        try!(read.read(&mut buf));
        if buf != GPT_MAGIC {
            return Err(GPTError::new(ErrorType::NoTable));
        }

        let mut buf = [0u8; 4];
        try!(read.read(&mut buf));
        if buf != [0x00, 0x00, 0x01, 0x00] {
            println!("Invalid header version");
            return Err(GPTError::new(ErrorType::InvalidVersion));
        }

        let hlen = try!(read.read_u32::<LittleEndian>());

        if hlen != 92 {
            println!("Header length is not 92");
            return Err(GPTError::new(ErrorType::InvalidHeader));
        }

        // FIXME: ignoring checksum for now
        let crc = try!(read.read_u32::<LittleEndian>());

        // Reserved. Let's ignore it.
        try!(read.read_i32::<LittleEndian>());
        
        let mypos = Block(try!(read.read_u64::<LittleEndian>()));

        let otherpos = Block(try!(read.read_u64::<LittleEndian>()));

        let first_usable = Block(try!(read.read_u64::<LittleEndian>()));

        let last_usable = Block(try!(read.read_u64::<LittleEndian>()));

        let uuid = try!(read_uuid(read));

        let part_start = Block(try!(read.read_u64::<LittleEndian>()));
        if part_start != Block(2) {
            // In primary GPT this is ALWAYS 2
            println!("Invalid start of partition table");
            return Err(GPTError::new(ErrorType::InvalidHeader));
        }

        let part_count = try!(read.read_u32::<LittleEndian>());

        let part_size = try!(read.read_u32::<LittleEndian>());
        if part_size != 128 {
            println!("Invalid partition table entry size");
            return Err(GPTError::new(ErrorType::InvalidHeader));
        }

        let part_checksum = try!(read.read_u32::<LittleEndian>());

        if !options.ignore_csum {
            // Time to verify checksum
            try!(read.seek(SeekFrom::Start(block_size as u64)));
            let mut buf = Vec::new();
            buf.resize(hlen as usize, 0u8);
            try!(read.read(&mut buf));
            // Zero out checksum field
            cp(&[0x00, 0x00, 0x00, 0x00], &mut buf[16..20]);

            let csum = CRC32::new().checksum(&buf);

            if csum != crc {
                return Err(GPTError::new(ErrorType::ChecksumError));
            }

            // Time to checksum the partition table
            try!(read.seek(SeekFrom::Start(part_start.to_bytes(block_size))));
            
            let mut buf = Vec::new();
            buf.resize(part_size as usize * part_count as usize, 0u8);
            try!(read.read(&mut buf));

            let csum = CRC32::new().checksum(&buf);
            if csum != part_checksum {
                return Err(GPTError::new(ErrorType::ChecksumError));
            }
        }

        // Okay, Lets read the actual partition table
        
        try!(read.seek(SeekFrom::Start(part_start.to_bytes(block_size))));

        // Stuff might break on 64 bit once we get huuuuuge hard disks.
        // But eh, 32 bit will be gone by then anyways
        let mut partitions = Vec::with_capacity(part_count as usize);

        for _ in 0..part_count {
            let part_type = try!(read_uuid(read));
            let part_id = try!(read_uuid(read));
            let part_start = Block(try!(read.read_u64::<LittleEndian>()));
            let part_end = Block(try!(read.read_u64::<LittleEndian>()));
            let part_flags = try!(read.read_u64::<LittleEndian>());
            let part_label = try!(read_utf16_le(read, options.ignore_utf16_errors));


            if part_type.is_nil() {
                partitions.push(None)
            } else {
                partitions.push(Some(PartitionEntry {
                    part_type: part_type,
                    part_id: part_id,
                    start: part_start,
                    end: part_end,
                    flags: part_flags,
                    name: part_label
                }));
            }
        }


        Ok(GPTTable {
            primary_gpt: mypos,
            backup_gpt: otherpos,
            first_usable: first_usable,
            last_usable: last_usable,
            gpt_uuid: uuid,
            partitions: partitions,
            checksum: crc
        })
    }

    /// Write a GPT to file. will write both primary and backup
    pub fn write<W: Write + Seek>(&self, write: &mut W, options: &GPTOptions) -> Result<(), GPTError> {
        try!(self.write_gpt(write, options, true));
        try!(self.write_gpt(write, options, false));
        Ok(())
    }

    fn write_gpt<W: Write + Seek>(&self, write: &mut W, options: &GPTOptions, primary: bool) -> Result<(), GPTError> {

        let mut gpt = Vec::new();
        gpt.resize(92, 0u8);

        let mut cur = Cursor::new(gpt);

        // Magic Bytes
        try!(cur.write(&GPT_MAGIC));
        // Revision
        try!(cur.write(&[0x00, 0x00, 0x01, 0x00]));
        // Header size
        try!(cur.write_u32::<LittleEndian>(92));
        // CRC32 sum - for now 0
        try!(cur.write_u32::<LittleEndian>(0));
        // Reserved
        try!(cur.write_i32::<LittleEndian>(0));

        let mypos = if primary {
            try!(cur.write_u64::<LittleEndian>(self.primary_gpt.0));
            try!(cur.write_u64::<LittleEndian>(self.backup_gpt.0));
            self.primary_gpt
        } else {
            try!(cur.write_u64::<LittleEndian>(self.backup_gpt.0));
            try!(cur.write_u64::<LittleEndian>(self.primary_gpt.0));
            self.backup_gpt
        };

        try!(cur.write_u64::<LittleEndian>(self.first_usable.0));
        try!(cur.write_u64::<LittleEndian>(self.last_usable.0));

        try!(write_uuid(&mut cur, self.gpt_uuid));

        let part_start = if primary {
            Block(2)
        } else {
            self.backup_gpt - self.ptable_len(self.partitions.len() as u64, options)
        };

        try!(cur.write_u64::<LittleEndian>(part_start.0));

        try!(cur.write_u32::<LittleEndian>(self.partitions.len() as u32));

        try!(cur.write_u32::<LittleEndian>(128));


        // Write part table
        let mut part_tab = Vec::new();
        part_tab.resize(self.partitions.len() * 128, 0u8);

        let mut pcur = Cursor::new(part_tab);

        let empty = PartitionEntry::empty();

        for p in &self.partitions {
            let p = match p {
                &Some(ref p) => p,
                &None => &empty
            };

            try!(write_uuid(&mut pcur, p.part_type));
            try!(write_uuid(&mut pcur, p.part_id));
            try!(pcur.write_u64::<LittleEndian>(p.start.0));
            try!(pcur.write_u64::<LittleEndian>(p.end.0));
            try!(pcur.write_u64::<LittleEndian>(p.flags));
            try!(write_utf16_le(&mut pcur, &p.name));
        }

        let part_crc = CRC32::new().checksum(pcur.get_ref());

        // Write CRC of partition table
        try!(cur.write_u32::<LittleEndian>(part_crc));

        // Now we actually write the table to disk
        try!(write.seek(SeekFrom::Start(part_start.to_bytes(options.block_size))));
        try!(write.write(pcur.get_ref()));

        try!(cur.seek(SeekFrom::Start(16)));

        let hdr_crc = {
            let buf = cur.get_ref();
            CRC32::new().checksum(&buf)
        };
        try!(cur.write_u32::<LittleEndian>(hdr_crc));

        // Fully zero the sector for the actual GPT
        let mut buf = Vec::new();
        buf.resize(options.block_size as usize, 0u8);
        try!(write.seek(SeekFrom::Start(mypos.to_bytes(options.block_size))));
        try!(write.write(&buf));

        // Write the actual GPT
        try!(write.seek(SeekFrom::Start(mypos.to_bytes(options.block_size))));
        try!(write.write(cur.get_ref()));

        Ok(())

    }

    fn ptable_len(&self, pcount: u64, options: &GPTOptions) -> Block {
        Block::from_bytes(pcount * 128, options.block_size).expect("Partition count must be devidable by 4")
    }

    /// Gets the amount of partitions that are in use.
    ///
    /// Please note that if there is an empty part inbetween, it is not counted. So say
    /// you have /dev/sda1, /dev/sda2 and /dev/sda4, but no /dev/sda3, this would still return 3
    pub fn part_count(&self) -> u64 {
        self.partitions.iter().filter(|p| p.is_some()).count() as u64
    }

    /// Gives you readonly access to all partitions
    pub fn partitions(&self) -> &[Option<PartitionEntry>] {
        &self.partitions
    }

    /// Get the first free partition ID
    ///
    /// Returns Some(id) if there is still space  
    /// Returns None if all partition slots are occupied
    pub fn next_id(&self) -> Option<u64> {
        for p in self.partitions.iter().enumerate() {
            if p.1.is_none() {
                return Some(p.0 as u64);
            }
        }
        None
    }

    /// Set a partition entry to whatever you specified
    pub fn set_partition(&mut self, id: u64, part: PartitionEntry) -> Result<(), GPTError> {
        if id as usize > self.partitions.len() - 1 {
            return Err(GPTError::new(ErrorType::InvalidID));
        }
        self.partitions[id as usize] = Some(part);
        Ok(())
    }

    /// Mark a partition slot as empty
    pub fn delete_partition(&mut self, id: u64) -> Result<(), GPTError> {
        if id as usize > self.partitions.len() - 1 {
            return Err(GPTError::new(ErrorType::InvalidID));
        }
        self.partitions[id as usize] = None;
        Ok(())
    }
}

fn write_utf16_le(write: &mut Write, s: &str) -> Result<(), GPTError> {
    let buf = s.encode_utf16().take(36).collect::<Vec<_>>();
    let mut buf2 = [0u16; 36];
    cp(&buf, &mut buf2);
    try!(write_u16_buf::<LittleEndian>(write, &buf2));
    Ok(())
}

fn write_u16_buf<T: ByteOrder>(write: &mut Write, buf: &[u16]) -> IOResult<()> {
    for i in 0..buf.len() {
        try!(write.write_u16::<T>(buf[i]));
    }
    Ok(())
}

fn read_utf16_le(read: &mut Read, ignore_err: bool) -> Result<String, GPTError> {
    let mut buf = [0u16; 36];
    try!(read_u16_buf::<LittleEndian>(read, &mut buf));
    let ret = match String::from_utf16(&buf) {
        Ok(x) => x,
        Err(_) => if ignore_err {
            String::new()
        } else {
            return Err(GPTError::new(ErrorType::UTF16Error))
        }
    };

    Ok(match ret.find('\0') {
        Some(x) => String::from(&ret[0..x]),
        None => ret
    })
}

fn read_u16_buf<T: ByteOrder>(read: &mut Read, buf: &mut[u16]) -> IOResult<()> {
    for i in 0..buf.len() {
        buf[i] = try!(read.read_u16::<T>());
    }
    Ok(())
}

fn read_uuid(read: &mut Read) -> Result<UUID, GPTError> {
    let mut buf = [0u8; 16];
    let mut buf_endian_ffs = [0u8; 16];
    try!(read.read(&mut buf));
    cp(&buf, &mut buf_endian_ffs);
    // Lets fix endianness
    swap_endian(&buf[0..4], &mut buf_endian_ffs[0..4]);
    swap_endian(&buf[4..6], &mut buf_endian_ffs[4..6]);
    swap_endian(&buf[6..8], &mut buf_endian_ffs[6..8]);

    Ok(try!(UUID::from_bytes(&buf_endian_ffs)))
}

fn write_uuid(write: &mut Write, uuid: UUID) -> Result<(), GPTError> {
    let buf = uuid.as_bytes();
    let mut buf_out = [0u8; 16];
    cp(buf, &mut buf_out);
    swap_endian(&buf[0..4], &mut buf_out[0..4]);
    swap_endian(&buf[4..6], &mut buf_out[4..6]);
    swap_endian(&buf[6..8], &mut buf_out[6..8]);

    try!(write.write(&buf_out));
    Ok(())
}

fn swap_endian(input: &[u8], output: &mut [u8]) {
    let len = cmp::min(input.len(), output.len());
    for i in 0..len {
        output[len - 1 - i] = input[i];
    }
}

fn cp<T: Copy>(input: &[T], output: &mut [T]) {
    let len = cmp::min(input.len(), output.len());
    for i in 0..len {
        output[i] = input[i];
    }
}
