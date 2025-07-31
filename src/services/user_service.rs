use crate::errors::{AppError, AppResult};
use crate::models::{Claims, User};
use crate::utils::{jwt, password};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

/// User service for authentication and user management
pub struct UserService;

impl UserService {
    /// Register a new user
    pub async fn register_user(pool: &PgPool, email: &str, password: &str) -> AppResult<User> {
        // Check if user already exists
        let existing_user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(email)
            .fetch_optional(pool)
            .await?;

        if existing_user.is_some() {
            return Err(AppError::ValidationError(
                "User with this email already exists".to_string(),
            ));
        }

        // Hash password
        let password_hash = password::hash_password(password)?;

        // Create new user
        let user_id = Uuid::new_v4();
        let now = Utc::now();

        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (id, email, password_hash, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(email)
        .bind(password_hash)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await?;

        log::info!("New user registered: {}", email);
        Ok(user)
    }

    /// Authenticate user and return user if valid
    pub async fn authenticate_user(pool: &PgPool, email: &str, password: &str) -> AppResult<User> {
        // Find user by email
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(email)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::AuthError("Invalid email or password".to_string()))?;

        // Verify password
        if !password::verify_password(password, &user.password_hash)? {
            return Err(AppError::AuthError("Invalid email or password".to_string()));
        }

        log::info!("User authenticated: {}", email);
        Ok(user)
    }

    /// Get user by ID
    pub async fn get_user_by_id(pool: &PgPool, user_id: Uuid) -> AppResult<User> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

        Ok(user)
    }

    /// Verify refresh token and return claims
    pub fn verify_refresh_token(token: &str, secret: &str) -> AppResult<Claims> {
        let claims = jwt::verify_token(token, secret)?;

        if claims.token_type != "refresh" {
            return Err(AppError::AuthError("Invalid token type".to_string()));
        }

        Ok(claims)
    }
}
