# Ale, My Eyes! API

The server runs on `0.0.0.0:8000` and exposes OpenAI-backed ASR, TTS, and VLM helpers through Axum routes.

## Health

```http
GET /health
GET /
```

Response:

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "cloud_ready": true
}
```

`cloud_ready` is `true` when the user configuration contains an API key and the engine attached a cloud backend during startup.

## Status

```http
GET /status
```

Response:

```json
{
  "version": "0.1.0",
  "cloud_ready": true,
  "tts_ready": false,
  "config_language": "zh-CN",
  "config_model": "gpt-4o",
  "config_api_url": "https://api.openai.com/v1"
}
```

## Models

```http
GET /models
```

Response:

```json
{
  "models": [
    { "id": "whisper-tiny", "name": "Whisper Tiny", "downloaded": false },
    { "id": "piper-zh_CN", "name": "Piper 中文语音", "downloaded": true }
  ]
}
```

## ASR Transcription

```http
POST /asr/transcribe
Content-Type: multipart/form-data
```

Send one non-empty file field. The server reads the first non-empty multipart field and passes its bytes to `AleEngine::transcribe`.

Example:

```bash
curl -F "file=@input.wav" http://localhost:8000/asr/transcribe
```

Success response:

```json
{
  "text": "recognized text",
  "success": true,
  "error": null
}
```

Error response:

```json
{
  "text": "",
  "success": false,
  "error": "API key is required"
}
```

## TTS Synthesis

```http
POST /tts/synthesize
Content-Type: application/json
```

Request:

```json
{
  "text": "hello"
}
```

Example:

```bash
curl -X POST http://localhost:8000/tts/synthesize \
  -H "Content-Type: application/json" \
  -d '{"text":"hello"}'
```

Success response:

```json
{
  "audio_base64": "<base64 wav bytes>",
  "success": true,
  "error": null
}
```

The current cloud implementation asks OpenAI-compatible TTS for WAV output.

## VLM Image Description

```http
POST /vlm/describe
Content-Type: multipart/form-data
```

Send one non-empty image file field. The server reads the first non-empty multipart field and passes its bytes to `AleEngine::describe_image`.

Example:

```bash
curl -F "file=@screenshot.png" http://localhost:8000/vlm/describe
```

Success response:

```json
{
  "description": "image description",
  "success": true,
  "error": null
}
```

## Configuration

The server uses the same default configuration path as `ale-core`: the user config directory under `ale-my-eyes/config.json`.

Required cloud settings:

```json
{
  "cloud_api": {
    "provider": "openai",
    "api_key": "sk-...",
    "api_url": "https://api.openai.com/v1",
    "model": "gpt-4o",
    "max_tokens": 1024,
    "timeout": 30
  }
}
```

Any OpenAI-compatible API server is supported by changing `api_url`. For example:

```json
{
  "cloud_api": {
    "api_key": "your-key",
    "api_url": "https://your-server.example.com/v1",
    "model": "your-model"
  }
}
```

This works with self-hosted proxies, OpenRouter, Azure OpenAI (with compatible endpoint), and other OpenAI-compatible providers.

Do not commit real API keys.
