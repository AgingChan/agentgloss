import { existsSync } from "node:fs";
import * as nodePath from "node:path";
import MarkdownIt from "markdown-it";
import footnote from "markdown-it-footnote";
import taskLists from "markdown-it-task-lists";

export interface LineRange {
  start_line: number;
  end_line: number;
}

export interface RenderOptions {
  source: string;
  documentDir: string;
  allowedRoots: string[];
  imageUri: (absolutePath: string) => string | undefined;
}

export function isSafeHref(href: string): boolean {
  const trimmed = href.trim();
  if (trimmed.startsWith("#")) {
    return true;
  }
  if (/^(https?|mailto):/i.test(trimmed)) {
    return true;
  }
  if (/^[a-z][a-z0-9+.-]*:/i.test(trimmed)) {
    return false;
  }
  return true;
}

export function isInside(root: string, absolutePath: string): boolean {
  const relative = nodePath.relative(root, absolutePath);
  return (
    relative === "" ||
    (!relative.startsWith("..") && !nodePath.isAbsolute(relative))
  );
}

export function resolveLocalImage(
  src: string,
  documentDir: string,
  allowedRoots: string[],
  imageUri: (absolutePath: string) => string | undefined,
): string | undefined {
  const trimmed = src.trim();
  if (
    !trimmed ||
    /^[a-z][a-z0-9+.-]*:/i.test(trimmed) ||
    trimmed.startsWith("//")
  ) {
    return undefined;
  }
  const withoutQuery = trimmed.split(/[?#]/)[0] ?? "";
  let decoded: string;
  try {
    decoded = decodeURIComponent(withoutQuery);
  } catch {
    return undefined;
  }
  const absolute = nodePath.resolve(documentDir, decoded);
  if (!allowedRoots.some((root) => isInside(root, absolute))) {
    return undefined;
  }
  if (!existsSync(absolute)) {
    return undefined;
  }
  return imageUri(absolute);
}

export function fenceInnerRange(
  map: [number, number] | null | undefined,
  sourceLines: string[],
): LineRange | null {
  if (!map) {
    return null;
  }
  const [start, end] = map;
  let contentStart = start + 2;
  let contentEnd = end;
  const lastLine = (sourceLines[end - 1] ?? "").replace(/\r$/, "");
  if (/^\s{0,3}(`{3,}|~{3,})\s*$/.test(lastLine)) {
    contentEnd = end - 1;
  }
  while (contentEnd >= contentStart) {
    const line = (sourceLines[contentEnd - 1] ?? "").replace(/\r$/, "");
    if (line.trim() !== "") {
      break;
    }
    contentEnd -= 1;
  }
  if (contentEnd < contentStart) {
    return { start_line: start + 1, end_line: start + 1 };
  }
  return { start_line: contentStart, end_line: contentEnd };
}

function tokenRange(
  token: { map?: [number, number] | null },
  sourceLines: string[],
): LineRange | null {
  if (!token.map) {
    return null;
  }
  const start = token.map[0] + 1;
  let end = Math.max(start, token.map[1]);
  while (end > start && (sourceLines[end - 1] ?? "").trim() === "") {
    end -= 1;
  }
  return { start_line: start, end_line: end };
}

function rangeAttrs(range: LineRange | null, kind: string): string {
  if (!range) {
    return "";
  }
  return ` data-start-line="${range.start_line}" data-end-line="${range.end_line}" data-kind="${kind}"`;
}

function safeClass(value: string): string {
  return value.replace(/[^a-zA-Z0-9_-]/g, "");
}

export function renderMarkdown(options: RenderOptions): string {
  const md = new MarkdownIt({
    html: false,
    linkify: false,
    typographer: false,
  });
  md.use(taskLists, { label: true });
  md.use(footnote);

  const sourceLines = options.source.split("\n");
  const rules = md.renderer.rules as Record<string, any>;
  const annotate = (rule: string, kind: string) => {
    const original = rules[rule];
    rules[rule] = (
      tokens: any[],
      idx: number,
      opts: unknown,
      env: unknown,
      self: any,
    ) => {
      const token = tokens[idx];
      const range = tokenRange(token, sourceLines);
      if (range) {
        token.attrSet("data-start-line", String(range.start_line));
        token.attrSet("data-end-line", String(range.end_line));
        token.attrSet("data-kind", kind);
      }
      return original
        ? original(tokens, idx, opts, env, self)
        : self.renderToken(tokens, idx, opts);
    };
  };

  annotate("heading_open", "heading");
  annotate("paragraph_open", "paragraph");
  annotate("list_item_open", "list_item");
  annotate("table_open", "table");

  rules.hr = (tokens: any[], idx: number) => {
    const range = tokenRange(tokens[idx], sourceLines);
    return `<hr${rangeAttrs(range, "rule")}>\n`;
  };

  rules.fence = (tokens: any[], idx: number) => {
    const token = tokens[idx];
    const info = String(token.info ?? "").trim();
    const language = (info.split(/\s+/)[0] ?? "").toLowerCase();
    const range = fenceInnerRange(token.map, sourceLines);
    const content = md.utils.escapeHtml(String(token.content ?? ""));
    if (language === "mermaid") {
      return `<div class="mermaid-block"${rangeAttrs(
        range,
        "mermaid",
      )}><pre class="mermaid-source">${content}</pre></div>\n`;
    }
    const languageClass = language
      ? ` class="language-${safeClass(language)}"`
      : "";
    return `<div class="code-block"${rangeAttrs(
      range,
      "code_block",
    )}><pre><code${languageClass}>${content}</code></pre></div>\n`;
  };

  rules.code_block = (tokens: any[], idx: number) => {
    const token = tokens[idx];
    const range = tokenRange(token, sourceLines);
    return `<div class="code-block"${rangeAttrs(
      range,
      "code_block",
    )}><pre><code>${md.utils.escapeHtml(
      String(token.content ?? ""),
    )}</code></pre></div>\n`;
  };

  rules.image = (tokens: any[], idx: number) => {
    const token = tokens[idx];
    const src = String(token.attrGet("src") ?? "");
    const alt = String(token.content ?? "");
    const title = String(token.attrGet("title") ?? "");
    const local = resolveLocalImage(
      src,
      options.documentDir,
      options.allowedRoots,
      options.imageUri,
    );
    if (local) {
      const titleAttr = title
        ? ` title="${md.utils.escapeHtml(title)}"`
        : "";
      return `<img src="${md.utils.escapeHtml(
        local,
      )}" alt="${md.utils.escapeHtml(alt)}"${titleAttr}>`;
    }
    const href = isSafeHref(src) ? src : "#";
    return `<a class="image-link" href="${md.utils.escapeHtml(
      href,
    )}">[image: ${md.utils.escapeHtml(alt || src)}]</a>`;
  };

  const defaultLinkOpen = rules.link_open;
  rules.link_open = (
    tokens: any[],
    idx: number,
    opts: unknown,
    env: unknown,
    self: any,
  ) => {
    const token = tokens[idx];
    const href = String(token.attrGet("href") ?? "");
    if (!isSafeHref(href)) {
      token.attrSet("href", "#");
      token.attrSet("data-blocked", "true");
    }
    token.attrSet("rel", "noopener noreferrer");
    return defaultLinkOpen
      ? defaultLinkOpen(tokens, idx, opts, env, self)
      : self.renderToken(tokens, idx, opts);
  };

  return md.render(options.source);
}
