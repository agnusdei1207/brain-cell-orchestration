# Architecture

## Architecture Statement

`brain-cell-orchestration` should be a harness-aware orchestration runtime, not a single-purpose agent.

The stable center is the orchestration spine. Everything else should plug into that spine through explicit contracts.

The orchestration reference point is Codex-style goal-directed execution. OpenClaw is the reference for autonomy and persistence behaviors. OpenCode is the reference for provider-agnostic model connectivity and fast switching. Claude Code is only a terminal UX reference.

Reference extraction details live in [REFERENCE_ANALYSIS.md](/Users/pf/workspace/brain-cell-orchestration/docs/research/REFERENCE_ANALYSIS.md).

## High-Level Shape

```text
Operator
  -> TUI / CLI shell
  -> Submission queue
  -> Intent classifier
  -> Objective tracker
  -> Harness resolver
  -> Model/provider resolver
  -> Orchestrator control plane
  -> Planner cell
  -> Coordinator cell
  -> Executor cells
  -> Reviewer cell
  -> Inter-cell message bus
  -> Autonomy scheduler / wake manager
  -> Session + evidence store
  -> Checkpoint + pending-work journal
  -> Event queue
  -> Tool/runtime adapters
```

## Crate Responsibilities

### `apps/bco`

- binary entrypoint
- CLI argument parsing
- process lifecycle
- interactive vs headless mode switch
- top-level commands such as `exec`, `review`, `resume`, `fork`, `providers`, and `models`

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
- objective tracking and next-action explanation
- orchestrator control plane
- inter-cell communication protocol
- parent-child cell topology and subtree shutdown
- orchestration event emission

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

Structural rules learned from Codex:

- cells are first-class runtime resources
- parent-child lineage must be explicit
- orchestration control should be separate from cell business logic
- inter-cell communication should use typed messages
- deep or runaway spawning must be bounded by policy

## Orchestrator Control Plane

The runtime should have a dedicated control-plane type that owns orchestration-only actions.

Responsibilities:

- spawn a cell
- send a message to a cell
- interrupt a cell
- shut down a cell subtree
- inspect lineage and active state

This must remain separate from the code that performs the cell's actual work.

## Operation Contract

The runtime should support pluggable run types instead of one monolithic session loop.

Examples of future run types:

- regular interactive turn
- review turn
- background summarization turn
- wakeup retry turn
- export or report turn

Recommended shape:

- one trait for pluggable operations
- one typed execution context
- one typed abort or interruption path

## Inter-Cell Communication Contract

Cell-to-cell interaction should use explicit typed messages.

Required properties:

- sender identity
- recipient identity
- optional broadcast or fan-out support
- payload type
- delivery mode such as queue-only or trigger-now

This keeps cells decoupled and makes replay or observability easier.

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

Recommended data types:

- `ProviderRef`
- `ModelRef`
- `ConnectionProfile`
- `ActiveModelState`
- `ModelSwitchEvent`

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
- cell lineage and topology
- append-only event history
- checkpoint snapshots
- queued pending work
- memory summaries
- model/provider transition log

## Observability Contract

The runtime should separate:

- submission queue: operator or automation requests entering the runtime
- event queue: runtime events emitted outward for UI, logs, and replay

Useful event categories:

- cell spawn begin/end
- cell interaction begin/end
- model switch
- approval pending/resolved
- task completed/replanned
- wakeup or retry scheduled/fired

This is required for good TUI feedback and post-run reconstruction.

## Runtime Services Contract

Runtime dependencies should be grouped behind an explicit service container or dependency injection boundary.

Examples:

- model manager
- exec manager
- tool adapter registry
- hooks
- session store
- policy evaluator

Reason:

- orchestration logic becomes testable
- service replacements or mocks stay localized
- runtime construction stays explicit

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

Primary operator actions should stay one command away:

- switch model
- connect provider
- inspect cells
- inspect memory
- resume prior work
- approve or reject pending actions

The TUI should avoid decorative terminal chrome. Density and clarity matter more than ornament.

## Initial Technical Decision Record

- Rust workspace: yes
- Docker-first build: yes
- local file-first persistence: yes
- TUI-first operator interface: yes
- harness abstraction as core primitive: yes
- plugin marketplace in bootstrap: no
