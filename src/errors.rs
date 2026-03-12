use std::string::FromUtf8Error;
use thiserror::Error;
use tokio::sync::AcquireError;

pub type Result<T> = std::result::Result<T, RedisError>;

#[derive(Error, Debug)]
pub enum RedisError {
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Internal(#[from] InternalError),
    #[error(transparent)]
    Connection(#[from] ConnectionError),
}

#[derive(Error, Debug, PartialEq)]
pub enum ConnectionError {
    #[error("connection reset by peer")]
    ResetByPeer,
    #[error("Failed to read frame: {frame}")]
    ReadError { frame: String },
    #[error("Failed to write frame: {frame}")]
    WriteError { frame: String },
    #[error("Timed out while writing to server")]
    IdleTimeout,
}

#[derive(Error, Debug, PartialEq)]
pub enum CommandError {
    #[error("wrong number of arguments for '{command}'")]
    WrongNumberOfArguments { command: String },
    #[error("wrong type for '{command}' argument")]
    WrongArgumentType { command: String },
    #[error("unknown command '{command}'")]
    UnknownCommand { command: String },
    #[error("parse error while parsing for '{command}")]
    ParseCommand { command: String },
}

#[derive(Error, Debug, PartialEq)]
pub enum ProtocolError {
    #[error("invalid command format")]
    InvalidCommandFormat,
    #[error("unexpected end of stream")]
    UnexpectedEnd,
    #[error("Incomplete data to parse a complete frame")]
    Incomplete,
    #[error("Invalid frame format")]
    InvalidFrameFormat,
}

#[derive(Error, Debug)]
pub enum InternalError {
    #[error("internal server error: {message}")]
    Server { message: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("utf error: {0}")]
    Utf(#[from] FromUtf8Error),
    #[error("Acquire error: {0}")]
    Acquire(#[from] AcquireError),
    #[error("Prometheus error: {0}")]
    Prometheus(#[from] prometheus::Error),
}

// Convert errors to Redis protocol format
impl From<RedisError> for Vec<u8> {
    fn from(error: RedisError) -> Vec<u8> {
        format!("ERR {}\r\n", error).into_bytes()
    }
}

impl ConnectionError {
    pub fn reset_by_peer() -> RedisError {
        RedisError::Connection(ConnectionError::ResetByPeer)
    }

    pub fn read_error(frame: &str) -> RedisError {
        RedisError::Connection(ConnectionError::ReadError {
            frame: frame.to_string(),
        })
    }

    pub fn write_error(frame: &str) -> RedisError {
        RedisError::Connection(ConnectionError::WriteError {
            frame: frame.to_string(),
        })
    }

    pub fn idle_timeout_error() -> RedisError {
        RedisError::Connection(ConnectionError::IdleTimeout)
    }
}

impl CommandError {
    pub fn wrong_args(command: &str) -> RedisError {
        RedisError::Command(CommandError::WrongNumberOfArguments {
            command: command.to_string(),
        })
    }

    pub fn wrong_type(command: &str) -> RedisError {
        RedisError::Command(CommandError::WrongArgumentType {
            command: command.to_string(),
        })
    }

    pub fn unknown(command: &str) -> RedisError {
        RedisError::Command(CommandError::UnknownCommand {
            command: command.to_string(),
        })
    }

    pub fn parse(command: &str) -> RedisError {
        RedisError::Command(CommandError::ParseCommand {
            command: command.to_string(),
        })
    }
}

impl ProtocolError {
    pub fn invalid_format() -> RedisError {
        RedisError::Protocol(ProtocolError::InvalidCommandFormat)
    }

    pub fn unexpected_end() -> RedisError {
        RedisError::Protocol(ProtocolError::UnexpectedEnd)
    }

    pub fn incomplete() -> RedisError {
        RedisError::Protocol(ProtocolError::Incomplete)
    }

    pub fn invalid_frame_format() -> RedisError {
        RedisError::Protocol(ProtocolError::InvalidFrameFormat)
    }
}

impl InternalError {
    pub fn internal_server(message: &str) -> RedisError {
        RedisError::Internal(InternalError::Server {
            message: message.to_string(),
        })
    }
}

impl From<std::io::Error> for RedisError {
    fn from(error: std::io::Error) -> RedisError {
        RedisError::Internal(InternalError::Io(error))
    }
}

impl From<FromUtf8Error> for RedisError {
    fn from(error: FromUtf8Error) -> RedisError {
        RedisError::Internal(InternalError::Utf(error))
    }
}

impl From<AcquireError> for RedisError {
    fn from(error: AcquireError) -> RedisError {
        RedisError::Internal(InternalError::Acquire(error))
    }
}

impl From<prometheus::Error> for RedisError {
    fn from(error: prometheus::Error) -> RedisError {
        RedisError::Internal(InternalError::Prometheus(error))
    }
}
