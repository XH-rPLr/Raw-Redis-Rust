use crate::{
    CommandError, Frame, ProtocolError, RedisError, Result,
    cmd::RedisCommand,
    db::Db,
    frame::IterFrame,
    metrics,
    types::{CacheValue, RedisData},
};
use bytes::Bytes;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug)]
pub struct Set {
    key: String,
    value: Bytes,
    expiration: Option<Duration>,
}

#[async_trait::async_trait]
impl RedisCommand for Set {
    fn parse(frame: &mut IterFrame) -> Result<Set> {
        let key = frame.extract_string()?;
        let value = frame.next_bytes()?;
        let mut expiration = None;

        loop {
            match frame.extract_string() {
                Ok(str) => match str.to_uppercase().as_str() {
                    "EX" => {
                        let time_value = frame.next_integer()?;
                        expiration = Some(Duration::from_secs(time_value));
                    }
                    "PX" => {
                        let time_value = frame.next_integer()?;
                        expiration = Some(Duration::from_millis(time_value));
                    }
                    _ => return Err(CommandError::unknown("SET")),
                },
                Err(RedisError::Protocol(ProtocolError::UnexpectedEnd)) => break,
                Err(e) => return Err(e),
            }
        }

        Ok(Set {
            key,
            value,
            expiration,
        })
    }

    async fn apply(self, db: &Db) -> Result<Frame> {
        let mut expiry = None;
        if let Some(duration) = self.expiration {
            expiry = Some(Instant::now() + duration);
        };

        let cache_value: CacheValue = CacheValue {
            value: RedisData::String(self.value),
            expiry,
        };
        {
            let _db_latency = metrics::DB_LATENCY
                .with_label_values(&["SET"])
                .start_timer();
            db.set(self.key, cache_value);
        };

        Ok(Frame::simple("OK"))
    }

    fn name(&self) -> &'static str {
        "SET"
    }
}
