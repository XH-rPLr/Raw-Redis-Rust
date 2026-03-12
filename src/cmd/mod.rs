pub(crate) use crate::{CommandError, Frame, Result, db::Db, frame::IterFrame};

#[derive(Debug)]
pub enum Command {
    Ping(Ping),
    Echo(Echo),
    Get(Get),
    Set(Set),
    Rpush(Rpush),
    Del(Del),
    Unlink(Unlink),
    FlushAll(FlushAll),
}

pub mod del;
pub mod echo;
pub mod flushall;
pub mod get;
pub mod ping;
pub mod rpush;
pub mod set;
pub mod unlink;

pub use del::Del;
pub use echo::Echo;
pub use flushall::FlushAll;
pub use get::Get;
pub use ping::Ping;
pub use rpush::Rpush;
pub use set::Set;
pub use unlink::Unlink;

use tracing::instrument;

impl Command {
    pub fn from_frame(value: Frame) -> Result<Command> {
        let mut frame_to_iter = IterFrame::new(value)?;
        let cmd_name = frame_to_iter.extract_string()?;

        match cmd_name.as_str() {
            "PING" => Ok(Command::Ping(Ping::parse(&mut frame_to_iter)?)),
            "ECHO" => Ok(Command::Echo(Echo::parse(&mut frame_to_iter)?)),
            "GET" => Ok(Command::Get(Get::parse(&mut frame_to_iter)?)),
            "SET" => Ok(Command::Set(Set::parse(&mut frame_to_iter)?)),
            "RPUSH" => Ok(Command::Rpush(Rpush::parse(&mut frame_to_iter)?)),
            "DEL" => Ok(Command::Del(Del::parse(&mut frame_to_iter)?)),
            "UNLINK" => Ok(Command::Unlink(Unlink::parse(&mut frame_to_iter)?)),
            "FLUSHALL" => Ok(Command::FlushAll(FlushAll::parse(&mut frame_to_iter)?)),
            _ => Err(CommandError::unknown(&cmd_name)),
        }
    }

    #[instrument(level = "trace", skip(self, db))]
    pub async fn apply(self, db: &Db) -> Result<Frame> {
        match self {
            Command::Ping(ping) => ping.apply(db).await,
            Command::Echo(echo) => echo.apply(db).await,
            Command::Get(get) => get.apply(db).await,
            Command::Set(set) => set.apply(db).await,
            Command::Rpush(rpush) => rpush.apply(db).await,
            Command::Del(del) => del.apply(db).await,
            Command::Unlink(unlink) => unlink.apply(db).await,
            Command::FlushAll(flushall) => flushall.apply(db).await,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Command::Ping(ping) => ping.name(),
            Command::Echo(echo) => echo.name(),
            Command::Get(get) => get.name(),
            Command::Set(set) => set.name(),
            Command::Rpush(rpush) => rpush.name(),
            Command::Del(del) => del.name(),
            Command::Unlink(unlink) => unlink.name(),
            Command::FlushAll(flushall) => flushall.name(),
        }
    }
}

#[async_trait::async_trait]
trait RedisCommand: Sized {
    fn parse(frame: &mut IterFrame) -> Result<Self>;

    async fn apply(self, db: &Db) -> Result<Frame>;

    fn name(&self) -> &'static str;
}
