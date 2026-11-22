# AgentGloss Rename and Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename `mdreview` to `agentgloss` across the crate, CLI, agent integrations, docs, and harness; add crates.io metadata; add a GitHub Releases installer with Linux x86_64 and macOS Apple Silicon builds; publish v0.1.0.

**Architecture:** One mechanical rename commit series keeps the test suite green at every task boundary. Distribution is a tag-triggered GitHub Actions workflow that builds the two native targets, packages stable-name tarballs with `SHA256SUMS`, attaches them to the GitHub Release, and publishes the crate to crates.io from the `CARGO_REGISTRY_TOKEN` repository secret; a POSIX installer downloads and verifies the release assets.

**Tech Stack:** Rust 2024, cargo 1.100, GitHub Actions, POSIX shell, curl, tar, SHA-256.

**Spec:** `docs/superpowers/specs/2026-11-22-install-distribution-design.md`

## Global Constraints

- Verification commands, from the repo root:
  - `source scripts/env.sh && cargo fmt --check`
  - `source scripts/env.sh && cargo clippy --all-targets --all-features -- -D warnings`
  - `source scripts/env.sh && cargo test --all-features`
  - `sh tests/package.sh` (after Task 3)
  - `sh tests/install.sh` (after Task 4)
  - `sh tests/workflows.sh` (after Task 5)
- No new Rust dependencies.
- The review sidecar path `<file>.review.json` is unchanged.
- Historical identifiers stay: `MDREVIEW-*` task IDs, `state/` QA/progress notes, and the historical plan under `docs/superpowers/plans/2026-11-22-table-rendering.md`.
- The local checkout stays at `/home/eugene/projects/mdreview`; only the GitHub repository name and crate/binary names change.
- Do not include `.cargo/` in the crates.io package: its `config.toml` pins the musl target and would break `cargo install agentgloss` on hosts without that target.
- Outward-facing steps (push, repo rename, tag, GitHub Release, crates.io publish) happen in Task 7 only after explicit go-ahead. Task 7 is irreversible for the crates.io name. The crates.io publish requires the repository secret `CARGO_REGISTRY_TOKEN`; the human partner adds it, never pasted into the conversation.
- End every commit message with `Co-Authored-By: Claude Code <noreply@anthropic.com>`.

## Review Focus

1. **`cargo install agentgloss` on a clean host:** the packaged crate must not carry `.cargo/config.toml` or other dev-only files, or the install fails on hosts without the musl target. (Task 3: `tests/package.sh`.)
2. **Renamed config and marker surfaces:** `~/.config/mdreview/config.toml`, `<!-- mdreview:* -->` blocks, and `.claude/commands/mdreview-review.md` are not silently interpreted after the rename; the README and CHANGELOG state the new paths, and the local skill is regenerated. (Tasks 1–2, 6.)
3. **Installer failure safety:** a bad checksum must fail without installing a binary; latest and pinned versions both work. (Task 4: `tests/install.sh`.)
4. **Release matrix:** Linux static musl and macOS arm64 only, native runners, version tag guard, and no release on `workflow_dispatch`. (Task 5: `tests/workflows.sh` plus the dry run in Task 7.)
5. **Publish readiness:** valid Cargo metadata, a clean package listing, a successful dry run, and the `CARGO_REGISTRY_TOKEN` secret before the irreversible automated crates.io publish. (Task 3 and Task 7.)

---

### Task 1: Rename the crate, binary, runtime strings, and agent integration

**Files:**
- Modify: `Cargo.toml`, `Cargo.lock` (via cargo), `src/main.rs`, `src/cli.rs`, `src/config.rs`, `src/media.rs`, `src/review/export.rs`, `src/tui/mod.rs`, `src/tui/render.rs`, `src/agent.rs`
- Modify: `tests/cli.rs`, `tests/tui.rs`, `tests/performance.rs`
- Modify (untracked, local): `.claude/commands/mdreview-review.md` → `.claude/commands/agentgloss-review.md`

**Interfaces:**
- Consumes: the existing CLI, config, review, TUI, and agent-recipe modules.
- Produces: crate and binary name `agentgloss`; all user-visible strings, config paths, temp prefixes, marker comments, recipe file names, and tests use `agentgloss`.

- [ ] **Step 1: Sync the branch**

Run:
```bash
git fetch origin
git switch dev
git merge --ff-only origin/main
```
Expected: `dev` fast-forwards to the merge commit `ad68375`.

- [ ] **Step 2: Update the tests first**

Replace `mdreview` with `agentgloss` in the test surfaces below. Do not touch `src/` production strings yet.

- `tests/cli.rs`: imports `agentgloss::...`; `env!("CARGO_BIN_EXE_agentgloss")`; every expected string (`agentgloss review`, `agentgloss resolve`, `agentgloss wait`, recipe path `.claude/commands/agentgloss-review.md`, marker `<!-- agentgloss:start -->`).
- `tests/tui.rs`: imports `agentgloss::...`; the header assertion `text.contains("agentgloss")`.
- `tests/performance.rs`: import `agentgloss::document::{Document, layout}`.
- `src/agent.rs` test module: assertions contain `agentgloss request/export/review/wait/resolve`; generic recipe path `agentgloss-agent.md`; marker literals. Add this migration test:

```rust
#[test]
fn legacy_mdreview_markers_are_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("AGENTS.md");
    let legacy = "<!-- mdreview:start -->\nold\n<!-- mdreview:end -->\n";
    std::fs::write(&path, legacy).unwrap();
    install(InstallAgent::Codex, dir.path()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(legacy), "{text}");
    assert_eq!(text.matches(START).count(), 1);
    assert_eq!(text.matches(END).count(), 1);
}
```

Before the rename this fails because the `mdreview` markers are the current marker pair and their contents are replaced; after the rename the old block is ordinary text and a new `agentgloss` block is appended.
- `src/review/export.rs` test module: all golden prompts use `# agentgloss review` and `agentgloss resolve ...`.
- `src/config.rs` test: `cache_dir().ends_with("agentgloss")`.

Run: `source scripts/env.sh && cargo test --all-features`
Expected: FAIL — unresolved crate `mdreview` or missing `CARGO_BIN_EXE_agentgloss`; the package is still named `mdreview`.

- [ ] **Step 3: Implement the rename**

- `Cargo.toml`: `name = "agentgloss"`.
- `src/main.rs`: `std::process::exit(agentgloss::cli::run());`.
- `src/cli.rs`: clap `name = "agentgloss"`; error prefixes `agentgloss:`; `Human:`/`Agent:` handoff lines.
- `src/config.rs`: config and cache directories join `"agentgloss"`; unknown-key warning prefix.
- `src/media.rs`: temp prefix `agentgloss-mmdc-`.
- `src/review/export.rs`: `# agentgloss review` heading and `agentgloss resolve` commands.
- `src/tui/mod.rs`: terminal-session error prefix.
- `src/tui/render.rs`: header `" agentgloss  {}  round {}  open {}  [{}]"` and the tiny-terminal message.
- `src/agent.rs`: marker constants `<!-- agentgloss:start -->` / `<!-- agentgloss:end -->`; recipe paths `agentgloss-review.md` / `agentgloss-agent.md`; every command string; tmux window name and title; result-file prefix `agentgloss-tui-result-`.
- Run `source scripts/env.sh && cargo check` to refresh `Cargo.lock`.

- [ ] **Step 4: Run the tests**

Run: `source scripts/env.sh && cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 5: Regenerate the local Claude recipe**

Run:
```bash
source scripts/env.sh
cargo run -- install claude
rm -f .claude/commands/mdreview-review.md
```
Expected: `.claude/commands/agentgloss-review.md` exists, contains `agentgloss request`, and no `mdreview-review.md` remains.

- [ ] **Step 6: Run fmt, clippy, and the full suite**

Run: `source scripts/env.sh && cargo fmt && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features`
Expected: clean, all tests pass.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src tests
git commit -m "Rename the crate and runtime to agentgloss" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 2: Rename live docs, specs, harness, and scripts

**Files:**
- Rename: `docs/superpowers/specs/2026-11-20-mdreview-design.md` → `docs/superpowers/specs/2026-11-20-agentgloss-design.md`
- Modify: `README.md`, `CHANGELOG.md`, `specs/*.md`, `harness/PROMPT.md`, `harness/*.sh`, `init.sh`, `scripts/env.sh`, `.gitignore`
- Add: `docs/superpowers/specs/2026-11-22-install-distribution-design.md`, `docs/superpowers/plans/2026-11-22-agentgloss-rename-and-release.md`

**Interfaces:**
- Consumes: the renamed binary and commands from Task 1.
- Produces: live documentation, specs, and harness scripts that use `agentgloss`; historical records remain as written.

- [ ] **Step 1: Rename the main design doc**

Run:
```bash
git mv docs/superpowers/specs/2026-11-20-mdreview-design.md docs/superpowers/specs/2026-11-20-agentgloss-design.md
```
Update its title to `# agentgloss — Terminal Spec Reviewer Design`, change the “Working name” bullet to `**Name:** agentgloss (formerly mdreview, which collides with an existing Python/Textual project)`, and update the Name table row. Replace product-command references with `agentgloss`, but keep the prior-art sentence naming `mdreview` (Python/Textual) and its `https://github.com/lazyoft/mdreview` link.

- [ ] **Step 2: Update live docs and scripts**

Replace product-name occurrences with `agentgloss` in: `README.md`, `specs/*.md`, `harness/PROMPT.md`, `harness/*.sh`, `init.sh`, `scripts/env.sh`. This includes recipe file names (`agentgloss-review.md`, `agentgloss-agent.md`), marker blocks, command examples, log-file prefixes, and the design-doc path `2026-11-20-agentgloss-design.md`.

- In `README.md`, replace the “Working name” warning with `> Formerly mdreview. The crate and binary are now agentgloss.` Keep the project's prior-art/history note where it names the old Python/Textual tool.
- In `CHANGELOG.md`, under 0.1.0 add bullets: renamed from mdreview to agentgloss; config moved to `~/.config/agentgloss`; agent marker blocks and generated recipe file names changed.
- In `.gitignore`, add `.claude/` and `.superpowers/`.

Do **not** change `state/` historical notes, `MDREVIEW-*` task IDs, or `docs/superpowers/plans/2026-11-22-table-rendering.md`.

- [ ] **Step 3: Verify the rename surface**

Run: `git grep -in mdreview`
Expected remaining hits are only:
- `state/` historical notes,
- `MDREVIEW-*` IDs,
- `docs/superpowers/plans/2026-11-22-table-rendering.md`,
- this plan,
- the legacy-marker migration test in `src/agent.rs`,
- the “formerly mdreview” notes in `README.md`, `CHANGELOG.md`, the install design, and the prior-art sentence/link in `2026-11-20-agentgloss-design.md`.

Run: `bash -n init.sh scripts/env.sh harness/*.sh`
Expected: no syntax errors.

- [ ] **Step 4: Run the harness gate and tests**

Run: `source scripts/env.sh && cargo test --all-features`
Expected: all tests pass.

Run: `bash harness/gate.sh`
Expected: no new errors attributable to the rename (stale `state/` task warnings may remain).

- [ ] **Step 5: Commit**

```bash
git add README.md CHANGELOG.md .gitignore specs harness init.sh scripts docs/superpowers/specs docs/superpowers/plans
git commit -m "Rename docs, specs, and harness to agentgloss" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 3: Add crates.io metadata and package hygiene

**Files:**
- Modify: `Cargo.toml`
- Add: `tests/package.sh`

**Interfaces:**
- Consumes: the `agentgloss` package from Task 1.
- Produces: publishable metadata; a package that contains `src/`, `tests/`, `README.md`, `LICENSE`, `CHANGELOG.md`, `install.sh`, and `Cargo.lock`, and excludes dev-only directories.

- [ ] **Step 1: Write the failing package test**

Create `tests/package.sh`:

```sh
#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$root"
list=$(cargo package --list --allow-dirty)
for path in .cargo/ state/ harness/ docs/ specs/ .github/ .claude/ .superpowers/ scripts/ init.sh; do
    if printf '%s\n' "$list" | grep -q "^$path"; then
        echo "unexpected in package: $path" >&2
        exit 1
    fi
done
for path in README.md LICENSE CHANGELOG.md Cargo.toml src/lib.rs src/main.rs; do
    printf '%s\n' "$list" | grep -qx "$path" || { echo "missing from package: $path" >&2; exit 1; }
done
echo "package contents ok"
```

Run: `sh tests/package.sh`
Expected: FAIL — `.cargo/`, `.claude/`, `docs/`, `harness/`, `specs/`, etc. are currently included.

- [ ] **Step 2: Add metadata and exclusions**

In `Cargo.toml` under `[package]`:

```toml
description = "A terminal reviewer for long Markdown specs and agent-generated plans"
license = "MIT"
repository = "https://github.com/AgingChan/agentgloss"
readme = "README.md"
keywords = ["markdown", "tui", "review", "agents", "spec"]
categories = ["command-line-utilities", "development-tools", "text-processing"]
exclude = [
    ".cargo/**",
    ".claude/**",
    ".github/**",
    ".superpowers/**",
    "docs/**",
    "harness/**",
    "init.sh",
    "scripts/**",
    "specs/**",
    "state/**",
]
```

- [ ] **Step 3: Verify the package**

Run: `sh tests/package.sh`
Expected: `package contents ok`.

Run: `source scripts/env.sh && cargo publish --dry-run --locked --allow-dirty`
Expected: dry run succeeds; no manifest warnings about missing description/license/repository.

- [ ] **Step 4: Run fmt, clippy, and the full suite**

Run: `source scripts/env.sh && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock tests/package.sh
git commit -m "Add crates.io metadata and package exclusions" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 4: Add the install script and its tests

**Files:**
- Add: `install.sh`, `tests/install.sh`

**Interfaces:**
- Consumes: the stable asset names and release layout from the design.
- Produces: `install.sh` supporting `--version`, `--dir`, `--help`; `AGENTGLOSS_INSTALL_DIR`, `AGENTGLOSS_RELEASE_BASE`.

- [ ] **Step 1: Write the failing installer test**

Create `tests/install.sh`:

```sh
#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) asset=agentgloss-x86_64-unknown-linux-musl.tar.gz ;;
    Darwin-arm64) asset=agentgloss-aarch64-apple-darwin.tar.gz ;;
    *) echo "skipping installer test on this platform"; exit 0 ;;
esac

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

make_release() {
    version=$1
    dir="$tmp/releases/$2"
    mkdir -p "$dir"
    printf '#!/bin/sh\necho agentgloss-test\n' > "$tmp/agentgloss"
    chmod +x "$tmp/agentgloss"
    tar -czf "$dir/$asset" -C "$tmp" agentgloss
    (cd "$dir" && sha256 "$asset" > SHA256SUMS)
}

make_release v0.0.0-test "latest/download"
make_release v0.0.0-test "download/v0.0.0-test"

AGENTGLOSS_RELEASE_BASE="file://$tmp/releases" sh "$root/install.sh" --version v0.0.0-test --dir "$tmp/pinned"
test -x "$tmp/pinned/agentgloss"
test "$("$tmp/pinned/agentgloss")" = "agentgloss-test"

AGENTGLOSS_RELEASE_BASE="file://$tmp/releases" sh "$root/install.sh" --dir "$tmp/latest"
test -x "$tmp/latest/agentgloss"

make_release v0.0.0-bad "download/v0.0.0-bad"
(cd "$tmp/releases/download/v0.0.0-bad" && printf '0000000000000000000000000000000000000000000000000000000000000000  %s\n' "$asset" > SHA256SUMS)
if AGENTGLOSS_RELEASE_BASE="file://$tmp/releases" sh "$root/install.sh" --version v0.0.0-bad --dir "$tmp/bad"; then
    echo "expected checksum failure" >&2
    exit 1
fi
test ! -e "$tmp/bad/agentgloss"

echo "installer tests passed"
```

Run: `sh tests/install.sh`
Expected: FAIL — `install.sh` does not exist.

- [ ] **Step 2: Implement `install.sh`**

POSIX `sh`, `set -eu`, and:
- Parse `--version`, `--dir`, `-h`/`--help`; default version `latest`, default dir `${AGENTGLOSS_INSTALL_DIR:-$HOME/.local/bin}`.
- Detect `Linux`/`x86_64` → `x86_64-unknown-linux-musl`; `Darwin`/`arm64` → `aarch64-apple-darwin`; otherwise print the supported list and exit 1.
- Base URL `${AGENTGLOSS_RELEASE_BASE:-https://github.com/AgingChan/agentgloss/releases}`; latest URL `${base}/latest/download`; pinned URL `${base}/download/vX.Y.Z`.
- Download `${asset}` and `SHA256SUMS` with `curl -fsSL` into a `mktemp -d` cleaned by trap.
- Verify with `sha256sum` or `shasum -a 256`; extract `agentgloss` from the tarball; `install -m 0755` into the directory.
- Warn when the directory is not on `PATH`.
- Never require `sudo`.

- [ ] **Step 3: Verify**

Run: `sh -n install.sh && sh tests/install.sh`
Expected: `installer tests passed`.

- [ ] **Step 4: Commit**

```bash
git add install.sh tests/install.sh
git commit -m "Add the agentgloss install script" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 5: Add CI and release workflows

**Files:**
- Add: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `tests/workflows.sh`

**Interfaces:**
- Consumes: `install.sh` and the package tests from Tasks 3–4.
- Produces: PR CI; a tag-driven release that builds `x86_64-unknown-linux-musl` on `ubuntu-latest` and `aarch64-apple-darwin` on `macos-15`, produces `SHA256SUMS`, and uploads the tarballs.

- [ ] **Step 1: Write the failing workflow test**

Create `tests/workflows.sh`:

```sh
#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
ci="$root/.github/workflows/ci.yml"
release="$root/.github/workflows/release.yml"
test -f "$ci" && test -f "$release" || { echo "workflow files missing" >&2; exit 1; }

grep -q 'cargo test --all-features' "$ci"
grep -q 'tests/install.sh' "$ci"
grep -q 'tests/package.sh' "$ci"
grep -q 'tests/workflows.sh' "$ci"
grep -q 'x86_64-unknown-linux-musl' "$release"
grep -q 'aarch64-apple-darwin' "$release"
grep -q 'macos-15' "$release"
grep -q 'SHA256SUMS' "$release"
grep -q 'sha256sum' "$release"
grep -q 'tags:' "$release"
grep -q 'workflow_dispatch' "$release"

if python3 -c 'import yaml' 2>/dev/null; then
    python3 -c 'import sys, yaml; [yaml.safe_load(open(p)) for p in sys.argv[1:]]' "$ci" "$release"
fi
echo "workflow checks passed"
```

Run: `sh tests/workflows.sh`
Expected: FAIL — workflow files missing.

- [ ] **Step 2: Add `.github/workflows/ci.yml`**

On pushes to `main`/`dev` and pull requests to `main`, on `ubuntu-latest`:
- `actions/checkout@v4`
- `dtolnay/rust-toolchain@stable` with `components: rustfmt, clippy` and `targets: x86_64-unknown-linux-musl`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `sh tests/package.sh`, `sh tests/install.sh`, `sh tests/workflows.sh`

- [ ] **Step 3: Add `.github/workflows/release.yml`**

Trigger on `push` of tags `v*` and on `workflow_dispatch`. `permissions: contents: write`.

- `test` job: same steps as CI on `ubuntu-latest`.
- `build` job (needs `test`) matrix:
  - `ubuntu-latest` / `x86_64-unknown-linux-musl` / `agentgloss-x86_64-unknown-linux-musl.tar.gz`
  - `macos-15` / `aarch64-apple-darwin` / `agentgloss-aarch64-apple-darwin.tar.gz`
  Steps: checkout; Rust stable with `targets: ${{ matrix.target }}`; `cargo build --release --locked --target ${{ matrix.target }}`; strip (`strip` on Linux, `strip -x` plus `codesign --force --sign -` on macOS); stage `agentgloss`, `LICENSE`, `README.md` into `dist/`; `tar -czf "${{ matrix.archive }}" -C dist .`; upload the archive with `actions/upload-artifact@v4`.
- `verify-tag` job (tag pushes only): checkout with `fetch-depth: 0`, `git fetch origin main --no-tags`, and `git merge-base --is-ancestor "$GITHUB_SHA" origin/main`; `release` and `publish-crates` depend on it.
- `release` job (needs `test`, `build`, and `verify-tag`, `if: startsWith(github.ref, 'refs/tags/v')`):
  - Checkout; version guard: `test "v$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)" = "$GITHUB_REF_NAME"`.
  - `actions/download-artifact@v4` with `merge-multiple: true` into `dist/`.
  - `cd dist && sha256sum *.tar.gz > SHA256SUMS`.
  - `gh release view "$GITHUB_REF_NAME" >/dev/null 2>&1 || gh release create "$GITHUB_REF_NAME" --title "$GITHUB_REF_NAME" --generate-notes`, then `gh release upload "$GITHUB_REF_NAME" dist/* --clobber`, with `GH_TOKEN: ${{ github.token }}`.
- `publish-crates` job (needs `test`, `release`, and `verify-tag`, tag pushes only): install Rust with the musl target, then `cargo publish --locked` with `CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}`; skip when `https://crates.io/api/v1/crates/agentgloss/<version>` already exists. `tests/workflows.sh` asserts the job, the secret, the publish command, and the main-ancestry guard.

- [ ] **Step 4: Verify**

Run: `sh tests/workflows.sh`
Expected: `workflow checks passed`.

Run: `source scripts/env.sh && cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add .github tests/workflows.sh
git commit -m "Add CI and release workflows" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 6: Document install, supported platforms, and releases

**Files:**
- Modify: `README.md`, `CHANGELOG.md`

**Interfaces:**
- Consumes: the installer, workflows, and crates.io metadata from Tasks 3–5.
- Produces: user-facing install documentation and a maintainer release checklist.

- [ ] **Step 1: Add the Install section**

Before the Build section, document:
- `curl -fsSL https://raw.githubusercontent.com/AgingChan/agentgloss/main/install.sh | sh`
- Install location `${AGENTGLOSS_INSTALL_DIR:-$HOME/.local/bin}`, `--version v0.1.0`, `--dir <path>`.
- Supported platforms: Linux x86_64 (static musl) and macOS Apple Silicon. macOS Intel, Windows, and Linux aarch64 are not built.
- Manual tarball download from Releases.
- `cargo install agentgloss` (requires Rust; builds locally) and `cargo install --git https://github.com/AgingChan/agentgloss` for unreleased code.
- macOS ad-hoc signing note and the quarantine workaround.
- The rename note: config moved to `~/.config/agentgloss`; old `<!-- mdreview:* -->` marker blocks are not recognized; regenerate agent recipes with `agentgloss install <agent>`.

- [ ] **Step 2: Update status and release docs**

- Update the README status counts to the current suite total after Task 1 (145 tests: 97 lib, 15 CLI, 1 performance, 32 TUI).
- Add a maintainer release checklist: update `CHANGELOG.md`, tag `v<version>`, let the workflow build the release and publish to crates.io via `CARGO_REGISTRY_TOKEN`, verify the installer on Linux and Apple Silicon.
- Update `CHANGELOG.md` with the rename, config-path move, marker/recipe changes, installer, and crates.io publication.

- [ ] **Step 3: Verify**

Run: `git grep -in mdreview README.md CHANGELOG.md`
Expected: only the “formerly mdreview” notes and the prior-art mention.

Run: `source scripts/env.sh && cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "Document agentgloss installation and releases" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 7: Publish v0.1.0

**Files:** none (outward-facing operations).

**Interfaces:**
- Consumes: Tasks 1–6.
- Produces: renamed GitHub repo, `v0.1.0` GitHub Release with the two tarballs and `SHA256SUMS`, `agentgloss` on crates.io.

**STOP:** every step in this task is outward-facing. Confirm with the human partner before the push, the repo rename, the tag push, and `cargo publish`. The crates.io name claim is irreversible.

- [ ] **Step 1: Push the branch and open the PR**

```bash
git push origin dev
gh pr create --base main --head dev --title "Rename to AgentGloss and add release distribution" --body "..."
```
Merge after review. Verify `gh pr view` shows merged.

- [ ] **Step 2: Rename the GitHub repository**

```bash
gh repo rename agentgloss --yes
git remote set-url origin git@github.com:AgingChan/agentgloss.git
git fetch origin
gh repo view --json name,defaultBranchRef
```
Expected: `name` is `agentgloss`; the default branch is `main`.

- [ ] **Step 3: Dry-run the release workflow**

```bash
gh workflow run release.yml --ref main
gh run list --workflow release.yml --limit 1
gh run watch <run-id>
```
Expected: test and build jobs pass; downloadable artifacts exist for both targets; no GitHub Release is created.

- [ ] **Step 4: Tag and publish the GitHub Release**

```bash
git switch main && git pull
git tag v0.1.0
git push origin v0.1.0
```
The `verify-tag` job fails unless the tagged commit is an ancestor of `main`.
gh run watch <release-run-id>
gh release view v0.1.0
```
Expected: `agentgloss-x86_64-unknown-linux-musl.tar.gz`, `agentgloss-aarch64-apple-darwin.tar.gz`, and `SHA256SUMS` are attached.

- [ ] **Step 5: Confirm the crates.io secret and verify the automated publish**

Before tagging, confirm the repository secret exists (the value is never read or printed):
```bash
gh secret list --repo AgingChan/agentgloss
```
Expected: `CARGO_REGISTRY_TOKEN` is listed. If it is missing, ask the human partner to add it with `gh secret set CARGO_REGISTRY_TOKEN` (a crates.io token with publish scope) or the repo Actions settings.

The tag from Step 4 triggers `publish-crates` after the GitHub Release. Verify:
```bash
gh run view <release-run-id> --log
curl -sS https://crates.io/api/v1/crates/agentgloss | head -c 400
```
Expected: the `publish-crates` job passes and the crates.io API returns the `agentgloss` crate metadata. A re-run skips the version if it is already published.

- [ ] **Step 6: Verify the install paths**

Linux:
```bash
tmp=$(mktemp -d)
curl -fsSL https://raw.githubusercontent.com/AgingChan/agentgloss/main/install.sh \
  | sh -s -- --version v0.1.0 --dir "$tmp"
"$tmp/agentgloss" --version
cargo install agentgloss --version 0.1.0 --root "$tmp/cargo"
"$tmp/cargo/bin/agentgloss" --version
```
Expected: all three report `agentgloss 0.1.0`.

Apple Silicon: run the same one-liner with `--dir ~/.local/bin` (or a temp dir) and `agentgloss --version`.
