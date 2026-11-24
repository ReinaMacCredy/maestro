import {
  CliError,
  requiredPosition,
  type CliResult,
} from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import type { DecisionService } from "../decision.ts";
import type { ProofRecord } from "./done.ts";
import { blockersFor, requireWork, type WorkRecord, type WorkService } from "./records.ts";

interface WorkNoteRow {
  text: string;
  created_at: string;
}

function formatWork(work: WorkRecord): string {
  const fields = [
    `${work.id} [${work.state}] ${work.title}`,
    `kind: ${work.kind}`,
    work.parentId ? `parent: ${work.parentId}` : null,
    work.heldBy ? `held by: ${work.heldBy}` : null,
    work.reclaimedFrom ? `reclaimed from: ${work.reclaimedFrom}` : null,
    work.reclaimedBy ? `reclaimed by: ${work.reclaimedBy}` : null,
    work.reclaimReason ? `reclaim reason: ${work.reclaimReason}` : null,
    work.acceptance ? `acceptance: ${work.acceptance}` : null,
    work.atomicReason ? `atomic reason: ${work.atomicReason}` : null,
    work.cancelReason ? `cancel reason: ${work.cancelReason}` : null,
    work.evidence ? `evidence: ${work.evidence}` : null,
    work.candidate !== null ? `candidate: ${work.candidate}` : null,
  ];
  return fields.filter((field): field is string => field !== null).join("\n");
}

function latestProof(context: PluginContext, id: string): ProofRecord | null {
  const done = context.log.list("work", id).filter((event) => event.type === "work.done").at(-1);
  return (done?.payload as { proof?: ProofRecord } | undefined)?.proof ?? null;
}

function formatProof(proof: ProofRecord): string[] {
  if (proof.kind === "text") return ["proof kind: text"];
  const tree = proof.clean === null ? "no tree" : proof.clean ? "clean tree" : "dirty tree";
  return [
    "proof kind: check",
    `check: ${proof.command} (exit ${proof.exitCode}, ${proof.durationMs} ms, commit ${proof.commit ?? "none"}, ${tree})`,
  ];
}

export function registerWorkShow(context: PluginContext, service: WorkService): void {
  context.effect(() =>
    context.cli.register("work show", (invocation): CliResult => {
      const work = requireWork(context, requiredPosition(invocation, 0, "work id"));
      const children = service.children(work.id);
      const blockers = blockersFor(context, work.id);
      const allNotes = context.store.database
        .query<WorkNoteRow, [string]>(
          "SELECT text, created_at FROM work_notes WHERE work_id = ? ORDER BY id",
        )
        .all(work.id)
        .map((note) => ({ text: note.text, createdAt: note.created_at }));
      // The live slice is the last few notes (d821); a relay can leave dozens.
      const window = invocation.options.notes;
      let noteLimit = 5;
      if (window === "all") noteLimit = allNotes.length;
      else if (typeof window === "string") {
        noteLimit = Number(window);
        if (!Number.isInteger(noteLimit) || noteLimit < 0) {
          throw new CliError("INVALID_VALUE", "--notes takes a non-negative integer or all", { notes: window });
        }
      }
      const notes = allNotes.slice(Math.max(0, allNotes.length - noteLimit));
      const noteCountLine = notes.length < allNotes.length
        ? `notes: ${allNotes.length}, showing the last ${notes.length}; maestro work show ${work.id} --notes all`
        : null;
      const childLines = children.map(
        (child) => `child: ${child.id} [${child.state}] ${child.title}`,
      );
      const blockerLines = blockers.map(
        (blocker) => `blocker: ${blocker.id} [${blocker.state}] ${blocker.title}`,
      );
      const decisions = ((context.decision as DecisionService | undefined)?.list() ?? [])
        .filter((decision) => decision.workId === work.id);
      // What done will demand is only discoverable on rejection otherwise:
      // each active policy that gates work done states its requirement here.
      const gates = work.state === "done" || work.state === "cancelled"
        ? []
        : context.loader.records
            .filter(
              (record) =>
                record.status === "active" &&
                record.name.startsWith("policy-") &&
                /^gates [^:]*\bdone\b/.test(record.requires ?? ""),
            )
            .map((record) => ({ policy: record.name, requires: record.requires ?? "" }));
      const proof = work.state === "done" ? latestProof(context, work.id) : null;
      return {
        data: { work, proof, children, blockers, notes, noteCount: allNotes.length, decisions, gates },
        text: [
          formatWork(work),
          ...(proof ? formatProof(proof) : []),
          ...gates.map((gate) => `gate: ${gate.policy} ${gate.requires}`),
          ...blockerLines,
          ...childLines,
          ...(noteCountLine ? [noteCountLine] : []),
          ...notes.map((note) => `note: ${note.text}`),
          ...decisions.map(
            (decision) =>
              `decision: ${decision.id} [${decision.state}] ${decision.text}`,
          ),
        ].join("\n"),
      };
    }, {
      description: "Show one work item with its done gates, blockers, evidence, children, and the last five notes.",
      flags: { "--notes": { description: "How many notes to print, newest last: an integer or all (default 5).", value: true } },
      mutates: false,
      positionals: [{ name: "id", required: true }],
    }),
  );
}
