// models/mod.rs - Database models and structures
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

/// User model representing a registered user in the system
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Transcript model representing a transcription result
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Transcript {
    pub id: Uuid,
    pub user_id: Uuid,
    pub filename: String,
    pub transcription: String,
    pub file_size: i64,
    pub duration_seconds: Option<f64>,
    pub created_at: DateTime<Utc>,
}

/// Request models for API endpoints

/// User registration request
#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}

/// User login request
#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    
    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

/// Token refresh request
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

/// Response models for API endpoints

/// Authentication response containing tokens
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserResponse,
}

/// User response (without sensitive data)
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            created_at: user.created_at,
        }
    }
}

/// Transcription response
#[derive(Debug, Serialize)]
pub struct TranscriptResponse {
    pub id: Uuid,
    pub filename: String,
    pub transcription: String,
    pub file_size: i64,
    pub duration_seconds: Option<f64>,
    pub created_at: DateTime<Utc>,
}

impl From<Transcript> for TranscriptResponse {
    fn from(transcript: Transcript) -> Self {
        Self {
            id: transcript.id,
            filename: transcript.filename,
            transcription: transcript.transcription,
            file_size: transcript.file_size,
            duration_seconds: transcript.duration_seconds,
            created_at: transcript.created_at,
        }
    }
}

/// Paginated response wrapper
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
}

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // Subject (user ID)
    pub email: String,
    pub iat: i64, // Issued at
    pub exp: i64, // Expiration time
    pub token_type: String, // "access" or "refresh"
}

/// File upload metadata
#[derive(Debug, Clone)]
pub struct FileUpload {
    pub filename: String,
    pub content_type: String,
    pub size: usize,
    pub data: Vec<u8>,
}