use crate::{
    Frame, ProtocolError, RedisError, Result, cmd::RedisCommand, db::Db, frame::IterFrame,
};
use bytes::Bytes;

#[derive(Debug)]
pub struct Ping {
    message: Option<Bytes>,
}

#[async_trait::async_trait]
impl RedisCommand for Ping {
    fn parse(frame: &mut IterFrame) -> Result<Ping> {
        match frame.next_bytes() {
            Ok(bytes) => {
                frame.finish()?;
                let message = Some(bytes);
                Ok(Ping { message })
            }
            Err(RedisError::Protocol(ProtocolError::UnexpectedEnd)) => Ok(Ping { message: None }),
            _ => Err(ProtocolError::invalid_format()),
        }
    }

    async fn apply(self, _db: &Db) -> Result<Frame> {
        if let Some(message) = self.message {
            return Ok(Frame::Bulk(message));
        }
        Ok(Frame::simple("PONG"))
    }

    fn name(&self) -> &'static str {
        "PING"
    }
}
