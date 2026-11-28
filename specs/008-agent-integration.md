# 008 — Agent handshake, tmux adapter, and install recipes

## Handshake contract

The core contract is file-based and works with every agent:

1. Agent: `agloss request <file>` — creates/updates the sidecar and prints the human command.
2. Human: `agloss review <file>` — reviews and signs off.
3. Agent: `agloss export <file>` or `agloss wait <file>` — receives the numbered prompt and exit code.

`wait` polls until the sidecar has a signoff whose document hash matches the current file. It exits 10 for feedback, 0 for approval, 3 on timeout, and 2 for errors.

## tmux adapter

`agloss review <file> --tmux`:

- Requires `$TMUX`; otherwise exits 2 with instructions to run the standalone command.
- Runs `tmux display-popup -E -w 90% -h 90% -T "agloss: <file>" "<current-exe> __tui <file> [options]"`.
- Captures the popup exit status. On signoff, prints the export prompt and returns 0/10. On abort, returns 1.
- If `display-popup` is unavailable, exits 2 with a clear fallback command; it does not silently open a new window.
- `--detach` uses `tmux new-window -d -n agloss "<exe> __tui <file>"` and returns 0. The agent then calls `wait`.

`__tui` is hidden from help and runs the TUI without printing the prompt.

## Install recipes

`agloss install <agent> [--dir <project>]` writes idempotent instructions:

- **claude**: `.claude/commands/agentgloss-review.md` with frontmatter:
  ```markdown
  ---
  description: Review a Markdown spec with AgentGloss
  ---
  Run `agloss request <file>`, ask the user to run `agloss review <file>`,
  then run `agloss export <file>` and address every open item.
  ```
- **codex**: an `AGENTS.md` marker block:
  ```markdown
  <!-- agentgloss:start -->
  ...instructions...
  <!-- agentgloss:end -->
  ```
  If `AGENTS.md` exists, append the block once; otherwise create it.
- **gemini**: the same marker block in `GEMINI.md`.
- **generic**: `agentgloss-agent.md` with the same instructions.

All writes create parent directories and are idempotent. Marker-block installs (`AGENTS.md`, `GEMINI.md`) preserve content outside the marker block; the dedicated Claude command file and generic recipe file are replaced. The command prints the files written.

## Tests

- Fake `tmux` executable in a temp PATH records arguments; assert popup and detached commands.
- `--tmux` without `$TMUX` exits 2.
- `wait` timeout with a short interval; feedback and approval exit codes.
- Recipe generation for all four agents; content contains request/review/export.
- Idempotency: running install twice yields one block.
- Existing `AGENTS.md`/`GEMINI.md` content preserved.
