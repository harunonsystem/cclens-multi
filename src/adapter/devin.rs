//! Devin CLI's local ATIF v1.7 transcript format.
use super::multi::{prompt, string, timestamp};
use crate::multi::{Batch, Event, Evidence, Session, Tokens};
use serde_json::Value;

pub fn parse(text: &str, path: &str) -> Batch {
    let mut batch = Batch::default();
    let Ok(doc) = serde_json::from_str::<Value>(text) else {
        batch.warnings.push("invalid Devin transcript JSON".into());
        return batch;
    };
    if doc["schema_version"] != "ATIF-v1.7" {
        batch.unknown_records += 1;
        batch
            .warnings
            .push("unsupported Devin transcript schema".into());
        return batch;
    }
    let (Some(id), Some(steps)) = (string(&doc["session_id"]), doc["steps"].as_array()) else {
        batch
            .warnings
            .push("missing Devin session ID or steps".into());
        return batch;
    };
    let default_model = string(&doc["agent"]["model_name"]);
    batch.sessions.push(Session {
        id: id.clone(),
        started_ms: steps
            .iter()
            .filter_map(|s| timestamp(&s["timestamp"]))
            .min(),
        ..Default::default()
    });
    for step in steps {
        let kind = match step["source"].as_str() {
            Some("agent") if !step["metrics"].is_null() => "usage",
            Some("user") => "prompt",
            Some("system" | "agent") => continue,
            _ => {
                batch.unknown_records += 1;
                continue;
            }
        };
        let Some(step_id) = step["step_id"].as_u64() else {
            batch.unknown_records += 1;
            continue;
        };
        let mut event = Event {
            id: format!("{id}:{step_id}"),
            session_id: id.clone(),
            timestamp_ms: timestamp(&step["timestamp"]),
            kind: kind.into(),
            measurement: "measured".into(),
            evidence: Evidence {
                path: path.into(),
                row_id: Some(step_id.to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        if kind == "prompt" {
            if let Some(message) = step["message"].as_str() {
                batch.events.extend(prompt(event, message));
            } else {
                batch.unknown_records += 1;
            }
            continue;
        }
        event.model = string(&step["model_name"]).or_else(|| default_model.clone());
        let metrics = &step["metrics"];
        let input = metrics["prompt_tokens"].as_u64();
        let output = metrics["completion_tokens"].as_u64();
        let cache_read = metrics["cached_tokens"].as_u64();
        if input.is_none()
            || output.is_none()
            || (!metrics["cached_tokens"].is_null() && cache_read.is_none())
        {
            batch.warnings.push(format!(
                "missing or invalid token counters at Devin step {step_id}"
            ));
        }
        event.tokens = Tokens {
            input,
            output,
            cache_read,
            total: input.zip(output).and_then(|(i, o)| i.checked_add(o)),
            ..Default::default()
        };
        batch.events.push(event);
    }
    batch
}
