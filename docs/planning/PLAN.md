# Project Plan

## 1. Goal

Build a Rust-only orchestration platform that can execute broadly across:

- CTF workflows
- offensive security, adversary-emulation, and red-team tasks
- coding and software engineering work
- general terminal-native operator tasks

The system should not hardcode one domain runtime. Instead, it should infer intent, select the correct harness dynamically, and keep a shared orchestration backbone across all domains.

## 2. Product Thesis

Current agent tools often fail in one of two ways:

- they are generic but shallow, so domain execution quality drops
- they are domain-specialized but rigid, so the runtime cannot generalize

`brain-cell-orchestration` should solve that by separating:

- orchestration spine
- harness selection
- tool capability policy
- session continuity and evidence model
- autonomy and persistence plane
- operator-facing TUI

This lets one runtime execute many classes of work without turning into a monolith.

Core insight from this project reset:

- model quality matters, but orchestration structure matters more
- domain switching should come from thin harness replacement, not separate thick runtimes
- the stable center should be the orchestration spine
- harnesses should stay thin, swappable, and policy-driven
- the model layer should remain replaceable underneath that spine

See [PHILOSOPHY.md](/Users/pf/workspace/brain-cell-orchestration/docs/philosophy/PHILOSOPHY.md) for the durable decision rules behind this plan.

Product transition insight:

- this repository is intended to replace `../pentesting` when complete
- the old Pentesting implementation is expected to be removed
- the new runtime is expected to ship under the existing `pentesting` npm identity

Session storage insight:

- Codex orchestration is the benchmark, but not its server-backed session topology
- this project should store sessions locally where it runs
- resume, fork, replay, checkpoints, and runtime metadata should work without any server dependency

## 3. Benchmark Targets

### Codex traits to benchmark

- Rust workspace modularity
- goal-directed execution that keeps working toward task completion
- terminal-first execution loop
- strong orchestration feel around tool usage and approvals
- durable separation between app shell, runtime logic, and execution machinery
- task-oriented pluggable execution model
- dedicated orchestration control plane
- explicit inter-agent communication protocol
- hierarchical agent tree with cascade shutdown
- bounded spawn depth and other resource guardrails
- strong event-based observability
- explicit runtime service injection

### Pentesting traits to retain

- planner/coordinator/executor/reviewer cell model
- evidence-aware workflow and append-only session discipline
- operational traceability for high-risk tasks
- strong CTF/pentest task decomposition

### OpenClaw traits to absorb

- autonomy and always-on posture
- resumable session continuity
- memory flush and durable local state patterns
- wakeup, retry, pending-work, and automation primitives
- per-session actor queue serialization
- reason-aware failover policy
- hook-driven automation and session memory writeback
- post-run session metadata writeback

### OpenCode traits to absorb

- provider-agnostic model abstraction
- easy runtime provider and model switching
- lightweight connection UX for attaching local endpoints or direct provider configuration
- stable `provider/model` identity format

### Claude Code TUI cues to emulate

- dense terminal layout
- command bar first interaction
- low-noise operator surface
- transcript + status + plan visibility without excessive chrome

## 4. Reference Extraction Summary

Detailed analysis lives in [REFERENCE_ANALYSIS.md](/Users/pf/workspace/brain-cell-orchestration/docs/research/REFERENCE_ANALYSIS.md).

The short version:

- `codex` is the main orchestration reference
- `pentesting` is the main cell-runtime reference
- `openclaw` is the main autonomy and persistence reference
- `opencode` is the main model-connectivity reference
- `claude-code` is UI only

All local reference projects are expected to be available one directory up from this repo, such as `../codex`, `../opencode`, `../openclaw`, `../claude-code`, and `../pentesting`.

## 5. Implementation Environment Assumption

Primary implementation assumption:

- the project may be built interactively through `claude-code`
- the working model may be `minimax-m2.7`

Design implication:

- the plan must be explicit enough that a weaker or cheaper implementation model can still make steady progress
- abstractions must be defined before large implementation bursts
- checklists and definition-of-done criteria matter more than aspirational prose

## 6. Non-Goals For Phase 0

- no GUI or web dashboard
- no TypeScript runtime
- no provider-specific optimization first
- no multi-tenant SaaS control plane
- no premature plugin marketplace

## 7. Core Requirements

### Functional

- classify user intent into a domain and task shape
- resolve a harness automatically, with operator override
- switch model/provider without restarting the whole runtime
- execute through a common orchestration lifecycle
- support typed inter-cell communication and subtree lifecycle control
- serialize mutating work per session
- preserve a clear task objective and subgoal chain across the full run
- persist plan, evidence, approvals, and results
- support resumable long-lived sessions
- accept external triggers for wake, continue, and scheduled follow-up
- render task state in a TUI suitable for long interactive sessions
- support both interactive and headless execution modes

### System

- Rust-only source tree
- Docker-first build and packaging
- isolated crate boundaries
- deterministic session file layout
- local-first file-based session persistence
- explicit capability and approval model
- reproducible test strategy
- durable restart-safe persistence semantics
- provider and model configuration must be hot-swappable
- orchestration events must be emitted through typed queues
- runtime services must be injected through explicit construction
- retry and failover policy must be reason-aware
- post-run session metadata must be persisted

Current implementation note:

- local session artifacts now include persisted `plan.jsonl`, `transcript.jsonl`, and `approvals.jsonl`
- approval-gated offensive objectives already rehydrate as waiting sessions on `bco resume`
- local operators can now resolve approval state with `bco approve` and `bco deny`
- approved sessions already advance their persisted plan snapshot to the next step without any server-side session state
- local approval resolution also writes turn-progression transcript/event artifacts so operators can audit what changed
- local operators can also drive the next active step with `bco continue`, which may immediately reopen approval if the next step is still high risk
- local artifact-driven transitions now refresh `session_runtime.json:last_updated`, so review and resume can trust local runtime metadata without any server session
- local review output now surfaces `pending_work.jsonl`, which gives operators a clearer wake/resume picture than plan state alone
- initial `bco exec` now seeds `pending_work.jsonl` as well, so the first approval-gated step is visible immediately in local review
- the next priority is not inventing more persistence types, but connecting those persisted states to richer autonomous execution

## 8. Proposed Runtime Model

The runtime should be composed from eight layers:

1. `intent layer`
   Parse objective, context, constraints, and operator risk preference.
2. `harness layer`
   Pick or blend the domain harness: CTF, pentest, coding, or generalist.
3. `cell orchestration layer`
   Drive planner, coordinator, executor, reviewer, and specialist subcells.
4. `execution layer`
   Attach tool runners, container/runtime adapters, and provider/model backends.
5. `model connectivity layer`
   Resolve providers, local configuration, endpoints, active model, and failover preferences.
6. `autonomy layer`
   Manage retries, wakeups, scheduled work, and dormant pending-work drains.
7. `persistence layer`
   Flush checkpoints, append-only logs, and durable memory summaries.
8. `operator layer`
   Expose status, approvals, transcript, memory, and intervention controls in TUI.

Cross-cutting runtime structures:

- submission queue for inbound work
- session actor queue for per-session serialization
- event queue for outward observability
- orchestrator control plane for lifecycle actions
- service container for runtime dependencies

## 9. Harness Strategy

Each harness should define:

- domain prompt policy
- task decomposition heuristics
- evidence expectations
- tool allowlists and default safety posture
- review criteria
- completion rubric
- whether it can continue autonomously after an interruption
- preferred model profile and fallback behavior

Harness design rule:

- harnesses are thin policy layers, not separate runtimes
- they should adjust decomposition, tool preference, review style, and artifact expectations
- they should not reimplement orchestration, persistence, approval, or session management

Initial target harnesses:

- `ctf-harness`
- `pentest-harness`
- `coding-harness`
- `generalist-harness`

Later, hybrid harness composition can support flows like:

- code + exploit reproduction
- pentest + report drafting
- CTF + reverse engineering

## 10. Autonomy And Persistence

The system should explicitly model the OpenClaw-style lesson that good autonomy depends on durable state, predictable wake behavior, and bounded retries.

Required autonomy capabilities:

- manual resume of an interrupted session
- scheduled wakeup for queued work
- retry queues for transient failures
- background follow-up work such as summarization or review
- pending-work drains when fresh input or events arrive
- hook-triggered automation on session lifecycle events

Required persistence capabilities:

- append-only transcript, plan, evidence, and tool logs
- session checkpoints
- durable memory summaries
- pending-work journal
- replayable session artifacts
- persisted session metadata for model, usage, abort, and recovery state

Safety rule:

- autonomy must stay subordinate to capability policy, approval policy, and harness-specific safety posture

## 11. Model Connectivity

The runtime should adopt the OpenCode lesson that model operations must stay easy while the orchestration layer stays stable.

Required capabilities:

- provider-agnostic `provider/model` identity
- live model switching through a slash command such as `/model openai/gpt-5.4`
- live provider setup through a slash command such as `/connect openai`
- support for remote providers and local model endpoints
- model fallback policy without coupling the full runtime to one vendor
- reason-aware failover behavior

Operator UX goals:

- model switching should be fast and visible
- connection state should be inspectable in-session
- harness selection and model selection should remain separate decisions

## 12. TUI Plan

The TUI should learn from Claude Code only at the UI/UX level, not at the orchestration philosophy level.

Target layout:

- top transcript region
- right or bottom contextual panes for plan, cells, approvals, and evidence
- bottom multiline command composer
- compact footer for mode, harness, sandbox, and session status

Interaction targets:

- keyboard-first
- fast slash commands
- `/model` and `/connect` command flow for provider/model operations
- overlay panels for `/memory`, `/cells`, `/approvals`, `/sessions`
- visible harness switching and capability provenance
- explicit state for resumed, queued, or scheduled work

Planned primary slash commands:

- `/model`
- `/connect`
- `/harness`
- `/cells`
- `/memory`
- `/resume`
- `/approvals`

## 13. File and Data Model

Suggested append-only storage layout:

```text
.bco/
  sessions/<session-id>/
    session.json
    transcript.jsonl
    plan.jsonl
    approvals.jsonl
    evidence.jsonl
    tool_runs.jsonl
    orchestrator_events.jsonl
    cell_topology.jsonl
    model_events.jsonl
    session_runtime.json
    pending_work.jsonl
    checkpoints/
    memory/
    artifacts/
```

The session layer should support replay, export, cross-turn continuity, and restart-safe resume.

## 14. Command Surface Plan

Interactive command targets:

- `bco`
- `bco resume`
- `bco fork`
- `bco exec`
- `bco review`
- `bco providers`
- `bco models`

The command surface intentionally follows the Codex and OpenCode lesson that session operations and model operations should be visible at the top level.

## 15. Execution Checklist

The practical build checklist lives in [RUNBOOK.md](/Users/pf/workspace/brain-cell-orchestration/docs/RUNBOOK.md).

This file is not optional process overhead. It is part of the product plan.

Implementation rule:

- no new milestone should begin until the previous milestone's verification items are complete

## 16. Phased Delivery

### Phase 0: bootstrap

- create workspace
- create docs
- define crate boundaries
- compile a minimal binary in Docker
- define objective, autonomy, and persistence contracts
- define provider/model switching contract
- document reference extraction and adoption rules

### Phase 1: orchestration core

- intent classification
- harness registry
- operation or task trait
- orchestrator control plane
- inter-cell message protocol
- planner/coordinator/executor/reviewer runtime
- session persistence
- explicit objective tracking model
- provider registry and `provider/model` parsing
- command parser skeleton for interactive and headless modes
- lineage tracking and subtree shutdown rules
- session actor queue

### Phase 2: terminal UX

- ratatui shell
- transcript and overlays
- approvals and intervention controls
- session resume
- resumed and queued work indicators
- `/model` and `/connect` command UX
- session queue and connection health visibility

### Phase 3: domain execution

- CTF harness MVP
- coding harness MVP
- pentest harness MVP
- tool contracts and capability gates
- harness-specific preferred-model policies

### Phase 4: autonomy hardening

- wakeup and retry flow
- reviewer-driven replanning
- confidence scoring
- provenance summaries
- richer test fixtures and replay tests
- crash recovery coverage
- model failover and reconnect coverage
- spawn-depth and runaway-lane guardrail coverage
- provider reconnect and failure-class coverage

## 17. Engineering Standards

- every crate has a narrow responsibility
- runtime behavior must be observable from logs and session artifacts
- high-risk actions require explicit policy checks
- docs stay ahead of implementation for safety-critical features
- Docker build must stay green
- autonomous paths must remain auditable
- provider/model transitions must be logged and explainable

Additional standards from the reference analysis:

- avoid oversized central modules
- keep UI separate from orchestration state
- prefer explicit operator controls over magic
- keep plugin and integration surfaces out of the core until contracts are stable
- write plans and checklists so an implementation model with narrower reasoning can still execute accurately
- keep orchestration control logic separate from execution logic
- prefer typed messages and events over implicit coupling

## 18. Immediate Next Work

- implement workspace-wide shared types for task, policy, and event flow
- define harness trait and registry contract
- add session file writer and deterministic fixture tests
- add checkpoint and pending-work primitives
- define explicit objective and subgoal tracking
- define provider registry and `/model` `/connect` command contracts
- replace bootstrap stdout flow with a real command parser
- introduce `ratatui` once orchestration state shape is stable
- define top-level CLI commands `resume`, `fork`, `exec`, `review`, `providers`, and `models`
- keep [RUNBOOK.md](/Users/pf/workspace/brain-cell-orchestration/docs/RUNBOOK.md) aligned with reality after each implementation slice
