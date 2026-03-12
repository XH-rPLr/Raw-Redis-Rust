use crate::{
    Frame, Result,
    cmd::{IterFrame, RedisCommand},
    db::Db,
};
use bytes::Bytes;

#[derive(Debug)]
pub struct Echo {
    message: Bytes,
}

#[async_trait::async_trait]
impl RedisCommand for Echo {
    fn parse(frame: &mut IterFrame) -> Result<Echo> {
        let message = frame.next_bytes()?;
        frame.finish()?;
        Ok(Echo { message })
    }

    async fn apply(self, _db: &Db) -> Result<Frame> {
        Ok(Frame::bulk(self.message))
    }

    fn name(&self) -> &'static str {
        "ECHO"
    }
}
