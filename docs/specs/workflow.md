# Workflow improvement product

## Contract

`cclens-multi workflow` turns existing local adapter reports into a durable,
inspectable experiment. It never changes agent configuration, installs skills,
runs a proposed command, publishes data, or automatically rolls back a change.

The loop is audit → advise → record one trial → evaluate → show. A Rust/Cargo
installation of the existing binary is sufficient; no service or new account
is required. Native advice optionally uses an already authenticated pi or
Claude CLI. An existing agent can instead import a structured response.

## Audit

`workflow audit` requires the exact recorded `--project`, respects the common
agent/model/time filters and defaults to the preceding fourteen days.
It selects the latest five parent sessions and at most eight events per
selected session, retaining original evidence references. Each session first
gets up to four prompts, two failures and two edits; remaining slots use its
other recent candidates. Busy edits cannot erase other sessions or all prompts. These are sessions,
not proven completed tasks. Omitted events and unknown completion are explicit.

Native parent metadata excludes known child sessions from the parent sample.
Child events belonging to the selected work may remain as evidence, but are
not direct human interventions. Body retrieval belongs to the adapter, never
the workflow domain or store. No original text is read without
`--include-text`. That opt-in scans at most 64 MiB per evidence source and
returns at most 2,000 characters per event. The source snapshot fingerprint
must still match the analyzed source; changed, unavailable, oversized or
unparseable sources remain unknown rather than supplying a different record. A direct user role is only a candidate, not a proven correction.
No inference equates missing actor data or missing traces with zero use.

Up to five explicitly supplied `--context` files, each at most 16 KiB of UTF-8,
provide current SSOT evidence. The caller must not supply secrets.
Historical failures alone cannot prove a current bug or deployed fix.

Runs use a new private directory (0700 on Unix) and exclusive files (0600).
The default is a unique directory beside the derived database under
`workflow/`. Explicit artifact directories inside known transcript inputs
are refused, including paths through an existing symlink. Existing runs are
never overwritten. An interrupted incomplete artifact is not silently
treated as a successful result.

Source coverage metadata is collector-wide, not evidence of complete coverage
of the selected project. Null and unavailable values remain unknown.

## Advice

`workflow brief RUN` emits the exact bounded prompt for inspection or use by
an existing agent. Bundled, attributed benchmark patterns come from pinned
pstack and Matt Pocock primary-source revisions. They are hypotheses, not
scores of similarity to another person's setup. No network lookup is implicit.

`workflow advise RUN --response FILE` imports a JSON response without
contacting a model. Alternatively `--advisor pi|claude` requires both
`--advisor-model` and `--allow-model-input`. This explicitly authorizes the
selected packet, excerpts and context to reach that CLI's provider.
pi requires a fully qualified provider/model and receives an explicit provider
flag. No implicit model/provider fallback or new credential storage is added.

The runner uses a subprocess argument vector, not a shell, with stdin input,
tools disabled, no session persistence and ambient extensions/context disabled
using the native CLI's flags. stdout is bounded to 1 MiB; the default timeout
is 120 seconds (configurable from 1 to 600). Unix runners use an owned process
group, terminated before pipe collection, including descendants retaining pipes.
Other platforms can import a response but do not launch a native advisor.
Process errors, timeout, malformed
JSON and invalid references save no advice. Native runners are trusted local
programs; this is not an OS sandbox for a malicious replacement executable.

A response has a nonempty `summary` and zero to three `findings`. Each has:

- `title`, `hypothesis`, `change`, `evaluation`, `rollback`: bounded strings;
- `evidence`: at least one exact `agent:event.id` from the packet, optionally
  explicit `context-N` references;
- `benchmarks`: zero or more supplied catalog IDs;
- `current_state`: `current`, `mitigation_added`, `resolved` or `unverified`.

Current-state claims other than unverified require a supplied current context.
Unknown evidence and benchmark references are rejected. Schema validation
proves reference membership, not the truth of the model's explanation.
The saved advice also records generation time, source, runner and chosen model.
Existing advice is immutable; another proposal attempt requires a new audit.

## Trial

`workflow trial start RUN --finding N --baseline FILE --change DESCRIPTION`
records one change, but does not apply it. Mitigation-added and resolved
findings are observation targets, not new changes. An unverified finding also
requires the user's explicit `--confirm-current`. Current-state confirmation
is an attestation, not independently established by the CLI.

Measurement JSON contains `workload`, `model`, positive `tasks`, and nullable
`quality_passed`, `human_interventions`, `rework`, `elapsed_seconds`.
These are explicitly labelled user-supplied observations; never fabricate
values or use zero for missing data.

`workflow trial evaluate RUN --after FILE` conservatively decides:

- **revert**: quality acceptance failed, or comparable observations show
  increased human intervention or rework;
- **hold**: models/workloads/task counts differ, fewer than three comparable
  tasks, a required measure is unknown, time increases, or no improvement;
- **adopt**: declared quality passes and at least one observed measure improves
  without measured regression under the comparison contract.

The same evaluation can be replayed. Different observations cannot overwrite
a result. Decisions do not prove causality or modify configuration.
`workflow show RUN` resumes inspection of packet, advice, trial and result
without reading or refreshing transcript sources.

## Verification

All fixtures are hand-authored synthetic events. CLI tests cover project
scope, body opt-in, private permissions, input-path protection, model consent,
disabled tools/context, evidence validation, immutable advice, current-state
confirmation, replay and quality/comparability decisions. Real-data smoke
artifacts must stay outside the repository and are never packaged.
