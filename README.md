# agentgloss

A terminal reviewer for long Markdown specs and plans produced by coding agents.

`agentgloss` renders Markdown (including local images and Mermaid diagrams), lets you attach comments and suggested edits to exact source lines or paragraphs, and returns a numbered, paste-ready review to Claude Code, Codex CLI, Gemini CLI, OpenCode, Aider, or any other agent.

> Formerly mdreview. The crate and binary are now agentgloss.

## Status

MVP implemented and tested:

- 104 library tests, 15 CLI integration tests, 33 TUI integration tests, 1 performance test (153 total)
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

## Install

Prebuilt binaries are published for Linux x86_64 (static musl) and macOS Apple Silicon. macOS Intel, Windows, and Linux aarch64 are not built.

```sh
curl -fsSL https://raw.githubusercontent.com/AgingChan/agentgloss/main/install.sh | sh
```

The installer downloads the latest release, verifies its SHA-256 checksum, and installs to `${AGENTGLOSS_INSTALL_DIR:-$HOME/.local/bin}`. Pin a version or choose a directory with:

```sh
curl -fsSL https://raw.githubusercontent.com/AgingChan/agentgloss/main/install.sh \
  | sh -s -- --version v0.1.0 --dir "$HOME/bin"
```

Manual downloads are on the [Releases](https://github.com/AgingChan/agentgloss/releases) page: pick the tarball for your platform, verify it against `SHA256SUMS`, and put `agentgloss` on your `PATH`.

Rust users can install from crates.io (this compiles locally and requires a Rust toolchain):

```sh
cargo install agentgloss
```

For unreleased code:

```sh
cargo install --git https://github.com/AgingChan/agentgloss
```

macOS binaries are ad-hoc signed. If Gatekeeper quarantines the downloaded binary, run `xattr -d com.apple.quarantine ~/.local/bin/agentgloss`.

> Renaming from mdreview: the config directory is now `~/.config/agentgloss` (`~/Library/Application Support/agentgloss` on macOS); an existing `~/.config/mdreview` is not read. Old `<!-- mdreview:* -->` marker blocks are not recognized; delete the old block, then re-run `agentgloss install <agent>` to install the new recipes.

## Build

The project targets a static `x86_64-unknown-linux-musl` binary. On this machine, Rust and a host linker are provided by:

- Rust 1.100 + `rustfmt`, `clippy`, and the musl target
- a conda environment named `rustbuild` with GCC, used only for host build scripts/proc macros
- `rust-lld` for the final static link

```sh
source scripts/env.sh
cargo build --release
./target/x86_64-unknown-linux-musl/release/agentgloss --version
```

If you already have a working C linker, you can remove `.cargo/config.toml` and build normally. The generated binary is static:

```sh
file target/x86_64-unknown-linux-musl/release/agentgloss
# ELF 64-bit LSB pie executable ... static-pie linked
```

## Releases

Maintainers publish from `main`:

1. Update `CHANGELOG.md` and the `Cargo.toml` version, then merge the change to `main`.
2. Tag a commit that is on `main` — the workflow refuses a tag whose commit is not an ancestor of `main` — then `git tag v<version> && git push origin v<version>`.
3. The release workflow builds the Linux and macOS tarballs, writes `SHA256SUMS`, and publishes the GitHub Release.
4. The workflow then publishes `agentgloss` to crates.io using the `CARGO_REGISTRY_TOKEN` repository secret. Add it once with `gh secret set CARGO_REGISTRY_TOKEN` (a crates.io token with publish scope) or under Settings → Secrets and variables → Actions. Re-running a tag skips an already-published version.
5. Verify the installer on Linux x86_64 and Apple Silicon, and `cargo install agentgloss --version <version>` in a clean environment.

## Quick start

```sh
# Review in your terminal
agentgloss review docs/specs/checkout.md

# If the agent needs to request a review first
agentgloss request docs/specs/checkout.md
# ... the human runs:
agentgloss review docs/specs/checkout.md
# ... then the agent runs:
agentgloss export docs/specs/checkout.md
```

Inside tmux:

```sh
agentgloss review docs/specs/checkout.md --tmux
agentgloss review docs/specs/checkout.md --detach   # agent polls with wait
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

1. The agent writes a spec and runs `agentgloss request <file>`.
2. You run `agentgloss review <file>` (separate pane, or the agent opens a tmux popup).
3. Select text or a block, press `c` for a comment or `s` for a suggested edit, then `F10` (or `Ctrl+Enter`) to save.
4. `F10` (or `Ctrl+Enter`) again opens signoff. A clean review is approved; otherwise the review is sent to the agent.
5. The agent runs `agentgloss export <file>` or `agentgloss wait <file>`, addresses the numbered items, and runs `agentgloss resolve <file> <ids>`.
6. Reopen the review after the revision. Open items re-anchor; stale items remain visible for reattachment.

### Agent recipes

```sh
agentgloss install claude    # .claude/commands/agentgloss-review.md
agentgloss install codex     # AGENTS.md marker block
agentgloss install gemini    # GEMINI.md marker block
agentgloss install generic   # agentgloss-agent.md
```

Codex and Gemini installs update only the marker block and preserve the rest of `AGENTS.md`/`GEMINI.md`. Claude and generic installs replace their dedicated recipe files. Re-running any install is idempotent.

## Keys

Vim mode is the default. `F2` switches between Vim and standard input.

| Action | Vim | Standard |
| --- | --- | --- |
| Move | `j`/`k` | Up/Down |
| Half page | `Ctrl-d`/`Ctrl-u` | PgUp/PgDn |
| Top/bottom | `gg`/`G` | Home/End |
| Headings | `]]`/`[[` | `Ctrl-]`/`Ctrl-[` |
| Blocks | `}`/`{` | `Ctrl-Down`/`Ctrl-Up` |
| Select | `v`, `V`, `P` | Shift+arrows, Shift+Home/End, `Ctrl-p` |
| Comment / suggestion | `c` / `s` | `c` / `s` |
| Search | `/`, `n`/`N` | `Ctrl-f`, `n`/`N` |
| Resolve / reopen / edit | `x` / `X` / `e` | `x` / `X` / `e` |
| Reattach stale item | `r` | `r` |
| Delete item | `d` | Delete |
| Sign off | `F10` / `Ctrl+Enter` | `F10` / `Ctrl+Enter` |
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

Writes are atomic. Corrupt JSON is backed up to `<sidecar>.bak.<timestamp>`. Unknown schema versions are rejected without overwriting. Mutating commands and `export` take an exclusive sidecar lock; `export` exits 2 if a review currently holds it, so run it after the TUI closes.

## Configuration

Optional config file: `~/.config/agentgloss/config.toml` on Linux, or `~/Library/Application Support/agentgloss/config.toml` on macOS:

```toml
keys = "vim"            # vim | standard
mouse = true
protocol = "auto"       # auto | kitty | iterm2 | sixel | halfblocks
theme = "auto"          # auto | dark | light
mermaid_engine = "auto" # auto | rust | mmdc | source
cache_dir = "~/.cache/agentgloss"
```

CLI flags override the config file.

For high-DPI terminal images that look soft, set `AGENTGLOSS_CELL_SIZE=WxH` to the terminal's real cell pixel size (for example `20x40`), or `AGENTGLOSS_RENDER_SCALE=2` to supersample the detected cell size. The layout aspect and rasterization use the same cell size. `--protocol` selects the graphics protocol when detection is wrong.

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

State lives in `state/tasks.json`, `state/progress.md`, and `state/fix_plan.md`. Feature specs live in `specs/`; the design doc is `docs/superpowers/specs/2026-11-20-agentgloss-design.md`.

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
tmux new-session -d -s agentgloss -x 100 -y 30 \
  "cd $tmp && /path/to/agentgloss review spec.md"
sleep 2
tmux send-keys -t agentgloss q
```

## Known limitations

- The former name `mdreview` collides with an existing Python/Textual project.
- Media cache is in-memory; disk caching and an LRU bound are deferred.
- `mermaid-rs-renderer` is pre-1.0; use `mmdc` for official fidelity.
- Remote image fetching, ACP/MCP, CI gating, comment threads, and full Vim emulation are out of scope; remote URLs render as link-styled text.
- `loop.sh`, `plan.sh`, and `qa.sh` require a `claude` CLI on PATH. `init.sh`, `gate.sh`, and `report.sh` do not.

## License

MIT. See [LICENSE](LICENSE).
