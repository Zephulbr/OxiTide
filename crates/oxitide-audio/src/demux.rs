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
}
