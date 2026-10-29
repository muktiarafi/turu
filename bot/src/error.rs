use crate::host::HostError;

#[derive(Debug, thiserror::Error)]
pub enum BotError {
    /// The script was asked to stop. Unwinds the script (VibeSkua: OperationCanceledException).
    #[error("script stopped")]
    Stopped,
    #[error("flash: {0}")]
    Host(String),
    #[error("couldn't read game data from {path}: {message}")]
    Json { path: String, message: String },
    #[error("{0}")]
    InvalidArgument(String),
    #[error("unknown bot method `{0}`")]
    UnknownMethod(String),
    #[error("{0}")]
    Script(String),
    #[error("{0}")]
    Other(String),
}

impl From<HostError> for BotError {
    fn from(value: HostError) -> Self {
        Self::Host(value.to_string())
    }
}

pub type BotResult<T> = Result<T, BotError>;
