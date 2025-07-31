# AI Scribe Backend

A full-featured Rust backend for AI-powered audio transcription using Actix Web, PostgreSQL, JWT authentication, and OpenAI's Whisper model.

## 🚀 Features

- **User Authentication**: Register, login, and JWT-based authentication
- **Audio Transcription**: Upload audio files and get AI-powered transcriptions
- **Secure API**: JWT access/refresh tokens with middleware protection
- **Database Integration**: PostgreSQL with SQLx for async operations
- **File Upload**: Support for multiple audio formats (WAV, MP3, M4A, FLAC, OGG)
- **Modular Architecture**: Clean separation of concerns with controllers, services, and models

## 🛠️ Tech Stack

- **Framework**: Actix Web 4.4
- **Database**: PostgreSQL with SQLx
- **Authentication**: JWT with Argon2 password hashing
- **AI Model**: Whisper via whisper-rs
- **Async Runtime**: Tokio

## 📋 Prerequisites

Before running the application, ensure you have:

1. **Rust** (latest stable version)
2. **PostgreSQL** (version 12+)
3. **Whisper Model**: Download a Whisper model file (e.g., `ggml-base.en.bin`)

### Download Whisper Model

```bash
# Create models directory
mkdir -p models

# Download base English model (recommended for development)
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin -O models/ggml-base.en.bin
```

## 🔧 Setup

1. **Clone and setup the project**:

```bash
git clone <your-repo>
cd ai-scribe-backend
```

2. **Install dependencies**:

```bash
cargo build
```

3. **Setup PostgreSQL database**:

```sql
-- Connect to PostgreSQL and create database
CREATE DATABASE ai_scribe_db;
CREATE USER ai_scribe_user WITH PASSWORD 'your_password';
GRANT ALL PRIVILEGES ON DATABASE ai_scribe_db TO ai_scribe_user;
```

4. **Configure environment variables**:

```bash
cp .env.example .env
# Edit .env with your actual values
```

5. **Run database migrations**:

```bash
# The application will automatically run migrations on startup
# Or you can use sqlx-cli:
cargo install sqlx-cli
sqlx migrate run
```

6. **Run the application**:

```bash
cargo run
```

The server will start at `http://127.0.0.1:8080`

## 📚 API Documentation

### Authentication Endpoints

#### Register User

```bash
POST /api/v1/auth/register
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "securepassword123"
}
```

#### Login User

```bash
POST /api/v1/auth/login
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "securepassword123"
}
```

#### Refresh Token

```bash
POST /api/v1/auth/refresh
Content-Type: application/json

{
  "refresh_token": "your_refresh_token_here"
}
```

#### Get User Profile

```bash
GET /api/v1/me
Authorization: Bearer your_access_token_here
```

### Transcription Endpoints

#### Upload and Transcribe Audio

```bash
POST /api/v1/transcripts
Authorization: Bearer your_access_token_here
Content-Type: multipart/form-data

# Form data:
# audio_file: [your audio file]
```

#### Get User's Transcripts

```bash
GET /api/v1/transcripts?page=1&limit=10
Authorization: Bearer your_access_token_here
```

#### Get Specific Transcript

```bash
GET /api/v1/transcripts/{transcript_id}
Authorization: Bearer your_access_token_here
```

#### Delete Transcript

```bash
DELETE /api/v1/transcripts/{transcript_id}
Authorization: Bearer your_access_token_here
```

### Health Check

```bash
GET /health
```

## 🧪 Testing with cURL

### 1. Register a new user

```bash
curl -X POST http://localhost:8080/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@example.com",
    "password": "testpassword123"
  }'
```

### 2. Login and get tokens

```bash
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@example.com",
    "password": "testpassword123"
  }'
```

### 3. Upload audio for transcription

```bash
# Replace YOUR_ACCESS_TOKEN with the token from login response
curl -X POST http://localhost:8080/api/v1/transcripts \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -F "audio_file=@/path/to/your/audio.wav"
```

### 4. Get your transcripts

```bash
curl -X GET "http://localhost:8080/api/v1/transcripts?page=1&limit=5" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

### 5. Get user profile

```bash
curl -X GET http://localhost:8080/api/v1/me \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

## 🏗️ Project Structure

```
ai-scribe-backend/
├── src/
│   ├── main.rs              # Application entry point
│   ├── config/
│   │   └── mod.rs           # Configuration management
│   ├── controllers/
│   │   └── mod.rs           # Request handlers
│   ├── errors/
│   │   └── mod.rs           # Error types and handling
│   ├── middleware/
│   │   └── mod.rs           # JWT authentication middleware
│   ├── models/
│   │   └── mod.rs           # Database models and DTOs
│   ├── routes/
│   │   └── mod.rs           # Route configuration
│   ├── services/
│   │   └── mod.rs           # Business logic
│   └── utils/
│       └── mod.rs           # Utility functions
├── migrations/
│   └── 001_initial.sql      # Database schema
├── Cargo.toml               # Dependencies
├── .env.example             # Environment variables template
└── README.md
```

## 🔐 Security Features

- **Password Hashing**: Uses Argon2 for secure password storage
- **JWT Tokens**:
  - Access tokens (15 minutes expiration)
  - Refresh tokens (7 days expiration)
- **Input Validation**: Request validation using the `validator` crate
- **File Size Limits**: Configurable maximum file size for uploads
- **CORS**: Configured for cross-origin requests

## 🎵 Supported Audio Formats

- WAV (`.wav`)
- MP3 (`.mp3`)
- M4A (`.m4a`)
- FLAC (`.flac`)
- OGG (`.ogg`)

_Note: The current implementation includes a simplified audio conversion. For production use, consider integrating FFmpeg for robust audio format support._

## ⚙️ Configuration

Key environment variables:

| Variable                   | Description                       | Default           |
| -------------------------- | --------------------------------- | ----------------- |
| `DATABASE_URL`             | PostgreSQL connection string      | Required          |
| `JWT_SECRET`               | Secret key for JWT signing        | Required          |
| `WHISPER_MODEL_PATH`       | Path to Whisper model file        | Required          |
| `HOST`                     | Server host address               | `127.0.0.1`       |
| `PORT`                     | Server port                       | `8080`            |
| `ACCESS_TOKEN_EXPIRES_IN`  | Access token expiration (minutes) | `15`              |
| `REFRESH_TOKEN_EXPIRES_IN` | Refresh token expiration (days)   | `7`               |
| `MAX_FILE_SIZE`            | Maximum upload size (bytes)       | `52428800` (50MB) |
| `TEMP_DIR`                 | Temporary file storage directory  | `/tmp`            |

## 🚀 Production Deployment

### 1. Build for production

```bash
cargo build --release
```

### 2. Set production environment variables

```bash
export RUST_LOG=warn
export DATABASE_URL=postgresql://user:pass@prod-db:5432/ai_scribe_db
# ... other production values
```

### 3. Run with systemd (example service file)

```ini
# /etc/systemd/system/ai-scribe.service
[Unit]
Description=AI Scribe Backend
After=network.target

[Service]
Type=simple
User=ai-scribe
WorkingDirectory=/opt/ai-scribe
ExecStart=/opt/ai-scribe/target/release/ai-scribe-backend
EnvironmentFile=/opt/ai-scribe/.env
Restart=always

[Install]
WantedBy=multi-user.target
```

## 🔍 Logging

The application uses `env_logger` for logging. Set the `RUST_LOG` environment variable to control log levels:

```bash
# Development
export RUST_LOG=debug

# Production
export RUST_LOG=warn
```

## 🧪 Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the LICENSE file for details.

## 🐛 Troubleshooting

### Common Issues

1. **"Whisper model not found"**

    - Ensure you've downloaded a Whisper model file
    - Check that `WHISPER_MODEL_PATH` points to the correct file

2. **"Database connection failed"**

    - Verify PostgreSQL is running
    - Check `DATABASE_URL` format: `postgresql://user:password@host:port/database`
    - Ensure database exists and user has proper permissions

3. **"JWT token invalid"**

    - Check that `JWT_SECRET` is set and consistent
    - Verify token hasn't expired
    - Ensure proper Authorization header format: `Bearer <token>`

4. **"File upload failed"**
    - Check file size doesn't exceed `MAX_FILE_SIZE`
    - Verify audio format is supported
    - Ensure `TEMP_DIR` exists and is writable

### Performance Tips

1. **Database Optimization**

    - Use connection pooling (already configured with SQLx)
    - Consider adding database indexes for frequent queries
    - Monitor query performance

2. **Whisper Model Selection**

    - `tiny`: Fastest, lowest accuracy
    - `base`: Good balance (recommended for development)
    - `small/medium/large`: Higher accuracy, slower processing

3. **File Processing**
    - Consider async file processing for large files
    - Implement file cleanup for temporary files
    - Use streaming for large file uploads

## 📊 Monitoring

Consider adding monitoring for production:

- **Metrics**: Prometheus + Grafana
- **Logging**: ELK stack or similar
- **Health Checks**: Database connectivity, disk space, memory usage
- **Performance**: Request latency, transcription processing time
