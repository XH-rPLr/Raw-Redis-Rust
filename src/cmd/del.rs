use crate::{
    Frame, Result,
    cmd::{IterFrame, RedisCommand},
    db::Db,
};

#[derive(Debug)]
pub struct Del {
    keys: Vec<String>,
}

#[async_trait::async_trait]
impl RedisCommand for Del {
    fn parse(frame: &mut IterFrame) -> Result<Del> {
        let mut keys: Vec<String> = vec![frame.extract_string()?];

        while let Ok(next_key) = frame.extract_string() {
            keys.push(next_key);
        }

        frame.finish()?;
        Ok(Del { keys })
    }

    async fn apply(self, db: &Db) -> Result<Frame> {
        let deleted = db.del(self.keys);
        Ok(Frame::integer(deleted as u64))
    }

    fn name(&self) -> &'static str {
        "Del"
    }
}
