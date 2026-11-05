use serde_json::{Value, json};
use std::{fs, process::Command};
#[path = "support/workflow.rs"]
mod support;
use support::{Fixture, run};

#[test]
fn imported_advice_requires_real_evidence_and_is_never_overwritten() {
    let f = Fixture::new();
    f.audit();
    let response = f.root.join("response.json");
    fs::write(&response,json!({"summary":"one candidate","findings":[{
        "title":"verify before stopping","evidence":["pi:missing"],"benchmarks":[],
        "current_state":"unverified","hypothesis":"verification handoff might be missing",
        "change":"use the existing verification command","evaluation":"compare quality and interventions","rollback":"restore only this change"
    }]}).to_string()).unwrap();
    let args = [
        "workflow",
        "advise",
        f.run.to_str().unwrap(),
        "--response",
        response.to_str().unwrap(),
    ];
    let bad = run(&f.root, &args);
    assert!(!bad.status.success());
    assert!(!f.run.join("advice.json").exists());
    let mut v: Value = serde_json::from_slice(&fs::read(&response).unwrap()).unwrap();
    v["findings"][0]["evidence"] = json!(["pi:direct"]);
    fs::write(&response, v.to_string()).unwrap();
    let mut dry = args.to_vec();
    dry.push("--dry-run");
    assert!(!run(&f.root, &dry).status.success());
    assert!(
        !f.run.join("advice.json").exists(),
        "unsupported dry-run must fail before a saved command writes or contacts a model"
    );
    assert!(run(&f.root, &args).status.success());
    assert!(f.run.join("advice.json").exists());
    assert!(!run(&f.root, &args).status.success());
}
#[cfg(unix)]
#[test]
fn large_diagnostics_are_drained_without_breaking_a_valid_advisor() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    f.audit();
    let bin = f.root.join("bin");
    fs::create_dir(&bin).unwrap();
    let runner = bin.join("pi");
    fs::write(&runner,"#!/bin/sh\nset -e\ncat >/dev/null\ndd if=/dev/zero bs=1024 count=256 >&2 2>/dev/null\nprintf '%s\\n' '{\"summary\":\"no justified change\",\"findings\":[]}'\n").unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o700)).unwrap();
    let r = Command::new(env!("CARGO_BIN_EXE_cclens-multi"))
        .args([
            "workflow",
            "advise",
            f.run.to_str().unwrap(),
            "--advisor",
            "pi",
            "--advisor-model",
            "openai/example",
            "--allow-model-input",
            "--timeout-seconds",
            "5",
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(f.run.join("advice.json").exists());
}
#[cfg(unix)]
#[test]
fn advisor_timeout_also_closes_inherited_descendant_pipes() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    f.audit();
    let bin = f.root.join("bin");
    fs::create_dir(&bin).unwrap();
    let runner = bin.join("pi");
    fs::write(&runner, "#!/bin/sh\ncat >/dev/null\nsleep 4 &\nwait\n").unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o700)).unwrap();
    let start = std::time::Instant::now();
    let r = Command::new(env!("CARGO_BIN_EXE_cclens-multi"))
        .args([
            "workflow",
            "advise",
            f.run.to_str().unwrap(),
            "--advisor",
            "pi",
            "--advisor-model",
            "openai/example",
            "--allow-model-input",
            "--timeout-seconds",
            "1",
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert!(!f.run.join("advice.json").exists());
    assert!(
        start.elapsed() < std::time::Duration::from_secs(3),
        "timeout must bound the whole advisor invocation"
    );
}
#[cfg(unix)]
#[test]
fn native_advisor_needs_consent_and_has_no_tools_or_ambient_context() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    f.audit();
    let bin = f.root.join("bin");
    fs::create_dir(&bin).unwrap();
    let runner = bin.join("pi");
    fs::write(&runner,"#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$CAPTURE_ARGS\"\ncat > \"$CAPTURE_INPUT\"\nprintf '%s\\n' '{\"summary\":\"no justified change\",\"findings\":[]}'\n").unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o700)).unwrap();
    let invoke = |allow: bool, model: &str| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_cclens-multi"));
        c.args([
            "workflow",
            "advise",
            f.run.to_str().unwrap(),
            "--advisor",
            "pi",
            "--advisor-model",
            model,
            "--json",
        ]);
        if allow {
            c.arg("--allow-model-input");
        }
        c.env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("CAPTURE_ARGS", f.root.join("args"))
        .env("CAPTURE_INPUT", f.root.join("input"));
        c.output().unwrap()
    };
    assert!(!invoke(false, "openai/example").status.success());
    assert!(!f.root.join("args").exists());
    assert!(!invoke(true, "ambiguous-model").status.success());
    assert!(
        !f.root.join("args").exists(),
        "an ambiguous provider must not launch the CLI"
    );
    let result = invoke(true, "openai/example");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let args = fs::read_to_string(f.root.join("args")).unwrap();
    for flag in [
        "--no-tools",
        "--no-extensions",
        "--no-skills",
        "--no-context-files",
        "--no-session",
    ] {
        assert!(args.contains(flag), "{flag}");
    }
    let input = fs::read_to_string(f.root.join("input")).unwrap();
    assert!(input.contains("untrusted"));
    assert!(input.contains("evidence"));
    assert!(f.run.join("advice.json").exists());
    let saved: Value =
        serde_json::from_slice(&fs::read(f.run.join("advice.json")).unwrap()).unwrap();
    assert_eq!(saved["advisor_model"], "openai/example");
    assert_eq!(saved["source"], "native_advisor");
}
