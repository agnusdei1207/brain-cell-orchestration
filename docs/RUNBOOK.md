# Runbook

## Purpose

This document is the execution checklist for building `brain-cell-orchestration`.

It is written for practical implementation in a terminal-first workflow, including the case where the primary implementation agent is `claude-code` using `minimax-m2.7`.

Success criterion:

- a contributor should be able to open this file, work top to bottom, and know what to build, what done means, and what to verify before moving on

Instruction for implementation agents:

- treat this as the primary document
- use other docs only when this runbook references them

## Working Rules

- complete one checkpointed slice at a time
- do not start broad UI work before state contracts stabilize
- do not add integrations before the core traits and logs exist
- every milestone must end with a commit and push
- every item marked `verify` must be checked before proceeding

## Current Repo Audit

This section records what appears to be implemented already versus what is still only planned.

### Confirmed implemented now

- [x] Rust workspace exists
- [x] top-level CLI subcommands exist for `exec`, `review`, `resume`, `fork`, `providers`, `models`
- [x] objective and risk-related core types exist
- [x] harness contract types and placeholder harnesses exist
- [x] orchestrator crate contains first-pass control-plane and cell identity types
- [x] session layout and runtime metadata types exist
- [x] TUI layout and status types exist
- [x] current Debian-based Docker build still compiles

### Confirmed missing or incomplete now

- [x] real inter-cell message bus is not yet fully implemented (MessageBus added, wired to OrchestratorRuntime)
- [x] real submission queue and event queue are not yet wired end-to-end (SubmissionQueue wired, handle_operator_input implemented)
- [x] session actor queue is not yet implemented (SessionActorQueue added with enqueue/dequeue)
- [x] reason-aware failover is not yet implemented in Rust runtime (ModelManager.handle_model_failure with reason classification, retry delays)
- [x] auth profile rotation and cooldown system is not yet implemented (AuthProfile, AuthCredentials, AuthProfileState, AuthRotationPolicy, AuthRotationManager with cooldown tracking)
- [x] hook-driven automation and memory flushes are not yet implemented (HookRegistry, HookEvent, HookAction, CheckpointManager.trigger_hooks with MemoryFlushPolicy automation)
- [x] subtree shutdown and lineage persistence need verification in code, not just types (Blackboard::shutdown_subtree recursively marks cells Cancelled, lineage tracked in BlackboardState)
- [x] session writeback files and append-only logs need implementation verification (SessionBootstrap writes session_runtime.json, append_jsonl for all log files)
- [x] CTF harness is still placeholder-level, not competition-ready (now has challenge-type classification, tool hints, artifact expectations per type)
- [x] current main Dockerfile is not yet aligned with the requested Kali runtime path

### Local worktree note

- [x] untracked files currently exist: `Dockerfile.base`, `test.sh`
- [x] these must be reviewed before merge, not blindly adopted
- [x] `Dockerfile.base` currently contains a likely typo: symlink uses `bcargo` instead of `cargo`

## Pattern Audit

This section is the explicit audit for what to copy and what not to copy from references.

### Must copy from Codex

- [x] goal-directed orchestration as the central runtime philosophy
- [x] task or operation abstraction instead of one giant loop
- [x] dedicated control plane separate from execution logic
- [x] typed inter-agent or inter-cell communication contract in the plan
- [x] parent-child lineage and subtree shutdown in the plan
- [x] resource guardrails such as spawn-depth limits in the plan
- [x] service injection or runtime services boundary in the plan
- [x] event-first observability in the plan
- [ ] all of the above must also be verified in implementation, not just docs

### Must copy from OpenClaw

- [x] autonomy and persistence as product primitives
- [x] pending-work, wakeup, retry, and resume concepts in the plan
- [x] hook-based automation in the plan
- [x] durable memory flush concepts in the plan
- [x] per-session serialization requirement in the plan
- [x] reason-aware failover requirement in the plan
- [x] auth profile state and cooldown requirement in the plan
- [x] session metadata writeback requirement in the plan
- [ ] all of the above must also be verified in implementation, not just docs

### Must copy from OpenCode

- [x] `provider/model` identity
- [x] `/model` and `/connect`
- [x] provider registry and model switching separation
- [x] top-level provider and model command surface

### Must copy from Claude Code

- [x] TUI density and terminal-first UX cues only
- [x] visible operator controls
- [ ] do not use Claude Code as orchestration reference

### Must not copy

- [ ] do not copy Codex command names blindly when the domain semantics differ
- [ ] do not copy OpenClaw's messaging-channel product scope
- [ ] do not copy OpenClaw's plugin breadth before core contracts stabilize
- [ ] do not copy OpenCode's entire agent taxonomy before core runtime matures
- [ ] do not copy Claude Code's runtime philosophy; only copy UI/UX cues

## CTF And Kali Runtime Requirement

Primary product target:

- the runtime is expected to be used mainly for CTF competition workflows

Implications:

- the `ctf-harness` is not optional or later-only decoration
- Docker runtime planning must optimize for CTF and offensive tooling compatibility
- Kali-based runtime packaging should be treated as a first-class requirement

Current state:

- [x] reference runtime exists in `../pentesting/Dockerfile`
- [x] local repo has an untracked `Dockerfile.base` intended to move toward Kali
- [ ] current tracked `Dockerfile` is still `rust:1.90-bookworm -> debian:bookworm-slim`
- [ ] the tracked runtime is therefore not yet aligned with the CTF-first Kali requirement

Required adoption from `../pentesting`

- [ ] use the Kali-based runtime approach as the canonical runtime direction
- [ ] keep builder/runtime split explicit
- [ ] document why Kali is required for the `ctf-harness`
- [ ] validate the local `Dockerfile.base` before adopting because it currently appears unfinished

## Execution Order

- Phase A: contracts first
- Phase B: persistence and CLI spine
- Phase C: model connectivity
- Phase D: orchestration runtime
- Phase E: TUI MVP
- Phase F: harness MVPs
- Phase G: autonomy hardening
- Phase H: CTF-first Kali runtime alignment

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

### A4. Operation and control-plane contract

Status: `pending`

Tasks:

- [ ] define pluggable operation or task trait
- [ ] define orchestrator control-plane type
- [ ] define typed execution context
- [ ] define typed interruption or abort path
- [ ] define parent-child cell identity model

Definition of done:

- [ ] runtime can represent different run types without one monolithic loop
- [ ] control-plane actions are separated from cell business logic
- [ ] parent-child lineage is explicit in types

Verify:

- [ ] tests cover spawn, interrupt, and subtree shutdown

### A5. Messaging and observability contract

Status: `pending`

Tasks:

- [ ] define inter-cell message type
- [ ] define delivery mode such as queue-only vs trigger-now
- [ ] define submission queue type
- [ ] define event queue type
- [ ] define core orchestration event categories

Definition of done:

- [ ] cells can communicate without direct hidden coupling
- [ ] orchestration activity can be emitted for UI and replay

Verify:

- [ ] event fixtures cover cell spawn, interaction, approval, and model switch

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
- [ ] add `orchestrator_events.jsonl`
- [ ] add `cell_topology.jsonl`
- [ ] add `model_events.jsonl`
- [ ] add `session_runtime.json`
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

### B4. Session serialization

Status: `pending`

Tasks:

- [ ] add session actor queue keyed by session id
- [ ] route mutating session operations through the queue
- [ ] track pending count per session
- [ ] expose queued state to runtime status

Definition of done:

- [ ] one session cannot race itself on reset, resume, writeback, or turn mutation
- [ ] queue state can be inspected for observability

Verify:

- [ ] tests cover overlapping session operations

## Phase C: Model Connectivity

### C1. Provider registry

Status: `pending`

Tasks:

- [ ] implement provider registry trait
- [ ] implement local configuration store for provider connections
- [ ] support remote provider descriptors
- [ ] support local endpoint descriptors
- [ ] define auth loading boundary
- [ ] define auth profile state and cooldown model

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
- [ ] support reason-aware failover behavior

Definition of done:

- [ ] operator can switch current model at runtime
- [ ] the active provider/model is visible in status and transcript metadata

Verify:

- [ ] tests cover valid switch, invalid switch, and fallback selection
- [ ] tests cover rate-limit vs auth vs model-not-found behavior

## Phase D: Orchestration Runtime

### D1. Blackboard and event flow

Status: `pending`

Tasks:

- [ ] define blackboard state
- [ ] define event types
- [ ] define turn lifecycle
- [ ] define queue semantics
- [ ] define next-action emission
- [ ] define lineage updates

Definition of done:

- [ ] all cells communicate through explicit shared state or events
- [ ] no hidden mutable globals drive turn logic

Verify:

- [ ] turn lifecycle tests cover submit, execute, review, complete
- [ ] event queue tests cover emitted orchestration events

### D2. Core cells

Status: `pending`

Tasks:

- [ ] implement `planner-cell`
- [ ] implement `coordinator-cell`
- [ ] implement `executor-cell`
- [ ] implement `reviewer-cell`
- [ ] define specialist cell hook points
- [ ] define subtree shutdown behavior

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

### D4. Runtime services

Status: `pending`

Tasks:

- [ ] define runtime service container
- [ ] inject model manager
- [ ] inject exec manager
- [ ] inject tool registry
- [ ] inject session store
- [ ] inject policy evaluator

Definition of done:

- [ ] orchestrator construction is explicit
- [ ] tests can replace services with doubles or fakes

Verify:

- [ ] at least one orchestrator test uses mocked services

### D5. Runtime writeback

Status: `pending`

Tasks:

- [ ] persist active model after a run
- [ ] persist token or usage summary after a run
- [ ] persist abort status after a run
- [ ] persist compaction or recovery counters after a run

Definition of done:

- [ ] session runtime metadata survives restart
- [ ] operators can inspect recent runtime state without replaying full logs

Verify:

- [ ] tests cover writeback after success, abort, and failover

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
- [ ] define challenge-type classification
- [ ] define artifact expectations for flags, exploit notes, and scripts
- [ ] define CTF-first tool and runtime assumptions

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
- [ ] `ctf-harness` is clearly treated as a primary target, not a placeholder

## Phase G: Autonomy Hardening

### G1. Wake and retry

Status: `pending`

Tasks:

- [ ] implement manual resume wake
- [ ] implement scheduled wake primitive
- [ ] implement bounded retry policy
- [ ] implement pending-work drain loop
- [ ] implement reason-aware retry classification

### G2. Memory and checkpoints

Status: `pending`

Tasks:

- [ ] checkpoint active state
- [ ] checkpoint approval state
- [ ] flush durable memory summaries
- [ ] define memory compaction or rollover policy
- [ ] define session lifecycle hook points for memory flush

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

## Phase H: CTF-First Kali Runtime Alignment

### H1. Runtime base decision

Status: `pending`

Tasks:

- [ ] compare tracked `Dockerfile` against `../pentesting/Dockerfile`
- [ ] decide canonical builder image
- [ ] decide canonical Kali runtime image
- [ ] document why CTF workflows require Kali runtime support

Definition of done:

- [ ] runtime packaging direction is explicit
- [ ] the team is no longer split between Debian runtime and Kali runtime assumptions

Verify:

- [ ] decision is reflected in docs and Dockerfiles

### H2. Local Dockerfile audit

Status: `pending`

Tasks:

- [ ] review `Dockerfile.base`
- [ ] review `Dockerfile.builder`
- [ ] review `test.sh`
- [ ] confirm whether these files are valid, stale, or partial work
- [ ] fix or discard them explicitly instead of leaving them ambiguous

Definition of done:

- [ ] no critical Docker path remains untracked and unexplained
- [ ] runtime entrypoint expectations are clear

Verify:

- [ ] Docker build path chosen by the project is reproducible

## Cross-Cutting Verification Checklist

- [ ] every new runtime concept has a Rust type in the right crate
- [ ] every session mutation has a corresponding log or persisted artifact
- [ ] every operator-facing command has help text and a test or smoke check
- [ ] every approval-gated action is replayable after restart
- [ ] every model switch is logged
- [ ] every harness can explain why it was selected
- [ ] every spawned cell has explicit parentage
- [ ] every orchestrator lifecycle action emits an event
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
