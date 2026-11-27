import { spawn } from "node:child_process";

export interface Selection {
  start_line: number;
  end_line: number;
}

export type ApplyOp =
  | "sync"
  | "comment"
  | "suggestion"
  | "edit"
  | "resolve"
  | "unresolve"
  | "delete"
  | "reattach"
  | "signoff";

export interface ApplyRequest {
  schema_version: 1;
  op: ApplyOp;
  expect_hash?: string;
  selection?: Selection;
  body?: string;
  replacement?: string;
  id?: string;
  include_source?: boolean;
}

export interface ReviewAnchor {
  heading_path: string[];
  block_index: number;
  quote: string;
  prefix: string;
  suffix: string;
  source_start: number;
  source_end: number;
  line_start: number;
  line_end: number;
  doc_hash: string;
}

export interface ReviewItem {
  id: string;
  kind: "comment" | "suggestion";
  status: "open" | "resolved" | "stale";
  body: string;
  created_round: number;
  resolved_round: number | null;
  created_at: string;
  updated_at: string;
  anchor: ReviewAnchor;
  suggestion?: { original: string; replacement: string } | null;
  history: Array<Record<string, unknown>>;
}

export interface ReviewStore {
  version: number;
  file: string;
  doc_hash: string;
  round: number;
  requested_at?: string | null;
  last_signoff?: "feedback" | "approved" | null;
  approval?: { hash: string; at: string } | null;
  next_id: number;
  items: ReviewItem[];
  draft?: unknown;
}

export interface BridgeBlock {
  index: number;
  kind: string;
  level: number | null;
  start_line: number;
  end_line: number;
  heading_path: string[];
  marker: string | null;
}

export interface BridgeCounts {
  open: number;
  stale: number;
  resolved: number;
  total: number;
}

export interface BridgeState {
  schema_version: number;
  ok: true;
  agentgloss_version: string;
  file: string;
  sidecar: string;
  sidecar_exists: boolean;
  doc_hash: string;
  line_count: number;
  source?: string;
  blocks: BridgeBlock[];
  review: ReviewStore | null;
  status: string;
  doc_changed: boolean;
  approval_valid: boolean;
  counts: BridgeCounts;
  changed?: boolean;
  id?: string | null;
  reconciled?: boolean;
  recovered?: boolean;
  backup_path?: string | null;
  terminal_exit_code?: number | null;
}

interface BridgeErrorEnvelope {
  ok: false;
  error: {
    code: string;
    message: string;
    found?: number;
    supported?: number;
  };
}

export class BridgeError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "BridgeError";
    this.code = code;
  }
}

export function parseLastJson(stdout: string): unknown {
  const lines = stdout.trim().split(/\r?\n/).reverse();
  for (const line of lines) {
    try {
      const value: unknown = JSON.parse(line);
      if (value && typeof value === "object") {
        return value;
      }
    } catch {
      // Ignore non-JSON log lines before the envelope.
    }
  }
  return null;
}

export function runBridge(
  binary: string,
  args: string[],
  payload?: unknown,
  timeoutMs = 15000,
): Promise<BridgeState> {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, { stdio: ["pipe", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    let settled = false;

    const finish = (action: () => void) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      action();
    };

    const timer = setTimeout(() => {
      child.kill();
      finish(() =>
        reject(new BridgeError("timeout", "agentgloss bridge timed out")),
      );
    }, timeoutMs);

    child.on("error", (error: NodeJS.ErrnoException) => {
      finish(() => {
        if (error.code === "ENOENT") {
          reject(
            new BridgeError(
              "not_found",
              `agentgloss binary not found: ${binary}. Set agentgloss.path to the installed binary.`,
            ),
          );
        } else {
          reject(new BridgeError("spawn_error", error.message));
        }
      });
    });

    child.stdin.on("error", () => {
      // The close handler reports the useful error.
    });
    child.stdout.on("data", (chunk: Buffer) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString();
    });

    child.on("close", (code) => {
      finish(() => {
        const value = parseLastJson(stdout);
        if (value) {
          const envelope = value as Partial<BridgeState> &
            Partial<BridgeErrorEnvelope>;
          if (envelope.ok === false && envelope.error) {
            reject(
              new BridgeError(envelope.error.code, envelope.error.message),
            );
            return;
          }
          if (envelope.ok === true) {
            resolve(envelope as BridgeState);
            return;
          }
        }
        reject(
          new BridgeError(
            "bridge_failure",
            `agentgloss exited ${code ?? "unknown"}: ${(
              stderr ||
              stdout ||
              "no output"
            ).trim()}`,
          ),
        );
      });
    });

    if (payload === undefined) {
      child.stdin.end();
    } else {
      child.stdin.end(JSON.stringify(payload));
    }
  });
}

export function bridgeState(
  binary: string,
  file: string,
  sidecar?: string,
  noSource = false,
): Promise<BridgeState> {
  const args = ["bridge", "state", file];
  if (sidecar) {
    args.push("--sidecar", sidecar);
  }
  if (noSource) {
    args.push("--no-source");
  }
  return runBridge(binary, args);
}

export function bridgeApply(
  binary: string,
  file: string,
  sidecar: string | undefined,
  request: ApplyRequest,
): Promise<BridgeState> {
  const args = ["bridge", "apply", file];
  if (sidecar) {
    args.push("--sidecar", sidecar);
  }
  return runBridge(binary, args, request);
}
