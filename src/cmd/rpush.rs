use crate::{
    CommandError, Frame, ProtocolError, RedisError, Result, cmd::*, db::Db, frame::IterFrame,
};

use bytes::Bytes;

#[derive(Debug)]
pub struct Rpush {
    key: String,
    values: Vec<Bytes>,
}

#[async_trait::async_trait]
impl RedisCommand for Rpush {
    fn parse(frame: &mut IterFrame) -> Result<Rpush> {
        let key = frame.extract_string()?;
        let mut values: Vec<Bytes> = Vec::new();
        values.push(frame.next_bytes()?);

        loop {
            match frame.next_bytes() {
                Ok(value) => values.push(value),
                Err(RedisError::Protocol(ProtocolError::UnexpectedEnd)) => break,
                Err(e) => return Err(e),
            }
        }

        Ok(Rpush { key, values })
    }

    async fn apply(self, db: &Db) -> Result<Frame> {
        match db.rpush(self.key, self.values) {
            Some(len) => Ok(Frame::integer(len as u64)),
            _ => Err(CommandError::wrong_type("RPUSH")),
        }
    }

    fn name(&self) -> &'static str {
        "RPUSH"
    }
}
