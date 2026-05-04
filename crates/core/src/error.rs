use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("aiken project not found at or above {path}")]
    ProjectNotFound { path: String },

    #[error("aiken executable not available on PATH")]
    AikenNotInstalled,

    #[error("aiken process failed (exit {exit_code:?}): {stderr}")]
    AikenProcessFailed { exit_code: Option<i32>, stderr: String },

    #[error("could not parse aiken output as expected: {context}")]
    OutputParseFailed { context: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

impl CoreError {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
