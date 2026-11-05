use cclens::multi::{self, Batch, Event, Evidence, Session};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
struct TempDir(std::path::PathBuf);
impl TempDir {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "workflow-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cclens-multi"))
        .args(args)
        .arg("--db")
        .arg(root.join("analysis.db"))
        .arg("--json")
        .output()
        .unwrap()
}
fn fixture() -> TempDir {
    let root = TempDir::new();
    drop(cclens::store::Store::open(root.path().join("analysis.db")).unwrap());
    let store = rusqlite::Connection::open(root.path().join("analysis.db")).unwrap();
    multi::store::initialize(&store).unwrap();
    let now = chrono::Utc::now().timestamp_millis();
    for (agent, project, id, sidechain) in [
        ("claude", "/tmp/example", "direct", false),
        ("claude", "/tmp/example", "child", true),
        ("pi", "/tmp/unrelated", "other", false),
    ] {
        let source = root.path().join(format!("{id}.jsonl"));
        fs::write(&source,json!({"type":"message","isSidechain":sidechain,"message":{"role":"user","content":[{"type":"text","text":"Please continue to verification."}]}}).to_string()+"\n").unwrap();
        let batch = Batch {
            sessions: vec![Session {
                id: id.into(),
                project: Some(project.into()),
                started_ms: Some(now),
                parent_id: sidechain.then(|| "direct".into()),
            }],
            events: vec![Event {
                id: id.into(),
                session_id: id.into(),
                kind: "prompt".into(),
                category: Some("steer".into()),
                timestamp_ms: Some(now),
                evidence: Evidence {
                    path: source.display().to_string(),
                    line: Some(1),
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        multi::store::replace(
            &store,
            agent,
            &source.display().to_string(),
            &cclens::adapter::multi::fingerprint(&fs::read_to_string(&source).unwrap()),
            &batch,
        )
        .unwrap();
    }
    root
}
fn audit(root: &Path, include: bool) -> Value {
    let out = root.join("run");
    let mut args = vec![
        "workflow",
        "audit",
        "--frozen",
        "--project",
        "/tmp/example",
        "--out",
        out.to_str().unwrap(),
    ];
    if include {
        args.push("--include-text");
    }
    let result = run(root, &args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&fs::read(out.join("packet.json")).unwrap()).unwrap()
}
#[test]
fn verified_large_transcript_still_returns_only_a_bounded_excerpt() {
    let root = TempDir::new();
    let source = root.path().join("large.jsonl");
    let mut body = json!({"message":{"role":"user","content":"observe this"}}).to_string() + "\n";
    body.push_str(&" ".repeat(17 * 1024 * 1024));
    fs::write(&source, &body).unwrap();
    let evidence = Evidence {
        path: source.display().to_string(),
        line: Some(1),
        ..Default::default()
    };
    let excerpt =
        cclens::adapter::workflow::excerpt(&evidence, &cclens::adapter::multi::fingerprint(&body));
    assert_eq!(excerpt.unwrap().text, "observe this");
}
#[test]
fn busy_edit_session_cannot_hide_other_selected_sessions_or_user_prompts() {
    let root = fixture();
    let conn = rusqlite::Connection::open(root.path().join("analysis.db")).unwrap();
    conn.execute_batch("DELETE FROM multi_observations; DELETE FROM multi_sessions;")
        .unwrap();
    let now = chrono::Utc::now().timestamp_millis();
    for id in ["busy", "quiet"] {
        let source = root.path().join(format!("{id}.jsonl"));
        fs::write(&source, "{}").unwrap();
        let events = (0..if id == "busy" { 50 } else { 1 })
            .map(|i| Event {
                id: format!("{id}-{i}"),
                session_id: id.into(),
                kind: if id == "quiet" || i < 5 {
                    "prompt".into()
                } else {
                    "edit".into()
                },
                timestamp_ms: Some(now - if id == "quiet" { 100_000 } else { 50 - i }),
                evidence: Evidence {
                    path: source.display().to_string(),
                    line: Some(1),
                    ..Default::default()
                },
                ..Default::default()
            })
            .collect();
        let batch = Batch {
            sessions: vec![Session {
                id: id.into(),
                project: Some("/tmp/example".into()),
                started_ms: Some(now - if id == "quiet" { 100_000 } else { 1000 }),
                ..Default::default()
            }],
            events,
            ..Default::default()
        };
        multi::store::replace(
            &conn,
            "pi",
            &source.display().to_string(),
            "synthetic",
            &batch,
        )
        .unwrap();
    }
    let packet = audit(root.path(), false);
    let events = packet["events"].as_array().unwrap();
    assert!(
        events.iter().any(|e| e["event"]["session_id"] == "quiet"),
        "every sampled session with candidates must be represented"
    );
    assert!(
        events
            .iter()
            .any(|e| e["event"]["session_id"] == "busy" && e["event"]["kind"] == "prompt"),
        "edit volume must not erase intervention candidates"
    );
    assert!(events.len() <= 16);
}
#[test]
fn workflow_selection_keeps_agent_identity_and_rejects_changed_source_excerpts() {
    let root = fixture();
    let conn = rusqlite::Connection::open(root.path().join("analysis.db")).unwrap();
    conn.execute_batch("DELETE FROM multi_observations; DELETE FROM multi_sessions;")
        .unwrap();
    let now = chrono::Utc::now().timestamp_millis();
    for i in 0..6 {
        let agent = if i == 5 { "claude" } else { "pi" };
        let id = if i == 0 || i == 5 {
            "same".to_owned()
        } else {
            format!("s{i}")
        };
        let source = root.path().join(format!("extra-{i}.jsonl"));
        let body =
            json!({"type":"message","message":{"role":"user","content":"original evidence"}})
                .to_string();
        fs::write(&source, &body).unwrap();
        let event = Event {
            id: format!("e{i}"),
            session_id: id.clone(),
            kind: "prompt".into(),
            timestamp_ms: Some(now - i * 1000),
            evidence: Evidence {
                path: source.display().to_string(),
                line: Some(1),
                ..Default::default()
            },
            ..Default::default()
        };
        let batch = Batch {
            sessions: vec![Session {
                id,
                project: Some("/tmp/example".into()),
                started_ms: Some(now - i * 1000),
                ..Default::default()
            }],
            events: vec![event],
            ..Default::default()
        };
        multi::store::replace(
            &conn,
            agent,
            &source.display().to_string(),
            &cclens::adapter::multi::fingerprint(&body),
            &batch,
        )
        .unwrap();
    }
    // A previously observed line is now a different valid record.
    fs::write(
        root.path().join("extra-0.jsonl"),
        json!({"type":"message","message":{"role":"user","content":"different sensitive record"}})
            .to_string(),
    )
    .unwrap();
    let packet = audit(root.path(), true);
    let events = packet["events"].as_array().unwrap();
    assert_eq!(
        events.len(),
        5,
        "outside-sample agent with the same session ID must stay out"
    );
    assert!(events.iter().all(|e| e["agent"] == "pi"));
    let changed = events.iter().find(|e| e["event"]["id"] == "e0").unwrap();
    assert!(
        changed["excerpt"].is_null(),
        "changed source must not be attributed to the old event"
    );
}
#[test]
fn workflow_requires_project_and_refuses_artifacts_inside_transcript_inputs() {
    let root = fixture();
    let missing = run(root.path(), &["workflow", "audit", "--frozen"]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("requires --project"));
    let input = root.path().join("logs");
    fs::create_dir(&input).unwrap();
    let out = input.join("artifact");
    let result = run(
        root.path(),
        &[
            "workflow",
            "audit",
            "--frozen",
            "--project",
            "/tmp/example",
            "--claude",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
    );
    assert!(!result.status.success());
    assert!(!out.exists());
}
#[test]
fn agent_contract_exposes_workflow_privacy_boundary() {
    let root = TempDir::new();
    let r = run(root.path(), &["agent-context"]);
    assert!(r.status.success());
    let v: Value = serde_json::from_slice(&r.stdout).unwrap();
    assert!(
        v["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == "workflow")
    );
    assert_eq!(
        v["workflow"]["provider_input_requires"],
        "--allow-model-input"
    );
    assert!(!root.path().join("analysis.db").exists());
}
#[test]
fn workflow_audit_is_scoped_private_and_body_opt_in() {
    let root = fixture();
    let packet = audit(root.path(), false);
    assert_eq!(packet["schema_version"], 1);
    assert_eq!(packet["scope"]["project"], "/tmp/example");
    let events = packet["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| e["excerpt"].is_null()));
    assert!(events.iter().any(|e| e["origin"] == "delegated"));
    assert!(!events.iter().any(|e| e["event"]["id"] == "other"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(root.path().join("run"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(root.path().join("run/packet.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let again = run(
        root.path(),
        &[
            "workflow",
            "audit",
            "--frozen",
            "--project",
            "/tmp/example",
            "--out",
            root.path().join("run").to_str().unwrap(),
        ],
    );
    assert!(!again.status.success());
    let root = fixture();
    let packet = audit(root.path(), true);
    assert!(packet["events"].as_array().unwrap().iter().any(|e| {
        e["excerpt"]
            .as_str()
            .is_some_and(|t| t.contains("continue to verification"))
    }));
}
