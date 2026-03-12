// src/parse.rs
use crate::{ProtocolError, RedisError, Result, frame::Frame};
use bytes::{Buf, Bytes, BytesMut};
use memchr::memchr;
use std::io::Cursor;
use tokio_util::codec::Decoder;

/// Internal representation for parsing
struct BufSplit(usize, usize);

impl BufSplit {
    fn as_bytes(&self, buf: &Bytes) -> Bytes {
        buf.slice(self.0..self.1)
    }
}

/// Intermediate parsing structure
enum FrameBufSplit {
    Simple(BufSplit),
    Error(BufSplit),
    Integer(u64),
    Bulk(BufSplit),
    Null,
    Array(Vec<FrameBufSplit>),
}

/// Find the next word (until \r\n)
#[inline]
fn word(buf: &mut Cursor<&[u8]>) -> Result<BufSplit> {
    let bytes = buf.chunk();
    let index = memchr(b'\r', bytes).ok_or(ProtocolError::Incomplete)?;

    if bytes.get(index + 1).is_some() && bytes[index + 1] == b'\n' {
        let position: usize = buf.position() as usize;
        buf.advance(index + 2);
        return Ok(BufSplit(position, position + index));
    }
    Err(ProtocolError::incomplete())
}

/// Parse simple string: +OK\r\n
fn simple_string(buf: &mut Cursor<&[u8]>) -> Result<FrameBufSplit> {
    let result = word(buf)?;
    Ok(FrameBufSplit::Simple(result))
}

/// Parse error: -ERR message\r\n
fn error(buf: &mut Cursor<&[u8]>) -> Result<FrameBufSplit> {
    let result = word(buf)?;
    Ok(FrameBufSplit::Error(result))
}

/// Parse integer: :1000\r\n
fn read_decimal(buf: &mut Cursor<&[u8]>) -> Result<i64> {
    let result = word(buf)?;
    let s = std::str::from_utf8(&buf.get_ref()[result.0..result.1])
        .map_err(|_| ProtocolError::invalid_frame_format())?;
    let i = s
        .parse()
        .map_err(|_| ProtocolError::invalid_frame_format())?;
    Ok(i)
}

/// Parse integer and wrap in FrameBufSplit
fn integer_frame(buf: &mut Cursor<&[u8]>) -> Result<FrameBufSplit> {
    let n = read_decimal(buf)? as u64;
    Ok(FrameBufSplit::Integer(n))
}

/// Parse bulk string: $5\r\nhello\r\n or $-1\r\n (null)
fn bulk_string(buf: &mut Cursor<&[u8]>) -> Result<FrameBufSplit> {
    let n: i64 = read_decimal(buf)?;
    if n == -1 {
        return Ok(FrameBufSplit::Null);
    };

    let end = n as usize;
    if buf.remaining() < end + 2 {
        return Err(ProtocolError::incomplete());
    }

    let pos = buf.position() as usize;
    if buf.get_ref().get(pos + end) != Some(&b'\r')
        || buf.get_ref().get(pos + end + 1) != Some(&b'\n')
    {
        return Err(ProtocolError::invalid_frame_format());
    }

    let start: usize = buf.position() as usize;
    buf.advance(end + 2);
    Ok(FrameBufSplit::Bulk(BufSplit(start, start + end)))
}

/// Parse any frame
fn parse_frame(buf: &mut Cursor<&[u8]>) -> Result<FrameBufSplit> {
    if !buf.has_remaining() {
        return Err(ProtocolError::incomplete());
    }

    let command_type = buf.chunk()[0];
    let start = buf.position();
    buf.advance(1);
    match command_type {
        b'+' => simple_string(buf),
        b'-' => error(buf),
        b':' => integer_frame(buf),
        b'$' => bulk_string(buf),
        b'*' => array(buf, start),
        _ => Err(ProtocolError::invalid_frame_format()),
    }
}

/// Parse array: *2\r\n$3\r\nGET\r\n$3\r\nkey\r\n
fn array(buf: &mut Cursor<&[u8]>, start: u64) -> Result<FrameBufSplit> {
    let len = match read_decimal(buf) {
        Ok(i) => {
            if i == -1 {
                return Ok(FrameBufSplit::Null);
            } else if i < -1 {
                return Err(ProtocolError::invalid_frame_format());
            }
            i as usize
        }
        Err(e) => {
            buf.set_position(start);
            return Err(e);
        }
    };

    let mut frames = Vec::with_capacity(len);

    for _ in 0..len {
        match parse_frame(buf) {
            Ok(frame) => {
                frames.push(frame);
            }
            Err(RedisError::Protocol(ProtocolError::Incomplete)) => {
                buf.set_position(start);
                return Err(ProtocolError::incomplete());
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    Ok(FrameBufSplit::Array(frames))
}

impl FrameBufSplit {
    /// Convert to Frame using the provided buffer
    fn into_frame(self, buf: &Bytes) -> Frame {
        match self {
            FrameBufSplit::Simple(bfs) => Frame::Simple(bfs.as_bytes(buf)),
            FrameBufSplit::Error(bfs) => {
                let bytes = bfs.as_bytes(buf);
                Frame::Error(String::from_utf8_lossy(&bytes).to_string())
            }
            FrameBufSplit::Integer(i) => Frame::Integer(i),
            FrameBufSplit::Bulk(bfs) => Frame::Bulk(bfs.as_bytes(buf)),
            FrameBufSplit::Null => Frame::Null,
            FrameBufSplit::Array(arr) => {
                Frame::Array(arr.into_iter().map(|item| item.into_frame(buf)).collect())
            }
        }
    }
}

/// Decoder for Redis frames
#[derive(Default, Debug)]
pub struct FrameDecoder;

impl Decoder for FrameDecoder {
    type Item = Frame;
    type Error = RedisError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>> {
        if buf.is_empty() {
            return Ok(None);
        }

        let mut cursor = Cursor::new(&buf[..]);

        match parse_frame(&mut cursor) {
            Ok(frame) => {
                let data = buf.split_to(cursor.position() as usize).freeze();
                Ok(Some(frame.into_frame(&data)))
            }
            Err(RedisError::Protocol(ProtocolError::Incomplete)) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_string() {
        let mut buf = BytesMut::from(&b"+OK\r\n"[..]);
        let mut decoder = FrameDecoder;

        let frame = decoder.decode(&mut buf).unwrap().unwrap();
        assert_eq!(frame, Frame::Simple(Bytes::from("OK")));
        assert!(buf.is_empty());
    }

    #[test]
    fn test_parse_error() {
        let mut buf = BytesMut::from(&b"-ERR unknown command\r\n"[..]);
        let mut decoder = FrameDecoder;

        let frame = decoder.decode(&mut buf).unwrap().unwrap();
        assert_eq!(frame, Frame::Error("ERR unknown command".to_string()));
    }

    #[test]
    fn test_parse_integer() {
        let mut buf = BytesMut::from(&b":1000\r\n"[..]);
        let mut decoder = FrameDecoder;

        let frame = decoder.decode(&mut buf).unwrap().unwrap();
        assert_eq!(frame, Frame::Integer(1000));
    }

    #[test]
    fn test_parse_bulk_string() {
        let mut buf = BytesMut::from(&b"$5\r\nhello\r\n"[..]);
        let mut decoder = FrameDecoder;

        let frame = decoder.decode(&mut buf).unwrap().unwrap();
        assert_eq!(frame, Frame::Bulk(Bytes::from("hello")));
    }

    #[test]
    fn test_parse_null() {
        let mut buf = BytesMut::from(&b"$-1\r\n"[..]);
        let mut decoder = FrameDecoder;

        let frame = decoder.decode(&mut buf).unwrap().unwrap();
        assert_eq!(frame, Frame::Null);
    }

    #[test]
    fn test_parse_array() {
        let mut buf = BytesMut::from(&b"*2\r\n$3\r\nGET\r\n$3\r\nkey\r\n"[..]);
        let mut decoder = FrameDecoder;

        let frame = decoder.decode(&mut buf).unwrap().unwrap();
        match frame {
            Frame::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0], Frame::Bulk(Bytes::from("GET")));
                assert_eq!(arr[1], Frame::Bulk(Bytes::from("key")));
            }
            _ => panic!("expected array"),
        }
    }
}
