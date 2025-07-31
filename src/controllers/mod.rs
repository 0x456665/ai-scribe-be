// controllers/mod.rs - Route handlers and response logic
use crate::errors::AppResult;
use actix_web::HttpResponse;
use serde_json::json;

pub mod auth_controller;
pub mod transcription_controller;

pub use auth_controller::*;
pub use transcription_controller::*;

/// Health check controller
pub struct HealthController;

impl HealthController {
    /// Health check endpoint
    pub async fn health() -> AppResult<HttpResponse> {
        Ok(HttpResponse::Ok().json(json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now()
        })))
    }
}

/// Query parameters for pagination
#[derive(serde::Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
}
