use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(
    export,
    export_to = "../../src/generated/",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum ErrorCode {
    InvalidTransition,
    SlotFull,
    BatchRequiresMicro,
    MissingRequiredField,
    SessionAlreadyActive,
    ExtensionUsed,
    NotPlannedForToday,
    MissingOutcome,
    InvalidDay,
    NothingToUndo,
    NotFound,
    Boulder,
    ReadyFull,
    BudgetExceeded,
    StuckTask,
    PlanLocked,
    InvalidEstimate,
    InvalidSetting,
    Internal,
}

#[derive(Debug, Clone, Serialize, TS, thiserror::Error)]
#[error("{message}")]
#[ts(export, export_to = "../../src/generated/")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub overridable: bool,
    pub hint: Option<String>,
}

pub type Result<T> = std::result::Result<T, AppError>;

impl AppError {
    pub fn hard(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            overridable: false,
            hint: None,
        }
    }

    pub fn soft(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            overridable: true,
            hint: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::hard(ErrorCode::Internal, message)
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn not_found(what: &str) -> Self {
        Self::hard(ErrorCode::NotFound, format!("{what} not found"))
    }

    pub fn invalid_transition(message: impl Into<String>) -> Self {
        Self::hard(ErrorCode::InvalidTransition, message)
    }

    pub fn missing(message: impl Into<String>) -> Self {
        Self::hard(ErrorCode::MissingRequiredField, message)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::internal(format!("database error: {value}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::internal(format!("serialization error: {value}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::internal(format!("io error: {value}"))
    }
}

impl From<uuid::Error> for AppError {
    fn from(value: uuid::Error) -> Self {
        Self::internal(format!("uuid error: {value}"))
    }
}
