import { describe, expect, it } from "vitest";
import pkg from "../package.json";

const openReview = "agentgloss.openReview";

describe("extension manifest", () => {
  it("adds a Markdown-editor title button with an icon", () => {
    const command = pkg.contributes.commands.find(
      (entry) => entry.command === openReview,
    );
    expect(command?.icon).toBe("$(open-preview)");
    expect(pkg.contributes.menus["editor/title"]).toContainEqual(
      expect.objectContaining({
        command: openReview,
        when: "resourceLangId == markdown",
      }),
    );
  });

  it("adds file-explorer and editor-tab context entries", () => {
    expect(pkg.contributes.menus["explorer/context"]).toContainEqual(
      expect.objectContaining({ command: openReview }),
    );
    expect(pkg.contributes.menus["editor/title/context"]).toContainEqual(
      expect.objectContaining({ command: openReview }),
    );
    expect(pkg.contributes.menus["editor/context"]).toContainEqual(
      expect.objectContaining({
        command: "agentgloss.commentSelection",
      }),
    );
  });
});
