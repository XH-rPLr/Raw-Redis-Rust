use crate::{
    Frame, Result,
    cmd::{IterFrame, RedisCommand},
    db::Db,
};

#[derive(Debug)]
pub struct FlushAll {}

#[async_trait::async_trait]
impl RedisCommand for FlushAll {
    fn parse(frame: &mut IterFrame) -> Result<FlushAll> {
        frame.finish()?;
        Ok(FlushAll {})
    }

    async fn apply(self, db: &Db) -> Result<Frame> {
        db.flush();
        Ok(Frame::simple("OK"))
    }

    fn name(&self) -> &'static str {
        "FlushAll"
    }
}
