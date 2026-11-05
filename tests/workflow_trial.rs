use serde_json::{Value, json};
use std::fs;
#[path = "support/workflow.rs"]
mod support;
use support::{Fixture, run};

fn advice(f: &Fixture) {
    f.audit();
    let response = f.root.join("response.json");
    fs::write(&response,json!({"summary":"one candidate","findings":[{"title":"verify before stopping","evidence":["pi:direct"],"benchmarks":["pstack-proof"],"current_state":"unverified","hypothesis":"verification needs another instruction","change":"use existing checks","evaluation":"quality and human interventions must not worsen","rollback":"revert this change only"}]}).to_string()).unwrap();
    let r = run(
        &f.root,
        &[
            "workflow",
            "advise",
            f.run.to_str().unwrap(),
            "--response",
            response.to_str().unwrap(),
        ],
    );
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
}
fn measures(model: &str, quality: Option<bool>, interventions: Option<u64>, time: u64) -> Value {
    json!({"workload":"same-fixed-tasks","model":model,"tasks":3,"quality_passed":quality,"human_interventions":interventions,"rework":0,"elapsed_seconds":time})
}
#[test]
fn trial_requires_current_confirmation_and_quality_blocks_fast_adoption() {
    let f = Fixture::new();
    advice(&f);
    let baseline = f.root.join("before.json");
    fs::write(
        &baseline,
        measures("example", Some(true), Some(2), 100).to_string(),
    )
    .unwrap();
    let args = [
        "workflow",
        "trial",
        "start",
        f.run.to_str().unwrap(),
        "--finding",
        "0",
        "--baseline",
        baseline.to_str().unwrap(),
        "--change",
        "use existing checks",
    ];
    assert!(!run(&f.root, &args).status.success());
    assert!(!f.run.join("trial.json").exists());
    let mut allowed = args.to_vec();
    allowed.push("--confirm-current");
    let r = run(&f.root, &allowed);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let after = f.root.join("after.json");
    fs::write(
        &after,
        measures("example", Some(false), Some(1), 30).to_string(),
    )
    .unwrap();
    let finish = [
        "workflow",
        "trial",
        "evaluate",
        f.run.to_str().unwrap(),
        "--after",
        after.to_str().unwrap(),
    ];
    let r = run(&f.root, &finish);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let v: Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v["decision"], "revert");
    assert!(
        run(&f.root, &finish).status.success(),
        "same observation can be safely replayed"
    );
    let shown = run(&f.root, &["workflow", "show", f.run.to_str().unwrap()]);
    let v: Value = serde_json::from_slice(&shown.stdout).unwrap();
    assert_eq!(v["result"]["decision"], "revert");
}
#[test]
fn trial_holds_unknown_or_incomparable_observations_and_adopts_only_nonregression() {
    for (model, known, expected) in [
        ("different", Some(1), "hold"),
        ("example", None, "hold"),
        ("example", Some(1), "adopt"),
    ] {
        let f = Fixture::new();
        advice(&f);
        let before = f.root.join("before.json");
        fs::write(
            &before,
            measures("example", Some(true), Some(2), 100).to_string(),
        )
        .unwrap();
        assert!(
            run(
                &f.root,
                &[
                    "workflow",
                    "trial",
                    "start",
                    f.run.to_str().unwrap(),
                    "--finding",
                    "0",
                    "--baseline",
                    before.to_str().unwrap(),
                    "--change",
                    "one change",
                    "--confirm-current"
                ]
            )
            .status
            .success()
        );
        let after = f.root.join("after.json");
        fs::write(&after, measures(model, Some(true), known, 70).to_string()).unwrap();
        let r = run(
            &f.root,
            &[
                "workflow",
                "trial",
                "evaluate",
                f.run.to_str().unwrap(),
                "--after",
                after.to_str().unwrap(),
            ],
        );
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        let v: Value = serde_json::from_slice(&r.stdout).unwrap();
        assert_eq!(v["decision"], expected);
    }
}
