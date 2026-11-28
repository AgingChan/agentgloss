# Changelog

## Unreleased

- Added the standalone Marketplace trusted-publishing workflow using GitHub
  OIDC (`vsce publish --oidc`); no PAT is stored.
- Upgraded `@vscode/vsce` to 4.1.1.

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
