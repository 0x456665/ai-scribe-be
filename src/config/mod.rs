// config/mod.rs - Configuration management for the application
use crate::errors::AppError;
use serde::Deserialize;
use std::env;

/// Application configuration loaded from environment variables
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Database connection URL
    pub database_url: String,
    
    /// JWT secret for signing tokens
    pub jwt_secret: String,
    
    /// Server host address
    pub host: String,
    
    /// Server port
    pub port: u16,
    
    /// Access token expiration time in minutes
    pub access_token_expires_in: i64,
    
    /// Refresh token expiration time in days
    pub refresh_token_expires_in: i64,
    
    /// Path to the Whisper model file
    pub whisper_model_path: String,
    
    /// Maximum file size for uploads in bytes (default: 50MB)
    pub max_file_size: usize,
    
    /// Directory for temporary file storage
    pub temp_dir: String,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, AppError> {
        // Load .env file if it exists
        dotenv::dotenv().ok();

        Ok(Config {
            database_url: env::var("DATABASE_URL")
                .map_err(|_| AppError::ConfigError("DATABASE_URL must be set".to_string()))?,
            
            jwt_secret: env::var("JWT_SECRET")
                .map_err(|_| AppError::ConfigError("JWT_SECRET must be set".to_string()))?,
            
            host: env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .map_err(|_| AppError::ConfigError("PORT must be a valid number".to_string()))?,
            
            access_token_expires_in: env::var("ACCESS_TOKEN_EXPIRES_IN")
                .unwrap_or_else(|_| "15".to_string()) // 15 minutes
                .parse()
                .map_err(|_| AppError::ConfigError("ACCESS_TOKEN_EXPIRES_IN must be a valid number".to_string()))?,
            
            refresh_token_expires_in: env::var("REFRESH_TOKEN_EXPIRES_IN")
                .unwrap_or_else(|_| "7".to_string()) // 7 days
                .parse()
                .map_err(|_| AppError::ConfigError("REFRESH_TOKEN_EXPIRES_IN must be a valid number".to_string()))?,
            
            whisper_model_path: env::var("WHISPER_MODEL_PATH")
                .map_err(|_| AppError::ConfigError("WHISPER_MODEL_PATH must be set".to_string()))?,
            
            max_file_size: env::var("MAX_FILE_SIZE")
                .unwrap_or_else(|_| "52428800".to_string()) // 50MB
                .parse()
                .map_err(|_| AppError::ConfigError("MAX_FILE_SIZE must be a valid number".to_string()))?,
            
            temp_dir: env::var("TEMP_DIR").unwrap_or_else(|_| "/tmp".to_string()),
        })
    }
}