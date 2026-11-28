import { describe, expect, it } from "vitest";
import {
  BridgeError,
  parseLastJson,
  runBridge,
} from "../src/bridge";

describe("bridge envelope parsing", () => {
  it("parses the last JSON line among diagnostic output", () => {
    const stdout = [
      "warning: something",
      '{"ok":true,"schema_version":1,"value":1}',
      "",
    ].join("\n");
    expect(parseLastJson(stdout)).toEqual({
      ok: true,
      schema_version: 1,
      value: 1,
    });
  });

  it("returns null when no JSON envelope exists", () => {
    expect(parseLastJson("not json\n")).toBeNull();
  });

  it("resolves success envelopes", async () => {
    const script =
      'console.log(JSON.stringify({ok:true,schema_version:1,status:"pending"}))';
    const value = await runBridge(process.execPath, ["-e", script]);
    expect(value.status).toBe("pending");
  });

  it("maps error envelopes to BridgeError codes", async () => {
    const script =
      'console.log(JSON.stringify({ok:false,error:{code:"locked",message:"locked"}}));process.exit(2)';
    await expect(
      runBridge(process.execPath, ["-e", script]),
    ).rejects.toMatchObject({
      code: "locked",
    } satisfies Partial<BridgeError>);
  });

  it("reports a missing binary", async () => {
    await expect(
      runBridge("/definitely/missing/agloss", ["bridge", "state", "x.md"]),
    ).rejects.toMatchObject({
      code: "not_found",
    } satisfies Partial<BridgeError>);
  });
});
