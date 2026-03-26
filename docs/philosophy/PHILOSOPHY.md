# Philosophy

## Why This Project Exists

This project exists because one lesson became clear:

- model quality matters
- orchestration quality matters more

The previous CTF-oriented runtime was useful, but the observed execution quality of `codex` showed that a stronger orchestration structure can outperform a domain-specific tool that has a weaker control model.

`brain-cell-orchestration` exists to rebuild around that lesson rather than continuing to patch the old shape.

## Core Beliefs

### 1. The orchestration spine is the product

The durable advantage is not one provider, one prompt, or one narrow workflow.

The durable advantage is:

- objective tracking
- planning
- coordination
- execution control
- review and replanning
- approval handling
- persistence
- observability

If these are strong, the system stays useful even when models change.

### 2. Harnesses should be thin

Domain switching should not require building a whole new runtime.

A harness should stay thin and only control:

- domain heuristics
- decomposition style
- tool preference
- review rubric
- artifact expectations
- preferred model profile

A harness must not become a second orchestration core.

### 3. Models are replaceable

Models are important execution backends, not the architectural center.

The project should assume:

- providers will change
- pricing will change
- strong models will appear and disappear
- different tasks may prefer different models

Therefore:

- model identity must stay provider-agnostic
- switching models should be cheap
- the runtime should not be structurally coupled to one vendor

### 4. CTF comes first

Although the architecture should generalize, the first real product target is CTF competition use.

That means:

- the CTF harness is primary, not decorative
- the runtime must favor fast, goal-directed execution
- the Docker/runtime story must respect offensive tooling needs
- Kali-based runtime assumptions are justified

Pentest, coding, and generalist modes are important, but they come after the CTF-first core is strong.

### 5. Persistence is mandatory

Useful orchestration cannot behave like a disposable prompt shell.

The runtime should keep:

- session continuity
- checkpoints
- pending work
- memory summaries
- event history
- evidence and artifacts

This is required for long tasks, interruptions, retries, and post-run analysis.

### 6. Observability is part of correctness

If the runtime cannot explain:

- what it is trying to do
- which harness it selected
- which model it is using
- which cell is active
- what it plans to do next
- why it stopped or replanned

then the system is not reliable enough.

Visibility is not cosmetic. It is required for trust and debugging.

### 7. Explicit control beats hidden magic

The project should prefer:

- explicit policies
- explicit queues
- explicit messages
- explicit approval states
- explicit runtime status

over hidden heuristics that are hard to inspect or recover from.

### 8. This project is a replacement, not a sidecar

The end state is not to maintain both this repo and `../pentesting`.

The intended product transition is:

- finish `brain-cell-orchestration`
- migrate it into `../pentesting`
- remove the old Pentesting implementation
- ship the new runtime under the existing `pentesting` name on npm

This means architecture and packaging choices should be made with eventual replacement in mind, not permanent coexistence.

## What To Optimize For

Optimize for:

- better orchestration over clever prompt tricks
- stronger control-plane design over UI novelty
- thin harness swaps over duplicated runtimes
- replayability over convenience shortcuts
- operator trust over black-box behavior
- smooth eventual migration into the `pentesting` product surface

Do not optimize for:

- vendor lock-in
- a giant all-in-one harness
- decorative interfaces that hide runtime truth
- fragile one-shot workflows that cannot resume
- long-term duplication between this repo and `../pentesting`

## Practical Rule

When making a design decision, prefer the option that makes the common orchestration core stronger and the harness layer thinner.

If a change makes the harness thicker but the core weaker, it is probably the wrong direction.

