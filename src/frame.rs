use crate::errors::{ProtocolError, Result};
use async_recursion::async_recursion;
use bytes::Bytes;
use std::{fmt, vec::IntoIter};
use tokio::{
    io::{AsyncWriteExt, BufWriter},
    net::TcpStream,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Frame {
    Simple(Bytes),
    Error(String),
    Integer(u64),
    Bulk(Bytes),
    Null,
    Array(Vec<Frame>),
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Frame::Simple(b) => write!(f, "{}", String::from_utf8_lossy(b)),
            Frame::Error(e) => write!(f, "{}", e),
            Frame::Integer(i) => write!(f, "{}", i),
            Frame::Bulk(b) => write!(f, "{}", String::from_utf8_lossy(b)),
            Frame::Null => write!(f, "nil"),
            Frame::Array(arr) => {
                for frame in arr {
                    write!(f, "({},  )", frame)?;
                }
                Ok(())
            }
        }
    }
}

impl Frame {
    /// Convert the frame to bytes for sending over the wire
    #[async_recursion]
    pub async fn encode(&self, buf: &mut BufWriter<TcpStream>) -> Result<()> {
        let crlf: &[u8] = b"\r\n";
        let null: &[u8] = b"$-1\r\n";

        match self {
            Frame::Simple(s) => {
                buf.write_all(b"+").await?;
                buf.write_all(s).await?;
                buf.write_all(crlf).await?;
                Ok(())
            }
            Frame::Error(msg) => {
                buf.write_all(b"-").await?;
                buf.write_all(msg.as_bytes()).await?;
                buf.write_all(crlf).await?;
                Ok(())
            }
            Frame::Integer(val) => {
                buf.write_all(b":").await?;
                buf.write_all(val.to_string().as_bytes()).await?;
                buf.write_all(crlf).await?;
                Ok(())
            }
            Frame::Bulk(data) => {
                buf.write_all(b"$").await?;
                buf.write_all(data.len().to_string().as_bytes()).await?;
                buf.write_all(crlf).await?;
                buf.write_all(data).await?;
                buf.write_all(crlf).await?;
                Ok(())
            }
            Frame::Null => {
                buf.write_all(null).await?;
                Ok(())
            }
            Frame::Array(items) => {
                buf.write_all(b"*").await?;
                buf.write_all(items.len().to_string().as_bytes()).await?;
                buf.write_all(crlf).await?;

                for item in items {
                    item.encode(buf).await?;
                }
                Ok(())
            }
        }
    }

    /// Helper functions
    pub fn simple(s: impl Into<Bytes>) -> Frame {
        Frame::Simple(s.into())
    }

    pub fn error(msg: impl Into<String>) -> Frame {
        Frame::Error(msg.into())
    }

    pub fn bulk(data: Bytes) -> Frame {
        Frame::Bulk(data)
    }

    pub fn integer(n: u64) -> Frame {
        Frame::Integer(n)
    }

    pub fn null() -> Frame {
        Frame::Null
    }

    pub fn array(items: Vec<Frame>) -> Frame {
        Frame::Array(items)
    }
}

#[derive(Debug, Clone)]
pub struct IterFrame {
    iter: IntoIter<Frame>,
}

impl IterFrame {
    pub fn new(frame: Frame) -> Result<IterFrame> {
        match frame {
            Frame::Array(commands) => Ok(IterFrame {
                iter: commands.into_iter(),
            }),
            _ => Err(ProtocolError::invalid_format()),
        }
    }

    pub fn next_frame(&mut self) -> Result<Frame> {
        match self.iter.next() {
            Some(frame) => Ok(frame),
            _ => Err(ProtocolError::unexpected_end()),
        }
    }

    pub fn next_bytes(&mut self) -> Result<Bytes> {
        let frame = self.next_frame()?;
        match frame {
            Frame::Bulk(bytes) => Ok(bytes),
            Frame::Simple(bytes) => Ok(bytes),
            _ => Err(ProtocolError::invalid_format()),
        }
    }

    pub fn next_integer(&mut self) -> Result<u64> {
        let frame = self.next_frame()?;
        match frame {
            Frame::Integer(expiry) => Ok(expiry),
            _ => Err(ProtocolError::invalid_format()),
        }
    }

    pub fn extract_string(&mut self) -> Result<String> {
        let frame = self.next_frame()?;
        match frame {
            Frame::Bulk(bytes) => Ok(String::from_utf8(bytes.to_vec())?),
            Frame::Simple(bytes) => Ok(String::from_utf8(bytes.to_vec())?),
            _ => Err(ProtocolError::invalid_format()),
        }
    }

    pub fn finish(&mut self) -> Result<()> {
        if self.iter.next().is_some() {
            return Err(ProtocolError::invalid_format());
        }
        Ok(())
    }
}
