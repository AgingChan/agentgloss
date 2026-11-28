# 011 — VS Code review extension

## Goals

Provide the full agentgloss review loop inside VS Code:

- rendered Markdown with high-quality Mermaid diagrams (the main motivation);
- select rendered text or blocks to add comments and suggested edits;
- edit, delete, resolve/reopen, reattach stale items, and sign off;
- write the same `*.review.json` sidecar so `agentgloss request`, `export`,
  `wait`, and `resolve` work unchanged.

The extension is a renderer plus a thin client over the hidden bridge
(spec 010). It never implements anchors, hashes, re-anchoring, rounds, or
approval itself.

## Non-goals

Character-level selection, per-table-cell precision, auto-opening on
`request`, webview drafts, multi-file reviews, remote/WSL extension hosts,
remote image fetching, Marketplace publishing, and replacing the terminal TUI.
The TUI and extension remain interchangeable front ends.

## Package

`vscode/` is a self-contained TypeScript extension:

- publisher `InnopiaPteLtd` (extension ID `InnopiaPteLtd.agentgloss`), name
  `agentgloss`, display name "agentgloss Review", version `1.0.0`,
  `engines.vscode: ^1.90`;
- bundled with esbuild; unit tests with Vitest + jsdom; packaged with
  `@vscode/vsce` into a `.vsix`;
- MIT license, README, CHANGELOG.

Commands:

- `agentgloss.openReview` — open the review panel for the active Markdown file
  or the file/URI passed by an Explorer or editor-tab context menu. The command
  is also contributed to the editor title bar with the branded 16 px
  `media/review-icon-{light,dark}.svg` annotation icons;
- `agentgloss.refresh` — re-run `state`;
- `agentgloss.commentSelection` — exact-selection comment from the source
  editor context menu;
- `agentgloss.suggestSelection` — exact-selection suggestion from the source
  editor context menu.

Settings:

- `agentgloss.path` (machine scope) — binary path, default `agentgloss`.
  Spawned with `execFile` and no shell. A missing binary shows an actionable
  error with install instructions.

## Trust and security

- The extension requires `workspace.isTrusted` before running the binary or
  opening a panel. The path setting is machine-scoped so a workspace cannot
  point at an arbitrary executable.
- Rendered Markdown uses `html: false`; raw HTML is escaped. Links are
  restricted to `https:`, `http:`, and `mailto:` and opened externally.
- The webview uses a nonce-based CSP: `script-src` limited to the nonce,
  `style-src 'unsafe-inline'` only because Mermaid injects styles,
  `img-src ${webview.cspSource} data:`, `connect-src 'none'`.
- Mermaid v11 is bundled locally (no CDN) with `securityLevel: 'strict'`;
  render failures show the diagram source.
- Local images are resolved through `asWebviewUri` only from the document
  directory and workspace folders; remote images render as links.

## Rendering and source annotations

The extension host parses the bridge-provided on-disk source with markdown-it
(tables, strikethrough, task lists, footnotes; `html: false`) and produces
HTML for the webview. Block tokens are annotated with
`data-start-line`, `data-end-line`, `data-kind`:

- headings, paragraphs, list items, tables, rules, and footnote definitions
  use the token map (1-based inclusive);
- blockquote children are annotated individually; the wrapper is not a
  selection target;
- fenced code and Mermaid blocks are annotated with their **content-only**
  line range (fences excluded). If no closing fence exists, the range extends
  to the end of the file;
- image paragraphs are annotated at paragraph level; local images render as
  images, remote images as links;
- inline formatting, links, code spans, and table cells are not separate
  selection targets.

Mermaid blocks render in the webview with the bundled renderer. Full HTML is
rebuilt only when the source or theme changes; state-only updates patch thread
cards and highlights to avoid scroll jumps and diagram flicker.

## Selection mapping

On selection change or mouseup:

1. Ignore collapsed selections and selections outside the document root.
2. Collect annotated elements intersecting the DOM range; keep only leaf-most
   annotated elements (a click in a nested list item resolves to that item, not
   its parent).
3. Union the line ranges, trim blank first/last source lines, and reject
   whitespace-only selections.
4. Show a confirmation preview with line numbers; the user can cancel.
5. Send `{start_line, end_line}` to the bridge. The extension never computes
   byte or UTF-16 offsets.

The source-editor commands use `TextEditor.selection` line numbers, giving
exact line precision (including lines inside tables/fences) without DOM
mapping. They are the escape hatch for constructs whose rendered mapping is
approximate.

## Panel and state lifecycle

- `openReview` creates a `WebviewPanel` beside the source with
  `retainContextWhenHidden`.
- On open, call `bridge apply` with `op: "sync"` (matching TUI startup:
  re-anchor, invalidate approval, increment a round only after a prior
  signoff). If the lock is held, fall back to lock-free `state` and show a
  "close the TUI to edit" banner while keeping the panel readable.
- Render only the bridge's on-disk `source`. If the editor is dirty, show
  "Save to refresh review"; interactions that need anchors are disabled.
- On `onDidSaveTextDocument` for the reviewed file, call `sync`.
- Watch the sidecar; external changes (TUI, agent, another editor) trigger a
  `state` refresh. If a composer is active, show a non-destructive
  "review changed externally" notice.
- All mutations go through a single promise queue. Actions are disabled while
  a mutation is pending. A process failure is never auto-retried; the panel
  refreshes `state` and shows the error.
- `doc_changed`, `locked`, unsaved changes, and bridge errors render as
  banners with the relevant action (Sync, Refresh, Retry).

## Review UI

- Inline thread cards under anchored blocks: comment body, or an
  original→replacement diff for suggestions, plus an optional note.
  Actions: edit, delete, resolve/reopen, reattach for stale items.
- Summary bar: round, status, open/stale/resolved counts, Sync, Sign off.
- Signoff uses a VS Code modal confirmation. With any open or stale item the
  confirmation reads "Send N open and M stale items to the agent?" and records
  feedback; otherwise "Approve this revision?" records approval. The result
  sets `terminal_exit_code` 0/10 in the bridge response.
- Stale items are listed explicitly and never range-highlighted as current.
  Resolved items are hidden by default with a toggle.

## Tests and verification

Vitest/jsdom unit tests cover annotation ranges (duplicate text, nested and
tight lists, tables, fence inner ranges including a missing closing fence,
image-only paragraphs, footnotes), selection reduction, bridge stdout/error
parsing, the mutation queue, link allowlisting, and hostile
Markdown/Mermaid failures.

Manual verification on VS Code 1.145: install the VSIX, run the
request → open → comment/suggestion → resolve → signoff → export/wait flow,
verify GFM and Mermaid fidelity, sidecar v1 compatibility, TUI interop, lock
and unsaved-change banners, corrupt/v2 sidecar behavior, and untrusted
workspace blocking.
