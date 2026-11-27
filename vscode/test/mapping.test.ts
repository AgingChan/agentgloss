import { describe, expect, it } from "vitest";
import { findTargetElement, selectionLineRange } from "../src/webview/mapping";
import { renderMarkdown } from "../src/render";

function mount(source: string): HTMLElement {
  document.body.innerHTML = `<main id="doc">${renderMarkdown({
    source,
    documentDir: "/tmp",
    allowedRoots: ["/tmp"],
    imageUri: () => undefined,
  })}</main>`;
  return document.getElementById("doc") as HTMLElement;
}

function rangeFor(node: Node): Range {
  const range = document.createRange();
  range.selectNodeContents(node);
  return range;
}

describe("selection mapping", () => {
  it("uses annotations, not duplicate text, to locate a selection", () => {
    const root = mount("alpha\n\nalpha\n");
    const paragraphs = Array.from(root.querySelectorAll("p"));
    const range = rangeFor(paragraphs[1].firstChild as Node);
    expect(selectionLineRange(root, range)).toEqual({
      start_line: 3,
      end_line: 3,
    });
  });

  it("reduces nested list items to the leaf-most annotated element", () => {
    const root = mount("- parent\n  - nested\n");
    const nested = Array.from(root.querySelectorAll("li")).find(
      (element) => element.textContent?.trim() === "nested",
    ) as HTMLElement;
    const range = rangeFor(nested.firstChild as Node);
    expect(selectionLineRange(root, range)).toEqual({
      start_line: 2,
      end_line: 2,
    });
  });

  it("unions a selection that spans blocks", () => {
    const root = mount("alpha\n\nbeta\n");
    const paragraphs = Array.from(root.querySelectorAll("p"));
    const range = document.createRange();
    range.setStart(paragraphs[0].firstChild as Node, 1);
    range.setEnd(paragraphs[1].firstChild as Node, 2);
    expect(selectionLineRange(root, range)).toEqual({
      start_line: 1,
      end_line: 3,
    });
  });

  it("ignores collapsed selections", () => {
    const root = mount("alpha\n");
    const range = document.createRange();
    range.setStart(root.querySelector("p")!.firstChild as Node, 1);
    range.collapse(true);
    expect(selectionLineRange(root, range)).toBeNull();
  });

  it("finds the tightest annotated target for an item range", () => {
    const root = mount("# Title\n\nalpha\n\n- parent\n  - nested\n");
    const target = findTargetElement(root, { start_line: 5, end_line: 5 });
    expect(target?.tagName).toBe("LI");
    const heading = findTargetElement(root, { start_line: 1, end_line: 1 });
    expect(heading?.tagName).toBe("H1");
  });
});
