use ale_core::actions::ActionPlan;
use serde::Serialize;
use std::io::Write;

#[derive(Serialize)]
struct AuditEvent<'a> {
    timestamp_ms: u128,
    event: &'a str,
    origin: &'a str,
    risk_level: String,
    action_count: usize,
    detail: Option<&'a str>,
}

/// Writes a redacted, append-only record of automation lifecycle events.
pub fn record(event: &str, origin: &str, plan: &ActionPlan, detail: Option<&str>) {
    let path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ale-my-eyes")
        .join("automation-audit.jsonl");
    let Ok(line) = serde_json::to_string(&AuditEvent {
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        event,
        origin,
        risk_level: format!("{:?}", plan.risk_level).to_lowercase(),
        action_count: plan.actions.len(),
        detail,
    }) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}
