# Architecture

## Architecture Statement

`brain-cell-orchestration` should be a harness-aware orchestration runtime, not a single-purpose agent.

The stable center is the orchestration spine. Everything else should plug into that spine through explicit contracts.

The orchestration reference point is Codex-style goal-directed execution. OpenClaw is the reference for autonomy and persistence behaviors. OpenCode is the reference for provider-agnostic model connectivity and fast switching. Claude Code is only a terminal UX reference.

## High-Level Shape

```text
Operator
  -> TUI / CLI shell
  -> Intent classifier
  -> Objective tracker
  -> Harness resolver
  -> Model/provider resolver
  -> Planner cell
  -> Coordinator cell
  -> Executor cells
  -> Reviewer cell
  -> Autonomy scheduler / wake manager
  -> Session + evidence store
  -> Checkpoint + pending-work journal
  -> Tool/runtime adapters
```

## Crate Responsibilities

### `apps/bco`

- binary entrypoint
- CLI argument parsing
- process lifecycle
- interactive vs headless mode switch

### `crates/bco-core`

- canonical task, domain, policy, and event types
- shared identifiers
- intent and objective primitives
- subgoal and progress model

### `crates/bco-harness`

- harness trait definitions
- harness registry
- resolution rules
- operator override path
- harness-to-model preference hints

### `crates/bco-orchestrator`

- brain-cell lifecycle
- planning state machine
- queueing and turn execution
- reviewer-driven replanning
- model-switch-aware execution routing

### `crates/bco-session`

- append-only storage
- session resume
- artifact manifests
- report/export format
- checkpoints and pending-work persistence

### `crates/bco-tui`

- ratatui shell
- pane layout
- overlays and command routing
- terminal degradation strategy
- Claude Code-inspired UI density only

## Future Runtime Modules

These should stay outside bootstrap until the contracts are stable:

- `bco-autonomy` for wakeups, retries, scheduling, and follow-up jobs
- `bco-memory` for durable memory flushes and retrieval-ready summaries
- `bco-adapters` for shell, Docker, provider, and tool integrations
- `bco-models` for provider registry, connection profiles, and active model state

## Brain-Cell Model

Target cells:

- `planner-cell`
- `coordinator-cell`
- `executor-cell`
- `reviewer-cell`
- optional specialist cells such as `recon-cell`, `exploit-cell`, `coding-cell`, `report-cell`

Key rule: cells should share a blackboard and event bus, not hidden state.

Secondary rule: the runtime should always be able to explain the current objective, active subgoal, and next action.

## Harness Contract

Each harness should answer:

- what task classes it accepts
- how it decomposes work
- which tools it prefers
- what safety posture it requires
- how it decides done vs replan
- which model profiles it prefers or forbids

Minimal future trait sketch:

```rust
pub trait Harness {
    fn id(&self) -> &'static str;
    fn accepts(&self, intent: &TaskIntent) -> bool;
    fn plan_policy(&self) -> PlanPolicy;
    fn review_policy(&self) -> ReviewPolicy;
}
```

## Model Connectivity Contract

The model layer should stay provider-agnostic and operator-friendly.

It should support:

- `provider/model` identity parsing
- provider connection profiles
- slash commands like `/model` and `/connect`
- active model changes without tearing down session state
- explicit logging for model and provider transitions

Harness selection and model selection should be related but independent:

- harness decides domain execution policy
- model layer decides which backend satisfies that policy

## Execution Contract

Execution backends should be adapter-driven.

Examples:

- local shell adapter
- Docker task adapter
- tool wrapper adapter
- model/provider adapter
- local endpoint adapter

This keeps the orchestrator independent from one model vendor or one tool transport.

It also keeps autonomous resume flows from being tied to a single backend.

## Session Contract

The session layer should be append-only and replayable.

Reasons:

- security-sensitive tasks need evidence retention
- replanning requires prior context
- debugging agent behavior requires provenance
- autonomous continuation requires restart-safe state

Persisted state should include:

- current objective and subgoal chain
- append-only event history
- checkpoint snapshots
- queued pending work
- memory summaries
- model/provider transition log

## Autonomy Contract

Autonomy exists to continue goal-directed execution, not to bypass control.

It should support:

- manual resume
- scheduled wakeups
- bounded retry
- pending-work drains
- background review or summarization

It must not:

- bypass approvals
- mutate history destructively
- ignore harness-specific policy

## UI Architecture

The TUI should present:

- transcript
- current plan
- active cells
- pending approvals
- harness identity
- current provider/model
- connection health
- risk and capability status
- resumed or scheduled execution state

The TUI should avoid decorative terminal chrome. Density and clarity matter more than ornament.

## Initial Technical Decision Record

- Rust workspace: yes
- Docker-first build: yes
- local file-first persistence: yes
- TUI-first operator interface: yes
- harness abstraction as core primitive: yes
- plugin marketplace in bootstrap: no
