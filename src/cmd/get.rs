use crate::{
    CommandError, Frame, Result, cmd::RedisCommand, db::Db, frame::IterFrame, metrics,
    types::RedisData,
};

#[derive(Debug)]
pub struct Get {
    key: String,
}

#[async_trait::async_trait]
impl RedisCommand for Get {
    fn parse(frame: &mut IterFrame) -> Result<Get> {
        let key = frame.extract_string()?;
        frame.finish()?;

        Ok(Get { key })
    }

    async fn apply(self, db: &Db) -> Result<Frame> {
        let _db_latency = metrics::DB_LATENCY
            .with_label_values(&["GET"])
            .start_timer();

        match db.get(self.key) {
            Some(result) => match &result {
                RedisData::String(val) => Ok(Frame::bulk(val.clone())),
                RedisData::List(_) => Err(CommandError::wrong_type("GET")),
            },
            _ => Ok(Frame::null()),
        }
    }

    fn name(&self) -> &'static str {
        "GET"
    }
}
