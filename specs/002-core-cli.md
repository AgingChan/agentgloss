# 002 — Core CLI, config, and sidecar storage

## Commands

```
agentgloss review <file> [--keys vim|standard] [--no-mouse]
                       [--protocol auto|kitty|iterm2|sixel|halfblocks]
                       [--theme auto|dark|light] [--sidecar <path>]
                       [--tmux] [--detach]
agentgloss request <file>
agentgloss wait <file> [--timeout <secs>] [--interval <millis>]
agentgloss export <file> [--format prompt|json] [--all]
agentgloss status <file> [--json]
agentgloss resolve <file> <id>...
agentgloss install <claude|codex|gemini|generic> [--dir <dir>]
agentgloss __tui <file> [review options]   # hidden tmux entry point
```

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Approved, no open feedback; or a read-only command succeeded |
| 10 | Signoff with open feedback (agent should act) |
| 1 | User aborted without signoff |
| 2 | Error: missing file, invalid input, tmux unavailable, corrupt state |
| 3 | `wait` timeout |

## Sidecar

- Default path: `<file>.review.json` beside the Markdown file.
- Lock: `<sidecar>.lock`, exclusive via `fs2`; a second writer gets a clear error.
- Writes are atomic: temp file in the same directory, flush, rename.
- Corrupt JSON is moved to `<sidecar>.bak.<unix-timestamp>` and reported. `review`/`request` may start a fresh store; read-only commands exit 2.
- The sidecar stores a schema version. Unknown future versions are rejected with a clear message.

## Config

On Linux the config is `~/.config/agentgloss/config.toml`; on macOS it is `~/Library/Application Support/agentgloss/config.toml`. The file is optional:

```toml
keys = "vim"            # vim | standard
mouse = true
protocol = "auto"       # auto | kitty | iterm2 | sixel | halfblocks
theme = "auto"          # auto | dark | light
mermaid_engine = "auto" # auto | rust | mmdc | source
cache_dir = "~/.cache/agentgloss"
```

CLI flags override config. Unknown keys warn, never crash. No network.

## Behavior

- `review`: validate the file, load/recover the sidecar, run the TUI. On signoff write state and print the export prompt; exit 0/10. On abort autosave and exit 1. `--tmux` delegates to the tmux adapter; without tmux it exits 2 with instructions. `--detach` starts a detached tmux window and returns 0.
- `request`: create the sidecar if needed, stamp `requested_at`, print the exact command the human should run and the export command for the agent. Exit 0.
- `wait`: poll until signoff or timeout. A signoff is valid only when its document hash matches the current file. Feedbacks exit 10; approval exits 0; timeout exits 3.
- `export`: print the prompt or JSON. Exit 10 if open or stale items exist, otherwise 0. `--all` includes resolved items.
- `status`: human summary or JSON with round, counts, approval, requested timestamp. Human/JSON status also reports `changed` when the document hash differs from the sidecar and `invalidated` when a stored approval no longer matches.
- `resolve`: load and validate the Markdown file, reconcile the store against the current revision, validate IDs, mark resolved with the current round, and save. A missing file or unknown ID exits 2.
- `install`: see spec 008.

## Tests

- Clap parsing for every command and flag.
- Sidecar round-trip, atomic replacement, lock contention, corrupt backup.
- `request` → `status` → `resolve` → `status` flow.
- Exit codes for `export`, `wait`, and errors.
- Config precedence: defaults < config file < CLI.
