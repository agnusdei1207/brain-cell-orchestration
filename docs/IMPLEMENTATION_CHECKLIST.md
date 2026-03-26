# Implementation Checklist

## Purpose

This document is the execution checklist for building `brain-cell-orchestration`.

It is written for practical implementation in a terminal-first workflow, including the case where the primary implementation agent is `claude-code` using `minimax-m2.7`.

Success criterion:

- a contributor should be able to open this file, work top to bottom, and know what to build, what done means, and what to verify before moving on

## Working Rules

- complete one checkpointed slice at a time
- do not start broad UI work before state contracts stabilize
- do not add integrations before the core traits and logs exist
- every milestone must end with a commit and push
- every item marked `verify` must be checked before proceeding

## Execution Order

- Phase A: contracts first
- Phase B: persistence and CLI spine
- Phase C: model connectivity
- Phase D: orchestration runtime
- Phase E: TUI MVP
- Phase F: harness MVPs
- Phase G: autonomy hardening

## Phase A: Contracts First

### A1. Objective model

Status: `pending`

Tasks:

- [ ] define `ObjectiveId`
- [ ] define `ObjectiveState`
- [ ] define `Subgoal`
- [ ] define `NextAction`
- [ ] define `ProgressStatus`
- [ ] define `TaskIntent`
- [ ] define `RiskProfile`

Definition of done:

- [ ] all objective-related types live in `bco-core`
- [ ] objective state can express current goal, active subgoal, and next action
- [ ] no placeholder `String` fields remain where enums/newtypes are clearer

Verify:

- [ ] binary compiles in Docker
- [ ] type names and field names are documented in architecture docs

### A2. Model identity and connection model

Status: `pending`

Tasks:

- [ ] define `ProviderRef`
- [ ] define `ModelRef`
- [ ] support canonical `provider/model` parsing
- [ ] define `ConnectionProfile`
- [ ] define `ActiveModelState`
- [ ] define `ModelSwitchEvent`
- [ ] define `ModelFallbackPolicy`

Definition of done:

- [ ] provider and model are not stored as one opaque string internally
- [ ] slash and CLI surfaces can still serialize to `provider/model`
- [ ] invalid model identifiers return clear parse errors

Verify:

- [ ] fixture tests cover valid and invalid `provider/model`
- [ ] session log schema has a place for model transition events

### A3. Harness contract

Status: `pending`

Tasks:

- [ ] define `HarnessId`
- [ ] define `Harness` trait
- [ ] define `PlanPolicy`
- [ ] define `ReviewPolicy`
- [ ] define `CapabilityPolicy`
- [ ] define harness-to-model preference contract
- [ ] define harness override semantics

Definition of done:

- [ ] a harness can declare acceptance, policy, and preferred model profile
- [ ] harness and model decisions are separate in code
- [ ] at least four placeholder harnesses exist: `ctf`, `pentest`, `coding`, `generalist`

Verify:

- [ ] registry tests cover default harness resolution
- [ ] architecture doc references exact trait responsibilities

## Phase B: Persistence And CLI Spine

### B1. Session layout

Status: `pending`

Tasks:

- [ ] create session root layout under `.bco/sessions/<session-id>/`
- [ ] add `session.json`
- [ ] add `transcript.jsonl`
- [ ] add `plan.jsonl`
- [ ] add `approvals.jsonl`
- [ ] add `evidence.jsonl`
- [ ] add `tool_runs.jsonl`
- [ ] add `model_events.jsonl`
- [ ] add `pending_work.jsonl`
- [ ] add `checkpoints/`
- [ ] add `memory/`

Definition of done:

- [ ] one session bootstrap call creates the full deterministic layout
- [ ] file writers are append-only where required
- [ ] session ids and timestamps are consistent across files

Verify:

- [ ] deterministic fixture tests compare generated layout
- [ ] replay can reconstruct basic session summary

### B2. CLI command skeleton

Status: `pending`

Tasks:

- [ ] implement `bco`
- [ ] implement `bco exec`
- [ ] implement `bco review`
- [ ] implement `bco resume`
- [ ] implement `bco fork`
- [ ] implement `bco providers`
- [ ] implement `bco models`

Definition of done:

- [ ] each command parses and reaches a typed internal handler
- [ ] help output is coherent
- [ ] commands do not duplicate orchestration logic in the CLI layer

Verify:

- [ ] `docker run --rm brain-cell-orchestration --help` works once CLI parser is wired
- [ ] smoke tests cover command parsing

### B3. Resume and fork semantics

Status: `pending`

Tasks:

- [ ] define session lookup flow
- [ ] define resume behavior
- [ ] define fork behavior
- [ ] define busy-session behavior
- [ ] define interrupted-session behavior

Definition of done:

- [ ] resume continues prior state
- [ ] fork clones prior context into a new session id
- [ ] user-facing behavior is documented

Verify:

- [ ] tests cover resume and fork from fixture sessions

## Phase C: Model Connectivity

### C1. Provider registry

Status: `pending`

Tasks:

- [ ] implement provider registry trait
- [ ] implement local configuration store for provider connections
- [ ] support remote provider descriptors
- [ ] support local endpoint descriptors
- [ ] define auth loading boundary

Definition of done:

- [ ] runtime can list known providers
- [ ] active connection state can be read without mutating the session

Verify:

- [ ] provider listing tests pass
- [ ] misconfigured provider state surfaces typed errors

### C2. `/connect` and `providers`

Status: `pending`

Tasks:

- [ ] define `/connect` slash-command behavior
- [ ] define `bco providers` behavior
- [ ] define connection health status model
- [ ] define reconnection policy

Definition of done:

- [ ] operator can attach or update a provider connection
- [ ] state is visible in logs and status surfaces

Verify:

- [ ] connection changes append `model_events` or dedicated provider events

### C3. `/model` and `models`

Status: `pending`

Tasks:

- [ ] define `/model` slash-command behavior
- [ ] define `bco models` behavior
- [ ] support active model switch without destroying session state
- [ ] support harness-aware recommended model view
- [ ] support fallback model selection

Definition of done:

- [ ] operator can switch current model at runtime
- [ ] the active provider/model is visible in status and transcript metadata

Verify:

- [ ] tests cover valid switch, invalid switch, and fallback selection

## Phase D: Orchestration Runtime

### D1. Blackboard and event flow

Status: `pending`

Tasks:

- [ ] define blackboard state
- [ ] define event types
- [ ] define turn lifecycle
- [ ] define queue semantics
- [ ] define next-action emission

Definition of done:

- [ ] all cells communicate through explicit shared state or events
- [ ] no hidden mutable globals drive turn logic

Verify:

- [ ] turn lifecycle tests cover submit, execute, review, complete

### D2. Core cells

Status: `pending`

Tasks:

- [ ] implement `planner-cell`
- [ ] implement `coordinator-cell`
- [ ] implement `executor-cell`
- [ ] implement `reviewer-cell`
- [ ] define specialist cell hook points

Definition of done:

- [ ] planner emits plan items
- [ ] coordinator assigns actionable work
- [ ] executor reports actions and evidence
- [ ] reviewer can accept or replan

Verify:

- [ ] replay tests cover happy path and reviewer-triggered replan

### D3. Capability and approval enforcement

Status: `pending`

Tasks:

- [ ] enforce `CapabilityPolicy`
- [ ] define approval gate model
- [ ] classify approval-required actions
- [ ] surface pending approvals in session state

Definition of done:

- [ ] restricted actions cannot execute without policy clearance
- [ ] approval state is recoverable after restart

Verify:

- [ ] tests cover denied, approved, and resumed approval flows

## Phase E: TUI MVP

### E1. Layout shell

Status: `pending`

Tasks:

- [ ] implement alternate-screen shell
- [ ] implement transcript region
- [ ] implement side/bottom status panes
- [ ] implement multiline composer
- [ ] implement compact footer

Definition of done:

- [ ] layout is readable on typical desktop terminal sizes
- [ ] no decorative UI blocks displace operational information

Verify:

- [ ] manual screenshot check on desktop and narrow terminal widths

### E2. Operator overlays

Status: `pending`

Tasks:

- [ ] `/cells`
- [ ] `/memory`
- [ ] `/approvals`
- [ ] `/resume`
- [ ] `/model`
- [ ] `/connect`

Definition of done:

- [ ] each overlay reads live runtime state
- [ ] each overlay has a clear title and escape path

Verify:

- [ ] interaction smoke tests cover command routing

### E3. Status density

Status: `pending`

Tasks:

- [ ] show objective
- [ ] show subgoal
- [ ] show selected harness
- [ ] show provider/model
- [ ] show connection health
- [ ] show approval state
- [ ] show resumed/scheduled state

Definition of done:

- [ ] an operator can understand current runtime state in one glance

Verify:

- [ ] manual operator review against the design goals

## Phase F: Harness MVPs

### F1. Generalist harness

Status: `pending`

Tasks:

- [ ] implement decomposition heuristics
- [ ] implement completion rubric
- [ ] implement preferred model hints

### F2. Coding harness

Status: `pending`

Tasks:

- [ ] implement code-edit oriented planning
- [ ] implement test/run review hooks
- [ ] implement artifact expectations

### F3. CTF harness

Status: `pending`

Tasks:

- [ ] implement recon/exploit/review decomposition
- [ ] implement evidence expectations
- [ ] implement high-signal next-step narrowing

### F4. Pentest harness

Status: `pending`

Tasks:

- [ ] implement scoped offensive workflow decomposition
- [ ] implement capability and approval emphasis
- [ ] implement reporting artifact expectations

Definition of done for Phase F:

- [ ] all four harnesses register through the same trait
- [ ] harness selection tests pass
- [ ] harness-specific model preferences are visible

## Phase G: Autonomy Hardening

### G1. Wake and retry

Status: `pending`

Tasks:

- [ ] implement manual resume wake
- [ ] implement scheduled wake primitive
- [ ] implement bounded retry policy
- [ ] implement pending-work drain loop

### G2. Memory and checkpoints

Status: `pending`

Tasks:

- [ ] checkpoint active state
- [ ] checkpoint approval state
- [ ] flush durable memory summaries
- [ ] define memory compaction or rollover policy

### G3. Failure handling

Status: `pending`

Tasks:

- [ ] define crash recovery path
- [ ] define model failover path
- [ ] define provider reconnect path
- [ ] define stale session recovery path

Definition of done for Phase G:

- [ ] interrupted sessions can recover useful state
- [ ] autonomy never bypasses approval or policy
- [ ] regression tests cover wake, retry, and restart

## Cross-Cutting Verification Checklist

- [ ] every new runtime concept has a Rust type in the right crate
- [ ] every session mutation has a corresponding log or persisted artifact
- [ ] every operator-facing command has help text and a test or smoke check
- [ ] every approval-gated action is replayable after restart
- [ ] every model switch is logged
- [ ] every harness can explain why it was selected
- [ ] Docker build stays green after every milestone
- [ ] work is committed and pushed after each meaningful slice

## Suggested Commit Cadence

- [ ] commit after contracts
- [ ] commit after session spine
- [ ] commit after CLI skeleton
- [ ] commit after model connectivity
- [ ] commit after core cells
- [ ] commit after TUI MVP
- [ ] commit after each harness MVP
- [ ] commit after autonomy hardening

