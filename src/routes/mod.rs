use crate::controllers::{AuthController, HealthController, TranscriptionController};
use crate::middlewares::JwtAuth;
use actix_web::{web, HttpResponse};

/// Configure all application routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Health check route (no authentication required)
        .route("/health", web::get().to(HealthController::health))
        
        // API v1 routes
        .service(
            web::scope("/api/v1")
                // Authentication routes (no JWT required)
                .service(
                    web::scope("/auth")
                        .route("/register", web::post().to(AuthController::register))
                        .route("/login", web::post().to(AuthController::login))
                        .route("/refresh", web::post().to(AuthController::refresh))
                )
                // Protected routes (JWT required)
                .service(
                    web::scope("")
                        .wrap(JwtAuth) // Apply JWT middleware to all routes in this scope
                        
                        // User profile routes
                        .route("/me", web::get().to(AuthController::me))
                        
                        // Transcription routes
                        .service(
                            web::scope("/transcripts")
                                .route("", web::post().to(TranscriptionController::upload_and_transcribe))
                                .route("", web::get().to(TranscriptionController::get_transcripts))
                                .route("/{id}", web::get().to(TranscriptionController::get_transcript))
                                .route("/{id}", web::delete().to(TranscriptionController::delete_transcript))
                        )
                )
        )
        // Catch-all route for undefined endpoints
        .default_service(web::route().to(not_found));
}

/// 404 handler for undefined routes
async fn not_found() -> HttpResponse {
    HttpResponse::NotFound().json(serde_json::json!({
        "error": "Not Found",
        "message": "The requested endpoint does not exist"
    }))
}