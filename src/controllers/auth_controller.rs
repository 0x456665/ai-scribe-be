// controllers/mod.rs - Route handlers and response logic
use crate::AppState;
use crate::errors::{AppError, AppResult};
use crate::middlewares::extract_user_id;
use crate::models::*;
use crate::services::UserService;
use crate::utils::{jwt, validation};
use actix_web::cookie::time::Duration;
use actix_web::{
    HttpRequest, HttpResponse,
    cookie::{Cookie, SameSite},
    web,
};
use uuid::Uuid;

// Authentication controller
pub struct AuthController;

impl AuthController {
    /// Register a new user
    pub async fn register(
        app_state: web::Data<AppState>,
        request: web::Json<RegisterRequest>,
    ) -> AppResult<HttpResponse> {
        // Validate request
        validation::validate_request(&*request)?;

        // Register user
        let user =
            UserService::register_user(&app_state.db, &request.email, &request.password).await?;

        // Generate tokens
        let access_token = jwt::generate_access_token(
            user.id,
            &user.email,
            &app_state.config.jwt_secret,
            app_state.config.access_token_expires_in,
        )?;

        let refresh_token = jwt::generate_refresh_token(
            user.id,
            &user.email,
            &app_state.config.jwt_secret,
            app_state.config.refresh_token_expires_in,
        )?;

        let cookie = Cookie::build("refresh_token", refresh_token)
            .path("/")
            .http_only(true)
            .secure(true)
            .max_age(Duration::days(7))
            .same_site(SameSite::Strict)
            .finish();

        let response = AuthResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: app_state.config.access_token_expires_in * 60, // Convert to seconds
            user: user.into(),
        };

        Ok(HttpResponse::Created().cookie(cookie).json(response))
    }

    /// Login user
    pub async fn login(
        app_state: web::Data<AppState>,
        request: web::Json<LoginRequest>,
    ) -> AppResult<HttpResponse> {
        // Validate request
        validation::validate_request(&*request)?;

        // Authenticate user
        let user = UserService::authenticate_user(&app_state.db, &request.email, &request.password)
            .await?;

        // Generate tokens
        let access_token = jwt::generate_access_token(
            user.id,
            &user.email,
            &app_state.config.jwt_secret,
            app_state.config.access_token_expires_in,
        )?;

        let refresh_token = jwt::generate_refresh_token(
            user.id,
            &user.email,
            &app_state.config.jwt_secret,
            app_state.config.refresh_token_expires_in,
        )?;

        let cookie = Cookie::build("refresh_token", refresh_token)
            .path("/")
            .http_only(true)
            .secure(true)
            .max_age(Duration::days(7))
            .same_site(SameSite::Strict)
            .finish();

        let response = AuthResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: app_state.config.access_token_expires_in * 60,
            user: user.into(),
        };

        Ok(HttpResponse::Ok().cookie(cookie).json(response))
    }

    /// Refresh access token
    pub async fn refresh(
        app_state: web::Data<AppState>,
        request: HttpRequest,
    ) -> AppResult<HttpResponse> {
        // Verify refresh token

        let refresh_token = match request.cookie("refresh_token") {
            Some(cookie) => cookie.value().to_string(),
            None => {
                return Err(AppError::AuthError(
                    "Refresh token not found in cookies".to_string(),
                ));
            }
        };
        let claims =
            UserService::verify_refresh_token(&refresh_token, &app_state.config.jwt_secret)?;

        // Get user from database to ensure they still exist
        let user_id: Uuid = claims
            .sub
            .parse()
            .map_err(|_| AppError::AuthError("Invalid user ID in token".to_string()))?;

        let user = UserService::get_user_by_id(&app_state.db, user_id).await?;

        // Generate new tokens
        let access_token = jwt::generate_access_token(
            user.id,
            &user.email,
            &app_state.config.jwt_secret,
            app_state.config.access_token_expires_in,
        )?;

        // let refresh_token = jwt::generate_refresh_token(
        //     user.id,
        //     &user.email,
        //     &app_state.config.jwt_secret,
        //     app_state.config.refresh_token_expires_in,
        // )?;

        let response = AuthResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: app_state.config.access_token_expires_in * 60,
            user: user.into(),
        };

        Ok(HttpResponse::Ok().json(response))
    }

    /// Get current user profile
    pub async fn me(app_state: web::Data<AppState>, req: HttpRequest) -> AppResult<HttpResponse> {
        let user_id = extract_user_id(&req)?;
        let user = UserService::get_user_by_id(&app_state.db, user_id).await?;

        Ok(HttpResponse::Ok().json(UserResponse::from(user)))
    }
}
