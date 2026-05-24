/**
 * Handoff routing — integration coverage.
 *
 * Exercises the MCP tool handlers (maestro_handoff_{emit,list,pickup}) wired
 * to the real on-disk `FsHandoffEmitter` adapter against a fresh tmp repo.
 * Pins routing semantics + on-disk byte-stability at the JSON round-trip layer.
 *
 * Scope boundary:
 * - The projection-key contract for `handoff list` summary is owned by
 *   `tests/integration/token-budget.test.ts`. This file does not duplicate it.
 * - Real-MCP-stdio transport coverage and CLI `--to-agent` flag coverage are
 *   manual smoke steps in the Phase 6 verification checklist (see plan
 *   `hello-pls-help-me-stateful-sunset.md` ~line 145-146); they are NOT
 *   exercised by this file.
 *
 * Serial-only: bun:test runs file-level tests serially; do not add
 * `it.concurrent` to this file — `let repoRoot` / `let handlers` are
 * describe-scope mutable state.
 */
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, readdir, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { registerHandoffTools } from "@/features/mcp/server/tools/handoff-tools.js";
import type { CallToolResult } from "@/features/mcp/server/errors.js";
import { FsHandoffEmitter } from "@/repo/fs-handoff-emitter.adapter.js";
import type { HandoffEnvelope, HandoffPickup } from "@/repo/handoff-emitter.port.js";
import type { Services } from "@/services.js";

type ToolHandler = (args: Record<string, unknown>) => Promise<CallToolResult>;

function makeServerStub(): { server: unknown; handlers: Map<string, ToolHandler> } {
  const handlers = new Map<string, ToolHandler>();
  const server = {
    registerTool: (name: string, _config: unknown, cb: ToolHandler): void => {
      handlers.set(name, cb);
    },
  };
  return { server, handlers };
}

function parsePayload<T>(result: CallToolResult): T {
  const text = result.content[0]?.text ?? "{}";
  return JSON.parse(text) as T;
}

describe("handoff-routing integration", () => {
  let repoRoot: string | undefined;
  let handlers: Map<string, ToolHandler>;

  beforeEach(async () => {
    repoRoot = await mkdtemp(join(tmpdir(), "maestro-handoff-routing-"));
    const emitter = new FsHandoffEmitter({ repoRoot });
    const stub = makeServerStub();
    handlers = stub.handlers;
    const services = { handoffEmitter: emitter } as unknown as Services;
    registerHandoffTools(
      stub.server as Parameters<typeof registerHandoffTools>[0],
      { getServices: () => services, sessionId: "test-session" },
    );
  });

  afterEach(async () => {
    if (repoRoot) {
      await rm(repoRoot, { recursive: true, force: true });
      repoRoot = undefined;
    }
  });

  it("Case A — emit with to_agent persists to_agent on envelope and on disk", async () => {
    const emit = handlers.get("maestro_handoff_emit")!;
    const result = await emit({
      task_id: "tsk-aaaaaa-bbbbbb",
      trigger_verb: "task:claim",
      to_agent: "codex",
    });
    expect(result.isError).toBeUndefined();
    const payload = parsePayload<{ envelope: HandoffEnvelope }>(result);
    expect(payload.envelope.to_agent).toBe("codex");

    const files = await readdir(join(repoRoot!, ".maestro/handoffs"));
    const envelopeFiles = files.filter(
      (f) => f.endsWith(".json") && !f.endsWith(".picked_up.json"),
    );
    expect(envelopeFiles).toHaveLength(1);
    expect(envelopeFiles[0]).toBe(`${payload.envelope.id}.json`);
    const raw = await readFile(join(repoRoot!, ".maestro/handoffs", envelopeFiles[0]!), "utf8");
    const onDisk = JSON.parse(raw) as HandoffEnvelope;
    expect(onDisk.to_agent).toBe("codex");
    expect(onDisk.id).toBe(payload.envelope.id);
  });

  it("Case B — list with to_agent filter returns only the targeted envelope (filter + projection round-trip)", async () => {
    const emit = handlers.get("maestro_handoff_emit")!;
    const targeted = await emit({
      task_id: "tsk-aaaaaa-bbbbbb",
      trigger_verb: "task:claim",
      to_agent: "codex",
    });
    await emit({
      task_id: "tsk-cccccc-dddddd",
      trigger_verb: "task:claim",
    });
    const targetedId = parsePayload<{ envelope: HandoffEnvelope }>(targeted).envelope.id;

    const list = handlers.get("maestro_handoff_list")!;
    const result = await list({ to_agent: "codex" });
    expect(result.isError).toBeUndefined();
    const payload = parsePayload<{
      items: { id: string; task_id: string; to_agent?: string }[];
    }>(result);
    expect(payload.items).toHaveLength(1);
    expect(payload.items[0]?.id).toBe(targetedId);
    expect(payload.items[0]?.task_id).toBe("tsk-aaaaaa-bbbbbb");
    expect(payload.items[0]?.to_agent).toBe("codex");
  });

  it("Case C — list with no filter returns all envelopes including untargeted/legacy", async () => {
    const emit = handlers.get("maestro_handoff_emit")!;
    const r1 = await emit({
      task_id: "tsk-aaaaaa-bbbbbb",
      trigger_verb: "task:claim",
      to_agent: "codex",
    });
    const r2 = await emit({
      task_id: "tsk-cccccc-dddddd",
      trigger_verb: "task:claim",
    });
    const codexId = parsePayload<{ envelope: HandoffEnvelope }>(r1).envelope.id;
    const legacyId = parsePayload<{ envelope: HandoffEnvelope }>(r2).envelope.id;

    // Byte-stable absence on disk: untargeted envelope's file must OMIT the to_agent key,
    // not write it as null/empty. See plan ~line 96 + docs/token-budget.md.
    const legacyRaw = await readFile(
      join(repoRoot!, ".maestro/handoffs", `${legacyId}.json`),
      "utf8",
    );
    expect(Object.prototype.hasOwnProperty.call(JSON.parse(legacyRaw), "to_agent")).toBe(false);

    const list = handlers.get("maestro_handoff_list")!;
    const result = await list({});
    expect(result.isError).toBeUndefined();
    const payload = parsePayload<{ items: { id: string; to_agent?: string }[] }>(result);
    // Set membership, not ordered: production sorts by created_at, but the
    // millisecond-precision tiebreak is fs.readdir order (non-deterministic
    // when two emits collide in the same ms). Assert presence of both ids;
    // ordering invariant only holds when timestamps differ.
    expect(new Set(payload.items.map((i) => i.id))).toEqual(new Set([codexId, legacyId]));

    const legacy = payload.items.find((i) => i.id === legacyId);
    expect(legacy?.to_agent).toBeUndefined();
  });

  it("Case D — pickup mismatch returns warnings and still writes the sidecar", async () => {
    const emit = handlers.get("maestro_handoff_emit")!;
    const emitResult = await emit({
      task_id: "tsk-aaaaaa-bbbbbb",
      trigger_verb: "task:claim",
      to_agent: "codex",
    });
    const envelopeId = parsePayload<{ envelope: HandoffEnvelope }>(emitResult).envelope.id;

    const pickup = handlers.get("maestro_handoff_pickup")!;
    const result = await pickup({ id: envelopeId, picked_up_by: "claude-code" });
    expect(result.isError).toBeUndefined();

    const payload = parsePayload<{
      envelope: HandoffEnvelope;
      pickup: HandoffPickup;
      warnings?: string[];
    }>(result);
    expect(payload.warnings).toEqual([
      "Envelope was addressed to 'codex'; picked up by 'claude-code'. Pickup recorded; verify this is the envelope you intended.",
    ]);

    const sidecarPath = join(repoRoot!, ".maestro/handoffs", `${envelopeId}.picked_up.json`);
    const sidecarStat = await stat(sidecarPath);
    expect(sidecarStat.isFile()).toBe(true);
    const sidecarRaw = await readFile(sidecarPath, "utf8");
    const sidecar = JSON.parse(sidecarRaw) as HandoffPickup;
    expect(sidecar.picked_up_by).toBe("claude-code");
    expect(sidecar.envelope_id).toBe(envelopeId);

    // At-most-once delivery: warn-but-allow does NOT make pickup idempotent.
    // A second pickup of the same id must error (HANDOFF_ALREADY_PICKED_UP).
    const secondPickup = await pickup({ id: envelopeId, picked_up_by: "claude-code" });
    expect(secondPickup.isError).toBe(true);
  });

  it("Case E — pickup with matching tool name omits warnings field (byte-stable)", async () => {
    const emit = handlers.get("maestro_handoff_emit")!;
    const emitResult = await emit({
      task_id: "tsk-aaaaaa-bbbbbb",
      trigger_verb: "task:claim",
      to_agent: "codex",
    });
    const envelopeId = parsePayload<{ envelope: HandoffEnvelope }>(emitResult).envelope.id;

    const pickup = handlers.get("maestro_handoff_pickup")!;
    const result = await pickup({ id: envelopeId, picked_up_by: "codex" });
    expect(result.isError).toBeUndefined();

    const payload = parsePayload<Record<string, unknown>>(result);
    // Byte-stable absence: warnings is OMITTED, not [] — see plan ~line 96 + docs/token-budget.md
    expect("warnings" in payload).toBe(false);
  });

  it("Case F — list with to_agent + default include_picked_up=false excludes already-picked-up envelopes (inbox composition)", async () => {
    const emit = handlers.get("maestro_handoff_emit")!;
    const r1 = await emit({
      task_id: "tsk-aaaaaa-bbbbbb",
      trigger_verb: "task:claim",
      to_agent: "codex",
    });
    const r2 = await emit({
      task_id: "tsk-cccccc-dddddd",
      trigger_verb: "task:claim",
      to_agent: "codex",
    });
    const pickedUpId = parsePayload<{ envelope: HandoffEnvelope }>(r1).envelope.id;
    const openId = parsePayload<{ envelope: HandoffEnvelope }>(r2).envelope.id;

    const pickup = handlers.get("maestro_handoff_pickup")!;
    await pickup({ id: pickedUpId, picked_up_by: "codex" });

    const list = handlers.get("maestro_handoff_list")!;
    const result = await list({ to_agent: "codex" });
    expect(result.isError).toBeUndefined();
    const payload = parsePayload<{ items: { id: string }[] }>(result);
    expect(payload.items.map((i) => i.id)).toEqual([openId]);
  });
});
