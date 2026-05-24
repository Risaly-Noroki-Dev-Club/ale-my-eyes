use ale_core::{AleEngine, AleEngineFactory};
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
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

async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let engine = state.engine.lock().await;
    let status = engine.status().await;
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: "0.1.0".to_string(),
        cloud_ready: status.cloud_ready,
    })
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
                Err(error) => transcription_error(StatusCode::BAD_GATEWAY, error.to_string()),
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
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(SynthesisResponse {
                audio_base64: String::new(),
                success: false,
                error: Some(error.to_string()),
            }),
        ),
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
                Err(error) => image_error(StatusCode::BAD_GATEWAY, error.to_string()),
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
async fn main() {
    tracing_subscriber::fmt::init();

    let engine = AleEngineFactory::create_default()
        .await
        .expect("failed to initialize AleEngine");
    let state = AppState {
        engine: Arc::new(Mutex::new(engine)),
    };

    let app = Router::new()
        .route("/", get(health_check))
        .route("/health", get(health_check))
        .route("/asr/transcribe", post(transcribe_audio))
        .route("/tts/synthesize", post(synthesize_text))
        .route("/vlm/describe", post(describe_image))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
