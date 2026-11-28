import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import pkg from "../package.json";

const openReview = "agentgloss.openReview";

describe("extension manifest", () => {
  it("uses the InnopiaPteLtd publisher and branded editor-title button", () => {
    expect(pkg.publisher).toBe("InnopiaPteLtd");
    const command = pkg.contributes.commands.find(
      (entry) => entry.command === openReview,
    );
    expect(command?.icon).toEqual({
      light: "media/review-icon-light.svg",
      dark: "media/review-icon-dark.svg",
    });
    expect(pkg.icon).toBe("media/icon.png");
    expect(existsSync(pkg.icon)).toBe(true);
    const icon = readFileSync(pkg.icon);
    expect(icon.subarray(1, 4).toString("ascii")).toBe("PNG");
    expect(icon.readUInt32BE(16)).toBe(128);
    expect(icon.readUInt32BE(20)).toBe(128);
    expect(existsSync("media/review-icon-light.svg")).toBe(true);
    expect(existsSync("media/review-icon-dark.svg")).toBe(true);
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
