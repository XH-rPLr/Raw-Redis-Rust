use crate::{
    Frame, Result,
    cmd::{IterFrame, RedisCommand},
    db::Db,
};

#[derive(Debug)]
pub struct Unlink {
    keys: Vec<String>,
}

#[async_trait::async_trait]
impl RedisCommand for Unlink {
    fn parse(frame: &mut IterFrame) -> Result<Unlink> {
        let mut keys: Vec<String> = vec![frame.extract_string()?];

        while let Ok(next_key) = frame.extract_string() {
            keys.push(next_key);
        }

        frame.finish()?;
        Ok(Unlink { keys })
    }

    async fn apply(self, db: &Db) -> Result<Frame> {
        let deleted = db.unlink(self.keys);
        Ok(Frame::integer(deleted as u64))
    }

    fn name(&self) -> &'static str {
        "Unlink"
    }
}
