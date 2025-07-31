use crate::errors::{AppError, AppResult};
use crate::models::{FileUpload, Transcript};
use chrono::Utc;
use sqlx::PgPool;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;
use tokio::fs;
use uuid::Uuid;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperState};

/// Transcription service for handling audio transcription
pub struct TranscriptionService;

/// Audio processing configuration
#[derive(Clone, Debug)]
pub struct AudioConfig {
    pub target_sample_rate: u32,
    pub target_channels: u16,
    pub max_duration_seconds: u32,
    pub min_duration_seconds: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            target_sample_rate: 16000,  // Whisper expects 16kHz
            target_channels: 1,         // Mono
            max_duration_seconds: 3600, // 1 hour max
            min_duration_seconds: 1,    // 1 second min
        }
    }
}

impl TranscriptionService {
    /// Transcribe audio file using Whisper with comprehensive error handling
    pub async fn transcribe_audio(
        whisper_ctx: Arc<WhisperContext>,
        file_upload: FileUpload,
        temp_dir: &str,
    ) -> AppResult<String> {
        let start_time = Instant::now();
        log::info!(
            "Starting transcription for file: {} ({} bytes)",
            file_upload.filename,
            file_upload.size
        );

        // Validate temp directory exists
        fs::create_dir_all(temp_dir)
            .await
            .map_err(|e| AppError::FileError(format!("Failed to create temp directory: {}", e)))?;

        // Generate unique temp file path
        let temp_file_id = Uuid::new_v4();
        let temp_file_path = format!("{}/upload_{}", temp_dir, temp_file_id);

        // Write uploaded file
        fs::write(&temp_file_path, &file_upload.data)
            .await
            .map_err(|e| AppError::FileError(format!("Failed to write temp file: {}", e)))?;

        let result = Self::transcribe_audio_internal(
            whisper_ctx,
            &file_upload.filename,
            &temp_file_path,
            temp_dir,
        )
        .await;

        // Always cleanup temp file
        if let Err(e) = fs::remove_file(&temp_file_path).await {
            log::warn!("Failed to cleanup temp file {}: {}", temp_file_path, e);
        }

        let transcription = result?;
        let duration = start_time.elapsed();

        log::info!(
            "Transcription completed for {} in {:.2}s: {} characters",
            file_upload.filename,
            duration.as_secs_f64(),
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

    /// Internal transcription logic with proper cleanup
    async fn transcribe_audio_internal(
        whisper_ctx: Arc<WhisperContext>,
        original_filename: &str,
        temp_file_path: &str,
        temp_dir: &str,
    ) -> AppResult<String> {
        // Convert audio to Whisper-compatible format
        let audio_data = Self::prepare_audio_for_whisper(temp_file_path, temp_dir).await?;

        if audio_data.is_empty() {
            return Err(AppError::FileError(
                "No audio data extracted from file".to_string(),
            ));
        }

        log::info!(
            "Audio preprocessing completed: {} samples ({:.2}s duration)",
            audio_data.len(),
            audio_data.len() as f64 / 16000.0
        );

        // Validate audio duration
        let duration_seconds = audio_data.len() as f64 / 16000.0;
        let config = AudioConfig::default();

        if duration_seconds < config.min_duration_seconds as f64 {
            return Err(AppError::ValidationError(format!(
                "Audio too short: {:.2}s (minimum: {}s)",
                duration_seconds, config.min_duration_seconds
            )));
        }

        if duration_seconds > config.max_duration_seconds as f64 {
            return Err(AppError::ValidationError(format!(
                "Audio too long: {:.2}s (maximum: {}s)",
                duration_seconds, config.max_duration_seconds
            )));
        }

        // Set up Whisper parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        Self::configure_whisper_params(&mut params);

        // Perform transcription
        let whisper_ctx_clone = whisper_ctx.clone();
        let filename_clone = original_filename.to_string();

        let transcription = tokio::task::spawn_blocking(move || -> AppResult<String> {
            Self::run_whisper_transcription(whisper_ctx_clone, params, audio_data, &filename_clone)
        })
        .await
        .map_err(|e| AppError::WhisperError(format!("Transcription task panicked: {}", e)))??;

        Ok(transcription)
    }

    /// Configure Whisper parameters for optimal transcription
    fn configure_whisper_params(params: &mut FullParams) {
        params.set_n_threads(num_cpus::get().min(8).try_into().unwrap()); // Use available cores, max 8
        params.set_translate(false);
        params.set_language(Some("auto")); // Auto-detect language
        params.set_print_special(false);
        params.set_print_progress(false); // Disable to avoid log spam
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Advanced parameters for better quality
        params.set_suppress_blank(true);
        params.set_suppress_non_speech_tokens(true);
        params.set_temperature(0.0); // Use greedy decoding for consistency
        params.set_max_initial_ts(1.0);
        params.set_length_penalty(-1.0);

        // Audio processing
        params.set_speed_up(false); // Don't speed up audio
        params.set_audio_ctx(0); // Use default audio context
    }

    /// Run Whisper transcription with proper error handling
    fn run_whisper_transcription(
        whisper_ctx: Arc<WhisperContext>,
        params: FullParams,
        audio_data: Vec<f32>,
        filename: &str,
    ) -> AppResult<String> {
        log::debug!("Starting Whisper inference for: {}", filename);

        // Create Whisper state
        let mut state = whisper_ctx.create_state().map_err(|e| {
            AppError::WhisperError(format!("Failed to create Whisper state: {}", e))
        })?;

        // Run transcription
        state
            .full(params, &audio_data)
            .map_err(|e| AppError::WhisperError(format!("Whisper transcription failed: {}", e)))?;

        // Extract segments
        Self::extract_transcription_text(&mut state)
    }

    /// Extract transcription text from Whisper state
    fn extract_transcription_text(state: &mut WhisperState) -> AppResult<String> {
        let num_segments = state
            .full_n_segments()
            .map_err(|e| AppError::WhisperError(format!("Failed to get segment count: {}", e)))?;

        log::debug!("Processing {} transcription segments", num_segments);

        if num_segments == 0 {
            log::warn!("No transcription segments found");
            return Ok(String::new());
        }

        let mut transcription_parts = Vec::new();

        for i in 0..num_segments {
            let segment_text = state.full_get_segment_text(i).map_err(|e| {
                AppError::WhisperError(format!("Failed to get segment {} text: {}", i, e))
            })?;

            let trimmed_text = segment_text.trim();
            if !trimmed_text.is_empty() {
                log::debug!("Segment {}: '{}'", i, trimmed_text);
                transcription_parts.push(trimmed_text.to_string());
            }
        }

        let final_transcription = transcription_parts.join(" ");

        log::debug!(
            "Extracted {} segments, final length: {} characters",
            transcription_parts.len(),
            final_transcription.len()
        );

        Ok(final_transcription)
    }

    /// Prepare audio data for Whisper using FFmpeg for robust conversion
    async fn prepare_audio_for_whisper(input_path: &str, temp_dir: &str) -> AppResult<Vec<f32>> {
        // Check if FFmpeg is available
        Self::check_ffmpeg_availability().await?;

        let config = AudioConfig::default();
        let output_id = Uuid::new_v4();
        let output_path = format!("{}/converted_{}.wav", temp_dir, output_id);

        log::debug!("Converting audio: {} -> {}", input_path, output_path);

        // Use FFmpeg to convert to Whisper-compatible format
        let ffmpeg_result = Command::new("ffmpeg")
            .args(&[
                "-i",
                input_path,
                "-ar",
                &config.target_sample_rate.to_string(),
                "-ac",
                &config.target_channels.to_string(),
                "-c:a",
                "pcm_s16le", // 16-bit PCM
                "-f",
                "wav",
                "-y", // Overwrite output
                "-loglevel",
                "error", // Reduce FFmpeg output
                &output_path,
            ])
            .output()
            .map_err(|e| AppError::FileError(format!("FFmpeg execution failed: {}", e)))?;

        if !ffmpeg_result.status.success() {
            let error_msg = String::from_utf8_lossy(&ffmpeg_result.stderr);
            return Err(AppError::FileError(format!(
                "Audio conversion failed: {}",
                error_msg
            )));
        }

        // Read converted audio data
        let audio_data = Self::read_wav_pcm_data(&output_path).await;

        // Cleanup converted file
        if let Err(e) = fs::remove_file(&output_path).await {
            log::warn!("Failed to cleanup converted file {}: {}", output_path, e);
        }

        audio_data
    }

    /// Check if FFmpeg is available and working
    async fn check_ffmpeg_availability() -> AppResult<()> {
        let output = Command::new("ffmpeg")
            .args(&["-version"])
            .output()
            .map_err(|e| {
                AppError::FileError(format!("FFmpeg not found. Please install FFmpeg: {}", e))
            })?;

        if !output.status.success() {
            return Err(AppError::FileError(
                "FFmpeg is not working properly".to_string(),
            ));
        }

        Ok(())
    }

    /// Read WAV file and extract PCM data with proper header parsing
    async fn read_wav_pcm_data(file_path: &str) -> AppResult<Vec<f32>> {
        let wav_bytes = fs::read(file_path).await.map_err(|e| {
            AppError::FileError(format!("Failed to read converted WAV file: {}", e))
        })?;

        if wav_bytes.len() < 44 {
            return Err(AppError::FileError(
                "WAV file too small - corrupt or invalid".to_string(),
            ));
        }

        // Verify WAV header
        if &wav_bytes[0..4] != b"RIFF" || &wav_bytes[8..12] != b"WAVE" {
            return Err(AppError::FileError("Invalid WAV file format".to_string()));
        }

        // Parse WAV chunks to find data chunk
        let (data_start, data_size) = Self::find_wav_data_chunk(&wav_bytes)?;

        if data_size == 0 {
            return Err(AppError::FileError(
                "No audio data found in WAV file".to_string(),
            ));
        }

        // Extract and convert PCM data
        let pcm_data = &wav_bytes[data_start..data_start + data_size];
        Self::convert_pcm_to_f32(pcm_data)
    }

    /// Find the data chunk in a WAV file
    fn find_wav_data_chunk(wav_bytes: &[u8]) -> AppResult<(usize, usize)> {
        let mut offset = 12; // Skip RIFF header

        while offset + 8 < wav_bytes.len() {
            let chunk_id = &wav_bytes[offset..offset + 4];
            let chunk_size = u32::from_le_bytes([
                wav_bytes[offset + 4],
                wav_bytes[offset + 5],
                wav_bytes[offset + 6],
                wav_bytes[offset + 7],
            ]) as usize;

            if chunk_id == b"data" {
                return Ok((offset + 8, chunk_size));
            }

            offset += 8 + chunk_size;
            // Handle padding byte for odd-sized chunks
            if chunk_size % 2 == 1 {
                offset += 1;
            }
        }

        Err(AppError::FileError(
            "No data chunk found in WAV file".to_string(),
        ))
    }

    /// Convert 16-bit PCM data to f32 samples
    fn convert_pcm_to_f32(pcm_data: &[u8]) -> AppResult<Vec<f32>> {
        if pcm_data.len() % 2 != 0 {
            return Err(AppError::FileError(
                "Invalid PCM data length (not aligned to 16-bit samples)".to_string(),
            ));
        }

        let mut samples = Vec::with_capacity(pcm_data.len() / 2);

        for chunk in pcm_data.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            // Normalize to [-1.0, 1.0] range
            samples.push(sample as f32 / 32768.0);
        }

        if samples.is_empty() {
            return Err(AppError::FileError(
                "No valid audio samples found".to_string(),
            ));
        }

        log::debug!("Converted {} PCM samples to f32", samples.len());
        Ok(samples)
    }

    /// Save transcription result to database with enhanced error handling
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

        // Validate transcription length
        if transcription.len() > 1_000_000 {
            // 1MB text limit
            return Err(AppError::ValidationError(
                "Transcription too long (>1MB)".to_string(),
            ));
        }

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
        .await
        .map_err(|e| {
            AppError::DatabaseError(e)
        })?;

        log::info!(
            "Transcription saved to database: {} (user: {}, size: {} chars)",
            transcript_id,
            user_id,
            transcription.len()
        );

        Ok(transcript)
    }

    /// Get user's transcripts with pagination and enhanced error handling
    pub async fn get_user_transcripts(
        pool: &PgPool,
        user_id: Uuid,
        page: i64,
        limit: i64,
    ) -> AppResult<(Vec<Transcript>, i64)> {
        // Validate pagination parameters
        if page < 1 {
            return Err(AppError::ValidationError("Page must be >= 1".to_string()));
        }
        if limit < 1 || limit > 100 {
            return Err(AppError::ValidationError(
                "Limit must be between 1 and 100".to_string(),
            ));
        }

        let offset = (page - 1) * limit;

        // Get transcripts and total count in parallel
        let (transcripts_result, total_result) = tokio::try_join!(
            sqlx::query_as::<_, Transcript>(
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
            .fetch_all(pool),
            sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM transcripts WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(pool)
        )?;

        Ok((transcripts_result, total_result.0))
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
        .ok_or_else(|| {
            log::warn!(
                "Transcript not found: {} for user: {}",
                transcript_id,
                user_id
            );
            AppError::NotFound("Transcript not found".to_string())
        })?;

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
            log::warn!(
                "Attempted to delete non-existent transcript: {} for user: {}",
                transcript_id,
                user_id
            );
            return Err(AppError::NotFound("Transcript not found".to_string()));
        }

        log::info!("Transcript deleted: {} (user: {})", transcript_id, user_id);
        Ok(())
    }

    // /// Health check for transcription service
    // pub async fn health_check(whisper_ctx: Arc<WhisperContext>) -> AppResult<()> {
    //     // Check if FFmpeg is available
    //     Self::check_ffmpeg_availability().await?;

    //     // Test Whisper context creation
    //     whisper_ctx
    //         .create_state()
    //         .map_err(|e| AppError::WhisperError(format!("Whisper context unhealthy: {}", e)))?;

    //     Ok(())
    // }
}