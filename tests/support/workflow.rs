use cclens::multi::{self, Batch, Event, Evidence, Session};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
pub struct Fixture {
    pub root: PathBuf,
    pub run: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "workflow-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let run = root.join("run");
        drop(cclens::store::Store::open(root.join("analysis.db")).unwrap());
        let conn = rusqlite::Connection::open(root.join("analysis.db")).unwrap();
        multi::store::initialize(&conn).unwrap();
        let source = root.join("source.jsonl");
        fs::write(
            &source,
            json!({"type":"message","message":{"role":"user","content":"Continue with tests"}})
                .to_string(),
        )
        .unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let batch = Batch {
            sessions: vec![Session {
                id: "session".into(),
                project: Some("/tmp/example".into()),
                started_ms: Some(now),
                ..Default::default()
            }],
            events: vec![Event {
                id: "direct".into(),
                session_id: "session".into(),
                timestamp_ms: Some(now),
                kind: "prompt".into(),
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
            &conn,
            "pi",
            &source.display().to_string(),
            &cclens::adapter::multi::fingerprint(&fs::read_to_string(&source).unwrap()),
            &batch,
        )
        .unwrap();
        Self { root, run }
    }
    pub fn audit(&self) {
        let r = run(
            &self.root,
            &[
                "workflow",
                "audit",
                "--frozen",
                "--project",
                "/tmp/example",
                "--out",
                self.run.to_str().unwrap(),
                "--include-text",
            ],
        );
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
pub fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cclens-multi"))
        .args(args)
        .arg("--db")
        .arg(root.join("analysis.db"))
        .arg("--json")
        .output()
        .unwrap()
}
