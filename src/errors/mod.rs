// errors/mod.rs - Central error handling for the application
use actix_web::{HttpResponse, ResponseError};
use serde_json::json;
use thiserror::Error;

/// Main application error type
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),

    #[error("Password hashing error: {0}")]
    ArgonError(#[from(std::error::Error)] argon2::password_hash::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal server error: {0}")]
    InternalError(String),

    #[error("Whisper transcription error: {0}")]
    WhisperError(String),

    #[error("File processing error: {0}")]
    FileError(String),

    #[error("Unauthorized access")]
    Unauthorized,

    #[error("Forbidden access")]
    Forbidden,
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let (status_code, error_message) = match self {
            AppError::AuthError(_) | AppError::Unauthorized => {
                (actix_web::http::StatusCode::UNAUTHORIZED, "Unauthorized")
            }
            AppError::ValidationError(_) | AppError::BadRequest(_) => {
                (actix_web::http::StatusCode::BAD_REQUEST, "Bad Request")
            }
            AppError::NotFound(_) => (actix_web::http::StatusCode::NOT_FOUND, "Not Found"),
            AppError::Forbidden => (actix_web::http::StatusCode::FORBIDDEN, "Forbidden"),
            _ => {
                log::error!("Internal server error: {}", self);
                (
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error",
                )
            }
        };

        HttpResponse::build(status_code).json(json!({
            "error": error_message,
            "message": self.to_string()
        }))
    }
}

/// Result type alias for convenience
pub type AppResult<T> = Result<T, AppError>;
