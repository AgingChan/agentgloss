<p align="center">
  <img src="media/icon.png" width="96" height="96" alt="AgentGloss icon">
</p>

# agentgloss Review for VS Code

Review long Markdown specs and agent-generated plans inside VS Code with the
same `*.review.json` sidecar as the `agentgloss` terminal reviewer. The webview
renders Markdown and Mermaid diagrams (bundled Mermaid v11), and every comment
or suggested edit is stored by the Rust bridge so `agentgloss request`,
`export`, `wait`, and `resolve` keep working unchanged.

## Requirements

- VS Code 1.90 or newer.
- The `agentgloss` binary with the hidden `bridge` command. Build it from this
  repository (`cargo build --release`) or install a release that includes it.
- A trusted workspace.

## Setup

Put `agentgloss` on `PATH`, or set the machine-scoped `agentgloss.path` setting
to the binary:

```json
{
  "agentgloss.path": "/Users/you/.local/bin/agentgloss"
}
```

The extension never runs the binary in an untrusted workspace and spawns it
with `execFile` (no shell).

## Use

1. The agent runs `agentgloss request spec.md` (or open any Markdown file).
2. Run **agentgloss: Open Review** from the Command Palette, the AgentGloss
   annotation button in the editor title bar, the Explorer file context menu,
   or the editor-tab context menu.
3. Select rendered text or a block to comment or suggest an edit.
4. Use the inline cards to edit, resolve, reopen, delete, or reattach stale
   items.
5. Choose **Sign off**. With open or stale items the review is sent as
   feedback; otherwise the revision is approved.
6. The agent runs `agentgloss export spec.md` or `agentgloss wait spec.md`.

The webview reads the on-disk document. If the editor has unsaved changes it
shows a banner and asks you to save before editing the review.

### Exact line selections

Rendered selections are block/paragraph level. For line-exact selections
(including inside tables and code fences), select text in the normal Markdown
editor and run **agentgloss: Comment on Selection** or **agentgloss: Suggest
Edit for Selection** from the context menu. Source-editor suggestions accept a
single-line replacement; use the rendered webview for multiline replacements.

## Modes and interoperability

- **TUI and webview are interchangeable.** Both write the sidecar; the open
  panel refreshes when another front end changes it.
- While a terminal `agentgloss review` session holds the sidecar lock, the
  webview stays readable but cannot mutate.
- Local images are shown when they live inside the document directory or a
  workspace folder; remote images render as links.
- Mermaid uses strict security mode. A diagram that fails to parse shows its
  source instead.

## Development

```sh
npm install
npm run check     # TypeScript
npm test          # Vitest + jsdom
npm run build     # esbuild bundles
npm run package   # VSIX in this directory
```

Install the VSIX with:

```sh
code --install-extension agentgloss-1.0.1.vsix --force
```

Or install `InnopiaPteLtd.agentgloss` from the Visual Studio Marketplace.

## Marketplace publishing

`.github/workflows/publish-vscode.yml` publishes the extension on `v*` tags or
on manual dispatch from `main` using Marketplace Trusted Publishing over GitHub
OIDC. It does not use a `VSCE_PAT`.

Configure the publisher (`InnopiaPteLtd`) Trusted Publishing policy with:

- Repository: `AgingChan/agentgloss`
- Workflow: `publish-vscode.yml`
- Environment: leave blank unless you add the same `environment:` to the
  workflow job

The workflow publishes every `v*` tag. It was first used for a manual `v1.0.0`
dispatch and now runs automatically on release tags.

## Known limitations

- Rendered selections snap to annotated blocks/paragraphs; character-level
  source selection is only available through the editor commands.
- Custom sidecar paths are not configurable in the webview yet.
- Remote extension hosts (WSL, SSH, containers) are out of scope for this
  version.
