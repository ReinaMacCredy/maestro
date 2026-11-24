import {
  CliError,
  stringOption,
  type CliInvocation,
  type CliResult,
} from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import {
  gateDetails,
  toWork,
  type GateResult,
  type WorkGateInput,
  type WorkRecord,
  type WorkRow,
} from "./records.ts";

interface ListedWork extends WorkRecord {
  blockedBy: Array<{ id: string; state: string }>;
  gate: { command: string | null; origin: string; reason: string } | null;
  holder: { harness: string | null; id: string; lastSeen: string | null; live: boolean } | null;
}

export function registerWorkList(context: PluginContext): void {
  context.effect(() =>
    context.cli.register(
      "work list",
      (invocation) => listWorkItems(context, invocation),
      {
        description: "List open and active work with holders, blockers and gates; done and cancelled work is counted.",
        flags: {
          "--all": { description: "List every item, done and cancelled included." },
          "--parent": { description: "List the children of one work ID in every state.", value: true },
        },
        mutates: false,
      },
    ),
  );
}

async function listWorkItems(context: PluginContext, invocation: CliInvocation): Promise<CliResult> {
  const parentId = stringOption(invocation, "parent");
  const showAll = invocation.options.all === true;
  const database = context.store.database;
  if (parentId && !database.query("SELECT 1 FROM work WHERE id = ?").get(parentId)) {
    throw new CliError("NOT_FOUND", `work not found: ${parentId}; run: maestro work list`, {
      command: "maestro work list",
      id: parentId,
    });
  }
  const order = "ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)";
  const rows = parentId
    ? database.query<WorkRow, [string]>(`SELECT * FROM work WHERE parent_id = ? ${order}`).all(parentId)
    : showAll
    ? database.query<WorkRow, []>(`SELECT * FROM work ${order}`).all()
    : database
        .query<WorkRow, []>(
          `SELECT * FROM work WHERE state IN ('open', 'active') AND cancelled_at IS NULL ${order}`,
        )
        .all();
  const counts = { open: 0, active: 0, done: 0, cancelled: 0 };
  for (const row of database
    .query<{ count: number; state: keyof typeof counts }, []>(
      `SELECT CASE WHEN cancelled_at IS NOT NULL THEN 'cancelled' ELSE state END AS state,
                      COUNT(*) AS count
               FROM work GROUP BY 1`,
    )
    .all()) counts[row.state] = row.count;

  const blockedBy = new Map<string, Array<{ id: string; state: string }>>();
  for (const edge of database
    .query<{ id: string; state: string; work_id: string }, []>(
      `SELECT work_blockers.work_id, blocker.id, blocker.state
               FROM work_blockers
               JOIN work AS item ON item.id = work_blockers.work_id
               JOIN work AS blocker ON blocker.id = work_blockers.blocker_id
               WHERE item.state != 'done' AND item.cancelled_at IS NULL
                 AND blocker.state != 'done' AND blocker.cancelled_at IS NULL
               ORDER BY CAST(SUBSTR(blocker.id, 2) AS INTEGER)`,
    )
    .all()) {
    blockedBy.set(edge.work_id, [
      ...blockedBy.get(edge.work_id) ?? [],
      { id: edge.id, state: edge.state },
    ]);
  }

  const holders = new Map<string, NonNullable<ListedWork["holder"]>>();
  const holderOf = (id: string): NonNullable<ListedWork["holder"]> => {
    let holder = holders.get(id);
    if (!holder) {
      const session = context.sessions.get(id);
      holder = {
        harness: session?.harness ?? null,
        id,
        lastSeen: session?.lastSeen ?? null,
        live: session?.live ?? false,
      };
      holders.set(id, holder);
    }
    return holder;
  };

  const childrenOf = new Map<string, WorkRecord[]>();
  for (const child of database
    .query<WorkRow, []>(
      `SELECT * FROM work WHERE parent_id IN (
                 SELECT id FROM work WHERE state = 'open' AND cancelled_at IS NULL
               ) ${order}`,
    )
    .all()
    .map(toWork)) {
    childrenOf.set(child.parentId as string, [...childrenOf.get(child.parentId as string) ?? [], child]);
  }
  const sessionId = context.sessions.current().id;
  const works: ListedWork[] = [];
  for (const work of rows.map(toWork)) {
    const blockers = work.state === "open" || work.state === "active"
      ? blockedBy.get(work.id) ?? []
      : [];
    let gate: ListedWork["gate"] = null;
    if (work.state === "open" && !work.heldBy && blockers.length === 0) {
      const result = await context.events.waterfall<WorkGateInput, GateResult>(
        "work.ready",
        { work, children: childrenOf.get(work.id) ?? [], sessionId },
        async () => ({ blocked: false }),
      );
      if (result.blocked) gate = { ...gateDetails(result), command: result.command ?? null };
    }
    works.push({
      ...work,
      blockedBy: blockers,
      holder: work.heldBy ? holderOf(work.heldBy) : null,
      gate,
    });
  }

  const shown = new Set(works.map((work) => work.id));
  const childrenByParent = new Map<string, ListedWork[]>();
  for (const work of works) {
    if (parentId || !work.parentId || !shown.has(work.parentId)) continue;
    childrenByParent.set(work.parentId, [...childrenByParent.get(work.parentId) ?? [], work]);
  }
  const render = (work: ListedWork, depth: number): string[] => {
    const indent = "  ".repeat(depth);
    const held = work.holder
      ? `  held by ${work.holder.id} (${work.holder.live ? "live" : "dead"})`
      : "";
    const blocked = work.blockedBy.length > 0
      ? `  blocked by ${work.blockedBy.map((blocker) => `${blocker.id} [${blocker.state}]`).join(", ")}`
      : "";
    return [
      `${indent}${work.id} [${work.state}] ${work.title}${held}${blocked}`,
      ...(work.gate ? [`${indent}  gated by ${work.gate.origin}: ${work.gate.reason}`] : []),
      ...(childrenByParent.get(work.id) ?? []).flatMap((child) => render(child, depth + 1)),
    ];
  };
  const lines = works
    .filter((work) => parentId || !work.parentId || !shown.has(work.parentId))
    .flatMap((work) => render(work, 0));
  if (!parentId && !showAll) {
    const hidden = [
      counts.done > 0 ? `${counts.done} done` : null,
      counts.cancelled > 0 ? `${counts.cancelled} cancelled` : null,
    ].filter((part): part is string => part !== null);
    if (hidden.length > 0) lines.push(`${hidden.join(", ")} not shown; maestro work list --all`);
  }
  return {
    data: { works, counts },
    text: lines.length > 0
      ? lines.join("\n")
      : parentId
      ? `${parentId} has no children; add one: maestro work add "<title>" --parent ${parentId}`
      : 'no tracked work; add some: maestro work add "<title>"',
  };
}
