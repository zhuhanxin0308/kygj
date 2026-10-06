use serde::{Deserialize, Serialize};

/// IPC只传递稳定错误码和中文说明，不将数据库或进程内部异常直接外发。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
#[serde(deny_unknown_fields)]
#[error("{message}")]
pub struct CoreError {
    pub code: String,
    pub message: String,
}

pub type CoreResult<T> = Result<T, CoreError>;

impl CoreError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.to_owned(), message: message.into() }
    }
}
