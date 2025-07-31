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
use uuid::Uuid;


/// Transcription controller
pub struct TranscriptionController;

impl TranscriptionController {
    /// Upload and transcribe audio file
    pub async fn upload_and_transcribe(
        app_state: web::Data<AppState>,
        req: HttpRequest,
        mut payload: Multipart,
    ) -> AppResult<HttpResponse> {
        let user_id = extract_user_id(&req)?;

        // Process multipart form data
        let mut file_upload: Option<FileUpload> = None;

        while let Some(mut field) = payload
            .try_next()
            .await
            .map_err(|_| AppError::BadRequest("Failed to read audio file".to_string()))?
        {
            let content_disposition = field.content_disposition();

            if let Some(name) = content_disposition.get_name() {
                if name == "audio_file" {
                    // Get filename
                    let filename = content_disposition
                        .get_filename()
                        .ok_or_else(|| AppError::BadRequest("Filename is required".to_string()))?
                        .to_string();

                    // Validate file format
                    if !file::is_supported_audio_format(&filename) {
                        return Err(AppError::ValidationError(
                            "Unsupported audio format. Supported formats: wav, mp3, m4a, flac, ogg"
                                .to_string(),
                        ));
                    }

                    // Read file data
                    let mut file_data = Vec::new();
                    while let Some(chunk) = field.try_next().await.map_err(|_| {
                        AppError::BadRequest("Failed to read audio file".to_string())
                    })? {
                        file_data.extend_from_slice(&chunk);
                    }

                    // Validate file size
                    file::validate_file_size(file_data.len(), app_state.config.max_file_size)?;

                    // Get content type
                    let content_type = field
                        .content_type()
                        .map(|ct| ct.to_string())
                        .unwrap_or_else(|| "application/octet-stream".to_string());

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

        log::info!(
            "Processing transcription for file: {} (size: {} bytes)",
            file_upload.filename,
            file_upload.size
        );

        log::info!("Starting transcription for file: {}", file_upload.filename);
        log::info!("Audio file size: {} bytes", file_upload.data.len());
        // Transcribe audio
        let transcription = TranscriptionService::transcribe_audio(
            app_state.whisper_ctx.clone(),
            file_upload.clone(),
            &app_state.config.temp_dir,
        )
        .await?;
        log::info!("Transcription result length: {}", transcription.len());
        log::info!(
            "Transcription preview: {:?}",
            transcription.chars().take(100).collect::<String>()
        );
        // Save transcription to database
        let transcript = TranscriptionService::save_transcription(
            &app_state.db,
            user_id,
            &file_upload.filename,
            &transcription,
            file_upload.size as i64,
            None, // Duration calculation would require audio analysis
        )
        .await?;

        Ok(HttpResponse::Created().json(TranscriptResponse::from(transcript)))
    }

    /// Get user's transcripts with pagination
    pub async fn get_transcripts(
        app_state: web::Data<AppState>,
        req: HttpRequest,
        query: web::Query<PaginationQuery>,
    ) -> AppResult<HttpResponse> {
        let user_id = extract_user_id(&req)?;

        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(10).min(100).max(1); // Max 100, min 1

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

        Ok(HttpResponse::Ok().json(response))
    }

    /// Get specific transcript by ID
    pub async fn get_transcript(
        app_state: web::Data<AppState>,
        req: HttpRequest,
        path: web::Path<Uuid>,
    ) -> AppResult<HttpResponse> {
        let user_id = extract_user_id(&req)?;
        let transcript_id = path.into_inner();

        let transcript =
            TranscriptionService::get_transcript_by_id(&app_state.db, transcript_id, user_id)
                .await?;

        Ok(HttpResponse::Ok().json(TranscriptResponse::from(transcript)))
    }

    /// Delete transcript by ID
    pub async fn delete_transcript(
        app_state: web::Data<AppState>,
        req: HttpRequest,
        path: web::Path<Uuid>,
    ) -> AppResult<HttpResponse> {
        let user_id = extract_user_id(&req)?;
        let transcript_id = path.into_inner();

        TranscriptionService::delete_transcript(&app_state.db, transcript_id, user_id).await?;

        Ok(HttpResponse::Ok().json(json!({
            "message": "Transcript deleted successfully"
        })))
    }
}