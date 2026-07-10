use crate::audit;
use crate::conversation::automation_tools;
use crate::platform::{self, PlatformService};
use crate::remote_crypto;
use ale_core::actions::{parse_action_plan_arguments, ActionPlan};
use ale_core::remote::{
    ClientHello, CommandInput, CommandPreview, ConfirmExecution, ExecutionState, ExecutionStatus,
    PairingInfo, RemoteError, RemoteMessage, ServerHello, DEFAULT_REMOTE_PORT,
    REMOTE_PROTOCOL_VERSION,
};
use ale_core::AleEngine;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use local_ip_address::list_afinet_netifas;
use qrcode::render::unicode;
use qrcode::QrCode;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

pub struct RemoteServerHandle {
    pub pairing: PairingInfo,
    pub qr_text: String,
}

pub async fn start(engine: Arc<Mutex<AleEngine>>) -> Result<RemoteServerHandle, String> {
    let code = remote_crypto::pairing_code();
    let session_id = remote_crypto::session_id();
    let name = remote_crypto::device_name();
    let host = local_addresses()
        .into_iter()
        .next()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let pairing = PairingInfo {
        host,
        port: DEFAULT_REMOTE_PORT,
        session_id,
        code,
        name,
    };
    let qr_text = render_qr(&pairing.uri()).unwrap_or_else(|_| pairing.uri());

    let listener = TcpListener::bind(("0.0.0.0", DEFAULT_REMOTE_PORT))
        .await
        .map_err(|error| error.to_string())?;
    let pending = Arc::new(Mutex::new(HashMap::<String, ActionPlan>::new()));
    let platform: Arc<dyn PlatformService> = Arc::from(platform::create_platform());
    let server_pairing = pairing.clone();

    tokio::spawn(async move {
        advertise_mdns(&server_pairing);
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let engine = engine.clone();
                    let pending = pending.clone();
                    let pairing = server_pairing.clone();
                    let platform = platform.clone();
                    tokio::spawn(async move {
                        if let Err(error) =
                            handle_connection(stream, addr, engine, pending, pairing, platform)
                                .await
                        {
                            tracing::warn!("Remote client disconnected: {}", error);
                        }
                    });
                }
                Err(error) => tracing::warn!("Remote accept failed: {}", error),
            }
        }
    });

    Ok(RemoteServerHandle { pairing, qr_text })
}

async fn handle_connection(
    stream: TcpStream,
    _addr: SocketAddr,
    engine: Arc<Mutex<AleEngine>>,
    pending: Arc<Mutex<HashMap<String, ActionPlan>>>,
    pairing: PairingInfo,
    platform: Arc<dyn PlatformService>,
) -> Result<(), String> {
    let mut socket = tokio_tungstenite::accept_async(stream)
        .await
        .map_err(|error| error.to_string())?;

    let client_handshake = socket
        .next()
        .await
        .ok_or_else(|| "missing handshake".to_string())?
        .map_err(|error| error.to_string())?
        .into_data();
    let (mut secure, server_handshake) =
        remote_crypto::server_handshake_reply(&pairing.code, &client_handshake)?;
    socket
        .send(Message::Binary(server_handshake))
        .await
        .map_err(|error| error.to_string())?;

    send_secure(
        &mut socket,
        &mut secure,
        &RemoteMessage::ServerHello(ServerHello {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            device_name: pairing.name.clone(),
            session_id: pairing.session_id.clone(),
        }),
    )
    .await?;

    while let Some(frame) = socket.next().await {
        let frame = frame.map_err(|error| error.to_string())?;
        if !frame.is_binary() {
            continue;
        }
        let message = secure.decrypt_message(&frame.into_data())?;
        match message {
            RemoteMessage::ClientHello(ClientHello { .. }) => {}
            RemoteMessage::CommandRequest(request) => {
                let request_id = request.request_id.clone();
                match handle_command(
                    engine.clone(),
                    platform.clone(),
                    &request.request_id,
                    &request.input,
                )
                .await
                {
                    Ok((preview, plan)) => {
                        if let Some(plan) = plan {
                            audit::record("created", "remote", &plan, None);
                            pending.lock().await.insert(request_id.clone(), plan);
                        }
                        send_secure(
                            &mut socket,
                            &mut secure,
                            &RemoteMessage::CommandPreview(preview),
                        )
                        .await?;
                    }
                    Err(error) => {
                        send_secure(
                            &mut socket,
                            &mut secure,
                            &RemoteMessage::Error(RemoteError {
                                request_id: Some(request_id),
                                message: error,
                            }),
                        )
                        .await?;
                    }
                }
            }
            RemoteMessage::ConfirmExecution(confirm) => {
                let status = handle_confirm(confirm, pending.clone(), platform.clone()).await;
                send_secure(
                    &mut socket,
                    &mut secure,
                    &RemoteMessage::ExecutionStatus(status),
                )
                .await?;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn handle_command(
    engine: Arc<Mutex<AleEngine>>,
    platform: Arc<dyn PlatformService>,
    request_id: &str,
    input: &CommandInput,
) -> Result<(CommandPreview, Option<ActionPlan>), String> {
    let request_id = request_id.to_string();
    let question = match input {
        CommandInput::Text { text } => text.clone(),
        CommandInput::AudioWav { wav_base64 } => {
            let audio = base64::engine::general_purpose::STANDARD
                .decode(wav_base64)
                .map_err(|error| error.to_string())?;
            let engine = engine.lock().await;
            engine
                .transcribe(&audio)
                .await
                .map_err(|error| error.to_string())?
        }
    };

    let image = platform.capture_image();
    let response = if let Some(image) = image {
        let engine = engine.lock().await;
        engine
            .ask_about_image_with_tools(&image, &question, automation_tools())
            .await
            .map_err(|error| error.to_string())?
    } else {
        let engine = engine.lock().await;
        let response = engine
            .ask_text(&question)
            .await
            .map_err(|error| error.to_string())?;
        return Ok((
            CommandPreview {
                request_id,
                response_text: response.content,
                action_steps: Vec::new(),
                confirmation_text: String::new(),
                requires_confirmation: false,
                has_plan: false,
            },
            None,
        ));
    };

    let mut action_steps = Vec::new();
    let mut plan = None;
    if let Some(calls) = response.tool_calls {
        let executable = calls
            .iter()
            .filter(|call| call.function.name == "execute_action_plan")
            .collect::<Vec<_>>();
        if executable.len() == 1 {
            if let Ok(parsed) = parse_action_plan_arguments(&executable[0].function.arguments) {
                action_steps = parsed.describe_steps();
                plan = Some(parsed);
            }
        }
    }

    let confirmation_text = plan
        .as_ref()
        .map(ActionPlan::speak_text)
        .unwrap_or_default();
    let requires_confirmation = plan
        .as_ref()
        .map(|plan| plan.requires_confirmation)
        .unwrap_or(false);
    let has_plan = plan.is_some();

    Ok((
        CommandPreview {
            request_id,
            response_text: response.content,
            action_steps,
            confirmation_text,
            requires_confirmation,
            has_plan,
        },
        plan,
    ))
}

async fn handle_confirm(
    confirm: ConfirmExecution,
    pending: Arc<Mutex<HashMap<String, ActionPlan>>>,
    platform: Arc<dyn PlatformService>,
) -> ExecutionStatus {
    if !confirm.approved {
        if let Some(plan) = pending.lock().await.remove(&confirm.request_id) {
            audit::record("cancelled", "remote", &plan, None);
        }
        return ExecutionStatus {
            request_id: confirm.request_id,
            state: ExecutionState::Cancelled,
            message: "已取消".to_string(),
            actions_executed: 0,
        };
    }

    let Some(plan) = pending.lock().await.remove(&confirm.request_id) else {
        return ExecutionStatus {
            request_id: confirm.request_id,
            state: ExecutionState::Failed,
            message: "找不到待执行计划".to_string(),
            actions_executed: 0,
        };
    };

    audit::record("approved", "remote", &plan, None);
    match platform.execute_plan(&plan, true) {
        Ok(result) => {
            audit::record("completed", "remote", &plan, None);
            ExecutionStatus {
                request_id: confirm.request_id,
                state: ExecutionState::Completed,
                message: format!("执行完成: {} 步", result.actions_executed),
                actions_executed: result.actions_executed,
            }
        }
        Err(error) => {
            audit::record("failed", "remote", &plan, Some(&error.to_string()));
            ExecutionStatus {
                request_id: confirm.request_id,
                state: ExecutionState::Failed,
                message: error.to_string(),
                actions_executed: 0,
            }
        }
    }
}

async fn send_secure(
    socket: &mut tokio_tungstenite::WebSocketStream<TcpStream>,
    secure: &mut remote_crypto::SecureChannel,
    message: &RemoteMessage,
) -> Result<(), String> {
    let frame = secure.encrypt_message(message)?;
    socket
        .send(Message::Binary(frame))
        .await
        .map_err(|error| error.to_string())
}

fn local_addresses() -> Vec<String> {
    list_afinet_netifas()
        .map(|interfaces| {
            interfaces
                .into_iter()
                .filter_map(|(_, ip)| {
                    if ip.is_ipv4() && !ip.is_loopback() {
                        Some(ip.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn render_qr(uri: &str) -> Result<String, String> {
    let code = QrCode::new(uri.as_bytes()).map_err(|error| error.to_string())?;
    Ok(code.render::<unicode::Dense1x2>().build())
}

fn advertise_mdns(pairing: &PairingInfo) {
    let pairing = pairing.clone();
    std::thread::spawn(move || {
        let Ok(daemon) = mdns_sd::ServiceDaemon::new() else {
            return;
        };
        let properties = [
            ("sid", pairing.session_id.as_str()),
            ("name", pairing.name.as_str()),
        ];
        let Ok(info) = mdns_sd::ServiceInfo::new(
            "_ale-my-eyes._tcp.local.",
            &pairing.name,
            &format!("{}.local.", pairing.name.replace(' ', "-")),
            &pairing.host,
            pairing.port,
            &properties[..],
        ) else {
            return;
        };
        let _ = daemon.register(info);
        loop {
            std::thread::park();
        }
    });
}
