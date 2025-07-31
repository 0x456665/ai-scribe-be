use crate::errors::{AppError, AppResult};
use crate::models::Claims;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use uuid::Uuid;

/// JWT utility functions
pub mod jwt {
    use super::*;

    /// Generate an access token for a user
    pub fn generate_access_token(
        user_id: Uuid,
        email: &str,
        secret: &str,
        expires_in_minutes: i64,
    ) -> AppResult<String> {
        let now = Utc::now();
        let exp = now + Duration::minutes(expires_in_minutes);

        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            token_type: "access".to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_ref()),
        )
        .map_err(AppError::JwtError)
    }

    /// Generate a refresh token for a user
    pub fn generate_refresh_token(
        user_id: Uuid,
        email: &str,
        secret: &str,
        expires_in_days: i64,
    ) -> AppResult<String> {
        let now = Utc::now();
        let exp = now + Duration::days(expires_in_days);

        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            token_type: "refresh".to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_ref()),
        )
        .map_err(AppError::JwtError)
    }

    /// Verify and decode a JWT token
    pub fn verify_token(token: &str, secret: &str) -> AppResult<Claims> {
        let validation = Validation::default();

        decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_ref()),
            &validation,
        )
        .map(|token_data| token_data.claims)
        .map_err(AppError::JwtError)
    }

    /// Extract token from Authorization header
    pub fn extract_token_from_header(auth_header: &str) -> AppResult<&str> {
        if auth_header.starts_with("Bearer ") {
            Ok(&auth_header[7..])
        } else {
            Err(AppError::AuthError(
                "Invalid authorization header format".to_string(),
            ))
        }
    }
}

/// Password hashing utilities
pub mod password {
    use super::*;

    /// Hash a password using Argon2
    pub fn hash_password(password: &str) -> AppResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .unwrap()
            .to_string();

        Ok(password_hash)
    }

    /// Verify a password against its hash
    pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|_| AppError::AuthError("Invalid password hash".to_string()))
            .unwrap();
        let argon2 = Argon2::default();

        match argon2.verify_password(password.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// File handling utilities
pub mod file {
    use super::*;
    use std::path::Path;

    /// Check if a file extension is supported for audio transcription
    pub fn is_supported_audio_format(filename: &str) -> bool {
        let supported_formats = ["wav", "mp3", "m4a", "flac", "ogg"];

        if let Some(ext) = Path::new(filename).extension() {
            if let Some(ext_str) = ext.to_str() {
                return supported_formats.contains(&ext_str.to_lowercase().as_str());
            }
        }

        false
    }

    /// Generate a unique filename for uploaded files
    pub fn generate_unique_filename(original_filename: &str) -> String {
        let uuid = Uuid::new_v4();
        let timestamp = Utc::now().timestamp();

        if let Some(ext) = Path::new(original_filename).extension() {
            format!("{}_{}.{}", timestamp, uuid, ext.to_string_lossy())
        } else {
            format!("{}_{}", timestamp, uuid)
        }
    }

    /// Validate file size
    pub fn validate_file_size(size: usize, max_size: usize) -> AppResult<()> {
        if size > max_size {
            return Err(AppError::ValidationError(format!(
                "File size {} bytes exceeds maximum allowed size of {} bytes",
                size, max_size
            )));
        }
        Ok(())
    }
}

/// Validation utilities
pub mod validation {
    use super::*;
    use validator::Validate;

    /// Validate a struct and return appropriate error
    pub fn validate_request<T: Validate>(request: &T) -> AppResult<()> {
        request.validate().map_err(|e| {
            let error_message = e
                .field_errors()
                .into_iter()
                .map(|(field, errors)| {
                    let field_errors: Vec<String> = errors
                        .iter()
                        .filter_map(|e| e.message.as_ref().map(|m| m.to_string()))
                        .collect();
                    format!("{}: {}", field, field_errors.join(", "))
                })
                .collect::<Vec<String>>()
                .join("; ");

            AppError::ValidationError(error_message)
        })
    }
}
