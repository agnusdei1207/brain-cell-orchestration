# brain-cell-orchestration

Rust-first orchestration workspace for dynamic task execution across CTF, pentesting, coding, and general operator workflows.

## Vision

`brain-cell-orchestration` is intended to combine:

- the goal-oriented orchestration model of `../codex`
- the domain rigor and cell-based execution model already explored in `../pentesting`
- the autonomy and persistence mindset from `../openclaw`
- the provider-agnostic model switching UX of `../opencode`
- the TUI UI/UX cues of `../claude-code`

The core product goal is simple: infer operator intent, mount the right harness automatically, and execute with a stable orchestration spine instead of building a separate agent for each domain.

The runtime should also remain resumable and automation-ready instead of behaving like a stateless prompt shell.

The key project insight is that model quality is important, but orchestration quality matters more. Domain changes should come from swapping thin harnesses on top of one strong orchestration core.

The primary operator target is not web-only CTF. It is a terminal-native TUI system that can drive CTF, offensive security, and broader red-team workflows with the same orchestration core.

## Design Principles

- Rust only
- Docker as the primary build surface
- codex-style goal-directed execution and orchestration
- task-intent driven harness switching
- provider-agnostic model connectivity and fast runtime switching
- append-only session and evidence trail
- resumable sessions and autonomous follow-up paths
- operator-visible planning, approvals, and provenance
- interchangeable domain harnesses without rewriting the core runtime

## Workspace Layout

This is an intentional Rust workspace split, not a misplaced source tree. `apps/` contains the operator-facing binary entrypoint and `crates/` holds the reusable runtime components. In other words, the code is still in `src/`, but each crate owns its own `src/`.

```text
apps/
  bco/                 entrypoint binary
crates/
  bco-core/            domain model and intent primitives
  bco-harness/         harness registry and resolution
  bco-orchestrator/    orchestration spine and brain-cell composition
  bco-session/         session, audit, and persistence model
  bco-tui/             TUI shell contracts and UI blueprint
docs/
  README.md            docs entrypoint
  RUNBOOK.md           primary execution runbook
  philosophy/
    PHILOSOPHY.md      core project philosophy
  architecture/
    ARCHITECTURE.md    target architecture
  planning/
    PLAN.md            detailed project plan
    ROADMAP.md         phased delivery roadmap
  research/
    REFERENCE_ANALYSIS.md
                       extracted strengths and adoption rules
Dockerfile             canonical build/runtime image
Dockerfile.base        Kali-heavy base mirrored from ../pentesting
```

## Quick Start

Build with Docker:

```bash
docker build -t brain-cell-orchestration .
```

The runtime image now targets `agnusdei1207/pentesting-base:latest`, and [Dockerfile.base](/Users/pf/workspace/brain-cell-orchestration/Dockerfile.base) mirrors the heavy Kali base strategy from `../pentesting`.

Run the bootstrap binary:

```bash
docker run --rm brain-cell-orchestration
```

## Current Status

This repository has moved past the initial bootstrap stage. The current implementation now provides a working local-first CLI/TUI runtime with:

- `exec`, `review`, `resume`, `fork`, `providers`, and `models`
- local session persistence under `.bco/sessions/<session-id>`
- append-only `transcript.jsonl`, `plan.jsonl`, `orchestrator_events.jsonl`, `cell_topology.jsonl`, and `pending_work.jsonl`
- persisted approval state in `approvals.jsonl` with resume-visible waiting cells
- first-pass planner/coordinator/executor/reviewer offensive workflow orchestration
- Kali-based runtime packaging aligned with `../pentesting`

What is still incomplete is the depth of autonomous execution, not the existence of the runtime shell. The detailed build plan and product scope live in [PLAN.md](/Users/pf/workspace/brain-cell-orchestration/docs/planning/PLAN.md).

For actual build execution, use [RUNBOOK.md](/Users/pf/workspace/brain-cell-orchestration/docs/RUNBOOK.md) as the working runbook.

For the project's durable design beliefs and replacement strategy, see [PHILOSOPHY.md](/Users/pf/workspace/brain-cell-orchestration/docs/philosophy/PHILOSOPHY.md).
