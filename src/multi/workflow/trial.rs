use super::advice::{CurrentState, SavedAdvice};
use super::*;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Record one planned change. Does not apply it or run arbitrary commands.
    Start {
        run: PathBuf,
        #[arg(long)]
        finding: usize,
        #[arg(long)]
        baseline: PathBuf,
        #[arg(long)]
        change: String,
        /// Explicitly attest that an otherwise unverified finding is current.
        #[arg(long)]
        confirm_current: bool,
    },
    /// Compare explicitly supplied observations; does not revert configuration.
    Evaluate {
        run: PathBuf,
        #[arg(long)]
        after: PathBuf,
    },
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Measurements {
    workload: String,
    model: String,
    tasks: u64,
    quality_passed: Option<bool>,
    human_interventions: Option<u64>,
    rework: Option<u64>,
    elapsed_seconds: Option<u64>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Trial {
    schema_version: u32,
    created_at: String,
    finding: usize,
    change: String,
    current_confirmed_by_user: bool,
    baseline: Measurements,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evaluation {
    schema_version: u32,
    decision: String,
    reason: String,
    after: Measurements,
    evidence_type: String,
    configuration_changed: bool,
}
fn valid(m: &Measurements) -> Result<()> {
    if m.workload.trim().is_empty() || m.model.trim().is_empty() || m.tasks == 0 {
        bail!("measurements require workload, model and a positive task count");
    }
    Ok(())
}
fn decision(before: &Measurements, after: &Measurements) -> (&'static str, &'static str) {
    if after.quality_passed == Some(false) {
        return (
            "revert",
            "Quality acceptance failed; speed or token savings cannot justify adoption.",
        );
    }
    if before.model != after.model
        || before.workload != after.workload
        || before.tasks != after.tasks
        || before.tasks < 3
    {
        return (
            "hold",
            "Comparison requires the same declared model/workload, equal task counts and at least three tasks.",
        );
    }
    let (Some(true), Some(_), Some(bi), Some(ai), Some(br), Some(ar), Some(bt), Some(at)) = (
        after.quality_passed,
        before.quality_passed,
        before.human_interventions,
        after.human_interventions,
        before.rework,
        after.rework,
        before.elapsed_seconds,
        after.elapsed_seconds,
    ) else {
        return (
            "hold",
            "Quality, human burden, rework or elapsed time is unknown; missing values are not zero.",
        );
    };
    if ai > bi || ar > br {
        return ("revert", "Human intervention or rework increased.");
    }
    if at > bt {
        return (
            "hold",
            "Elapsed time increased; the tradeoff needs further observation.",
        );
    }
    if ai < bi || ar < br || at < bt || before.quality_passed == Some(false) {
        return (
            "adopt",
            "Declared observations preserve quality and show improvement without measured regression; this is not a causal proof.",
        );
    }
    ("hold", "No observed improvement.")
}
pub(super) fn run(command: &Command) -> Result<Value> {
    match command {
        Command::Start {
            run,
            finding,
            baseline,
            change,
            confirm_current,
        } => {
            let saved: SavedAdvice = read_json(&run.join("advice.json"))?;
            if saved.schema_version != 1 {
                bail!("unsupported saved advice schema");
            }
            let selected = saved
                .advice
                .findings
                .get(*finding)
                .context("finding index not found")?;
            if matches!(
                selected.current_state,
                CurrentState::Resolved | CurrentState::MitigationAdded
            ) {
                bail!(
                    "this finding is an observation target, not a new change; do not duplicate an existing mitigation"
                );
            }
            if selected.current_state != CurrentState::Current && !confirm_current {
                bail!(
                    "confirm the current problem first; --confirm-current records your explicit verification"
                );
            }
            if change.trim().is_empty() || change.chars().count() > 4000 {
                bail!("describe one bounded change");
            }
            let baseline: Measurements = read_json(baseline)?;
            valid(&baseline)?;
            let trial = Trial {
                schema_version: 1,
                created_at: chrono::Utc::now().to_rfc3339(),
                finding: *finding,
                change: change.clone(),
                current_confirmed_by_user: *confirm_current,
                baseline,
            };
            write_new(&run.join("trial.json"), &trial)?;
            Ok(
                json!({"run_dir":run,"trial":trial,"configuration_changed":false,"next":"Apply only the authorized change through its SSOT and existing guards; then record comparable observations with workflow trial evaluate."}),
            )
        }
        Command::Evaluate { run, after } => {
            let trial: Trial = read_json(&run.join("trial.json"))?;
            if trial.schema_version != 1 {
                bail!("unsupported trial schema");
            }
            let after: Measurements = read_json(after)?;
            valid(&after)?;
            let result = run.join("result.json");
            if result.exists() {
                let previous: Evaluation = read_json(&result)?;
                if previous.after != after {
                    bail!(
                        "evaluation already recorded; a different experiment requires a new audit/trial"
                    );
                }
                return Ok(serde_json::to_value(previous)?);
            }
            let (decision, reason) = decision(&trial.baseline, &after);
            let result_value = Evaluation {
                schema_version: 1,
                decision: decision.into(),
                reason: reason.into(),
                after,
                evidence_type: "user_supplied_observations_not_independently_verified".into(),
                configuration_changed: false,
            };
            write_new(&result, &result_value)?;
            Ok(serde_json::to_value(result_value)?)
        }
    }
}
