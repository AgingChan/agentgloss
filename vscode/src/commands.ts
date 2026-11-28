import * as vscode from "vscode";
import {
  ApplyRequest,
  BridgeError,
  Selection,
  bridgeApply,
  bridgeState,
} from "./bridge";
import { ReviewPanel } from "./reviewPanel";

export function registerCommands(context: vscode.ExtensionContext): void {
  void setTrustContext();
  context.subscriptions.push(
    vscode.workspace.onDidGrantWorkspaceTrust(() => {
      void setTrustContext();
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "agentgloss.openReview",
      async (resource?: unknown) => {
        const target = resolveMarkdownUri(resource);
        if (!target) {
          void vscode.window.showErrorMessage(
            "agentgloss: open or select a Markdown file first.",
          );
          return;
        }
        if (!(await ensureTrusted())) {
          return;
        }
        ReviewPanel.open(context, target);
      },
    ),
    vscode.commands.registerCommand("agentgloss.refresh", async () => {
      if (!(await ensureTrusted())) {
        return;
      }
      const editor = vscode.window.activeTextEditor;
      if (editor && isMarkdown(editor.document)) {
        if (!ReviewPanel.refreshOpen(editor.document.uri)) {
          ReviewPanel.open(context, editor.document.uri);
        }
      }
    }),
    vscode.commands.registerCommand(
      "agentgloss.commentSelection",
      async () => commentSelection(),
    ),
    vscode.commands.registerCommand(
      "agentgloss.suggestSelection",
      async () => suggestSelection(),
    ),
  );
}

async function setTrustContext(): Promise<void> {
  await vscode.commands.executeCommand(
    "setContext",
    "agentgloss.trusted",
    vscode.workspace.isTrusted,
  );
}

async function ensureTrusted(): Promise<boolean> {
  if (!vscode.workspace.isTrusted) {
    void vscode.window.showWarningMessage(
      "agentgloss requires a trusted workspace to run.",
    );
    return false;
  }
  return true;
}

function isMarkdown(document: vscode.TextDocument): boolean {
  return (
    document.languageId === "markdown" ||
    /\.(md|markdown)$/i.test(document.fileName)
  );
}

function isMarkdownUri(uri: vscode.Uri): boolean {
  return uri.scheme === "file" && /\.(md|markdown)$/i.test(uri.fsPath);
}

function isUri(value: unknown): value is vscode.Uri {
  return (
    !!value &&
    typeof value === "object" &&
    "scheme" in value &&
    "path" in value &&
    typeof (value as { fsPath?: unknown }).fsPath === "string"
  );
}

function resolveMarkdownUri(resource: unknown): vscode.Uri | undefined {
  const candidates: unknown[] = Array.isArray(resource) ? resource : [resource];
  for (const candidate of candidates) {
    if (isUri(candidate) && isMarkdownUri(candidate)) {
      return candidate;
    }
  }
  const editor = vscode.window.activeTextEditor;
  if (editor && isMarkdown(editor.document)) {
    return editor.document.uri;
  }
  return undefined;
}

function binaryPath(): string {
  return vscode.workspace
    .getConfiguration("agentgloss")
    .get<string>("path", "agloss");
}

function selectionLines(editor: vscode.TextEditor): Selection {
  const selection = editor.selection;
  const start = selection.start.line + 1;
  let end = selection.end.line + 1;
  if (
    selection.end.character === 0 &&
    selection.end.line > selection.start.line
  ) {
    end = selection.end.line;
  }
  return { start_line: start, end_line: Math.max(start, end) };
}

async function commentSelection(): Promise<void> {
  if (!(await ensureTrusted())) {
    return;
  }
  const editor = vscode.window.activeTextEditor;
  if (!editor || !isMarkdown(editor.document)) {
    void vscode.window.showErrorMessage(
      "agentgloss: open a Markdown file first.",
    );
    return;
  }
  if (editor.document.isDirty) {
    void vscode.window.showErrorMessage(
      "agentgloss: save the document before adding a review item.",
    );
    return;
  }
  const selection = selectionLines(editor);
  const body = await vscode.window.showInputBox({
    title: "agentgloss comment",
    prompt: `Comment on lines ${selection.start_line}-${selection.end_line}`,
    placeHolder: "What should change?",
    ignoreFocusOut: true,
  });
  if (body === undefined) {
    return;
  }
  if (!body.trim()) {
    void vscode.window.showErrorMessage("agentgloss: comment cannot be empty.");
    return;
  }
  await applySelectionOp(editor, "comment", selection, {
    body,
  });
}

async function suggestSelection(): Promise<void> {
  if (!(await ensureTrusted())) {
    return;
  }
  const editor = vscode.window.activeTextEditor;
  if (!editor || !isMarkdown(editor.document)) {
    void vscode.window.showErrorMessage(
      "agentgloss: open a Markdown file first.",
    );
    return;
  }
  if (editor.document.isDirty) {
    void vscode.window.showErrorMessage(
      "agentgloss: save the document before adding a review item.",
    );
    return;
  }
  const selection = selectionLines(editor);
  const selected = editor.document
    .getText(editor.selection)
    .split(/\r?\n/)
    .join(" ");
  const replacement = await vscode.window.showInputBox({
    title: "agentgloss suggestion",
    prompt: `Replacement for lines ${selection.start_line}-${selection.end_line} (single line)`,
    value: selected,
    ignoreFocusOut: true,
  });
  if (replacement === undefined) {
    return;
  }
  if (!replacement.trim()) {
    void vscode.window.showErrorMessage(
      "agentgloss: replacement cannot be empty.",
    );
    return;
  }
  const note = await vscode.window.showInputBox({
    title: "agentgloss suggestion note",
    prompt: "Optional note for the agent",
    ignoreFocusOut: true,
  });
  await applySelectionOp(editor, "suggestion", selection, {
    body: note ?? "",
    replacement,
  });
}

async function applySelectionOp(
  editor: vscode.TextEditor,
  op: "comment" | "suggestion",
  selection: Selection,
  values: { body: string; replacement?: string },
): Promise<void> {
  try {
    const state = await bridgeState(
      binaryPath(),
      editor.document.uri.fsPath,
      undefined,
      true,
    );
    const request: ApplyRequest = {
      schema_version: 1,
      op,
      expect_hash: state.doc_hash,
      selection,
      body: values.body,
      replacement: values.replacement,
      include_source: false,
    };
    const response = await bridgeApply(
      binaryPath(),
      editor.document.uri.fsPath,
      undefined,
      request,
    );
    void vscode.window.showInformationMessage(
      `agentgloss: saved ${response.id ?? "review item"}.`,
    );
  } catch (error) {
    if (error instanceof BridgeError) {
      if (error.code === "locked") {
        void vscode.window.showErrorMessage(
          "agentgloss: close the review TUI before editing from the editor.",
        );
        return;
      }
      if (error.code === "not_found") {
        void vscode.window.showErrorMessage(
          `agentgloss: ${error.message}`,
        );
        return;
      }
      void vscode.window.showErrorMessage(`agentgloss: ${error.message}`);
      return;
    }
    void vscode.window.showErrorMessage(
      `agentgloss: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}
