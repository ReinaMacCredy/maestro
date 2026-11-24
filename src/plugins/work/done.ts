import {
  CliError,
  requiredPosition,
  stringOption,
  stringOptions,
  type CliInvocation,
  type CliResult,
} from "../../kernel/cli.ts";
import { constants } from "node:os";
import type { PluginContext } from "../../kernel/loader.ts";
import { resolveStoreLocation } from "../../kernel/store.ts";
import { modifiedTrackedFiles, namesCommit } from "../git-status.ts";
import { registerSessionCommand } from "../session-required.ts";
import {
  blockerDetails,
  blockIfNeeded,
  latestLeaseExpiration,
  requireWork,
  unresolvedBlockers,
  type GateResult,
  type WorkGateInput,
  type WorkRecord,
  type WorkService,
} from "./records.ts";

interface CheckRecord {
  command: string;
  exitCode: number;
  durationMs: number;
  commit: string | null;
  clean: boolean | null;
}

function gitOutput(root: string, args: string[]): string | null {
  const result = Bun.spawnSync(["git", ...args], { cwd: root, stderr: "ignore", stdout: "pipe" });
  return result.exitCode === 0 ? result.stdout.toString() : null;
}

// The check's stdout joins stderr so a --json caller still reads one envelope on stdout.
function runCheck(command: string, root: string): CheckRecord {
  const started = performance.now();
  const result = Bun.spawnSync(["sh", "-c", command], { cwd: root, stdio: ["ignore", 2, 2] });
  const durationMs = Math.round(performance.now() - started);
  const status = gitOutput(root, ["status", "--porcelain"]);
  return {
    command,
    exitCode: result.exitCode ?? 128 + (constants.signals[result.signalCode as keyof typeof constants.signals] ?? 0),
    durationMs,
    commit: gitOutput(root, ["rev-parse", "HEAD"])?.trim() ?? null,
    clean: status === null ? null : status.trim() === "",
  };
}

export type ProofRecord = { kind: "text" } | ({ kind: "check" } & CheckRecord);

export function registerWorkDone(context: PluginContext, service: WorkService): void {
  context.effect(() =>
    registerSessionCommand(
      context,
      "work done",
      (invocation) => completeWork(context, service, invocation),
      {
        description: "Complete work with policy-checked evidence, taking an unheld lease.",
        flags: {
          "--evidence": {
            description: "Record opaque completion evidence.",
            value: true,
          },
          "--candidate": {
            description: "Record an opaque commit or digest.",
            value: true,
          },
          "--check": {
            description: "Run this shell command in the repository root and record its exit code, duration, commit and tree state; a failing check is recorded and the card still closes.",
            value: true,
          },
        },
        positionals: [{ name: "id", required: true }],
      },
    ),
  );
}

async function completeWork(
  context: PluginContext,
  service: WorkService,
  invocation: CliInvocation,
): Promise<CliResult> {
  const id = requiredPosition(invocation, 0, "work id");
  const work = requireWork(context, id);
  if (work.state === "cancelled") {
    throw new CliError(
      "INVALID_STATE",
      `${id} is cancelled (${work.cancelReason ?? "no reason recorded"}); add a new work item instead`,
    );
  }
  if (work.state === "done") {
    throw new CliError("INVALID_STATE", `${id} is already done; nothing to complete`);
  }
  const sessionId = context.sessions.current().id;
  // A card nobody holds closes in one command: done takes the lease
  // itself. The start gates and blockers run first, so the implicit
  // claim is not a way around anything work start would refuse.
  const claimsLease = work.heldBy !== sessionId;
  const expired = claimsLease ? latestLeaseExpiration(context, id) : null;
  if (claimsLease) {
    if (work.heldBy) {
      throw new CliError("LEASE_HELD", `${id} is held by ${work.heldBy}`, {
        holder: work.heldBy,
      });
    }
    const blockers = unresolvedBlockers(context, id);
    if (blockers.length > 0) {
      const details = blockerDetails(context, id, blockers, sessionId);
      throw new CliError("BLOCKED", details.reason, {
        blockers: details.blockers,
        command: details.command,
      });
    }
    const start = await context.events.waterfall<WorkGateInput, GateResult>(
      "work.start",
      {
        work: { ...work, heldBy: null },
        children: service.children(id),
        sessionId,
      },
      async () => ({ blocked: false }),
    );
    blockIfNeeded(start);
  }
  const evidence = stringOption(invocation, "evidence") ?? "";
  const candidate = stringOption(invocation, "candidate") ?? null;
  const claims = stringOptions(invocation, "claim");
  const proofs = stringOptions(invocation, "proof");
  const check = stringOption(invocation, "check") ?? null;
  if (check !== null && check.trim() === "") {
    throw new CliError("INVALID_VALUE", "--check needs a command to run");
  }
  const result = await context.events.waterfall<
    {
      check: string | null;
      children: WorkRecord[];
      claims: string[];
      evidence: string;
      proofs: string[];
      sessionId: string;
      work: WorkRecord;
    },
    GateResult
  >(
    "work.done",
    {
      work,
      check,
      children: service.children(id),
      evidence,
      claims,
      proofs,
      sessionId,
    },
    async (completion) => ({ blocked: false, evidence: completion.evidence }),
  );
  blockIfNeeded(result);
  const recordedEvidence = result.evidence ?? evidence;
  const proof: ProofRecord = check === null
    ? { kind: "text" }
    : { kind: "check", ...runCheck(check, resolveStoreLocation(process.cwd()).root) };
  const completed = writeCompletion(context, service, {
    candidate,
    claims,
    claimsLease,
    id,
    proof,
    proofs,
    recordedEvidence,
    sessionId,
  });
  const lostLease = expired
    ? `; previous lease held by ${expired.holder} expired because ${expired.reason}`
    : "";
  const modified = candidate || namesCommit([recordedEvidence, ...claims, ...proofs].join(" "))
    ? 0
    : await modifiedTrackedFiles(process.cwd());
  const unlanded = modified > 0
    ? `\nwarning: ${modified} tracked files are modified and the evidence names no commit; a deliverable that only lives in the working tree is not landed`
    : "";
  const checked = proof.kind === "check"
    ? `\ncheck ${proof.exitCode === 0 ? "passed" : "failed"}: ${proof.command} exited ${proof.exitCode} in ${proof.durationMs} ms` +
      (proof.exitCode === 0 ? "" : "; recorded, and the card closed (observe mode)")
    : "";
  const reactions = (await context.events.serial("work.completed", completed)).filter((line) => typeof line === "string");
  return { data: { work: completed, proof }, text: [`${id} done${lostLease}${checked}${unlanded}`, ...reactions].join("\n") };
}

function writeCompletion(
  context: PluginContext,
  service: WorkService,
  completion: {
    candidate: string | null;
    claims: string[];
    claimsLease: boolean;
    id: string;
    proof: ProofRecord;
    proofs: string[];
    recordedEvidence: string;
    sessionId: string;
  },
): WorkRecord | null {
  const { candidate, claims, claimsLease, id, proof, proofs, recordedEvidence, sessionId } = completion;
  const complete = context.store.database.transaction(() => {
    const current = requireWork(context, id);
    if (current.state === "cancelled") {
      throw new CliError(
        "INVALID_STATE",
        `${id} is cancelled (${current.cancelReason ?? "no reason recorded"}); add a new work item instead`,
      );
    }
    if (current.state === "done") {
      throw new CliError("INVALID_STATE", `${id} is already done; nothing to complete`);
    }
    const now = new Date().toISOString();
    if (current.heldBy !== sessionId) {
      if (current.heldBy) {
        throw new CliError("LEASE_HELD", `${id} is held by ${current.heldBy}`, {
          holder: current.heldBy,
        });
      }
      const claimed = context.store.database
        .query(
          `UPDATE work
                   SET state = 'active', held_by = ?, updated_at = ?
                   WHERE id = ? AND state != 'done' AND cancelled_at IS NULL AND held_by IS NULL`,
        )
        .run(sessionId, now, id);
      if (claimed.changes === 0) {
        const refreshed = requireWork(context, id);
        throw new CliError("LEASE_HELD", `${id} is held by ${refreshed.heldBy}`, {
          holder: refreshed.heldBy,
        });
      }
    }
    const completed = context.store.database
      .query(
        `UPDATE work
                 SET state = 'done', evidence = ?, candidate = ?, held_by = NULL, updated_at = ?
                 WHERE id = ? AND state = 'active' AND cancelled_at IS NULL AND held_by = ?`,
      )
      .run(recordedEvidence, candidate, now, id, sessionId);
    if (completed.changes === 0) {
      throw new CliError("INVALID_STATE", `${id} changed while completion was pending`);
    }
    context.sessions.record("work.done");
    context.log.append({
      type: "work.done",
      entityType: "work",
      entityId: id,
      sessionId,
      payload: {
        candidate,
        ...(claimsLease ? { claimedOnDone: true } : {}),
        evidence: recordedEvidence,
        claims,
        proofs,
        proof,
      },
    });
    return service.get(id);
  });
  return complete.immediate();
}
