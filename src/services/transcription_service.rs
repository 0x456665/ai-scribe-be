use crate::errors::{AppError, AppResult};
use crate::models::{FileUpload, Transcript};
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

/// Transcription service for handling audio transcription
pub struct TranscriptionService;

impl TranscriptionService {
    /// Transcribe audio file using Whisper with automatic format conversion
    pub async fn transcribe_audio(
        whisper_ctx: Arc<WhisperContext>,
        file_upload: FileUpload,
        temp_dir: &str,
    ) -> AppResult<String> {
        // Save uploaded file to temporary location
        let temp_file_path = format!("{}/{}", temp_dir, file_upload.filename);
        tokio::fs::write(&temp_file_path, &file_upload.data).await?;

        // Convert audio to WAV format suitable for Whisper
        let wav_file_path = format!("{}/{}.wav", temp_dir, Uuid::new_v4());
        Self::convert_to_wav(&temp_file_path, &wav_file_path).await?;

        // Load audio data from the converted WAV file
        let audio_data = Self::load_wav_audio_samples(&wav_file_path).await?;

        // Set up Whisper parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(4);
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Perform transcription
        log::info!("Starting transcription for file: {}", file_upload.filename);
        log::info!("Audio data length: {} samples", audio_data.len());

        let whisper_ctx_clone = whisper_ctx.clone();
        let transcription = tokio::task::spawn_blocking(move || -> AppResult<String> {
            // Create state once and reuse it
            let mut state = whisper_ctx_clone.create_state().map_err(|e| {
                AppError::WhisperError(format!("Failed to create Whisper state: {}", e))
            })?;

            // Run transcription
            state.full(params, &audio_data).map_err(|e| {
                AppError::WhisperError(format!("Whisper transcription failed: {}", e))
            })?;

            // Get number of segments from the SAME state
            let num_segments = state
                .full_n_segments()
                .map_err(|e| AppError::WhisperError(format!("Failed to get segments: {}", e)))?;

            log::info!("Transcription found {} segments", num_segments);

            // Extract transcription text from the SAME state
            let mut transcription = String::new();
            for i in 0..num_segments {
                let segment_text = state.full_get_segment_text(i).map_err(|e| {
                    AppError::WhisperError(format!("Failed to get segment text: {}", e))
                })?;

                log::debug!("Segment {}: '{}'", i, segment_text);
                transcription.push_str(&segment_text);
                if i < num_segments - 1 {
                    transcription.push(' ');
                }
            }

            Ok(transcription.trim().to_string())
        })
        .await
        .map_err(|e| AppError::WhisperError(format!("Transcription task failed: {}", e)))??;

        // Clean up temporary files
        tokio::fs::remove_file(&temp_file_path).await.ok();
        tokio::fs::remove_file(&wav_file_path).await.ok();

        log::info!(
            "Transcription completed for file: {} - Length: {} characters",
            file_upload.filename,
            transcription.len()
        );

        if transcription.is_empty() {
            log::warn!(
                "Empty transcription result for file: {}",
                file_upload.filename
            );
        }

        Ok(transcription)
    }

    /// Convert audio file to WAV format using FFmpeg
    async fn convert_to_wav(input_path: &str, output_path: &str) -> AppResult<()> {
        let output = tokio::process::Command::new("ffmpeg")
            .args([
                "-i", input_path,        // Input file
                "-ar", "16000",          // Sample rate 16kHz (whisper requirement)
                "-ac", "1",              // Mono channel
                "-c:a", "pcm_s16le",     // 16-bit PCM encoding
                "-y",                    // Overwrite output file
                output_path
            ])
            .output()
            .await
            .map_err(|e| AppError::FileError(format!("Failed to run FFmpeg: {}", e)))?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::FileError(format!("FFmpeg conversion failed: {}", error_msg)));
        }

        log::info!("Successfully converted {} to {}", input_path, output_path);
        Ok(())
    }

    /// Load audio samples from a WAV file (optimized for Whisper)
    async fn load_wav_audio_samples(wav_path: &str) -> AppResult<Vec<f32>> {
        let audio_bytes = tokio::fs::read(wav_path).await
            .map_err(|e| AppError::FileError(format!("Failed to read WAV file: {}", e)))?;

        // Skip WAV header (44 bytes for standard WAV)
        if audio_bytes.len() < 44 {
            return Err(AppError::FileError("Invalid WAV file - too small".to_string()));
        }

        let pcm_data = &audio_bytes[44..];
        let mut samples = Vec::new();

        // Convert 16-bit PCM to f32 samples
        for chunk in pcm_data.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0;
            samples.push(sample);
        }

        if samples.is_empty() {
            return Err(AppError::FileError(
                "No audio data found in WAV file".to_string(),
            ));
        }

        log::info!("Loaded {} audio samples from WAV file", samples.len());
        Ok(samples)
    }

    // /// Alternative method: Convert and transcribe in one step (for direct file paths)
    // pub async fn convert_and_transcribe_file(
    //     whisper_ctx: Arc<WhisperContext>,
    //     input_file_path: &str,
    //     temp_dir: &str,
    // ) -> AppResult<String> {
    //     // Create temporary WAV file path
    //     let wav_file_path = format!("{}/{}.wav", temp_dir, Uuid::new_v4());
        
    //     // Convert to WAV
    //     Self::convert_to_wav(input_file_path, &wav_file_path).await?;

    //     // Load audio data
    //     let audio_data = Self::load_wav_audio_samples(&wav_file_path).await?;

    //     // Set up Whisper parameters
    //     let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    //     params.set_n_threads(4);
    //     params.set_language(Some("en"));
    //     params.set_translate(false);
    //     params.set_print_progress(false);
    //     params.set_print_realtime(false);
    //     params.set_print_timestamps(false);

    //     // Perform transcription
    //     let whisper_ctx_clone = whisper_ctx.clone();
    //     let transcription = tokio::task::spawn_blocking(move || -> AppResult<String> {
    //         let mut state = whisper_ctx_clone.create_state().map_err(|e| {
    //             AppError::WhisperError(format!("Failed to create Whisper state: {}", e))
    //         })?;

    //         state.full(params, &audio_data).map_err(|e| {
    //             AppError::WhisperError(format!("Whisper transcription failed: {}", e))
    //         })?;

    //         let num_segments = state
    //             .full_n_segments()
    //             .map_err(|e| AppError::WhisperError(format!("Failed to get segments: {}", e)))?;

    //         let mut transcription = String::new();
    //         for i in 0..num_segments {
    //             let segment_text = state.full_get_segment_text(i).map_err(|e| {
    //                 AppError::WhisperError(format!("Failed to get segment text: {}", e))
    //             })?;
    //             transcription.push_str(&segment_text);
    //             if i < num_segments - 1 {
    //                 transcription.push(' ');
    //             }
    //         }

    //         Ok(transcription.trim().to_string())
    //     })
    //     .await
    //     .map_err(|e| AppError::WhisperError(format!("Transcription task failed: {}", e)))??;

    //     // Clean up temporary WAV file
    //     tokio::fs::remove_file(&wav_file_path).await.ok();

    //     Ok(transcription)
    // }

    /// Save transcription result to database
    pub async fn save_transcription(
        pool: &PgPool,
        user_id: Uuid,
        filename: &str,
        transcription: &str,
        file_size: i64,
        duration_seconds: Option<f64>,
    ) -> AppResult<Transcript> {
        let transcript_id = Uuid::new_v4();
        let now = Utc::now();

        let transcript = sqlx::query_as::<_, Transcript>(
            r#"
            INSERT INTO transcripts (id, user_id, filename, transcription, file_size, duration_seconds, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#
        )
        .bind(transcript_id)
        .bind(user_id)
        .bind(filename)
        .bind(transcription)
        .bind(file_size)
        .bind(duration_seconds)
        .bind(now)
        .fetch_one(pool)
        .await?;

        log::info!("Transcription saved to database: {}", transcript_id);
        Ok(transcript)
    }

    /// Get user's transcripts with pagination
    pub async fn get_user_transcripts(
        pool: &PgPool,
        user_id: Uuid,
        page: i64,
        limit: i64,
    ) -> AppResult<(Vec<Transcript>, i64)> {
        let offset = (page - 1) * limit;

        // Get transcripts
        let transcripts = sqlx::query_as::<_, Transcript>(
            r#"
            SELECT * FROM transcripts 
            WHERE user_id = $1 
            ORDER BY created_at DESC 
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        // Get total count
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transcripts WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

        Ok((transcripts, total.0))
    }

    /// Get specific transcript by ID for a user
    pub async fn get_transcript_by_id(
        pool: &PgPool,
        transcript_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Transcript> {
        let transcript = sqlx::query_as::<_, Transcript>(
            "SELECT * FROM transcripts WHERE id = $1 AND user_id = $2",
        )
        .bind(transcript_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Transcript not found".to_string()))?;

        Ok(transcript)
    }

    /// Delete transcript by ID for a user
    pub async fn delete_transcript(
        pool: &PgPool,
        transcript_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<()> {
        let result = sqlx::query("DELETE FROM transcripts WHERE id = $1 AND user_id = $2")
            .bind(transcript_id)
            .bind(user_id)
            .execute(pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Transcript not found".to_string()));
        }

        log::info!("Transcript deleted: {}", transcript_id);
        Ok(())
    }

    /// Get audio duration using FFmpeg (helper function)
    pub async fn get_audio_duration(file_path: &str) -> AppResult<f64> {
        let output = tokio::process::Command::new("ffprobe")
            .args([
                "-v", "quiet",
                "-show_entries", "format=duration",
                "-of", "csv=p=0",
                file_path
            ])
            .output()
            .await
            .map_err(|e| AppError::FileError(format!("Failed to run FFprobe: {}", e)))?;

        if !output.status.success() {
            return Err(AppError::FileError("Failed to get audio duration".to_string()));
        }

        let duration_str = String::from_utf8_lossy(&output.stdout);
        let duration = duration_str.trim().parse::<f64>()
            .map_err(|_| AppError::FileError("Invalid duration format".to_string()))?;

        Ok(duration)
    }
}