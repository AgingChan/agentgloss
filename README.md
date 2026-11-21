# mdreview

A terminal reviewer for long Markdown specs and plans produced by coding agents.

`mdreview` renders Markdown (including local images and Mermaid diagrams), lets you attach comments and suggested edits to exact source lines or paragraphs, and returns a numbered, paste-ready review to Claude Code, Codex CLI, Gemini CLI, OpenCode, Aider, or any other agent.

> Working name: it collides with an existing Python/Textual project called mdreview. The binary and project name may change before release.

## Status

MVP implemented and tested:

- 61 library tests, 15 CLI integration tests, 25 TUI integration tests, 1 performance test
- `cargo fmt`, `cargo clippy -D warnings`, and `cargo test --all-features` pass
- PTY smoke test in tmux renders the document, Mermaid, and images; `q` quits and restores the terminal
- Static musl binary

## Features

- Rendered Markdown with a raw-source toggle
- Line, paragraph, and block selection
- Comments and suggested edits
- Multi-round reviews: unresolved items carry forward, re-anchor by quote and context, and become stale when they cannot be found
- Sidecar JSON persistence with atomic writes, locking, and corrupt-state recovery
- Numbered prompt export plus JSON export
- Outline, search, help, and review panels
- Vim modal keys, standard keys, and mouse
- Local images, SVG rasterization, and Mermaid (Rust renderer, optional mermaid-cli)
- Standalone, agent handshake, and tmux launch paths
- Agent install recipes for Claude Code, Codex, Gemini, and generic CLIs

## Build

The project targets a static `x86_64-unknown-linux-musl` binary. On this machine, Rust and a host linker are provided by:

- Rust 1.100 + `rustfmt`, `clippy`, and the musl target
- a conda environment named `rustbuild` with GCC, used only for host build scripts/proc macros
- `rust-lld` for the final static link

```sh
source scripts/env.sh
cargo build --release
./target/x86_64-unknown-linux-musl/release/mdreview --version
```

If you already have a working C linker, you can remove `.cargo/config.toml` and build normally. The generated binary is static:

```sh
file target/x86_64-unknown-linux-musl/release/mdreview
# ELF 64-bit LSB pie executable ... static-pie linked
```

## Quick start

```sh
# Review in your terminal
mdreview review docs/specs/checkout.md

# If the agent needs to request a review first
mdreview request docs/specs/checkout.md
# ... the human runs:
mdreview review docs/specs/checkout.md
# ... then the agent runs:
mdreview export docs/specs/checkout.md
```

Inside tmux:

```sh
mdreview review docs/specs/checkout.md --tmux
mdreview review docs/specs/checkout.md --detach   # agent polls with wait
```

## Commands

| Command | Purpose |
| --- | --- |
| `review <file>` | Open the TUI |
| `request <file>` | Mark a review requested and print the handoff commands |
| `wait <file>` | Block until signoff, then print the review |
| `export <file>` | Print the numbered prompt or JSON |
| `status <file>` | Show round, counts, request, and approval |
| `resolve <file> <ids...>` | Mark items resolved |
| `install <agent>` | Write Claude/Codex/Gemini/generic integration recipes |
| `__tui <file>` | Hidden tmux entry point |

Common flags: `--keys vim|standard`, `--no-mouse`, `--protocol auto|kitty|iterm2|sixel|halfblocks`, `--theme auto|dark|light`, `--sidecar <path>`, `--tmux`, `--detach`, `--format prompt|json`, `--all`. `--sidecar` is accepted by `review`, `request`, `wait`, `export`, `status`, and `resolve`.

Exit codes: `0` approved/no open items, `10` open feedback, `1` aborted, `2` error, `3` wait timeout.

## Review workflow

1. The agent writes a spec and runs `mdreview request <file>`.
2. You run `mdreview review <file>` (separate pane, or the agent opens a tmux popup).
3. Select text or a block, press `c` for a comment or `s` for a suggested edit, then `Ctrl+Enter` to save.
4. `Ctrl+Enter` again opens signoff. A clean review is approved; otherwise the review is sent to the agent.
5. The agent runs `mdreview export <file>` or `mdreview wait <file>`, addresses the numbered items, and runs `mdreview resolve <file> <ids>`.
6. Reopen the review after the revision. Open items re-anchor; stale items remain visible for reattachment.

### Agent recipes

```sh
mdreview install claude    # .claude/commands/mdreview-review.md
mdreview install codex     # AGENTS.md marker block
mdreview install gemini    # GEMINI.md marker block
mdreview install generic   # mdreview-agent.md
```

## Keys

Vim mode is the default. `F2` switches between Vim and standard input.

| Action | Vim | Standard |
| --- | --- | --- |
| Move | `j`/`k` | Up/Down |
| Half page | `Ctrl-d`/`Ctrl-u` | PgUp/PgDn |
| Top/bottom | `gg`/`G` | Home/End |
| Headings | `]]`/`[[` | `Ctrl-]`/`Ctrl-[` |
| Paragraphs | `}`/`{` | `Ctrl-Down`/`Ctrl-Up` |
| Select | `v`, `V`, `P` | Shift+arrows, Shift+Home/End, `Ctrl-p` |
| Comment / suggestion | `c` / `s` | `c` / `s` |
| Search | `/`, `n`/`N` | `Ctrl-f`, `n`/`N` |
| Resolve / reopen / edit | `x` / `X` / `e` | `x` / `X` / `e` |
| Delete item | `d` | Delete |
| Sign off | `Ctrl+Enter` | `Ctrl+Enter` |
| Toggle view | `F3` | `F3` |
| Focus panes | `Tab` | `Tab` |
| Help | `?` | `F1` |
| Quit | `q` | `q` |

Mouse: click to place the cursor, drag to select, wheel to scroll, click outline/review items, and use the footer action buttons.

## Media and terminals

| Terminal | Result |
| --- | --- |
| Kitty, Ghostty | Kitty graphics |
| WezTerm | iTerm2 graphics (Kitty-capable too) |
| Windows Terminal (WSL2) | Sixel when the terminal build supports it |
| foot, mlterm, xterm | Sixel |
| tmux 3.2a | half-blocks or Kitty placeholders with passthrough; no Sixel |
| Terax (xterm.js) | half-blocks |
| Alacritty | half-blocks |

Protocol detection is environment-based and can be overridden with `--protocol`. The interactive stdio query was intentionally removed: its reader thread stole keyboard input during PTY testing.

Images are local-only. Remote URLs render as links. Mermaid uses the compiled Rust renderer; `mermaid-cli` is optional via `mermaid_engine = "mmdc"` (or automatic fallback when the Rust parser fails and `mmdc` is on PATH). Media decoding and encoding run on worker threads; the TUI shows a placeholder until the result arrives.

## Sidecar

The sidecar lives at `<file>.review.json`, with a lock at `<file>.review.json.lock`. It stores:

- schema version, file, document hash, round
- request timestamp, last signoff, and approval bound to the content hash
- items with stable `cN` IDs, status, body, suggestion original/replacement, history
- anchors with heading path, block index, quote, prefix/suffix context, byte/line range, and document hash

Writes are atomic. Corrupt JSON is backed up to `<sidecar>.bak.<timestamp>`. Unknown schema versions are rejected without overwriting.

## Configuration

Optional `~/.config/mdreview/config.toml`:

```toml
keys = "vim"            # vim | standard
mouse = true
protocol = "auto"       # auto | kitty | iterm2 | sixel | halfblocks
theme = "auto"          # auto | dark | light
mermaid_engine = "auto" # auto | rust | mmdc | source
cache_dir = "~/.cache/mdreview"
```

CLI flags override the config file.

## Local harness

The repository mirrors the `sotto/harness` local development harness:

```sh
./init.sh                 # toolchain + state + baseline check
./harness/gate.sh         # tasks, placeholders, fmt, clippy, tests
./harness/report.sh       # task and code-health report
./harness/add-task.sh ... # add a task
./harness/plan.sh         # regenerate state/fix_plan.md (needs claude CLI)
./harness/qa.sh           # QA evaluator pass (needs claude CLI)
./harness/loop.sh         # autonomous session loop (needs claude CLI)
```

State lives in `state/tasks.json`, `state/progress.md`, and `state/fix_plan.md`. Feature specs live in `specs/`; the design doc is `docs/superpowers/specs/2026-11-20-mdreview-design.md`.

## Testing

```sh
source scripts/env.sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Manual PTY smoke test:

```sh
tmp=$(mktemp -d)
cp tests/fixtures/spec.md tests/fixtures/red.png "$tmp"/
tmux new-session -d -s mdreview -x 100 -y 30 \
  "cd $tmp && /path/to/mdreview review spec.md"
tmux send-keys -t mdreview q
```

## Known limitations

- The name collides with the existing Python `mdreview`.
- Media cache is in-memory; disk caching and an LRU bound are deferred.
- `mermaid-rs-renderer` is pre-1.0; use `mmdc` for official fidelity.
- Remote images, ACP/MCP, CI gating, comment threads, and full Vim emulation are out of scope.
- `loop.sh`, `plan.sh`, and `qa.sh` require a `claude` CLI on PATH. `init.sh`, `gate.sh`, and `report.sh` do not.

## License

MIT. See [LICENSE](LICENSE).
