use crate::controllers::PaginationQuery;
use crate::AppState;
use crate::errors::{AppError, AppResult};
use crate::middlewares::extract_user_id;
use crate::models::*;
use crate::services::TranscriptionService;
use crate::utils::file;
use actix_multipart::Multipart;
use actix_web::{HttpRequest, HttpResponse, web};
use futures_util::TryStreamExt;
use serde_json::json;
use std::time::Instant;
use uuid::Uuid;

/// Transcription controller
pub struct TranscriptionController;

impl TranscriptionController {
    /// Upload and transcribe audio file with enhanced processing
    pub async fn upload_and_transcribe(
        app_state: web::Data<AppState>,
        req: HttpRequest,
        mut payload: Multipart,
    ) -> AppResult<HttpResponse> {
        let start_time = Instant::now();
        let user_id = extract_user_id(&req)?;

        log::info!("Starting transcription request for user: {}", user_id);

        // Process multipart form data
        let mut file_upload: Option<FileUpload> = None;

        while let Some(mut field) = payload
            .try_next()
            .await
            .map_err(|e| AppError::BadRequest(format!("Failed to read multipart data: {}", e)))?
        {
            let content_disposition = field.content_disposition();

            if let Some(name) = content_disposition.get_name() {
                if name == "audio_file" {
                    // Get filename
                    let filename = content_disposition
                        .get_filename()
                        .ok_or_else(|| AppError::BadRequest("Filename is required".to_string()))?
                        .to_string();

                    log::info!("Processing uploaded file: {}", filename);

                    // Validate file format (now supports more formats thanks to FFmpeg)
                    if !Self::is_supported_audio_format(&filename) {
                        return Err(AppError::ValidationError(
                            "Unsupported audio format. Supported formats: wav, mp3, m4a, flac, ogg, aac, wma, aiff, au"
                                .to_string(),
                        ));
                    }

                    // Read file data
                    let mut file_data = Vec::new();
                    while let Some(chunk) = field.try_next().await.map_err(|e| {
                        AppError::BadRequest(format!("Failed to read audio file chunk: {}", e))
                    })? {
                        file_data.extend_from_slice(&chunk);
                    }

                    // Validate file size
                    file::validate_file_size(file_data.len(), app_state.config.max_file_size)?;

                    log::info!("File uploaded successfully: {} bytes", file_data.len());

                    // Get content type
                    let content_type = field
                        .content_type()
                        .map(|ct| ct.to_string())
                        .unwrap_or_else(|| Self::guess_content_type(&filename));

                    file_upload = Some(FileUpload {
                        filename: file::generate_unique_filename(&filename),
                        content_type,
                        size: file_data.len(),
                        data: file_data,
                    });
                    break;
                }
            }
        }

        let file_upload = file_upload
            .ok_or_else(|| AppError::BadRequest("No audio file provided".to_string()))?;

        let original_filename = file_upload.filename.clone();
        
        log::info!(
            "Processing transcription for file: {} (size: {} bytes)",
            file_upload.filename,
            file_upload.size
        );

        // Create temporary file path for duration calculation
        let temp_file_path = format!("{}/{}", app_state.config.temp_dir, file_upload.filename);
        
        // Write file temporarily to get duration
        tokio::fs::write(&temp_file_path, &file_upload.data).await
            .map_err(|e| AppError::FileError(format!("Failed to write temporary file: {}", e)))?;

        // Get audio duration before transcription
        let duration_seconds = match TranscriptionService::get_audio_duration(&temp_file_path).await {
            Ok(duration) => {
                log::info!("Audio duration: {:.2} seconds", duration);
                Some(duration)
            }
            Err(e) => {
                log::warn!("Failed to get audio duration: {}", e);
                None
            }
        };

        // Remove temporary file (transcription service will create its own)
        tokio::fs::remove_file(&temp_file_path).await.ok();

        // Transcribe audio using the enhanced service
        log::info!("Starting transcription for file: {}", file_upload.filename);
        
        let transcription_start = Instant::now();
        let transcription = TranscriptionService::transcribe_audio(
            app_state.whisper_ctx.clone(),
            file_upload.clone(),
            &app_state.config.temp_dir,
        )
        .await
        .map_err(|e| {
            log::error!("Transcription failed for file {}: {}", file_upload.filename, e);
            e
        })?;

        let transcription_duration = transcription_start.elapsed();
        
        log::info!(
            "Transcription completed in {:.2}s - Result length: {} characters",
            transcription_duration.as_secs_f64(),
            transcription.len()
        );

        // Log transcription preview for debugging
        if !transcription.is_empty() {
            let preview = transcription.chars().take(100).collect::<String>();
            log::info!("Transcription preview: {}", preview);
        } else {
            log::warn!("Empty transcription result for file: {}", file_upload.filename);
        }

        // Save transcription to database
        let transcript = TranscriptionService::save_transcription(
            &app_state.db,
            user_id,
            &original_filename, // Use original filename for display
            &transcription,
            file_upload.size as i64,
            duration_seconds,
        )
        .await?;

        let total_duration = start_time.elapsed();
        log::info!(
            "Complete transcription workflow finished in {:.2}s for file: {}",
            total_duration.as_secs_f64(),
            original_filename
        );

        // Return enhanced response with processing metadata
        let response = json!({
            "transcript": TranscriptResponse::from(transcript),
            "processing_time_seconds": total_duration.as_secs_f64(),
            "transcription_time_seconds": transcription_duration.as_secs_f64(),
            "audio_duration_seconds": duration_seconds,
            "file_size_bytes": file_upload.size,
            "status": "completed"
        });

        Ok(HttpResponse::Created().json(response))
    }

    // /// Alternative endpoint for direct file transcription (useful for testing)
    // pub async fn transcribe_file(
    //     app_state: web::Data<AppState>,
    //     req: HttpRequest,
    //     path: web::Path<String>,
    // ) -> AppResult<HttpResponse> {
    //     let user_id = extract_user_id(&req)?;
    //     let filename = path.into_inner();
        
    //     // Construct full file path (this would be configured based on your file storage)
    //     let file_path = format!("{}/{}", app_state.config.temp_dir, filename);
        
    //     // Verify file exists
    //     if !tokio::fs::try_exists(&file_path).await.unwrap_or(false) {
    //         return Err(AppError::NotFound("Audio file not found".to_string()));
    //     }

    //     log::info!("Transcribing existing file: {}", file_path);

    //     // Get audio duration
    //     let duration_seconds = TranscriptionService::get_audio_duration(&file_path).await.ok();

    //     // Transcribe the file directly
    //     let transcription = TranscriptionService::convert_and_transcribe_file(
    //         app_state.whisper_ctx.clone(),
    //         &file_path,
    //         &app_state.config.temp_dir,
    //     ).await?;

    //     // Get file size
    //     let file_metadata = tokio::fs::metadata(&file_path).await
    //         .map_err(|e| AppError::FileError(format!("Failed to get file metadata: {}", e)))?;

    //     // Save to database
    //     let transcript = TranscriptionService::save_transcription(
    //         &app_state.db,
    //         user_id,
    //         &filename,
    //         &transcription,
    //         file_metadata.len() as i64,
    //         duration_seconds,
    //     ).await?;

    //     Ok(HttpResponse::Ok().json(TranscriptResponse::from(transcript)))
    // }

    /// Get user's transcripts with enhanced pagination and filtering
    pub async fn get_transcripts(
        app_state: web::Data<AppState>,
        req: HttpRequest,
        query: web::Query<PaginationQuery>,
    ) -> AppResult<HttpResponse> {
        let user_id = extract_user_id(&req)?;

        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(10).min(100).max(1); // Max 100, min 1

        log::debug!("Fetching transcripts for user {} - page: {}, limit: {}", user_id, page, limit);

        let (transcripts, total) =
            TranscriptionService::get_user_transcripts(&app_state.db, user_id, page, limit).await?;

        let total_pages = (total + limit - 1) / limit; // Ceiling division

        let response = PaginatedResponse {
            data: transcripts
                .into_iter()
                .map(TranscriptResponse::from)
                .collect(),
            page,
            limit,
            total,
            total_pages,
        };

        log::debug!("Returning {} transcripts (total: {})", response.data.len(), total);

        Ok(HttpResponse::Ok().json(response))
    }

    /// Get specific transcript by ID with enhanced error handling
    pub async fn get_transcript(
        app_state: web::Data<AppState>,
        req: HttpRequest,
        path: web::Path<Uuid>,
    ) -> AppResult<HttpResponse> {
        let user_id = extract_user_id(&req)?;
        let transcript_id = path.into_inner();

        log::debug!("Fetching transcript {} for user {}", transcript_id, user_id);

        let transcript =
            TranscriptionService::get_transcript_by_id(&app_state.db, transcript_id, user_id)
                .await?;

        Ok(HttpResponse::Ok().json(TranscriptResponse::from(transcript)))
    }

    /// Delete transcript by ID with confirmation
    pub async fn delete_transcript(
        app_state: web::Data<AppState>,
        req: HttpRequest,
        path: web::Path<Uuid>,
    ) -> AppResult<HttpResponse> {
        let user_id = extract_user_id(&req)?;
        let transcript_id = path.into_inner();

        log::info!("Deleting transcript {} for user {}", transcript_id, user_id);

        TranscriptionService::delete_transcript(&app_state.db, transcript_id, user_id).await?;

        Ok(HttpResponse::Ok().json(json!({
            "message": "Transcript deleted successfully",
            "transcript_id": transcript_id,
            "deleted_at": chrono::Utc::now()
        })))
    }

    /// Health check endpoint for transcription service
    // pub async fn health_check(
    //     app_state: web::Data<AppState>,
    // ) -> AppResult<HttpResponse> {
    //     // Check if temp directory is accessible
    //     let temp_dir_exists = tokio::fs::try_exists(&app_state.config.temp_dir)
    //         .await
    //         .unwrap_or(false);

    //     // Check if FFmpeg is available
    //     let ffmpeg_available = tokio::process::Command::new("ffmpeg")
    //         .arg("-version")
    //         .output()
    //         .await
    //         .map(|output| output.status.success())
    //         .unwrap_or(false);

    //     let status = if temp_dir_exists && ffmpeg_available {
    //         "healthy"
    //     } else {
    //         "degraded"
    //     };

    //     Ok(HttpResponse::Ok().json(json!({
    //         "status": status,
    //         "temp_dir_accessible": temp_dir_exists,
    //         "ffmpeg_available": ffmpeg_available,
    //         "whisper_loaded": true, // Assuming whisper context is loaded if we reach here
    //         "timestamp": chrono::Utc::now()
    //     })))
    // }

    /// Helper function to check supported audio formats (expanded list)
    fn is_supported_audio_format(filename: &str) -> bool {
        let supported_extensions = [
            "wav", "mp3", "m4a", "flac", "ogg", "aac", "wma", 
            "aiff", "au", "webm", "opus", "3gp", "amr"
        ];
        
        if let Some(extension) = filename.split('.').last() {
            supported_extensions.contains(&extension.to_lowercase().as_str())
        } else {
            false
        }
    }

    /// Helper function to guess content type from filename
    fn guess_content_type(filename: &str) -> String {
        match filename.split('.').last().unwrap_or("").to_lowercase().as_str() {
            "mp3" => "audio/mpeg".to_string(),
            "wav" => "audio/wav".to_string(),
            "m4a" => "audio/mp4".to_string(),
            "flac" => "audio/flac".to_string(),
            "ogg" => "audio/ogg".to_string(),
            "aac" => "audio/aac".to_string(),
            "wma" => "audio/x-ms-wma".to_string(),
            "aiff" => "audio/aiff".to_string(),
            "webm" => "audio/webm".to_string(),
            "opus" => "audio/opus".to_string(),
            _ => "application/octet-stream".to_string(),
        }
    }
}