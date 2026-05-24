use ale_core::{AleEngine, AleEngineFactory, AleError};
use axum::{
    extract::{Multipart, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    engine: Arc<Mutex<AleEngine>>,
}

#[derive(Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    version: String,
    cloud_ready: bool,
}

#[derive(Serialize, Deserialize)]
struct TranscriptionResponse {
    text: String,
    success: bool,
    error: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SynthesisRequest {
    text: String,
}

#[derive(Serialize, Deserialize)]
struct SynthesisResponse {
    audio_base64: String,
    success: bool,
    error: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ImageDescriptionResponse {
    description: String,
    success: bool,
    error: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct StatusResponse {
    version: String,
    cloud_ready: bool,
    tts_ready: bool,
    config_language: String,
    config_model: String,
    config_api_url: String,
}

#[derive(Serialize, Deserialize)]
struct ModelsResponse {
    models: Vec<ModelInfoResponse>,
}

#[derive(Serialize, Deserialize)]
struct ModelInfoResponse {
    id: String,
    name: String,
    downloaded: bool,
}

fn classify_error(error: &AleError) -> StatusCode {
    match error {
        AleError::ConfigError(_) => StatusCode::BAD_REQUEST,
        AleError::NotInitialized(_) => StatusCode::SERVICE_UNAVAILABLE,
        AleError::CloudApiError(msg) => {
            if msg.contains("401") || msg.contains("Unauthorized") || msg.contains("Invalid") {
                StatusCode::UNAUTHORIZED
            } else if msg.contains("429") || msg.contains("Rate") {
                StatusCode::TOO_MANY_REQUESTS
            } else {
                StatusCode::BAD_GATEWAY
            }
        }
        AleError::AsrError(_) | AleError::VlmError(_) | AleError::TtsError(_) => {
            StatusCode::BAD_GATEWAY
        }
        AleError::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn request_logger(
    request: Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = Instant::now();

    let response = next.run(request).await;

    let duration = start.elapsed();
    tracing::info!(
        "{} {} -> {} ({:.1}ms)",
        method,
        uri,
        response.status().as_u16(),
        duration.as_secs_f64() * 1000.0,
    );

    response
}

async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let engine = state.engine.lock().await;
    let status = engine.status().await;
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: "0.1.0".to_string(),
        cloud_ready: status.cloud_ready,
    })
}

async fn get_status(State(state): State<AppState>) -> Json<StatusResponse> {
    let engine = state.engine.lock().await;
    let status = engine.status().await;
    let config = engine.config();
    Json(StatusResponse {
        version: "0.1.0".to_string(),
        cloud_ready: status.cloud_ready,
        tts_ready: status.tts_ready,
        config_language: config.ui.language.clone(),
        config_model: config.cloud_api.model.clone(),
        config_api_url: config.cloud_api.api_url.clone(),
    })
}

async fn get_models(State(state): State<AppState>) -> Json<ModelsResponse> {
    let engine = state.engine.lock().await;
    let downloaded = engine.downloaded_models().await;
    let available = engine.available_models().await;
    let mut models = Vec::new();
    for model in &available {
        models.push(ModelInfoResponse {
            id: model.id.clone(),
            name: model.name.clone(),
            downloaded: downloaded.iter().any(|d| d.id == model.id),
        });
    }
    Json(ModelsResponse { models })
}

async fn transcribe_audio(
    State(state): State<AppState>,
    multipart: Multipart,
) -> (StatusCode, Json<TranscriptionResponse>) {
    match first_multipart_bytes(multipart).await {
        Ok(audio) => {
            let engine = state.engine.lock().await;
            match engine.transcribe(&audio).await {
                Ok(text) => (
                    StatusCode::OK,
                    Json(TranscriptionResponse {
                        text,
                        success: true,
                        error: None,
                    }),
                ),
                Err(error) => {
                    let status = classify_error(&error);
                    transcription_error(status, error.to_string())
                }
            }
        }
        Err(error) => transcription_error(StatusCode::BAD_REQUEST, error),
    }
}

async fn synthesize_text(
    State(state): State<AppState>,
    Json(payload): Json<SynthesisRequest>,
) -> (StatusCode, Json<SynthesisResponse>) {
    let engine = state.engine.lock().await;
    match engine.synthesize(&payload.text).await {
        Ok(audio) => (
            StatusCode::OK,
            Json(SynthesisResponse {
                audio_base64: base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    audio,
                ),
                success: true,
                error: None,
            }),
        ),
        Err(error) => {
            let status = classify_error(&error);
            (
                status,
                Json(SynthesisResponse {
                    audio_base64: String::new(),
                    success: false,
                    error: Some(error.to_string()),
                }),
            )
        }
    }
}

async fn describe_image(
    State(state): State<AppState>,
    multipart: Multipart,
) -> (StatusCode, Json<ImageDescriptionResponse>) {
    match first_multipart_bytes(multipart).await {
        Ok(image) => {
            let engine = state.engine.lock().await;
            match engine.describe_image(&image).await {
                Ok(description) => (
                    StatusCode::OK,
                    Json(ImageDescriptionResponse {
                        description,
                        success: true,
                        error: None,
                    }),
                ),
                Err(error) => {
                    let status = classify_error(&error);
                    image_error(status, error.to_string())
                }
            }
        }
        Err(error) => image_error(StatusCode::BAD_REQUEST, error),
    }
}

async fn first_multipart_bytes(mut multipart: Multipart) -> Result<Vec<u8>, String> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| format!("Invalid multipart data: {error}"))?
    {
        let bytes = field
            .bytes()
            .await
            .map_err(|error| format!("Failed to read multipart field: {error}"))?;
        if !bytes.is_empty() {
            return Ok(bytes.to_vec());
        }
    }

    Err("No file field found".to_string())
}

fn transcription_error(
    status: StatusCode,
    error: String,
) -> (StatusCode, Json<TranscriptionResponse>) {
    (
        status,
        Json(TranscriptionResponse {
            text: String::new(),
            success: false,
            error: Some(error),
        }),
    )
}

fn image_error(status: StatusCode, error: String) -> (StatusCode, Json<ImageDescriptionResponse>) {
    (
        status,
        Json(ImageDescriptionResponse {
            description: String::new(),
            success: false,
            error: Some(error),
        }),
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let engine = AleEngineFactory::create_default().await?;
    let state = AppState {
        engine: Arc::new(Mutex::new(engine)),
    };

    let app = Router::new()
        .route("/", get(health_check))
        .route("/health", get(health_check))
        .route("/status", get(get_status))
        .route("/models", get(get_models))
        .route("/asr/transcribe", post(transcribe_audio))
        .route("/tts/synthesize", post(synthesize_text))
        .route("/vlm/describe", post(describe_image))
        .layer(middleware::from_fn(request_logger))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
