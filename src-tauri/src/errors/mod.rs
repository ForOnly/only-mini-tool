//! Application errors: serialize to `{ code, message }` for Tauri IPC.

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum AppError {
    #[error("{message}")]
    NotFound { message: String },

    #[error("{message}")]
    ValidationError { message: String },

    #[error("{message}")]
    DbError { message: String },

    #[error("{message}")]
    InternalError { message: String },

    #[error("{message}")]
    OcrNotConfigured { message: String },

    #[error("{message}")]
    OcrNetwork { message: String },

    #[error("{message}")]
    OcrEngine { message: String },

    #[error("{message}")]
    OcrTimeout { message: String },

    #[error("{message}")]
    OcrBadImage { message: String },

    #[error("{message}")]
    OcrClipboardEmpty { message: String },

    #[error("{message}")]
    OcrImageTooLarge { message: String },

    #[error("{message}")]
    OcrBusy { message: String },

    #[error("{message}")]
    OcrCancelled { message: String },
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let (code, message) = match self {
            AppError::NotFound { message } => ("not_found", message.as_str()),
            AppError::ValidationError { message } => ("validation_error", message.as_str()),
            AppError::DbError { message } => ("db_error", message.as_str()),
            AppError::InternalError { message } => ("internal_error", message.as_str()),
            AppError::OcrNotConfigured { message } => ("ocr.not_configured", message.as_str()),
            AppError::OcrNetwork { message } => ("ocr.network", message.as_str()),
            AppError::OcrEngine { message } => ("ocr.engine", message.as_str()),
            AppError::OcrTimeout { message } => ("ocr.timeout", message.as_str()),
            AppError::OcrBadImage { message } => ("ocr.bad_image", message.as_str()),
            AppError::OcrClipboardEmpty { message } => ("ocr.clipboard_empty", message.as_str()),
            AppError::OcrImageTooLarge { message } => ("ocr.image_too_large", message.as_str()),
            AppError::OcrBusy { message } => ("ocr.busy", message.as_str()),
            AppError::OcrCancelled { message } => ("ocr.cancelled", message.as_str()),
        };

        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", code)?;
        state.serialize_field("message", message)?;
        state.end()
    }
}
