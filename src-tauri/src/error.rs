// AppError — seven categories from PROJECT_SPEC.md §3.7.
// Manager-layer / platform-layer / provider-layer errors all funnel here and
// get emitted to the frontend as the `error` event.

use serde::Serialize;
use thiserror::Error;

#[allow(dead_code)] // Future variants land as later slices wire them up.
#[derive(Debug, Error, Serialize, Clone)]
#[serde(tag = "code", content = "message", rename_all = "snake_case")]
pub enum AppError {
    #[error("permission denied: {0}")]
    Permission(String),

    #[error("device error: {0}")]
    Device(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("inject error: {0}")]
    Inject(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn recoverable(&self) -> bool {
        // §3.7 table — Permission / Device / Network are recoverable;
        // Provider / Internal are not; Inject is partial (clipboard fallback).
        matches!(
            self,
            AppError::Permission(_) | AppError::Device(_) | AppError::Network(_)
        )
    }

    /// Stable snake_case code for the §3.6 `error` event payload.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Permission(_) => "permission",
            AppError::Device(_) => "device",
            AppError::Network(_) => "network",
            AppError::Provider(_) => "provider",
            AppError::Inject(_) => "inject",
            AppError::Internal(_) => "internal",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            AppError::Permission(m)
            | AppError::Device(m)
            | AppError::Network(m)
            | AppError::Provider(m)
            | AppError::Inject(m)
            | AppError::Internal(m) => m,
        }
    }
}

#[allow(dead_code)] // Convenience alias; managers consume it in later slices.
pub type AppResult<T> = std::result::Result<T, AppError>;
