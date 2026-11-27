import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import * as nodePath from "node:path";
import { describe, expect, it } from "vitest";
import { renderMarkdown } from "../src/render";

function render(source: string, documentDir = tmpdir()) {
  return renderMarkdown({
    source,
    documentDir,
    allowedRoots: [documentDir],
    imageUri: (absolutePath) => `webview://${absolutePath}`,
  });
}

function parse(html: string): Document {
  return new DOMParser().parseFromString(html, "text/html");
}

describe("markdown annotations", () => {
  it("annotates duplicate paragraphs, nested list items, and tables", () => {
    const source = [
      "# Title",
      "",
      "alpha",
      "",
      "alpha",
      "",
      "- one",
      "  - nested",
      "",
      "| A | B |",
      "|---|---|",
      "| 1 | 2 |",
      "",
    ].join("\n");
    const document = parse(render(source));
    const paragraphs = Array.from(
      document.querySelectorAll<HTMLElement>("p[data-start-line]"),
    );
    expect(paragraphs.map((element) => element.dataset.startLine)).toEqual([
      "3",
      "5",
    ]);
    const nested = Array.from(
      document.querySelectorAll<HTMLElement>("li[data-start-line]"),
    ).find((element) => element.textContent?.trim() === "nested");
    expect(nested?.dataset.startLine).toBe("8");
    expect(nested?.dataset.endLine).toBe("8");
    const table = document.querySelector<HTMLElement>(
      "table[data-start-line]",
    );
    expect(table?.dataset.startLine).toBe("10");
    expect(table?.dataset.endLine).toBe("12");
  });

  it("uses content-only ranges for code and Mermaid fences", () => {
    const source = [
      "```mermaid",
      "flowchart LR",
      "  A-->B",
      "```",
      "",
      "```rust",
      "fn main() {}",
      "```",
      "",
    ].join("\n");
    const document = parse(render(source));
    const mermaidBlock = document.querySelector(".mermaid-block");
    expect(mermaidBlock?.getAttribute("data-start-line")).toBe("2");
    expect(mermaidBlock?.getAttribute("data-end-line")).toBe("3");
    expect(
      mermaidBlock?.querySelector(".mermaid-source")?.textContent,
    ).toContain("A-->B");
    const codeBlock = document.querySelector(".code-block[data-kind='code_block']");
    expect(codeBlock?.getAttribute("data-start-line")).toBe("7");
    expect(codeBlock?.getAttribute("data-end-line")).toBe("7");
  });

  it("extends an unclosed fence to the last content line", () => {
    const source = "```mermaid\nflowchart LR\nA-->B\n";
    const document = parse(render(source));
    const block = document.querySelector(".mermaid-block");
    expect(block?.getAttribute("data-start-line")).toBe("2");
    expect(block?.getAttribute("data-end-line")).toBe("3");
  });

  it("escapes raw HTML instead of injecting it", () => {
    const html = render("<script>alert(1)</script>\n");
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
  });
});

describe("links and images", () => {
  it("blocks unsafe link protocols", () => {
    const document = parse(
      render("[bad](javascript:alert(1)) [good](https://example.com)\n"),
    );
    const links = Array.from(document.querySelectorAll("a"));
    expect(
      links.some((link) =>
        (link.getAttribute("href") ?? "").toLowerCase().startsWith("javascript:"),
      ),
    ).toBe(false);
    const good = links.find((link) => link.textContent === "good");
    expect(good?.getAttribute("href")).toBe("https://example.com");
  });

  it("renders local images and turns remote images into links", () => {
    const directory = mkdtempSync(nodePath.join(tmpdir(), "agentgloss-vsc-"));
    writeFileSync(nodePath.join(directory, "pic.png"), "not-a-real-png");
    const document = parse(
      render(
        "![local](pic.png)\n\n![remote](https://example.com/a.png)\n",
        directory,
      ),
    );
    const image = document.querySelector("img");
    expect(image?.getAttribute("src")).toBe(
      `webview://${nodePath.join(directory, "pic.png")}`,
    );
    expect(image?.getAttribute("alt")).toBe("local");
    const remote = document.querySelector("a.image-link");
    expect(remote?.textContent).toContain("remote");
    expect(remote?.getAttribute("href")).toBe("https://example.com/a.png");
  });

  it("rejects images outside the allowed roots", () => {
    const directory = mkdtempSync(nodePath.join(tmpdir(), "agentgloss-vsc-"));
    writeFileSync(nodePath.join(tmpdir(), "outside.png"), "x");
    const document = parse(render("![outside](../outside.png)\n", directory));
    expect(document.querySelector("img")).toBeNull();
    expect(document.querySelector("a.image-link")).not.toBeNull();
  });
});
