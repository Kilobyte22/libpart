use std::io;

struct Window <'a, T: 'a + io::Seek> {
    backend: &'a mut T,
    start: u64,
    len: u64,
    current_offset: u64
}

impl <'a, T: io::Seek> Window <'a, T> {
    pub fn new(backend: &'a mut T, start: u64, len: u64 ) -> Window<'a, T> {
        Window {
            backend,
            start,
            len,
            current_offset: 0
        }
    }

    fn clamp_seek(&self, offset: i64) -> u64 {
        if offset < 0 {
            0
        } else if offset >= self.len as i64 {
            self.len - 1
        } else {
            offset as u64
        }
    }
}

impl <'a, T: io::Seek> io::Seek for Window<'a, T> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        match pos {
            io::SeekFrom::Start(offset) => {
                self.current_offset = self.clamp_seek(offset as i64);
            },
            io::SeekFrom::End(offset) => {
                self.current_offset = self.clamp_seek((self.start + self.len) as i64 + offset);
            },
            io::SeekFrom::Current(offset) => {
                self.current_offset = self.clamp_seek(self.current_offset as i64 + offset);
            }
        }

        self.backend.seek(io::SeekFrom::Start(self.start + self.current_offset))?;
        Ok(self.current_offset)
    }
}

impl <'a, T: io::Seek + io::Write> io::Write for Window<'a, T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes = self.backend.write(buf)?;
        self.current_offset += bytes as u64;
        Ok(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.backend.flush()
    }
}

impl <'a, T: io::Seek + io::Read> io::Read for Window<'a, T> {
    fn read(&mut self, buf: &mut[u8]) -> io::Result<usize> {
        let bytes = self.backend.read(buf)?;
        self.current_offset += bytes as u64;
        Ok(bytes)
    }
}