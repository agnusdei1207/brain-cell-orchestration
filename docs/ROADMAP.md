# Roadmap

## Milestone 0: Bootstrap

Exit criteria:

- workspace compiles in Docker
- README and architecture docs exist
- binary prints orchestration bootstrap summary
- reference boundaries are clear: Codex for orchestration, OpenClaw for persistence, OpenCode for model switching, Claude Code for UI

## Milestone 1: Shared Runtime Contracts

Deliverables:

- `TaskIntent`, `TaskContext`, `RiskProfile`, `CapabilityPolicy`
- `ObjectiveState`, `Subgoal`, `ProgressStatus`
- harness trait and selection policy
- event and session id model
- provider registry contract
- `provider/model` parser

## Milestone 2: Session Spine

Deliverables:

- session directory creation
- append-only JSONL writers
- replay reader
- deterministic fixture tests
- checkpoint writer
- pending-work journal
- model transition log

## Milestone 3: Cell Runtime

Deliverables:

- planner/coordinator/executor/reviewer interfaces
- blackboard state
- turn lifecycle
- reviewer-driven replan path
- explicit objective tracking across turns

## Milestone 4: Autonomy Plane

Deliverables:

- manual resume flow
- wakeup primitives
- bounded retry policy
- pending-work drain loop

## Milestone 5: Model Connectivity

Deliverables:

- provider registry
- connection profile store
- `/connect` flow
- `/model` switching flow
- local endpoint support

## Milestone 6: TUI MVP

Deliverables:

- alternate-screen terminal shell
- transcript pane
- plan and status pane
- command composer
- overlays for memory and approvals
- resumed and queued state indicators
- model/provider status line

## Milestone 7: Harness MVPs

Deliverables:

- coding harness
- CTF harness
- pentest harness
- generalist fallback harness

## Milestone 8: Hardening

Deliverables:

- structured logging
- report export
- replay test corpus
- benchmark scenarios comparing harness behavior
- crash recovery tests
- autonomy regression scenarios
- provider failover and reconnect tests
