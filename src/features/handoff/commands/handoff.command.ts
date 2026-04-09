/**
 * `maestro handoff` command surface.
 */
import { access } from "node:fs/promises";
import { constants as fsConstants } from "node:fs";
import { join } from "node:path";
import type { Command } from "commander";
import { getServices } from "@/services.js";
import { output, resolveJsonFlag } from "@/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import type {
  ExecuteUkiHandoffContent,
  PlanUkiHandoffContent,
  UkiHandoff,
  UkiHandoffContent,
  UkiHandoffContentBase,
  UkiHandoffMode,
  UkiHandoffStatus,
  UkiMaestroRefs,
} from "../domain/uki-types.js";
import { UKI_HANDOFF_MODES, UKI_HANDOFF_STATUSES } from "../domain/uki-types.js";
import { createUkiHandoff } from "../usecases/create-uki-handoff.usecase.js";
import { listUkiHandoffs } from "../usecases/list-uki-handoffs.usecase.js";
import { pickupUkiHandoff } from "../usecases/pickup-uki-handoff.usecase.js";
import type { Services } from "@/services.js";
import { normalizeUkiToken, UKI_ANCHOR_PREFIXES } from "../lib/uki-token.js";

type CreateFormat = "json" | "paste" | "text" | "uki";
type PickupFormat = "json" | "paste" | "uki";

const HANDOFF_PASTE_PREAMBLE =
  "Use the following UKI as the canonical handoff packet. Interpret each block literally and continue from NEXT_ACTION.";

interface AutoCollectedContext {
  readonly currentState: string;
  readonly maestroRefs: UkiMaestroRefs;
  readonly artifacts: readonly string[];
  readonly readMore: readonly string[];
  readonly planPaths: readonly string[];
  readonly touchedFiles: readonly string[];
}

export function registerHandoffCommand(program: Command): void {
  const handoffCmd = program
    .command("handoff")
    .description("Agent handoff lifecycle (create, pickup, list)")
    .option("--json", "Output as JSON");

  handoffCmd
    .command("create")
    .description("Create a new plan or execute handoff")
    .requiredOption("--mode <mode>", "Handoff mode (plan | execute)")
    .requiredOption("--session-core <text>", "SESSION_CORE transfer value")
    .requiredOption("--summary <text>", "SUMMARY transfer value")
    .requiredOption("--next-action <text>", "NEXT_ACTION transfer value")
    .option("--current-state <text>", "CURRENT_STATE transfer value")
    .option("--decision <text...>", "DECISIONS token (repeatable)")
    .option("--signal <text...>", "SIGNAL_DELTA token (repeatable)")
    .option("--artifact <text...>", "ARTIFACTS token (repeatable)")
    .option("--read-more <text...>", "READ_MORE token (repeatable)")
    .option("--risk <text...>", "RISKS token (repeatable)")
    .option("--driver <text...>", "CAUSAL_DRIVERS token (repeatable)")
    .option("--divergence <text...>", "DIVERGENCES token (repeatable)")
    .option("--boundary <text...>", "BOUNDARY_STATE token (repeatable)")
    .option("--plan-path-item <text...>", "PLAN_PATHS token (repeatable)")
    .option("--maestro-sync <text...>", "MAESTRO_SYNC token (repeatable)")
    .option("--touched-file <text...>", "TOUCHED_FILES token (repeatable)")
    .option("--completed <text...>", "COMPLETED_WORK token (repeatable)")
    .option("--validation <text...>", "VALIDATION token (repeatable)")
    .option("--blind-spot <text>", "BLIND_SPOT transfer value")
    .option("--metaphor <text>", "METAPHOR transfer value")
    .option("--mission-id <text>", "Token-safe Maestro mission reference")
    .option("--feature-id <text>", "Token-safe Maestro feature reference")
    .option("--milestone-id <text>", "Token-safe Maestro milestone reference")
    .option("--plan-ref <text>", "Token-safe Maestro plan reference")
    .option("--spec-ref <text>", "Token-safe Maestro spec reference")
      .option("--confidence-work <number>", "CS.work (0..1)", parseFloatStrict)
      .option("--confidence-summary <number>", "CS.summary (0..1)", parseFloatStrict)
      .option("--agent <name>", "Override agent identity (default auto-detect)")
      .option("--session-id <id>", "Override session id (default auto-detect)")
      .option("--json", "Output as JSON")
      .option("--uki", "Output only the raw UKI transfer string")
      .option("--paste", "Output an agent-ready handoff prompt plus the raw UKI packet")
      .action(async (opts) => {
        const services = getServices();
        const format = resolveCreateFormat(opts, program);
        const content = await buildContentFromOptions(opts, services, process.cwd());
      const handoff = await createUkiHandoff(
        services.handoffStore,
        services.sessionDetect,
        process.cwd(),
        {
          content,
          agent: opts.agent,
          sessionId: opts.sessionId,
        },
      );

      printCreate(handoff, format);
    });

  handoffCmd
    .command("pickup")
      .description("Pick up the latest pending handoff (or a specific one by id)")
      .option("--id <id>", "Specific handoff id to pick up")
      .option("--claim", "Transition pending -> picked-up (atomic claim)")
      .option("--agent <name>", "pickedUpBy attribution when --claim is set")
      .option("--json", "Output as JSON")
      .option("--uki", "Output the raw UKI transfer string (default)")
      .option("--paste", "Output an agent-ready handoff prompt plus the raw UKI packet")
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

async function buildContentFromOptions(
  opts: Record<string, unknown>,
  services: Services,
  cwd: string,
): Promise<UkiHandoffContent> {
  const mode = assertHandoffMode(String(opts.mode));
  const auto = await collectAutoContext(services, cwd, mode);
  const confidenceWork = opts.confidenceWork as number | undefined;
  const confidenceSummary = opts.confidenceSummary as number | undefined;

  if (confidenceWork === undefined && confidenceSummary === undefined) {
    throw new MaestroError(
      "At least one of --confidence-work or --confidence-summary is required",
      ["UKI transfer strings must carry at least one confidence score"],
    );
  }

  const maestroRefs = {
    ...auto.maestroRefs,
    ...(typeof opts.missionId === "string" && opts.missionId.length > 0 ? { missionId: opts.missionId } : {}),
    ...(typeof opts.featureId === "string" && opts.featureId.length > 0 ? { featureId: opts.featureId } : {}),
    ...(typeof opts.milestoneId === "string" && opts.milestoneId.length > 0 ? { milestoneId: opts.milestoneId } : {}),
    ...(typeof opts.planRef === "string" && opts.planRef.length > 0 ? { planPath: opts.planRef } : {}),
    ...(typeof opts.specRef === "string" && opts.specRef.length > 0 ? { specPath: opts.specRef } : {}),
  } satisfies UkiMaestroRefs;

  const base = buildBaseContent(opts, auto, mode, maestroRefs, {
    confidenceWork,
    confidenceSummary,
  });

  if (mode === "plan") {
    return buildPlanContent(base, auto, opts);
  }

  return buildExecuteContent(base, auto, opts);
}

function buildBaseContent(
  opts: Record<string, unknown>,
  auto: AutoCollectedContext,
  mode: UkiHandoffMode,
  maestroRefs: UkiMaestroRefs,
  confidence: {
    readonly confidenceWork?: number;
    readonly confidenceSummary?: number;
  },
): UkiHandoffContentBase {
  return {
    mode,
    currentState: typeof opts.currentState === "string" && opts.currentState.length > 0
      ? opts.currentState
      : auto.currentState,
    sessionCore: String(opts.sessionCore),
    decisions: toStringArray(opts.decision),
    artifacts: uniqueTokens([...auto.artifacts, ...toStringArray(opts.artifact)]),
    readMore: uniqueTokens([...auto.readMore, ...toStringArray(opts.readMore)]),
    nextAction: String(opts.nextAction),
    summary: String(opts.summary),
    maestroRefs,
    cs: {
      ...(confidence.confidenceWork !== undefined ? { work: confidence.confidenceWork } : {}),
      ...(confidence.confidenceSummary !== undefined ? { summary: confidence.confidenceSummary } : {}),
    },
    signalDelta: toStringArray(opts.signal),
    boundaryState: toStringArray(opts.boundary),
    risks: toStringArray(opts.risk),
    blindSpot: typeof opts.blindSpot === "string" && opts.blindSpot.length > 0
      ? opts.blindSpot
      : undefined,
    metaphor: typeof opts.metaphor === "string" && opts.metaphor.length > 0
      ? opts.metaphor
      : undefined,
    causalDrivers: toStringArray(opts.driver),
    divergences: toStringArray(opts.divergence),
  };
}

function buildPlanContent(
  base: UkiHandoffContentBase,
  auto: AutoCollectedContext,
  opts: Record<string, unknown>,
): PlanUkiHandoffContent {
  return {
    ...base,
    mode: "plan",
    planPaths: uniqueTokens([...auto.planPaths, ...toStringArray(opts.planPathItem)]),
    maestroSync: toStringArray(opts.maestroSync),
  };
}

function buildExecuteContent(
  base: UkiHandoffContentBase,
  auto: AutoCollectedContext,
  opts: Record<string, unknown>,
): ExecuteUkiHandoffContent {
  return {
    ...base,
    mode: "execute",
    touchedFiles: uniqueTokens([...auto.touchedFiles, ...toStringArray(opts.touchedFile)]),
    completedWork: toStringArray(opts.completed),
    validation: toStringArray(opts.validation),
  };
}

async function collectAutoContext(
  services: Services,
  cwd: string,
  mode: UkiHandoffMode,
): Promise<AutoCollectedContext> {
  const [gitState, missionRefs, planPaths] = await Promise.all([
    collectGitContext(services, cwd),
    collectMissionRefs(services),
    mode === "plan" ? collectKnownPlanPaths(cwd) : Promise.resolve<readonly string[]>([]),
  ]);

  const artifacts = uniqueTokens([
    ...gitState.artifacts,
    ...planPaths.map((token) => `${UKI_ANCHOR_PREFIXES.file}${token}`),
  ]);

  if (missionRefs.missionId) {
    artifacts.push(`${UKI_ANCHOR_PREFIXES.mission}${missionRefs.missionId}`);
  }

  const readMore = mode === "plan"
    ? uniqueTokens(planPaths.length > 0 ? planPaths : gitState.readMore)
    : uniqueTokens(gitState.readMore);

  return {
    currentState: mode === "plan" ? "plan_ready" : "execute_in_progress",
    maestroRefs: missionRefs,
    artifacts: uniqueTokens(artifacts),
    readMore,
    planPaths,
    touchedFiles: gitState.touchedFiles,
  };
}

async function collectGitContext(
  services: Services,
  cwd: string,
): Promise<{
  readonly artifacts: readonly string[];
  readonly readMore: readonly string[];
  readonly touchedFiles: readonly string[];
  }> {
  try {
    const state = await services.git.getState(cwd);
    if (
      state.branch === "HEAD"
      && state.recentCommits.length === 0
      && state.changedFiles.length === 0
      && state.diffStat === "+0 -0"
    ) {
      return { artifacts: [], readMore: [], touchedFiles: [] };
    }
    const touchedFiles = state.changedFiles.map((file) => `${UKI_ANCHOR_PREFIXES.file}${normalizeUkiToken(file)}`);
    const branchArtifact = state.branch.length > 0
      ? [`${UKI_ANCHOR_PREFIXES.branch}${normalizeUkiToken(state.branch)}`]
      : [];
    const readMore = touchedFiles.length > 0 ? touchedFiles.slice(0, 5) : branchArtifact;

    return {
      artifacts: uniqueTokens([...branchArtifact, ...touchedFiles]),
      readMore: uniqueTokens(readMore),
      touchedFiles: uniqueTokens(touchedFiles),
    };
  } catch {
    return { artifacts: [], readMore: [], touchedFiles: [] };
  }
}

async function collectMissionRefs(services: Services): Promise<UkiMaestroRefs> {
  try {
    const missions = await services.missionStore.list();
    const active = missions.find((mission) =>
      mission.status === "executing"
      || mission.status === "validating"
      || mission.status === "approved"
      || mission.status === "paused",
    );

    if (!active) {
      return {};
    }

      return {
        missionId: normalizeUkiToken(active.id),
      };
  } catch {
    return {};
  }
}

async function collectKnownPlanPaths(cwd: string): Promise<readonly string[]> {
  const candidates = ["PLAN.md"];
  const found = await Promise.all(candidates.map(async (candidate) =>
    await pathExists(join(cwd, candidate))
      ? normalizeUkiToken(candidate)
      : undefined
  ));
  return found.filter((candidate): candidate is string => candidate !== undefined);
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await access(path, fsConstants.F_OK);
    return true;
  } catch {
    return false;
  }
}

function uniqueTokens(values: readonly string[]): string[] {
  return [...new Set(values.filter((value) => value.length > 0))];
}

function toStringArray(value: unknown): readonly string[] {
  if (value === undefined || value === null) return [];
  if (Array.isArray(value)) {
    return value.filter((entry): entry is string => typeof entry === "string" && entry.length > 0);
  }
  if (typeof value === "string") return [value];
  return [];
}

function parseFloatStrict(raw: string): number {
  const value = Number(raw);
  if (!Number.isFinite(value)) {
    throw new MaestroError(`Not a finite number: ${raw}`, [
      "Confidence values must be numeric, e.g. --confidence-work 0.95",
    ]);
  }
  return value;
}

function resolveCreateFormat(
  opts: Record<string, unknown>,
  program: { opts(): Record<string, unknown> },
): CreateFormat {
  return resolveHandoffFormat(opts, program, {
    command: "create",
    defaultFormat: "text",
  }) as CreateFormat;
}

function resolvePickupFormat(
  opts: Record<string, unknown>,
  program: { opts(): Record<string, unknown> },
): PickupFormat {
  return resolveHandoffFormat(opts, program, {
    command: "pickup",
    defaultFormat: "uki",
  }) as PickupFormat;
}

function resolveHandoffFormat(
  opts: Record<string, unknown>,
  program: { opts(): Record<string, unknown> },
  config: {
    readonly command: "create" | "pickup";
    readonly defaultFormat: CreateFormat | PickupFormat;
  },
): CreateFormat | PickupFormat {
  const flags = [Boolean(opts.uki), Boolean(opts.json), Boolean(opts.paste)];
  if (flags.filter(Boolean).length > 1) {
    throw new MaestroError("--json, --uki, and --paste are mutually exclusive", [
      `Pick one output format for maestro handoff ${config.command}`,
    ]);
  }
  if (opts.paste) return "paste";
  if (opts.uki) return "uki";
  if (opts.json || resolveJsonFlag(opts, program)) return "json";
  return config.defaultFormat;
}

function printCreate(handoff: UkiHandoff, format: CreateFormat): void {
  if (format === "uki") {
    console.log(handoff.uki);
    return;
  }
  if (format === "paste") {
    console.log(formatHandoffPaste(handoff.uki));
    return;
  }
  output(format === "json", handoff, formatHandoffCreate);
}

function printPickup(handoff: UkiHandoff, format: PickupFormat): void {
  if (format === "uki") {
    console.log(handoff.uki);
    return;
  }
  if (format === "paste") {
    console.log(formatHandoffPaste(handoff.uki));
    return;
  }
  console.log(JSON.stringify(handoff, null, 2));
}

function formatHandoffPaste(uki: string): string {
  return `${HANDOFF_PASTE_PREAMBLE}\n\n${uki}`;
}

function formatHandoffCreate(handoff: UkiHandoff): string[] {
  return [
    `[ok] Handoff created: ${handoff.id}`,
    `  Mode: ${handoff.content.mode}`,
    `  Agent: ${handoff.agent}`,
    `  Session: ${handoff.sessionId}`,
    `  Status: ${handoff.status}`,
    "",
    "UKI v5.4 string:",
    handoff.uki,
  ];
}

function formatHandoffList(handoffs: readonly UkiHandoff[]): string[] {
  if (handoffs.length === 0) {
    return ["No handoffs found"];
  }

  const lines = [`${handoffs.length} handoff(s)`, ""];
  for (const handoff of handoffs) {
    lines.push(
      `${handoff.id}  ${handoff.status.padEnd(10)}  ${handoff.content.mode.padEnd(7)}  ${handoff.content.summary.slice(0, 60)}`,
    );
  }
  return lines;
}

function assertHandoffMode(raw: string): UkiHandoffMode {
  if ((UKI_HANDOFF_MODES as readonly string[]).includes(raw)) {
    return raw as UkiHandoffMode;
  }
  throw new MaestroError(`Invalid --mode: ${raw}`, [
    `Allowed values: ${UKI_HANDOFF_MODES.join(" | ")}`,
  ]);
}

function assertHandoffStatus(raw: string): UkiHandoffStatus {
  if ((UKI_HANDOFF_STATUSES as readonly string[]).includes(raw)) {
    return raw as UkiHandoffStatus;
  }
  throw new MaestroError(`Invalid --status: ${raw}`, [
    `Allowed values: ${UKI_HANDOFF_STATUSES.join(" | ")}`,
  ]);
}
