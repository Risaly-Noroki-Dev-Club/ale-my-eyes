#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use crate::remote_crypto;
use ale_core::remote::{
    ClientHello, CommandInput, CommandPreview, CommandRequest, ConfirmExecution, ExecutionStatus,
    RemoteError, RemoteMessage, REMOTE_PROTOCOL_VERSION,
};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use std::time::Duration;

#[derive(Clone)]
pub struct RemoteClient {
    url: String,
    code: String,
}

impl RemoteClient {
    pub fn new(url: String, code: String) -> Self {
        Self { url, code }
    }

    pub async fn test(&self) -> Result<String, String> {
        let (_, server_name) = self.connect().await?;
        Ok(server_name)
    }

    pub async fn send_command(&self, input: CommandInput) -> Result<CommandPreview, String> {
        let request_id = ale_core::remote::new_request_id();
        let (mut socket, mut secure) = self.connect().await?.0;
        let message = RemoteMessage::CommandRequest(CommandRequest {
            request_id: request_id.clone(),
            input,
        });
        send_secure(&mut socket, &mut secure, &message).await?;

        loop {
            match read_secure(&mut socket, &mut secure).await? {
                RemoteMessage::CommandPreview(preview) => return Ok(preview),
                RemoteMessage::Error(RemoteError { message, .. }) => return Err(message),
                _ => {}
            }
        }
    }

    pub async fn confirm(&self, request_id: String, approved: bool) -> Result<ExecutionStatus, String> {
        let (mut socket, mut secure) = self.connect().await?.0;
        send_secure(
            &mut socket,
            &mut secure,
            &RemoteMessage::ConfirmExecution(ConfirmExecution { request_id, approved }),
        )
        .await?;

        loop {
            match read_secure(&mut socket, &mut secure).await? {
                RemoteMessage::ExecutionStatus(status) => return Ok(status),
                RemoteMessage::Error(RemoteError { message, .. }) => return Err(message),
                _ => {}
            }
        }
    }

    async fn connect(
        &self,
    ) -> Result<
        (
            (
                tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
                remote_crypto::SecureChannel,
            ),
            String,
        ),
        String,
    > {
        let (mut socket, _) = tokio_tungstenite::connect_async(&self.url)
            .await
            .map_err(|error| error.to_string())?;
        let (noise, client_handshake) = remote_crypto::client_handshake_message(&self.code)?;
        socket
            .send(Message::Binary(client_handshake))
            .await
            .map_err(|error| error.to_string())?;
        let server_handshake = socket
            .next()
            .await
            .ok_or_else(|| "missing server handshake".to_string())?
            .map_err(|error| error.to_string())?
            .into_data();
        let mut secure = remote_crypto::client_finish_handshake(noise, &server_handshake)?;

        let hello = read_secure(&mut socket, &mut secure).await?;
        let server_name = match hello {
            RemoteMessage::ServerHello(hello) => hello.device_name,
            _ => "Desktop".to_string(),
        };

        send_secure(
            &mut socket,
            &mut secure,
            &RemoteMessage::ClientHello(ClientHello {
                protocol_version: REMOTE_PROTOCOL_VERSION,
                device_name: "Android".to_string(),
            }),
        )
        .await?;

        Ok(((socket, secure), server_name))
    }
}

pub fn discover_first(code: String) -> Option<ale_core::remote::PairingInfo> {
    let daemon = mdns_sd::ServiceDaemon::new().ok()?;
    let receiver = daemon.browse("_ale-my-eyes._tcp.local.").ok()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        let Ok(event) = receiver.recv_timeout(Duration::from_millis(250)) else {
            continue;
        };
        if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
            let host = info.get_addresses().iter().next()?.to_string();
            let name = info
                .get_property_val_str("name")
                .map(str::to_string)
                .unwrap_or_else(|| info.get_fullname().to_string());
            let session_id = info
                .get_property_val_str("sid")
                .map(str::to_string)
                .unwrap_or_else(ale_core::remote::new_request_id);
            return Some(ale_core::remote::PairingInfo {
                host,
                port: info.get_port(),
                session_id,
                code,
                name,
            });
        }
    }
    None
}

async fn send_secure<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    secure: &mut remote_crypto::SecureChannel,
    message: &RemoteMessage,
) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let frame = secure.encrypt_message(message)?;
    socket
        .send(Message::Binary(frame))
        .await
        .map_err(|error| error.to_string())
}

async fn read_secure<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    secure: &mut remote_crypto::SecureChannel,
) -> Result<RemoteMessage, String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    loop {
        let frame = socket
            .next()
            .await
            .ok_or_else(|| "remote closed".to_string())?
            .map_err(|error| error.to_string())?;
        if frame.is_binary() {
            return secure.decrypt_message(&frame.into_data());
        }
    }
}
