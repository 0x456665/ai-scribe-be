use actix_cors::Cors;
use actix_web::{App, HttpServer, middleware::Logger, web};
use sqlx::PgPool;
use std::sync::Arc;
use whisper_rs::{self, WhisperContextParameters};
mod config;
mod controllers;
mod errors;
mod middlewares;
mod models;
mod routes;
mod services;
mod utils;

use config::Config;
use errors::AppError;

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub whisper_ctx: Arc<whisper_rs::WhisperContext>,
}

#[actix_web::main]
async fn main() -> Result<(), AppError> {
    // Initialize logger
    env_logger::init();

    // Load configuration
    let config = Arc::new(Config::from_env()?);
    log::info!("Configuration loaded successfully");

    // Connect to database
    let db = PgPool::connect(&config.database_url).await?;
    log::info!("Connected to PostgreSQL database");

    // Run database migrations
    sqlx::migrate!("./src/migrations").run(&db).await.unwrap();
    log::info!("Database migrations completed");

    // Initialize Whisper model
    log::info!("Loading Whisper model from: {}", config.whisper_model_path);
    let whisper_ctx = Arc::new(
        whisper_rs::WhisperContext::new_with_params(
            &config.whisper_model_path,
            WhisperContextParameters { use_gpu: false }, //I previously set this to true
        )
        .map_err(|e| AppError::WhisperError(format!("Failed to load Whisper model: {}", e)))?,
    );
    log::info!("Whisper model loaded successfully");

    // Create application state
    let app_state = AppState {
        db,
        config: config.clone(),
        whisper_ctx,
    };

    let bind_address = format!("{}:{}", config.host, config.port);
    log::info!("Starting server at http://{}", bind_address);

    // Start HTTP server
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(cors)
            .wrap(Logger::default())
            .configure(routes::configure_routes)
    })
    .bind(&bind_address)?
    .run()
    .await?;

    Ok(())
}
