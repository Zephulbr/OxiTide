use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;

use symphonia::core::io::MediaSource;

use crate::fetch::StreamBuffer;

pub struct StreamBufferSource {
    buffer: Arc<StreamBuffer>,
    pos: u64,
}

impl StreamBufferSource {
    pub fn new(buffer: Arc<StreamBuffer>) -> Self {
        Self { buffer, pos: 0 }
    }
}

impl Read for StreamBufferSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self
            .buffer
            .read_at(self.pos as usize, out)
            .map_err(|e| io::Error::other(e.to_string()))?;
        self.pos += n as u64;
        // Free decoded bytes so a long stream stays bounded.
        self.buffer.consume_to(self.pos as usize);
        Ok(n)
    }
}

impl Seek for StreamBufferSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // The buffer is a forward-only stream; symphonia will not seek because
        // `is_seekable` is false. Support only querying the position.
        match pos {
            SeekFrom::Start(p) if p <= self.pos => {
                self.pos = p;
                Ok(p)
            }
            SeekFrom::Current(0) => Ok(self.pos),
            _ => Err(io::Error::other("stream is not seekable")),
        }
    }
}

impl MediaSource for StreamBufferSource {
    fn is_seekable(&self) -> bool {
        false
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}

pub struct BytesSource {
    cursor: io::Cursor<Vec<u8>>,
}

impl BytesSource {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { cursor: io::Cursor::new(bytes) }
    }
}

impl Read for BytesSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.cursor.read(out)
    }
}

impl Seek for BytesSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(pos)
    }
}

impl MediaSource for BytesSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.cursor.get_ref().len() as u64)
    }
}

/// A seekable file source for cache hits.
pub struct FileSource {
    file: std::fs::File,
    len: u64,
    pos: u64,
}

impl FileSource {
    pub fn open(path: &std::path::Path) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self { file, len, pos: 0 })
    }
}

impl Read for FileSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self.file.read(out)?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for FileSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.pos = self.file.seek(pos)?;
        Ok(self.pos)
    }
}

impl MediaSource for FileSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bytes_as_they_are_pushed() {
        let buf = StreamBuffer::new();
        buf.push(b"hello ");
        buf.push(b"world");
        buf.finish();
        let mut src = StreamBufferSource::new(buf);
        let mut out = Vec::new();
        src.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"hello world");
    }

    #[test]
    fn end_of_stream_returns_zero() {
        let buf = StreamBuffer::new();
        buf.finish();
        let mut src = StreamBufferSource::new(buf);
        let mut out = [0u8; 4];
        assert_eq!(src.read(&mut out).unwrap(), 0);
    }

    #[test]
    fn unsupported_seek_errors() {
        let buf = StreamBuffer::new();
        buf.finish();
        let mut src = StreamBufferSource::new(buf);
        assert!(src.seek(SeekFrom::End(0)).is_err());
    }

    #[test]
    fn file_source_reads_seeks_and_reports_len() {
        let dir = std::env::temp_dir().join(format!("oxitide-demux-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("blob.bin");
        std::fs::write(&path, b"0123456789").unwrap();
        let mut src = FileSource::open(&path).unwrap();
        assert_eq!(src.byte_len(), Some(10));
        assert!(src.is_seekable());
        let mut out = [0u8; 4];
        assert_eq!(src.read(&mut out).unwrap(), 4);
        assert_eq!(&out, b"0123");
        src.seek(SeekFrom::Start(8)).unwrap();
        assert_eq!(src.read(&mut out).unwrap(), 2);
        assert_eq!(&out[..2], b"89");
        let _ = std::fs::remove_dir_all(dir);
    }
}
