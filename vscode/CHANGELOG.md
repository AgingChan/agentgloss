# Changelog

## 1.0.3 — 2026-11-28

- Default the `agentgloss.path` setting to `agloss`, the new CLI command. The
  setting key is unchanged; `agentgloss` remains a compatibility alias.
- Upgrading from 1.0.2 requires the 1.0.3 CLI (which installs `agloss`) or
  setting `agentgloss.path` to the existing `agentgloss` executable.
- Documented Windows use through VS Code Remote - WSL with the Linux binary.
  Native Windows extension hosts remain unsupported until a native CLI exists.

## 1.0.2 — 2026-11-28

- Renamed the extension display name to "Agent Gloss". The command palette
  category and settings section follow; the extension ID is unchanged.

## 1.0.1 — 2026-11-28

- Added the standalone Marketplace trusted-publishing workflow using GitHub
  OIDC (`vsce publish --oidc`); no PAT is stored.
- Upgraded `@vscode/vsce` to 4.1.1.
- Fixed the Marketplace icon: it is now a full-bleed 128 px square with opaque
  ink corners, rendered directly from the square brand mark without padding or
  a transparent rounded canvas.

## 1.0.0 — 2026-11-28

- First stable release of the extension.
- Changed the publisher ID to `InnopiaPteLtd`; the extension ID is now
  `InnopiaPteLtd.agentgloss`.

## 0.3.0 — 2026-11-28

- Added the AgentGloss product mark and extension icon.
- The Open Review editor-title button now uses the branded 16 px annotation
  icons (light and dark) instead of the Markdown preview codicon.

## 0.2.1 — 2026-11-28

- Added the **Open Review** editor-title button (preview icon), the Explorer
  file context menu entry, and the editor-tab context menu entry.
- Added a manifest test for the contributed commands and menus.

## 0.2.0

- Rendered Markdown review webview with bundled Mermaid v11 diagrams.
- Block/paragraph selection for comments and suggested edits.
- Inline edit, resolve, reopen, delete, and stale reattach actions.
- Signoff with approval or agent feedback; the existing sidecar and agent
  handshake are unchanged.
- Line-exact comment/suggestion commands from the Markdown editor.
- Lock, unsaved-change, document-changed, and corrupt-sidecar banners.
- Local image rendering within the document directory/workspace; remote images
  render as links.
