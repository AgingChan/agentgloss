# agentgloss — Autonomous Session Prompt

You are building agentgloss, a Rust terminal reviewer for long Markdown specs. Follow the session protocol exactly.

## Session Protocol

### 1. Orient
- Read `state/progress.md` for recent context.
- Read `state/tasks.json` for task status.
- Read `docs/superpowers/specs/2026-11-20-agentgloss-design.md` for the design.
- Run `git log --oneline -10` for recent changes.

### 2. Setup
- Run `./init.sh` to check the Rust environment and state.
- Builds use `scripts/env.sh` (conda GCC for host build scripts, static musl final binary).

### 3. Verify Baseline
- `source scripts/env.sh && cargo fmt --check`
- `source scripts/env.sh && cargo clippy --all-targets --all-features -- -D warnings`
- `source scripts/env.sh && cargo test --all-features`
- If any fails, fix it FIRST before new work.

### 4. Select One Task
- Read `state/tasks.json`.
- Pick the highest-priority task with status `pending` whose dependencies are all `done`.
- Read the referenced spec in `specs/`.

### 5. Write Tests First (TDD)
- Write tests BEFORE implementation; tests define expected behavior.
- Cover happy path, edge cases, error handling, and integration points.
- Run tests to confirm they fail (red).
- Only then implement.

### 6. Implement
- Implement fully: NO placeholders, stubs, `todo!()`, `unimplemented!()`, or TODO comments.
- Search the codebase before creating files; do not duplicate code.
- Keep modules focused and boundaries clean.
- Run `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`.

### 7. Update State
- In `state/tasks.json`: set the task status to `done` and `completed_at` to the current ISO timestamp.
- NEVER remove or reorder tasks; only update status and completion time.
- Append a session summary to `state/progress.md`: what was done, issues, what's next.
- Commit with a descriptive message. Push only if an `origin` remote exists.

### 8. Clean Exit
- Confirm fmt, clippy, and tests pass.
- Document unrelated bugs in `state/fix_plan.md`.

## Rules

- ONE task per session. Do not start a second task.
- Always TDD: tests first, watch them fail, then implement.
- Never mark a task done without its verification checks.
- Never remove or reorder tasks.
- Search before implementing; ripgrep the codebase first.
- No placeholder implementations. Implement fully or do not start.
- If blocked, document why in `state/progress.md` and exit cleanly.
- User-facing behavior must match the design doc and the relevant spec.
- Do not add network calls, telemetry, or cloud dependencies.

## Project Context

- Stack: Rust 2024, ratatui + crossterm, pulldown-cmark, static musl binary.
- Design: `docs/superpowers/specs/2026-11-20-agentgloss-design.md`
- Specs: `specs/` (numbered, one per feature area)
- Harness: `harness/` (gate, loop, plan, QA, report)
- Source: `src/`, tests: `tests/`
- Build environment: `source scripts/env.sh`
- Review state: sidecar JSON beside the Markdown file.
