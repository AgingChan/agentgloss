# Installation and distribution design

- **Date:** 2026-11-22
- **Status:** Implemented and released (v1.0.2)
- **Repo:** https://github.com/AgingChan/agentgloss (public, default branch `main`)

## Intent

Let people install agentgloss without a Rust toolchain, conda environment, or musl setup. The first release covers Linux x86_64 and macOS Apple Silicon, with a one-line installer for the prebuilt binary and a crates.io package for Rust users.

## Goals

- Prebuilt, self-contained binaries for:
  - Linux x86_64 (`x86_64-unknown-linux-musl`, static)
  - macOS Apple Silicon (`aarch64-apple-darwin`)
- One-line install from GitHub Releases, with checksum verification and a configurable install directory.
- Publish `agentgloss` to crates.io so `cargo install agentgloss` works for Rust users.
- A tag-driven release workflow and a minimal pull-request CI workflow.
- README install documentation and a maintainer release checklist.
- Tests for the installer and the packaged binary metadata.

## Non-goals

- Windows, Linux aarch64, macOS Intel, Homebrew, Nix, AUR, Docker images. Only Apple Silicon macOS is built and validated.
- Auto-update.
- Apple notarization or Developer ID signing; macOS binaries are ad-hoc signed.
- Managing crates.io tokens beyond a repository-scoped `CARGO_REGISTRY_TOKEN` secret.
- Changing the application binary or its runtime behavior.

## Constraints

- GitHub-hosted runners only. Current labels: `ubuntu-latest` and `macos-15` (arm64). macOS Intel (`macos-15-intel`) is deliberately not built; only Apple Silicon can be validated.
- `.cargo/config.toml` defaults builds to `x86_64-unknown-linux-musl`; macOS jobs must pass `--target` explicitly.
- Linux release assets must stay static (`file` reports `static-pie linked`).
- The crate name `agentgloss` is currently available on crates.io, but crates.io names are permanent. Publishing under this name forecloses a later rename to a new crate name.
- MIT license; released as v0.1.0, v0.1.1, v0.2.0, v0.2.1, v0.3.0, v1.0.0,
  v1.0.1, and v1.0.2. The VS Code extension is packaged as a VSIX artifact and
  published to the Marketplace via Trusted Publishing.
- Release tags are `v<semver>` and must match `Cargo.toml`; the workflow enforces this.

## Rename to AgentGloss

The project moves from `mdreview` to `agentgloss` before the first release. This is part of the same change:

- **GitHub:** rename `AgingChan/mdreview` to `AgingChan/agentgloss` and update `origin`; GitHub redirects the old URLs.
- **Rust:** package and binary name `agentgloss`; update CLI help/version, error prefixes, the TUI header, `Cargo.lock`, and tests that assert the name.
- **Docs:** README, live specs, the main design doc, CHANGELOG (note the rename), `init.sh`, and harness scripts. Historical harness task IDs (`MDREVIEW-N`) and historical `state/` QA notes stay as identifiers.
- **Agent integration:** `agentgloss install ...`; generated `agentgloss-review.md` and `agentgloss-agent.md`; marker comments `<!-- agentgloss:start -->`; replace the local `.claude/commands/mdreview-review.md` with `agentgloss-review.md`.
- **Unchanged:** the sidecar path `<file>.review.json` is name-independent.
- **New names everywhere:** install URLs, asset names, env vars (`AGENTGLOSS_INSTALL_DIR`, `AGENTGLOSS_RELEASE_BASE`), and the crates.io package are `agentgloss`.

## Approaches considered

1. **GitHub Releases + install script, plus crates.io (chosen).** Prebuilt tarballs cover non-Rust users; crates.io covers Rust users; one workflow owns packaging.
2. **crates.io only (`cargo install agentgloss`).** Rejected as the sole channel: it still compiles locally and requires a Rust toolchain, which is the workflow the user wants to avoid.
3. **Homebrew tap / Nix / distro packages.** Deferred: more maintenance, and the macOS binaries are untested for packaging conventions.
4. **Manual tarball download only.** Rejected as the primary UX: no checksum automation, more user steps.

## Design

### Assets

Stable, unversioned asset names so `releases/latest/download/<asset>` works without API parsing:

- `agentgloss-x86_64-unknown-linux-musl.tar.gz`
- `agentgloss-aarch64-apple-darwin.tar.gz`
- `SHA256SUMS`

Each tarball contains `agentgloss`, `LICENSE`, and `README.md` at its root.

### Release workflow (`.github/workflows/release.yml`)

- Triggers: push of a `v*` tag; `workflow_dispatch` builds artifacts without publishing a release.
- Job `test` (ubuntu): fmt, clippy, full tests, installer tests. Release requires this job.
- Job `build` (matrix): one leg per target above, on the matching native runner. Steps: install Rust stable with the target, `cargo build --release --locked --target <target>`, strip (Linux `strip`; macOS `strip -x`), ad-hoc codesign on macOS (`codesign --force --sign -`), stage the tarball, upload it as a workflow artifact.
- Job `verify-tag` (tag pushes only): fail unless the tagged commit is an ancestor of `origin/main`; `release` and `publish-crates` depend on it.
- Job `release`: download all tarballs, write `SHA256SUMS` with `sha256sum`, create the GitHub Release for the tag and upload the four assets. `permissions: contents: write`. On `workflow_dispatch`, stop after uploading workflow artifacts.
- Job `publish-crates` (needs `test` and `release`, tag pushes only): `cargo publish --locked` with `CARGO_REGISTRY_TOKEN` from the repository secret. It skips a version that is already on crates.io so a tag re-run is idempotent.
- A version guard fails the run if the tag is not `v` + the `Cargo.toml` version.

### PR CI (`.github/workflows/ci.yml`)

On pushes to `main`/`dev` and pull requests: format, clippy, full tests on ubuntu, plus the installer test. Keeps the release path honest without a macOS test matrix (behavior is platform-independent; the macOS legs are release builds only).

### Installer (`install.sh`)

POSIX `sh`, `set -eu`, no dependencies beyond `curl`, `tar`, and a SHA-256 tool.

- Flags: `--version <vX.Y.Z>` (default: latest), `--dir <path>` (default: `${AGENTGLOSS_INSTALL_DIR:-$HOME/.local/bin}`), `--help`.
- Detect `uname -s`/`uname -m`; map Linux x86_64 and Darwin arm64 to the two targets; fail clearly for anything else, including Intel macOS.
- Download the asset and `SHA256SUMS` over HTTPS, verify the matching checksum (`sha256sum` on Linux, `shasum -a 256` on macOS), extract `agentgloss`, install it executable, and warn when the install directory is not on `PATH`.
- `AGENTGLOSS_RELEASE_BASE` overrides the release base URL (default `https://github.com/AgingChan/agentgloss/releases`) so tests can serve a fake release from a `file://` directory.
- One-liner: `curl -fsSL https://raw.githubusercontent.com/AgingChan/agentgloss/main/install.sh | sh`

### crates.io

- Add Cargo metadata: `description`, `license = "MIT"`, `repository`, `readme`, `keywords`, `categories`, and an `exclude` list for `.cargo/`, `state/`, `harness/`, `docs/`, `specs/`, `.github/`, `.claude/`, `.superpowers/`, `scripts/`, and `init.sh`. Excluding `.cargo/config.toml` is required: it pins the musl target locally and would make `cargo install agentgloss` fail on hosts that do not have that target.
- Verify with `cargo publish --dry-run --locked` and `cargo package --list` in CI.
- Publishing runs in the release workflow on tag pushes, after the GitHub Release, using the repository's `CARGO_REGISTRY_TOKEN` secret. Add the secret once (crates.io token with publish scope); the job skips a version that already exists. `cargo install agentgloss` then builds locally for Rust users.

### README

Add an Install section before Build:

- Installer one-liner, supported platforms, `--version` / `--dir` examples.
- Manual tarball download from Releases.
- `cargo install agentgloss` (requires Rust; builds locally).
- `cargo install --git https://github.com/AgingChan/agentgloss` for unreleased code.
- macOS Gatekeeper note (ad-hoc signed; remove quarantine if needed).
- Keep the existing Build section for contributors; add a short maintainer release checklist (tag, workflow, `cargo publish`).

### Tests

- `tests/install.sh`: build a fake release directory containing a dummy `agentgloss` tarball and `SHA256SUMS`; run the installer with `AGENTGLOSS_RELEASE_BASE=file://...`; assert the binary lands in the target directory, that a pinned version path works, and that a bad checksum fails without installing.
- CI runs the installer test on ubuntu.
- `cargo publish --dry-run` / `cargo package --list` verifies the crate contents.
- Manual: after the first tag, install on Linux x86_64 and run `agentgloss --version`; repeat on Apple Silicon macOS.

## Release procedure (maintainer)

1. Ensure `main` is green and `Cargo.toml` version is correct; update `CHANGELOG.md`.
2. `git tag v1.0.2 && git push origin v1.0.2` from `main`; the workflow refuses a tag whose commit is not an ancestor of `main`.
3. The release workflow builds the two tarballs, writes `SHA256SUMS`, and publishes the GitHub Release.
4. The workflow publishes to crates.io using `CARGO_REGISTRY_TOKEN`; ensure the repository secret exists before tagging.
5. Verify: `curl ... | sh` on Linux and Apple Silicon; `cargo install agentgloss` in a clean environment.

## Risks

- Only Apple Silicon macOS is built; Intel Macs fall back to `cargo install` or are unsupported.
- Ad-hoc-signed macOS binaries may trip Gatekeeper when downloaded through a browser; the installer uses `curl` and the README documents the quarantine workaround.
- Checksums come from the same release as the binary; this detects corruption, not a compromised repository.
- Publishing to crates.io permanently claims the `agentgloss` name.
- If `CARGO_REGISTRY_TOKEN` is missing or expired, the crates.io job fails after the GitHub Release; fix the secret and re-run the tag workflow. The job skips versions already published.

## Left to the builder

- Exact action versions and whether to use a third-party release action versus `gh release`.
- The precise runner step ordering and artifact names inside the workflow.
- Installer argument parsing details and the checksum tool fallback.
- Release-notes content and whether to generate them from commits.
