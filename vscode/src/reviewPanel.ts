import { randomBytes } from "node:crypto";
import { existsSync } from "node:fs";
import * as nodePath from "node:path";
import * as vscode from "vscode";
import {
  ApplyOp,
  ApplyRequest,
  BridgeError,
  BridgeState,
  Selection,
  bridgeApply,
  bridgeState,
} from "./bridge";
import { renderMarkdown } from "./render";

const MUTATION_OPS: ApplyOp[] = [
  "comment",
  "suggestion",
  "edit",
  "resolve",
  "unresolve",
  "delete",
  "reattach",
  "signoff",
];

export class ReviewPanel {
  private static readonly panels = new Map<string, ReviewPanel>();

  private readonly panel: vscode.WebviewPanel;
  private readonly disposables: vscode.Disposable[] = [];
  private state: BridgeState | null = null;
  private renderedSource: string | undefined;
  private chain: Promise<void> = Promise.resolve();
  private sidecarTimer: ReturnType<typeof setTimeout> | undefined;
  private disposed = false;

  static open(
    context: vscode.ExtensionContext,
    uri: vscode.Uri,
  ): ReviewPanel {
    const key = uri.toString();
    const existing = ReviewPanel.panels.get(key);
    if (existing) {
      existing.panel.reveal(vscode.ViewColumn.Beside);
      void existing.refresh("state");
      return existing;
    }
    const panel = new ReviewPanel(context, uri);
    ReviewPanel.panels.set(key, panel);
    return panel;
  }

  static refreshOpen(uri: vscode.Uri): boolean {
    const existing = ReviewPanel.panels.get(uri.toString());
    if (!existing) {
      return false;
    }
    existing.panel.reveal(vscode.ViewColumn.Beside);
    void existing.refresh("state");
    return true;
  }

  private constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly uri: vscode.Uri,
  ) {
    this.panel = vscode.window.createWebviewPanel(
      "agentgloss.review",
      `agentgloss: ${nodePath.basename(uri.fsPath)}`,
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: this.resourceRoots(),
      },
    );
    this.panel.webview.html = this.htmlContent();
    this.panel.onDidDispose(() => this.dispose(), null, this.disposables);
    this.panel.webview.onDidReceiveMessage(
      (message) => void this.onMessage(message),
      null,
      this.disposables,
    );

    vscode.workspace.onDidSaveTextDocument(
      (document) => {
        if (this.matches(document.uri)) {
          void this.refresh("sync");
        }
      },
      null,
      this.disposables,
    );
    vscode.workspace.onDidChangeTextDocument(
      (event) => {
        if (this.matches(event.document.uri)) {
          this.updateDirtyBanner();
        }
      },
      null,
      this.disposables,
    );
    vscode.window.onDidChangeActiveColorTheme(
      () => this.postTheme(),
      null,
      this.disposables,
    );

    const sidecarName = `${nodePath.basename(uri.fsPath)}.review.json`;
    const watcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(
        vscode.Uri.file(nodePath.dirname(uri.fsPath)),
        sidecarName,
      ),
    );
    const schedule = () => this.scheduleSidecarRefresh();
    watcher.onDidChange(schedule, null, this.disposables);
    watcher.onDidCreate(schedule, null, this.disposables);
    watcher.onDidDelete(schedule, null, this.disposables);
    this.disposables.push(watcher);
    this.updateDirtyBanner();
  }

  private resourceRoots(): vscode.Uri[] {
    const roots = [
      this.context.extensionUri,
      vscode.Uri.file(nodePath.dirname(this.uri.fsPath)),
      ...(vscode.workspace.workspaceFolders ?? []).map((folder) => folder.uri),
    ];
    const seen = new Set<string>();
    return roots.filter((root) => {
      const key = root.toString();
      if (seen.has(key)) {
        return false;
      }
      seen.add(key);
      return true;
    });
  }

  private allowedImageRoots(): string[] {
    return [
      nodePath.dirname(this.uri.fsPath),
      ...(vscode.workspace.workspaceFolders ?? []).map(
        (folder) => folder.uri.fsPath,
      ),
    ];
  }

  private matches(uri: vscode.Uri): boolean {
    return uri.toString() === this.uri.toString();
  }

  private binary(): string {
    return vscode.workspace
      .getConfiguration("agentgloss", this.uri)
      .get<string>("path", "agloss");
  }

  private enqueue(task: () => Promise<void>): Promise<void> {
    const run = this.chain.then(task, task);
    this.chain = run.catch(() => undefined);
    return this.chain;
  }

  private refresh(mode: "sync" | "state"): Promise<void> {
    return this.enqueue(async () => {
      try {
        const next =
          mode === "sync"
            ? await bridgeApply(this.binary(), this.uri.fsPath, undefined, {
                schema_version: 1,
                op: "sync",
                include_source: true,
              })
            : await bridgeState(this.binary(), this.uri.fsPath);
        this.state = next;
        this.clearBanners(["locked", "error", "recovered"]);
        this.publish(next, false);
        if (next.recovered) {
          this.setBanner(
            "recovered",
            "warn",
            `Recovered a corrupt review state${
              next.backup_path ? `; backup: ${next.backup_path}` : ""
            }.`,
          );
        }
        this.updateDirtyBanner();
      } catch (error) {
        if (error instanceof BridgeError && error.code === "locked") {
          this.setBanner(
            "locked",
            "warn",
            "agentgloss is open in the terminal. Close it to edit; this view stays readable.",
          );
          try {
            const next = await bridgeState(
              this.binary(),
              this.uri.fsPath,
            );
            this.state = next;
            this.publish(next, false);
          } catch (stateError) {
            this.reportError(stateError);
          }
          return;
        }
        this.reportError(error);
      }
    });
  }

  private publish(next: BridgeState, force = false): void {
    const source = next.source;
    if (typeof source === "string" && (force || source !== this.renderedSource)) {
      const html = renderMarkdown({
        source,
        documentDir: nodePath.dirname(this.uri.fsPath),
        allowedRoots: this.allowedImageRoots(),
        imageUri: (absolutePath) => {
          try {
            return this.panel.webview
              .asWebviewUri(vscode.Uri.file(absolutePath))
              .toString();
          } catch {
            return undefined;
          }
        },
      });
      this.renderedSource = source;
      void this.panel.webview.postMessage({
        type: "render",
        html,
        state: next,
        theme: this.theme(),
      });
      return;
    }
    void this.panel.webview.postMessage({
      type: "state",
      state: next,
      theme: this.theme(),
    });
  }

  private theme(): "dark" | "light" {
    const kind = vscode.window.activeColorTheme.kind;
    return kind === vscode.ColorThemeKind.Light ||
      kind === vscode.ColorThemeKind.HighContrastLight
      ? "light"
      : "dark";
  }

  private postTheme(): void {
    void this.panel.webview.postMessage({
      type: "theme",
      theme: this.theme(),
    });
  }

  private async onMessage(message: any): Promise<void> {
    if (!message || typeof message !== "object") {
      return;
    }
    switch (message.type) {
      case "ready":
        await this.refresh("sync");
        break;
      case "sync":
        await this.refresh("sync");
        break;
      case "refresh":
        await this.refresh("state");
        break;
      case "apply":
        await this.applyRequest(message.request);
        break;
      case "signoff":
        await this.signoff();
        break;
      case "openLink":
        await this.openLink(String(message.href ?? ""));
        break;
    }
  }

  private async applyRequest(partial: any): Promise<void> {
    if (!vscode.workspace.isTrusted) {
      this.setBanner(
        "error",
        "error",
        "Open a trusted workspace to edit the review.",
      );
      return;
    }
    const op = String(partial?.op ?? "") as ApplyOp;
    if (!MUTATION_OPS.includes(op)) {
      this.reportError(new Error(`unknown review operation: ${op}`));
      return;
    }
    const document = await vscode.workspace.openTextDocument(this.uri);
    if (document.isDirty) {
      this.setBanner(
        "dirty",
        "warn",
        "Save the document before editing the review.",
      );
      return;
    }
    if (!this.state) {
      void this.refresh("state");
      return;
    }
    const request: ApplyRequest = {
      schema_version: 1,
      op,
      expect_hash: this.state.doc_hash,
      selection: validSelection(partial?.selection),
      body: typeof partial?.body === "string" ? partial.body : undefined,
      replacement:
        typeof partial?.replacement === "string"
          ? partial.replacement
          : undefined,
      id: typeof partial?.id === "string" ? partial.id : undefined,
      include_source: false,
    };

    await this.enqueue(async () => {
      try {
        const next = await bridgeApply(
          this.binary(),
          this.uri.fsPath,
          undefined,
          request,
        );
        this.state = next;
        this.clearBanners(["locked", "error", "dirty"]);
        this.publish(next, false);
        if (next.id) {
          this.postToast(`Saved ${next.id}`, "info");
        }
        if (op === "signoff") {
          const approved = next.terminal_exit_code === 0;
          void vscode.window.showInformationMessage(
            approved
              ? "agentgloss: revision approved."
              : "agentgloss: feedback sent to the agent.",
          );
        }
        this.updateDirtyBanner();
      } catch (error) {
        if (error instanceof BridgeError && error.code === "locked") {
          this.setBanner(
            "locked",
            "warn",
            "agentgloss is open in the terminal. Close it to edit; this view stays readable.",
          );
          return;
        }
        if (
          error instanceof BridgeError &&
          error.code === "document_changed"
        ) {
          this.setBanner(
            "changed",
            "warn",
            "The document changed; refreshing the review.",
          );
          void this.refresh("state");
          return;
        }
        this.reportError(error);
      }
    });
  }

  private async signoff(): Promise<void> {
    if (!this.state) {
      await this.refresh("state");
    }
    if (!this.state) {
      return;
    }
    const { open, stale } = this.state.counts;
    const message =
      open + stale === 0
        ? "Approve this revision?"
        : `Send ${open} open and ${stale} stale item(s) to the agent?`;
    const choice = await vscode.window.showWarningMessage(
      message,
      { modal: true },
      "Sign off",
    );
    if (choice !== "Sign off") {
      return;
    }
    await this.applyRequest({ op: "signoff" });
  }

  private async openLink(href: string): Promise<void> {
    if (!href) {
      return;
    }
    if (/^(https?|mailto):/i.test(href)) {
      await vscode.env.openExternal(vscode.Uri.parse(href));
      return;
    }
    if (href.startsWith("#")) {
      return;
    }
    try {
      const clean = decodeURIComponent(href.split("#")[0] ?? "");
      const absolute = nodePath.resolve(
        nodePath.dirname(this.uri.fsPath),
        clean,
      );
      if (!existsSync(absolute)) {
        throw new Error("not found");
      }
      await vscode.commands.executeCommand(
        "vscode.open",
        vscode.Uri.file(absolute),
      );
    } catch {
      void vscode.window.showErrorMessage(
        `agentgloss: cannot open link: ${href}`,
      );
    }
  }

  private scheduleSidecarRefresh(): void {
    clearTimeout(this.sidecarTimer);
    this.sidecarTimer = setTimeout(() => {
      void this.refresh("state");
    }, 250);
  }

  private updateDirtyBanner(): void {
    const document = vscode.workspace.textDocuments.find((candidate) =>
      this.matches(candidate.uri),
    );
    if (document?.isDirty) {
      this.setBanner(
        "dirty",
        "warn",
        "Save the document to refresh the review.",
      );
    } else {
      this.setBanner("dirty", "warn", null);
    }
  }

  private setBanner(
    key: string,
    kind: string,
    message: string | null,
  ): void {
    void this.panel.webview.postMessage({
      type: "banner",
      key,
      kind,
      message,
    });
  }

  private clearBanners(keys: string[]): void {
    for (const key of keys) {
      this.setBanner(key, "warn", null);
    }
  }

  private postToast(message: string, kind: string): void {
    void this.panel.webview.postMessage({ type: "toast", message, kind });
  }

  private reportError(error: unknown): void {
    const message =
      error instanceof Error ? error.message : String(error ?? "unknown error");
    this.setBanner("error", "error", message);
    void vscode.window.showErrorMessage(`agentgloss: ${message}`);
  }

  private htmlContent(): string {
    const nonce = randomBytes(16).toString("base64");
    const scriptUri = this.panel.webview.asWebviewUri(
      vscode.Uri.joinPath(this.context.extensionUri, "dist", "webview.js"),
    );
    const styleUri = this.panel.webview.asWebviewUri(
      vscode.Uri.joinPath(this.context.extensionUri, "media", "styles.css"),
    );
    const csp = [
      "default-src 'none'",
      `img-src ${this.panel.webview.cspSource} data:`,
      `style-src ${this.panel.webview.cspSource} 'unsafe-inline'`,
      `font-src ${this.panel.webview.cspSource} data:`,
      `script-src 'nonce-${nonce}'`,
      "connect-src 'none'",
    ].join("; ");
    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="${csp}">
  <link rel="stylesheet" href="${styleUri}">
  <title>agentgloss review</title>
</head>
<body>
  <div id="app"></div>
  <script nonce="${nonce}" src="${scriptUri}"></script>
</body>
</html>`;
  }

  private dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    ReviewPanel.panels.delete(this.uri.toString());
    clearTimeout(this.sidecarTimer);
    for (const disposable of this.disposables) {
      disposable.dispose();
    }
    this.disposables.length = 0;
    this.panel.dispose();
  }
}

function validSelection(value: unknown): Selection | undefined {
  if (!value || typeof value !== "object") {
    return undefined;
  }
  const candidate = value as { start_line?: unknown; end_line?: unknown };
  const start = Number(candidate.start_line);
  const end = Number(candidate.end_line);
  if (
    !Number.isInteger(start) ||
    !Number.isInteger(end) ||
    start < 1 ||
    end < start
  ) {
    return undefined;
  }
  return { start_line: start, end_line: end };
}
