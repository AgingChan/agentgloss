import mermaid from "mermaid";
import {
  findTargetElement,
  itemLineRange,
  LineRange,
  selectionLineRange,
} from "./mapping";

interface VsCodeApi {
  postMessage(message: unknown): void;
  getState(): unknown;
  setState(state: unknown): void;
}

declare function acquireVsCodeApi(): VsCodeApi;

const vscode = acquireVsCodeApi();
let state: any = null;
let theme: "dark" | "light" = "dark";
let sourceHtml = "";
let showResolved = false;
let pendingSelection: LineRange | null = null;
let mermaidCounter = 0;
let mermaidGeneration = 0;
let selectionTimer: number | undefined;
let toastTimer: number | undefined;

const bannerState = new Map<string, { kind: string; message: string }>();

const app = document.getElementById("app");
if (!app) {
  throw new Error("agentgloss: missing webview root");
}
app.innerHTML = `
  <div id="banners" class="banners"></div>
  <div id="summary" class="summary"></div>
  <main id="doc" class="doc" role="document"></main>
  <div id="toolbar" class="selection-toolbar hidden">
    <span id="toolbar-range" class="toolbar-range"></span>
    <button data-toolbar="comment">Comment</button>
    <button data-toolbar="suggest">Suggest edit</button>
  </div>
  <div id="composer" class="composer hidden"></div>
  <div id="toast" class="toast hidden"></div>
`;

const bannersEl = document.getElementById("banners") as HTMLElement;
const summaryEl = document.getElementById("summary") as HTMLElement;
const docEl = document.getElementById("doc") as HTMLElement;
const toolbarEl = document.getElementById("toolbar") as HTMLElement;
const toolbarRangeEl = document.getElementById("toolbar-range") as HTMLElement;
const composerEl = document.getElementById("composer") as HTMLElement;
const toastEl = document.getElementById("toast") as HTMLElement;

toolbarEl.addEventListener("mousedown", (event) => event.preventDefault());
toolbarEl.addEventListener("click", (event) => {
  const button = (event.target as Element).closest<HTMLButtonElement>(
    "button[data-toolbar]",
  );
  if (!button || !pendingSelection) {
    return;
  }
  if (button.dataset.toolbar === "comment") {
    openComposerForSelection("comment");
  } else if (button.dataset.toolbar === "suggest") {
    openComposerForSelection("suggestion");
  }
});

docEl.addEventListener("click", (event) => {
  const target = event.target as Element;
  const button = target.closest<HTMLButtonElement>("button[data-action]");
  if (button) {
    event.preventDefault();
    handleThreadAction(button);
    return;
  }
  const link = target.closest<HTMLAnchorElement>("a[href]");
  if (link) {
    event.preventDefault();
    const href = link.getAttribute("href") ?? "";
    if (href.startsWith("#")) {
      const id = decodeURIComponent(href.slice(1));
      document.getElementById(id)?.scrollIntoView({
        behavior: "smooth",
        block: "center",
      });
      return;
    }
    if (link.dataset.blocked === "true") {
      showToast("Blocked unsafe link", "warn");
      return;
    }
    vscode.postMessage({ type: "openLink", href });
  }
});

window.addEventListener("message", (event: MessageEvent) => {
  const message = event.data as any;
  if (!message || typeof message !== "object") {
    return;
  }
  switch (message.type) {
    case "render":
      state = message.state;
      theme = message.theme === "light" ? "light" : "dark";
      sourceHtml = String(message.html ?? "");
      draw();
      break;
    case "state":
      state = message.state;
      if (message.theme === "light" || message.theme === "dark") {
        theme = message.theme;
      }
      drawState();
      break;
    case "theme":
      theme = message.theme === "light" ? "light" : "dark";
      void renderMermaid();
      break;
    case "banner":
      setBanner(
        String(message.key ?? "banner"),
        String(message.kind ?? "warn"),
        message.message == null ? null : String(message.message),
      );
      break;
    case "toast":
      showToast(String(message.message ?? ""), String(message.kind ?? "info"));
      break;
  }
});

vscode.postMessage({ type: "ready" });

document.addEventListener("selectionchange", () => {
  window.clearTimeout(selectionTimer);
  selectionTimer = window.setTimeout(updateToolbar, 120);
});
document.addEventListener("mouseup", updateToolbar);
composerEl.addEventListener("mousedown", (event) => {
  if (event.target === composerEl) {
    closeComposer();
  }
});

function draw(): void {
  closeComposer();
  hideToolbar();
  docEl.innerHTML = sourceHtml;
  drawState();
  void renderMermaid();
}

function drawState(): void {
  if (!state) {
    return;
  }
  renderSummary();
  updateDerivedBanners();
  renderThreads();
}

function renderSummary(): void {
  summaryEl.replaceChildren();
  const review = state.review;
  const counts = state.counts ?? {
    open: 0,
    stale: 0,
    resolved: 0,
    total: 0,
  };

  const main = el("div", "summary-main");
  const status = el("span", `status status-${safeClass(String(state.status))}`);
  status.textContent = String(state.status ?? "pending");
  const round = el("span", "summary-meta");
  round.textContent = `Round ${review?.round ?? 1}`;
  const countsEl = el("span", "summary-meta");
  countsEl.textContent = `${counts.open} open · ${counts.stale} stale · ${counts.resolved} resolved`;
  main.append(status, round, countsEl);
  if (review?.last_signoff) {
    const signoff = el("span", "summary-meta");
    signoff.textContent = `Last signoff: ${review.last_signoff}`;
    main.append(signoff);
  }

  const actions = el("div", "summary-actions");
  const toggle = el("label", "resolved-toggle");
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.checked = showResolved;
  checkbox.addEventListener("change", () => {
    showResolved = checkbox.checked;
    renderThreads();
  });
  toggle.append(checkbox, document.createTextNode(" Show resolved"));
  const sync = button("Sync", "sync");
  sync.addEventListener("click", () => vscode.postMessage({ type: "sync" }));
  const signoff = button("Sign off", "signoff");
  signoff.addEventListener("click", () =>
    vscode.postMessage({ type: "signoff" }),
  );
  actions.append(toggle, sync, signoff);

  summaryEl.append(main, actions);
}

function updateDerivedBanners(): void {
  if (state?.doc_changed) {
    setBanner(
      "changed",
      "warn",
      "The document changed on disk. Run Sync to re-anchor items.",
    );
  } else {
    setBanner("changed", "warn", null);
  }
  if (state?.status === "invalidated") {
    setBanner(
      "invalidated",
      "warn",
      "The previous approval no longer matches this document.",
    );
  } else {
    setBanner("invalidated", "warn", null);
  }
}

function setBanner(
  key: string,
  kind: string,
  message: string | null,
): void {
  if (message == null || message === "") {
    bannerState.delete(key);
  } else {
    bannerState.set(key, { kind, message });
  }
  renderBanners();
}

function renderBanners(): void {
  bannersEl.replaceChildren();
  for (const { kind, message } of bannerState.values()) {
    const banner = el("div", `banner banner-${safeClass(kind)}`);
    banner.textContent = message;
    bannersEl.append(banner);
  }
}

function renderThreads(): void {
  docEl
    .querySelectorAll(".thread, .stale-section, .detached-note")
    .forEach((node) => node.remove());
  const items: any[] = Array.isArray(state?.review?.items)
    ? state.review.items
    : [];
  if (items.length === 0) {
    return;
  }
  const staleItems: any[] = [];
  const insertAfter = new Map<HTMLElement, HTMLElement>();
  const ordered = [...items].sort(
    (left, right) =>
      left.anchor.line_start - right.anchor.line_start ||
      String(left.id).localeCompare(String(right.id)),
  );
  for (const item of ordered) {
    if (item.status === "resolved" && !showResolved) {
      continue;
    }
    const range = itemLineRange(item);
    if (item.status === "stale") {
      staleItems.push(item);
      continue;
    }
    const target = findTargetElement(docEl, range);
    const thread = buildThread(item, range, false);
    if (target) {
      if (target.tagName === "LI") {
        target.append(thread);
        insertAfter.set(target, thread);
      } else {
        const previous = insertAfter.get(target) ?? target;
        previous.insertAdjacentElement("afterend", thread);
        insertAfter.set(target, thread);
      }
    } else {
      const note = el("div", "detached-note");
      note.textContent = `${item.id} is attached to lines ${range.start_line}-${range.end_line} (no rendered block found).`;
      thread.prepend(note);
      docEl.append(thread);
    }
  }
  if (staleItems.length > 0) {
    const section = el("section", "stale-section");
    const heading = el("h3", "stale-heading");
    heading.textContent = `Stale items (${staleItems.length})`;
    section.append(heading);
    const help = el("p", "stale-help");
    help.textContent =
      "These items could not be re-anchored. Select the intended block, then choose Reattach.";
    section.append(help);
    for (const item of staleItems) {
      section.append(buildThread(item, itemLineRange(item), true));
    }
    docEl.append(section);
  }
}

function buildThread(
  item: any,
  range: LineRange,
  stale: boolean,
): HTMLElement {
  const thread = el(
    "div",
    `thread thread-${safeClass(item.status)}${stale ? " stale-card" : ""}`,
  );
  thread.dataset.id = String(item.id);

  const header = el("div", "thread-header");
  const kind = el("span", "thread-kind");
  kind.textContent = item.kind === "suggestion" ? "✎ suggestion" : "● comment";
  const id = el("span", "thread-id");
  id.textContent = String(item.id);
  const status = el("span", `thread-status ${safeClass(item.status)}`);
  status.textContent = String(item.status);
  const lines = el("span", "thread-lines");
  lines.textContent = `Lines ${range.start_line}-${range.end_line}`;
  header.append(kind, id, status, lines);
  thread.append(header);

  if (item.kind === "suggestion" && item.suggestion) {
    const diff = el("div", "thread-suggestion");
    const original = el("pre", "suggestion-original");
    original.textContent = String(item.suggestion.original ?? "");
    const replacement = el("pre", "suggestion-replacement");
    replacement.textContent = String(item.suggestion.replacement ?? "");
    diff.append(
      el("span", "suggestion-label"),
      original,
      el("span", "suggestion-label"),
      replacement,
    );
    const labels = diff.querySelectorAll(".suggestion-label");
    if (labels[0]) {
      labels[0].textContent = "Original";
    }
    if (labels[1]) {
      labels[1].textContent = "Replacement";
    }
    thread.append(diff);
  }

  if (item.body) {
    const body = el("div", "thread-body");
    body.textContent = String(item.body);
    thread.append(body);
  }

  const actions = el("div", "thread-actions");
  if (stale) {
    actions.append(
      actionButton("reattach", "Reattach"),
      actionButton("edit", "Edit"),
      actionButton("delete", "Delete"),
    );
  } else if (item.status === "open") {
    actions.append(
      actionButton("resolve", "Resolve"),
      actionButton("edit", "Edit"),
      actionButton("delete", "Delete"),
    );
  } else {
    actions.append(
      actionButton("reopen", "Reopen"),
      actionButton("edit", "Edit"),
      actionButton("delete", "Delete"),
    );
  }
  thread.append(actions);
  return thread;
}

function actionButton(action: string, label: string): HTMLButtonElement {
  const element = button(label, action);
  element.dataset.action = action;
  return element;
}

function handleThreadAction(button: HTMLButtonElement): void {
  const thread = button.closest<HTMLElement>(".thread");
  const id = thread?.dataset.id;
  const action = button.dataset.action;
  if (!id || !action) {
    return;
  }
  const item = findItem(id);
  switch (action) {
    case "resolve":
      sendApply({ op: "resolve", id });
      break;
    case "reopen":
      sendApply({ op: "unresolve", id });
      break;
    case "delete":
      openComposer({
        title: `Delete ${id}?`,
        submitLabel: "Delete",
        fields: [],
        onSubmit: () => {
          sendApply({ op: "delete", id });
          return true;
        },
      });
      break;
    case "edit":
      if (item) {
        openEditor(item);
      }
      break;
    case "reattach":
      if (!pendingSelection) {
        showToast("Select the intended block first, then choose Reattach.", "warn");
        return;
      }
      sendApply({
        op: "reattach",
        id,
        selection: pendingSelection,
      });
      break;
  }
}

function findItem(id: string): any {
  return (state?.review?.items ?? []).find((item: any) => item.id === id);
}

function sendApply(partial: Record<string, unknown>): void {
  closeComposer();
  vscode.postMessage({
    type: "apply",
    request: { ...partial, include_source: false },
  });
}

function updateToolbar(): void {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) {
    hideToolbar();
    return;
  }
  if (!docEl.contains(selection.anchorNode) && !docEl.contains(selection.focusNode)) {
    hideToolbar();
    return;
  }
  const range = selection.getRangeAt(0);
  const lines = selectionLineRange(docEl, range);
  if (!lines) {
    hideToolbar();
    return;
  }
  pendingSelection = lines;
  toolbarRangeEl.textContent = `Lines ${lines.start_line}-${lines.end_line}`;
  const rect = range.getBoundingClientRect();
  const left = Math.min(
    Math.max(rect.left + rect.width / 2, 120),
    window.innerWidth - 120,
  );
  const top = Math.max(rect.top - 44, 8);
  toolbarEl.style.left = `${left}px`;
  toolbarEl.style.top = `${top}px`;
  toolbarEl.classList.remove("hidden");
}

function hideToolbar(): void {
  toolbarEl.classList.add("hidden");
  pendingSelection = null;
}

function openComposerForSelection(kind: "comment" | "suggestion"): void {
  if (!pendingSelection) {
    return;
  }
  const selection = pendingSelection;
  if (kind === "comment") {
    openComposer({
      title: `Comment on lines ${selection.start_line}-${selection.end_line}`,
      submitLabel: "Add comment",
      fields: [{ key: "body", label: "Comment", value: "", multiline: true }],
      onSubmit: (values) => {
        if (!values.body.trim()) {
          showToast("Comment cannot be empty.", "warn");
          return false;
        }
        sendApply({ op: "comment", selection, body: values.body });
        return true;
      },
    });
  } else {
    openComposer({
      title: `Suggest an edit for lines ${selection.start_line}-${selection.end_line}`,
      submitLabel: "Add suggestion",
      fields: [
        {
          key: "replacement",
          label: "Replacement",
          value: sourceForLines(selection),
          multiline: true,
        },
        {
          key: "body",
          label: "Note (optional)",
          value: "",
          multiline: true,
        },
      ],
      onSubmit: (values) => {
        if (!values.replacement.trim()) {
          showToast("Replacement cannot be empty.", "warn");
          return false;
        }
        sendApply({
          op: "suggestion",
          selection,
          body: values.body,
          replacement: values.replacement,
        });
        return true;
      },
    });
  }
}

function openEditor(item: any): void {
  const range = itemLineRange(item);
  if (item.kind === "comment") {
    openComposer({
      title: `Edit ${item.id}`,
      submitLabel: "Save",
      fields: [
        {
          key: "body",
          label: "Comment",
          value: String(item.body ?? ""),
          multiline: true,
        },
      ],
      onSubmit: (values) => {
        if (!values.body.trim()) {
          showToast("Comment cannot be empty.", "warn");
          return false;
        }
        sendApply({ op: "edit", id: item.id, body: values.body });
        return true;
      },
    });
  } else {
    openComposer({
      title: `Edit ${item.id} (lines ${range.start_line}-${range.end_line})`,
      submitLabel: "Save",
      fields: [
        {
          key: "replacement",
          label: "Replacement",
          value: String(item.suggestion?.replacement ?? ""),
          multiline: true,
        },
        {
          key: "body",
          label: "Note (optional)",
          value: String(item.body ?? ""),
          multiline: true,
        },
      ],
      onSubmit: (values) => {
        if (!values.replacement.trim()) {
          showToast("Replacement cannot be empty.", "warn");
          return false;
        }
        sendApply({
          op: "edit",
          id: item.id,
          body: values.body,
          replacement: values.replacement,
        });
        return true;
      },
    });
  }
}

interface ComposerField {
  key: string;
  label: string;
  value: string;
  multiline: boolean;
}

function openComposer(options: {
  title: string;
  submitLabel: string;
  fields: ComposerField[];
  onSubmit: (values: Record<string, string>) => boolean;
}): void {
  composerEl.replaceChildren();
  const card = el("div", "composer-card");
  const heading = el("h2", "composer-title");
  heading.textContent = options.title;
  card.append(heading);

  const inputs = new Map<string, HTMLTextAreaElement | HTMLInputElement>();
  for (const field of options.fields) {
    const label = el("label", "composer-field");
    const caption = el("span", "composer-label");
    caption.textContent = field.label;
    let input: HTMLTextAreaElement | HTMLInputElement;
    if (field.multiline) {
      const textarea = document.createElement("textarea");
      textarea.rows = field.key === "replacement" ? 6 : 4;
      textarea.value = field.value;
      input = textarea;
    } else {
      const text = document.createElement("input");
      text.type = "text";
      text.value = field.value;
      input = text;
    }
    input.spellcheck = true;
    label.append(caption, input);
    card.append(label);
    inputs.set(field.key, input);
  }

  const actions = el("div", "composer-actions");
  const cancel = button("Cancel", "cancel");
  cancel.addEventListener("click", () => closeComposer());
  const submit = button(options.submitLabel, "primary");
  submit.addEventListener("click", () => {
    const values: Record<string, string> = {};
    for (const [key, input] of inputs) {
      values[key] = input.value;
    }
    if (options.onSubmit(values)) {
      closeComposer();
    }
  });
  actions.append(cancel, submit);
  card.append(actions);
  composerEl.append(card);
  composerEl.classList.remove("hidden");
  const first = inputs.values().next().value;
  first?.focus();
}

function closeComposer(): void {
  composerEl.classList.add("hidden");
  composerEl.replaceChildren();
}

function sourceForLines(range: LineRange): string {
  const lines = String(state?.source ?? "").split("\n");
  return lines
    .slice(range.start_line - 1, range.end_line)
    .map((line) => line.replace(/\r$/, ""))
    .join("\n");
}

async function renderMermaid(): Promise<void> {
  const generation = ++mermaidGeneration;
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    suppressErrorRendering: true,
    theme: theme === "dark" ? "dark" : "default",
  });
  const blocks = Array.from(
    docEl.querySelectorAll<HTMLElement>(".mermaid-block"),
  );
  for (const block of blocks) {
    const source = block.querySelector(".mermaid-source")?.textContent ?? "";
    block.querySelector("svg")?.remove();
    block.querySelector(".mermaid-error")?.remove();
    block.classList.remove("rendered", "failed");
    if (!source.trim()) {
      continue;
    }
    try {
      const result = await mermaid.render(
        `agentgloss-mermaid-${++mermaidCounter}`,
        source,
      );
      if (generation !== mermaidGeneration) {
        return;
      }
      block.insertAdjacentHTML("beforeend", result.svg);
      block.classList.add("rendered");
    } catch {
      if (generation !== mermaidGeneration) {
        return;
      }
      block.classList.add("failed");
      const note = el("div", "mermaid-error");
      note.textContent = "Diagram failed to render; showing source.";
      block.append(note);
    }
  }
}

function showToast(message: string, kind: string): void {
  if (!message) {
    return;
  }
  toastEl.textContent = message;
  toastEl.className = `toast toast-${safeClass(kind)}`;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toastEl.className = "toast hidden";
  }, 3500);
}

function el(tag: string, className = ""): HTMLElement {
  const element = document.createElement(tag);
  if (className) {
    element.className = className;
  }
  return element;
}

function button(label: string, className = ""): HTMLButtonElement {
  const element = document.createElement("button");
  element.type = "button";
  if (className) {
    element.className = className;
  }
  element.textContent = label;
  return element;
}

function safeClass(value: string): string {
  return value.replace(/[^a-zA-Z0-9_-]/g, "-");
}
