/**
 * `maestro handoff` command surface.
 *
 * Three subcommands:
 *   - create: builds a new UKI v5.3 handoff from structured flags
 *   - pickup: fetches the latest pending (or by id) with --json/--markdown/--uki output
 *   - list:   lists handoffs with optional status filter
 */
import type { Command } from "commander";
import { getServices } from "../services.js";
import { output, resolveJsonFlag } from "../lib/output.js";
import { MaestroError } from "../domain/errors.js";
import type { UkiSlots } from "../lib/uki-format.js";
import { DEFAULT_STANCE_COLLAPSE } from "../lib/uki-format.js";
import {
  UKI_HANDOFF_STATUSES,
  type UkiHandoff,
  type UkiHandoffStatus,
} from "../domain/uki-types.js";
import { createUkiHandoff } from "../usecases/create-uki-handoff.usecase.js";
import { pickupUkiHandoff } from "../usecases/pickup-uki-handoff.usecase.js";
import { listUkiHandoffs } from "../usecases/list-uki-handoffs.usecase.js";

type PickupFormat = "json" | "markdown" | "uki";

  export function registerHandoffCommand(program: Command): void {
  const handoffCmd = program
    .command("handoff")
    .description("UKI v5.3 handoff lifecycle (create, pickup, list)")
    .option("--json", "Output as JSON");

  handoffCmd
    .command("create")
      .description("Create a new UKI handoff from structured slots")
      .requiredOption("--session-core <text>", "SESSION_CORE slot (essence of the work)")
      .requiredOption("--summary <text>", "SUMMARY slot (under 140 chars)")
      .requiredOption("--next-action <text>", "NEXT_ACTION slot (one concrete next step)")
      .option("--driver <text...>", "CAUSAL_DRIVERS token (repeatable)")
      .option("--divergence <text...>", "DIVERGENCES token (repeatable)")
      .option("--decision <text...>", "KEY_DECISIONS token (repeatable)")
      .option("--decision-basis <text...>", "DECISION_BASIS token (repeatable)")
      .option("--signal <text...>", "SIGNAL_DELTA token (repeatable, may include before~after)")
      .option("--validation <text...>", "VALIDATION_STATE token (repeatable)")
      .option("--artifact <text...>", "ARTIFACTS token (repeatable, must include >=1 commit_/branch_/version_/file_)")
      .option("--boundary <text...>", "BOUNDARY_STATE token (repeatable)")
      .option("--execution-state <text>", "EXECUTION_STATE slot")
      .option("--stance-collapse <text>", "STANCE_COLLAPSE slot (default NONE_DETECTED_LOW_FRICTION)")
      .option("--blind-spot <text>", "BLIND_SPOT slot")
      .option("--metaphor <text>", "METAPHOR slot")
      .option("--confidence-work <number>", "CS.work (0..1)", parseFloatStrict)
      .option("--confidence-summary <number>", "CS.summary (0..1)", parseFloatStrict)
      .option("--agent <name>", "Override agent identity (default: auto-detect)")
    .option("--session-id <id>", "Override session id (default: auto-detect)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);

      const slots = buildSlotsFromOptions(opts);
      const handoff = await createUkiHandoff(
        services.handoffStore,
        services.sessionDetect,
        process.cwd(),
        {
          slots,
          agent: opts.agent,
          sessionId: opts.sessionId,
        },
      );

      output(isJson, handoff, formatHandoffCreate);
    });

  handoffCmd
    .command("pickup")
    .description("Pick up the latest pending handoff (or a specific one by id)")
    .option("--id <id>", "Specific handoff id to pick up")
      .option("--claim", "Transition pending -> picked-up (atomic claim)")
      .option("--agent <name>", "pickedUpBy attribution when --claim is set")
      .option("--json", "Output as JSON (default)")
      .option("--markdown", "Output as human-readable markdown")
      .option("--uki", "Output only the raw UKI v5.3 compressed string")
    .action(async (opts) => {
      const services = getServices();
      const format = resolvePickupFormat(opts, program);

      const handoff = await pickupUkiHandoff(services.handoffStore, {
        id: opts.id,
        claim: Boolean(opts.claim),
        pickedUpBy: opts.agent,
      });

      printPickup(handoff, format);
    });

  handoffCmd
    .command("list")
    .description("List handoffs, optionally filtered by status")
    .option("--status <status>", "Filter by status (pending | picked-up | completed)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const status = opts.status ? assertHandoffStatus(opts.status) : undefined;
      const handoffs = await listUkiHandoffs(services.handoffStore, { status });
      output(isJson, handoffs, formatHandoffList);
    });
}

function buildSlotsFromOptions(opts: Record<string, unknown>): UkiSlots {
  const causalDrivers = toStringArray(opts.driver);
  const divergences = toStringArray(opts.divergence);
  const keyDecisions = toStringArray(opts.decision);
  const decisionBasis = toStringArray(opts.decisionBasis);
  const signalDelta = toStringArray(opts.signal);
  const validationState = toStringArray(opts.validation);
  const artifacts = toStringArray(opts.artifact);
  const boundaryState = toStringArray(opts.boundary);

  const confidenceWork = opts.confidenceWork as number | undefined;
  const confidenceSummary = opts.confidenceSummary as number | undefined;
  if (confidenceWork === undefined && confidenceSummary === undefined) {
      throw new MaestroError(
        "At least one of --confidence-work or --confidence-summary is required",
        [
          "UKI v5.3 CS must be scoped (R5)",
          "Example: --confidence-work 0.95 --confidence-summary 0.9",
        ],
      );
  }

  return {
      sessionCore: String(opts.sessionCore),
      causalDrivers,
      divergences,
      keyDecisions,
      decisionBasis,
      signalDelta,
      validationState,
      executionState: typeof opts.executionState === "string" && opts.executionState.length > 0
        ? opts.executionState
        : "unspecified",
      boundaryState,
      nextAction: String(opts.nextAction),
      artifacts,
      stanceCollapse: typeof opts.stanceCollapse === "string" && opts.stanceCollapse.length > 0
        ? opts.stanceCollapse
        : DEFAULT_STANCE_COLLAPSE,
      blindSpot: typeof opts.blindSpot === "string" && opts.blindSpot.length > 0
        ? opts.blindSpot
        : undefined,
      metaphor: typeof opts.metaphor === "string" && opts.metaphor.length > 0
        ? opts.metaphor
        : undefined,
      cs: {
        ...(confidenceWork !== undefined ? { work: confidenceWork } : {}),
        ...(confidenceSummary !== undefined ? { summary: confidenceSummary } : {}),
    },
    summary: String(opts.summary),
  };
}

function toStringArray(value: unknown): readonly string[] {
  if (value === undefined || value === null) return [];
  if (Array.isArray(value)) {
    return value.filter((v): v is string => typeof v === "string" && v.length > 0);
  }
  if (typeof value === "string") return [value];
  return [];
}

function parseFloatStrict(raw: string): number {
  const n = Number(raw);
  if (!Number.isFinite(n)) {
    throw new MaestroError(`Not a finite number: ${raw}`, [
      "Confidence values must be numeric, e.g. --confidence-work 0.95",
    ]);
  }
  return n;
}

function resolvePickupFormat(
  opts: Record<string, unknown>,
  program: { opts(): Record<string, unknown> },
): PickupFormat {
  const flags = [Boolean(opts.uki), Boolean(opts.markdown), Boolean(opts.json)];
  const count = flags.filter(Boolean).length;
  if (count > 1) {
    throw new MaestroError(
      "--json, --markdown, and --uki are mutually exclusive",
      ["Pick one output format for maestro handoff pickup"],
    );
  }
  if (opts.uki) return "uki";
  if (opts.markdown) return "markdown";
  if (opts.json || resolveJsonFlag(opts, program)) return "json";
  return "json";
}

function printPickup(handoff: UkiHandoff, format: PickupFormat): void {
  if (format === "uki") {
    console.log(handoff.uki);
    return;
  }
  output(format === "json", handoff, formatHandoffMarkdown);
}

function assertHandoffStatus(raw: string): UkiHandoffStatus {
  if ((UKI_HANDOFF_STATUSES as readonly string[]).includes(raw)) {
    return raw as UkiHandoffStatus;
  }
  throw new MaestroError(`Invalid --status: ${raw}`, [
    `Allowed values: ${UKI_HANDOFF_STATUSES.join(" | ")}`,
  ]);
}

function formatHandoffCreate(handoff: UkiHandoff): string[] {
  return [
    `[ok] Handoff created: ${handoff.id}`,
    `  Agent: ${handoff.agent}`,
      `  Session: ${handoff.sessionId}`,
      `  Status: ${handoff.status}`,
      "",
      "UKI v5.3 string:",
      handoff.uki,
    ];
  }

function formatHandoffMarkdown(handoff: UkiHandoff): string[] {
  const s = handoff.slots;
  const lines: string[] = [
    `# Handoff ${handoff.id}`,
    "",
    `- Status: ${handoff.status}`,
    `- Agent: ${handoff.agent}`,
    `- Session: ${handoff.sessionId}`,
    `- Timestamp: ${handoff.timestamp}`,
    "",
    `## Session core`,
    s.sessionCore,
    "",
    `## Summary`,
    s.summary,
    "",
    `## Next action`,
    s.nextAction,
    "",
  ];
    for (const [heading, items] of [
      ["Causal drivers", s.causalDrivers],
      ["Divergences", s.divergences],
      ["Key decisions", s.keyDecisions],
      ["Decision basis", s.decisionBasis],
      ["Signal delta", s.signalDelta],
      ["Validation state", s.validationState],
      ["Artifacts", s.artifacts],
      ["Boundary state", s.boundaryState],
    ] satisfies ReadonlyArray<readonly [string, readonly string[]]>) {
      appendMarkdownListSection(lines, heading, items);
    }
    if (s.blindSpot) {
      lines.push("## Blind spot", s.blindSpot, "");
    }
    if (s.metaphor) {
      lines.push("## Metaphor", s.metaphor, "");
    }
    lines.push(
      "## Execution state",
      s.executionState,
    "",
    "## Stance collapse",
    s.stanceCollapse,
    "",
      "## Confidence",
      `- work: ${s.cs.work ?? "n/a"}`,
      `- summary: ${s.cs.summary ?? "n/a"}`,
      "",
      "## UKI v5.3 string",
      handoff.uki,
    );
  return lines;
}

function appendMarkdownListSection(
  lines: string[],
  heading: string,
  items: readonly string[],
): void {
  if (items.length === 0) {
    return;
  }
  lines.push(`## ${heading}`, ...items.map((item) => `- ${item}`), "");
}

function formatHandoffList(handoffs: readonly UkiHandoff[]): string[] {
  if (handoffs.length === 0) return ["No handoffs found"];
  const lines: string[] = [`${handoffs.length} handoff(s)`, ""];
  for (const h of handoffs) {
    const status = h.status.padEnd(10);
    const agent = h.agent.padEnd(14);
    const summary = h.slots.summary.slice(0, 60);
    lines.push(`${h.id}  ${status}  ${agent}  ${summary}`);
  }
  return lines;
}
