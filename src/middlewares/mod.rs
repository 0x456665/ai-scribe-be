// middleware/mod.rs - JWT authentication middleware
use crate::errors::{AppError, AppResult};
use crate::models::Claims;
use crate::utils::jwt;
use crate::AppState;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{ready, Ready},
    rc::Rc,
};

/// JWT Authentication middleware
pub struct JwtAuth;

impl<S, B> Transform<S, ServiceRequest> for JwtAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = JwtAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(JwtAuthMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct JwtAuthMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();

        Box::pin(async move {
            // Extract app state
            let app_state = req
                .app_data::<actix_web::web::Data<AppState>>()
                .ok_or_else(|| AppError::InternalError("App state not found".to_string()))?;

            // Get authorization header
            let auth_header = req
                .headers()
                .get("Authorization")
                .and_then(|h| h.to_str().ok())
                .ok_or_else(|| AppError::AuthError("Missing authorization header".to_string()))?;

            // Extract and verify token
            let token = jwt::extract_token_from_header(auth_header)?;
            let claims = jwt::verify_token(token, &app_state.config.jwt_secret)?;

            // Validate token type (should be access token for protected routes)
            if claims.token_type != "access" {
                return Err(AppError::AuthError("Invalid token type".to_string()).into());
            }

            // Add claims to request extensions for use in handlers
            req.extensions_mut().insert(claims);

            // Continue with the request
            let res = service.call(req).await?;
            Ok(res)
        })
    }
}

/// Extract user claims from request extensions
/// This function should be called from protected route handlers
pub fn extract_claims(req: &actix_web::HttpRequest) -> AppResult<Claims> {
    req.extensions()
        .get::<Claims>()
        .cloned()
        .ok_or_else(|| AppError::AuthError("User claims not found in request".to_string()))
}

/// Extract user ID from request (convenience function)
pub fn extract_user_id(req: &actix_web::HttpRequest) -> AppResult<uuid::Uuid> {
    let claims = extract_claims(req)?;
    claims
        .sub
        .parse()
        .map_err(|_| AppError::AuthError("Invalid user ID in token".to_string()))
}