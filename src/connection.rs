use crate::{ConnectionError, Result};

use crate::Frame;
use crate::parse::FrameDecoder;
use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufWriter},
    net::TcpStream,
};
use tokio_util::codec::Decoder;
use tracing::{instrument, trace};

#[derive(Debug)]
pub struct Connection {
    stream: BufWriter<TcpStream>,
    read_buffer: BytesMut,
    decoder: FrameDecoder,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        let stream = BufWriter::new(stream);
        let read_buffer = BytesMut::with_capacity(4 * 1024);
        let decoder = FrameDecoder;
        Connection {
            stream,
            read_buffer,
            decoder,
        }
    }

    #[instrument(level = "trace", skip(self))]
    pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            if let Some(frame) = self.parse_frame()? {
                trace!(%frame, "Parsed incoming frame");
                return Ok(Some(frame));
            }

            if 0 == self.stream.read_buf(&mut self.read_buffer).await? {
                if self.read_buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(ConnectionError::ResetByPeer.into());
                }
            }
        }
    }

    #[instrument(level = "trace", skip(self, frame), fields(frame = %frame))]
    pub async fn write_frame(&mut self, frame: &Frame) -> Result<()> {
        frame.encode(&mut self.stream).await?;
        self.stream.flush().await?;
        Ok(())
    }

    fn parse_frame(&mut self) -> Result<Option<Frame>> {
        self.decoder.decode(&mut self.read_buffer)
    }
}
