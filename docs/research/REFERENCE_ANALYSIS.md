# Reference Analysis

## Purpose

This document extracts the strongest design ideas from the local reference projects and turns them into explicit adoption rules for `brain-cell-orchestration`.

Reference boundaries:

- `codex`: goal-directed orchestration, sessioned CLI shape, approval and sandbox posture
- `pentesting`: cell-based domain rigor, evidence discipline, offensive workflow decomposition
- `openclaw`: autonomy, persistence, wakeup, plugin/memory orientation
- `opencode`: provider-agnostic model operations, agent surface, client/server separation
- `claude-code`: terminal UX cues only

## Analysis Method

Sources inspected:

- repo READMEs and vision/contributing docs
- top-level workspace structure and crate/package layout
- installed CLI surfaces via `--help`
- limited executable metadata using `file` and `otool`

Binary-level note:

- `codex` and `opencode` on this machine resolve to Node launchers, so source and CLI inspection were more useful than reverse engineering.
- `claude` resolves to a native Mach-O executable. Without source, the useful analysis surface here is CLI behavior, linked libraries, and user-visible command model. Low-level string extraction was not high-signal enough to justify deeper reversing at this stage.

## Codex

### Observed strengths

- Strong goal-directed CLI posture with interactive and headless paths in one product surface.
- Clear session lifecycle with `resume`, `fork`, `exec`, `review`, `mcp`, and `app-server`.
- Explicit approval and sandbox policy at runtime rather than hidden behavior.
- Rust workspace decomposition with narrow crates and clean separation of concerns.
- Operator-visible remote/app-server surface, which implies the TUI is not the only future client.
- Task-oriented execution abstraction instead of one giant hardcoded loop.
- Dedicated agent control plane separated from task execution logic.
- Explicit inter-agent message protocol rather than hidden direct coupling.
- Hierarchical agent tree with parent-child ownership and cascade shutdown semantics.
- Resource guardrails such as spawn depth limits and safe shared ownership patterns.
- Strong service injection pattern for models, exec, hooks, and other runtime services.
- Event-first observability so orchestration steps can be rendered and replayed.

### Adopt directly

- Objective-first orchestration spine.
- Session semantics: resume, fork, headless exec, review mode.
- Strong policy model for sandbox, approval, and capability gating.
- Crate-per-responsibility architecture.
- Future app-server boundary so TUI does not own all runtime logic.
- Task or operation trait for pluggable run types.
- Explicit orchestration control plane object.
- Message-based cell communication protocol.
- Agent tree ownership model.
- Event queue and submission queue separation.
- Service injection for runtime dependencies.

### Adapt, not copy

- Do not copy Codex command names blindly.
- Keep the orchestration philosophy, but orient it toward dynamic harness routing for multi-domain work.

### Concrete implications

- `bco-orchestrator` should own objective state, subgoal chain, and replanning.
- `bco-session` should support resume and fork as first-class operations.
- `apps/bco` should eventually expose interactive, exec, review, resume, and fork subcommands.
- `bco-orchestrator` should expose a control-plane type analogous to `AgentControl`.
- cells should communicate through typed messages or events, not direct internal mutation only.
- the runtime should track parent-child cell lineage and support subtree shutdown.
- the runtime should emit typed orchestration events for UI and replay.

## Pentesting

### Observed strengths

- Planner/coordinator/executor/reviewer cell model is already aligned with the target runtime.
- Append-only evidence model is appropriate for security-sensitive workflows.
- Domain decomposition is practical rather than abstract.
- TUI already favors dense operational visibility over decorative UI.

### Adopt directly

- Cell topology as the default execution skeleton.
- Evidence and artifact discipline.
- Domain-specific harness heuristics for CTF and pentest paths.

### Adapt, not copy

- Avoid making the whole runtime security-only.
- Generalize the cell runtime so coding and general engineering can use the same spine.

### Concrete implications

- Base cell set should remain `planner`, `coordinator`, `executor`, `reviewer`.
- Specialist cells should be harness-driven, not globally mandatory.
- Every high-risk harness should emit evidence entries and review checkpoints.

## OpenClaw

### Observed strengths

- Strong product stance around always-on usefulness.
- Core stays lean while optional capability is pushed outward.
- Memory, hooks, automation, and wakeup behavior are treated as product primitives.
- Flexible connector mindset without collapsing all capability into the core.
- Clear bias toward operator-controlled power rather than hidden autonomy.
- ACP control plane exists as a distinct runtime management layer.
- Per-session actor queue serialization prevents the same session from racing itself.
- Failure handling is explicitly reason-aware rather than hidden behind a generic retry loop.
- Failover policy is reason-aware rather than a single generic retry path.
- Session store writeback captures runtime model, usage, compaction, and abort state.
- Hook system turns automation into an explicit evented extension point.
- Session memory capture converts completed sessions into durable workspace memory artifacts.
- Resume and spawn semantics exist even across ACP and embedded runtime boundaries.

### Adopt directly

- Autonomy plane with wakeups, retries, and pending-work drains.
- Durable memory and checkpoint concepts.
- Lean core plus pluggable integrations.
- Safe defaults with visible high-power overrides.
- Per-session serialized actor queue for mutating operations.
- Reason-aware failover and retry categorization.
- Local-first runtime state with durable recovery and no server dependency.
- Evented hooks for automation and memory flushes.
- Session-store writeback after runs.

### Adapt, not copy

- Avoid becoming a messaging-channel platform.
- Avoid an oversized plugin surface before the runtime contracts stabilize.
- Keep autonomy subordinate to orchestration and policy.
- Avoid importing ACP wholesale before local contracts stabilize.

### Concrete implications

- Add a future `bco-autonomy` crate for wakeups and scheduled work.
- Add a future `bco-memory` crate for durable summaries and retrieval.
- Introduce checkpoint files and pending-work journals early.
- Add per-session queueing so one session cannot interleave conflicting mutations.
- Add failure reason taxonomy rather than one generic retry bucket.
- Persist post-run session metadata such as active model, token usage, abort status, and compaction count.
- Add hook points for session reset, resume, and summarization events.

## OpenCode

### Observed strengths

- Provider-agnostic model posture is explicit and central.
- CLI surface clearly separates `providers`, `models`, `serve`, `attach`, `session`, and `agent`.
- Model identity uses a stable `provider/model` shape.
- Runtime includes both TUI and client/server behavior.
- Agent definitions can carry model and permission profiles.
- Workspace/control-plane split keeps the terminal UI from being the only client.
- Event bus and server routes indicate the runtime is built to be remotely driven.
- Built-in agent modes encode permission posture directly into agent definitions.
- System prompt varies by model family, which is a practical provider/model adaptation layer.

### Adopt directly

- `provider/model` as the canonical model identifier.
- First-class `/model` and `/connect` UX.
- Provider registry and connection profiles.
- Harness policy and model policy as separate layers.
- Future attach/serve flow so frontends are not tightly coupled to local execution.

### Adapt, not copy

- Keep model switching simple and operationally visible.
- Avoid overly broad agent taxonomy at bootstrap.

### Concrete implications

- Add future `bco-models` crate for provider registry and active model state.
- Log model/provider transitions in session artifacts.
- Support local endpoints and remote providers under one abstraction.

## Claude Code

### Observed strengths

- Compact terminal-first UI.
- Strong command-line surface for session continuation, agents, permissions, and tools.
- Operator-facing knobs are visible instead of hidden in a GUI.
- Native executable packaging gives a tight end-user install and startup experience.
- Session-oriented flags show a strong preference for resumable, explicitly configurable work.

### Adopt directly

- Dense transcript-first layout.
- Multiline bottom composer.
- Minimal chrome with high information density.
- Strong slash-command culture.

### Do not adopt

- Claude Code is not the orchestration reference.
- The runtime philosophy should stay anchored in Codex plus the local pentesting runtime.

### Concrete implications

- `bco-tui` should emulate the feel of speed and density, not the product architecture.

## Extracted Design Rules

### Rule 1: Objective is always explicit

At any point, the runtime should be able to show:

- current objective
- active subgoal
- selected harness
- selected model
- next action

### Rule 2: Harness and model are separate decisions

- Harness decides domain behavior and safety posture.
- Model layer decides backend selection, local configuration, and failover.

### Rule 3: Autonomy never bypasses policy

- Wakeups and retries can continue work.
- They cannot silently cross approval or capability boundaries.

### Rule 4: TUI is a client, not the runtime

- The TUI is important, but orchestration should remain accessible through headless and future remote surfaces.

### Rule 5: Evidence and provenance are required

- High-risk or multi-step execution must be reconstructible from session artifacts.

### Rule 6: One session must not race itself

- Mutating work for the same session should be serialized through a session actor queue or equivalent.

### Rule 7: Retry logic must be reason-aware

- Rate limits, overload, provider failure, permanent config errors, and model-not-found should not all behave the same.

### Rule 8: Post-run state must write back to session metadata

- Active model, usage, abort status, and compaction or recovery counters should survive the run.

## Implementation Checklist Derived From Analysis

- Add `ObjectiveState`, `Subgoal`, and `NextAction` types.
- Add a pluggable task or operation trait for run types.
- Add an orchestrator control-plane type.
- Add typed inter-cell communication messages.
- Add parent-child cell lineage and subtree shutdown semantics.
- Add spawn-depth or lane-depth guardrails.
- Add explicit runtime service container or dependency injection boundary.
- Add submission queue and event queue concepts.
- Add per-session serialized mutation queue.
- Add failover reason taxonomy and reason-aware retry policy.
- Add session metadata writeback for model, usage, abort, and recovery state.
- Add hook points for reset, resume, summarization, and memory flush workflows.
- Define `Harness` trait with policy and preferred model profile hooks.
- Define `ProviderRef` and `ModelRef` using `provider/model`.
- Add `model_events.jsonl`, `pending_work.jsonl`, and checkpoint persistence.
- Plan CLI commands for `exec`, `review`, `resume`, `fork`, `providers`, and `models`.
- Add slash commands `/model`, `/connect`, `/memory`, `/cells`, `/resume`, `/approvals`.
- Keep future server mode in scope even if bootstrap remains local-first.
