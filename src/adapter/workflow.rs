//! Explicit, bounded retrieval of original evidence. Raw formats stop here.
use crate::multi::Evidence;
use serde_json::Value;
use std::{fs::File, io::Read};

pub struct Excerpt {
    pub text: String,
    pub origin: &'static str,
}

/// At most 64 MiB is scanned and at most 2,000 characters leave the adapter.
/// Missing, SQLite-only, oversized or no-longer-parseable evidence is unknown.
pub fn excerpt(evidence: &Evidence, expected_fingerprint: &str) -> Option<Excerpt> {
    let line = evidence.line?.checked_sub(1)?;
    let mut snapshot = String::new();
    File::open(&evidence.path)
        .ok()?
        .take(64 * 1024 * 1024 + 1)
        .read_to_string(&mut snapshot)
        .ok()?;
    if snapshot.len() > 64 * 1024 * 1024
        || super::multi::fingerprint(&snapshot) != expected_fingerprint
    {
        return None;
    }
    let text = snapshot.lines().nth(line)?;
    let raw: Value = serde_json::from_str(text).ok()?;
    let message = raw
        .get("message")
        .or_else(|| raw.get("payload"))
        .unwrap_or(&raw);
    let role = message.get("role").and_then(Value::as_str);
    let content = message.get("content").or_else(|| raw.get("content"))?;
    let text = if let Some(s) = content.as_str() {
        s.to_owned()
    } else {
        content
            .as_array()?
            .iter()
            .filter_map(|c| c.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let origin = if raw.get("isSidechain").and_then(Value::as_bool) == Some(true)
        || raw.get("agentId").and_then(Value::as_str).is_some()
    {
        "delegated"
    } else if [
        "<task-notification>",
        "<subagent_notification>",
        "This session is being continued",
    ]
    .iter()
    .any(|p| text.trim_start().starts_with(p))
    {
        "automated"
    } else if role == Some("user") {
        "direct_user_candidate"
    } else {
        "unknown"
    };
    Some(Excerpt {
        text: text.chars().take(2000).collect(),
        origin,
    })
}
