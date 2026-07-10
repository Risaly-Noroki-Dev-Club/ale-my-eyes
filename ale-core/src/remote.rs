use crate::actions::ActionPlan;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const REMOTE_PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_REMOTE_PORT: u16 = 37654;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteMessage {
    ClientHello(ClientHello),
    ServerHello(ServerHello),
    CommandRequest(CommandRequest),
    CommandPreview(CommandPreview),
    ConfirmExecution(ConfirmExecution),
    ExecutionStatus(ExecutionStatus),
    Error(RemoteError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHello {
    pub protocol_version: u32,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHello {
    pub protocol_version: u32,
    pub device_name: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub request_id: String,
    pub input: CommandInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "input", rename_all = "snake_case")]
pub enum CommandInput {
    Text { text: String },
    AudioWav { wav_base64: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandPreview {
    pub request_id: String,
    pub response_text: String,
    pub action_steps: Vec<String>,
    pub confirmation_text: String,
    pub requires_confirmation: bool,
    pub has_plan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmExecution {
    pub request_id: String,
    pub approved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStatus {
    pub request_id: String,
    pub state: ExecutionState,
    pub message: String,
    pub actions_executed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    PreviewReady,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteError {
    pub request_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct PairingInfo {
    pub host: String,
    pub port: u16,
    pub session_id: String,
    pub code: String,
    pub name: String,
}

impl PairingInfo {
    pub fn uri(&self) -> String {
        format!(
            "ale-my-eyes://pair?host={}&port={}&sid={}&code={}&name={}",
            urlencoding::encode(&self.host),
            self.port,
            urlencoding::encode(&self.session_id),
            urlencoding::encode(&self.code),
            urlencoding::encode(&self.name)
        )
    }

    pub fn websocket_url(&self) -> String {
        format!("ws://{}:{}", self.host, self.port)
    }

    pub fn from_uri(uri: &str) -> Result<Self, String> {
        let parsed = url::Url::parse(uri).map_err(|error| error.to_string())?;
        if parsed.scheme() != "ale-my-eyes" || parsed.host_str() != Some("pair") {
            return Err("不是 Ale, My Eyes! 配对链接".to_string());
        }

        let mut host = None;
        let mut port = None;
        let mut session_id = None;
        let mut code = None;
        let mut name = None;

        for (key, value) in parsed.query_pairs() {
            match key.as_ref() {
                "host" => host = Some(value.to_string()),
                "port" => port = value.parse::<u16>().ok(),
                "sid" => session_id = Some(value.to_string()),
                "code" => code = Some(value.to_string()),
                "name" => name = Some(value.to_string()),
                _ => {}
            }
        }

        Ok(Self {
            host: host.ok_or_else(|| "缺少 host".to_string())?,
            port: port.unwrap_or(DEFAULT_REMOTE_PORT),
            session_id: session_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            code: code.ok_or_else(|| "缺少配对码".to_string())?,
            name: name.unwrap_or_else(|| "Desktop".to_string()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct PendingRemotePlan {
    pub request_id: String,
    pub plan: ActionPlan,
}

pub fn new_request_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_uri_roundtrips() {
        let info = PairingInfo {
            host: "192.168.1.2".to_string(),
            port: 37654,
            session_id: "session".to_string(),
            code: "123456".to_string(),
            name: "MacBook".to_string(),
        };

        let restored = PairingInfo::from_uri(&info.uri()).unwrap();
        assert_eq!(restored.host, info.host);
        assert_eq!(restored.port, info.port);
        assert_eq!(restored.session_id, info.session_id);
        assert_eq!(restored.code, info.code);
        assert_eq!(restored.name, info.name);
    }
}
