import type { Command } from "commander";
import { homedir, userInfo } from "node:os";
import { basename } from "node:path";
import { type Services } from "@/services.js";
import {
  DEFAULT_HANDOFF_MODELS,
  launchHandoff,
  listProjectHandoffs,
  pickupHandoff,
  showProjectHandoff,
  type HandoffAgent,
  type HandoffRecord,
} from "@/features/handoff";
import { getHandoffDisplayState } from "../domain/handoff-state.js";
import {
  buildTaskContinuationSummary,
  buildTaskOwnerId,
  loadTaskContinuationSummary,
  type TaskContinuationEvent,
  type TaskContinuationSummary,
} from "@/features/task";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag, warn } from "@/shared/lib/output.js";
import { summarizeHandoff } from "@/shared/lib/projection.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";

interface HandoffCommandDeps {
  readonly getServices: () => Pick<
    Services,
    | "missions"
    | "git"
    | "handoffStore"
    | "handoffLaunchers"
    | "taskStore"
    | "contracts"
    | "taskContinuationStore"
    | "taskContinuationHistory"
  >;
}

export function registerHandoffCommand(
  program: Command,
  deps: HandoffCommandDeps,
): void {
  const handoffCmd = program
    .command("handoff")
    .description("Launch a handoff packet (pass <task> or --prompt-file). Bare `maestro handoff` (no args, no launch flags) lists existing packets; see subcommands `list`, `pickup`, `show`.")
    .argument("[task]", "Task description for a new handoff launch")
    .option("--agent <agent>", "Target agent (codex|claude|hermes)")
    .option("--task-id <id>", "Link the handoff to a specific task id")
    .option("--model <model>", "Override the agent default model")
    .option("--worktree [slug]", "Create and use a sibling git worktree for the handoff")
    .option("--base <branch>", "Base branch to use with --worktree")
    .option("--name <title>", "Display name for the launched session")
    .option("--prompt-file <path>", "Path to a pre-written brief; skips auto-generation")
    .option("--wait", "Wait for the external agent to finish before returning")
    .option("--json", "Output as JSON")
    .action(async (task: string | undefined, opts): Promise<void> => {
      const promptFile = typeof opts.promptFile === "string" ? opts.promptFile : undefined;
      const name = typeof opts.name === "string" ? opts.name : undefined;
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);

      // Bare `maestro handoff` (no positional, no --prompt-file, no launch
      // flags) is treated as a listing query: agents arriving fresh discover
      // the verb without crashing into a "Task description required" error.
      // Any explicit launch signal (positional, --prompt-file, --agent,
      // --task-id, --model, --worktree, --base, --wait) still forces the
      // launch path so a mis-typed task arg can't silently become a list.
      const hasLaunchSignal = task !== undefined
        || promptFile !== undefined
        || opts.agent !== undefined
        || opts.taskId !== undefined
        || opts.model !== undefined
        || opts.worktree !== undefined
        || opts.base !== undefined
        || opts.wait === true;
      if (!hasLaunchSignal) {
        const currentProjectRoot = resolveMaestroProjectRoot(process.cwd());
        const records = await listProjectHandoffs(services.handoffStore, {
          openOnly: false,
          taskStore: services.taskStore,
          currentProjectRoot,
        });
        if (isJson) {
          output(true, records.slice(0, 20).map(summarizeHandoff), () => []);
          return;
        }
        output(false, records, formatHandoffList);
        return;
      }

      // When the caller supplies a pre-written brief via --prompt-file, the
      // positional task arg is optional: the brief itself carries the task
      // description. Synthesize a short task string from --name (preferred)
      // or a stable fallback, so the launch record and prompt remain well-
      // formed without forcing every skill example to re-spell the task.
      const resolvedTask = task
        ?? (promptFile ? (name?.trim().length ? name!.trim() : "Handoff") : undefined);
      if (!resolvedTask) {
        throw new MaestroError("Task description required for handoff launch", [
          "Use `maestro handoff <task>` to create a packet",
          "Or pass --prompt-file <path> to skip the positional (the brief is enough)",
          "Or use `maestro handoff pickup` to consume an existing packet",
          "Or run `maestro handoff` (no args) to see existing packets",
        ]);
      }
      const agent = parseAgent(opts.agent);
      const linkedTask = await resolveLinkedTask(deps, typeof opts.taskId === "string" ? opts.taskId : undefined);
      const result = await launchHandoff({
        missions: services.missions,
        git: services.git,
        handoffStore: services.handoffStore,
        launchers: services.handoffLaunchers,
      }, {
        cwd: process.cwd(),
        task: resolvedTask,
        agent,
        model: typeof opts.model === "string" ? opts.model : undefined,
        name,
        wait: Boolean(opts.wait),
        worktree: opts.worktree as string | boolean | undefined,
        baseBranch: typeof opts.base === "string" ? opts.base : undefined,
        promptFile,
        refs: {
          taskId: linkedTask.taskId,
          createdByAgent: linkedTask.summary?.activeAgent?.type,
          createdBySessionId: linkedTask.summary?.activeAgent?.sessionId,
        },
        ...(linkedTask.summary
          ? {
              continuation: {
                summary: linkedTask.summary,
                recentEvents: linkedTask.recentEvents,
              },
            }
          : {}),
      });

      if (linkedTask.taskId) {
        await services.taskContinuationHistory.append(linkedTask.taskId, {
          kind: "handoff_created",
          at: result.record.createdAt,
          summary: `Created handoff ${result.record.id} for ${agent}`,
          handoffId: result.record.id,
          agent,
        });
      }

      output(isJson, result.record, formatHandoffRecord);
    });

  handoffCmd
    .command("pickup")
    .description("Pick up an open handoff packet and resume its linked task unless --standalone is passed")
    .option("--id <id>", "Specific handoff id to pick up")
    .option("--agent <agent>", "Current agent when auto-detection is unavailable")
    .option("--session <id>", "Current session id when auto-detection is unavailable")
    .option("--standalone", "Consume the packet without resuming its linked task")
    .option("--json", "Output as JSON")
    .action(async (opts, command: Command): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const currentProjectRoot = resolveMaestroProjectRoot(process.cwd());
      const handoffId = await resolvePickupId(deps, typeof opts.id === "string" ? opts.id : undefined);
      const launch = await services.handoffStore.get(handoffId);
      if (!launch) {
        throw new MaestroError(`Handoff not found: ${handoffId}`);
      }
      const standalone = Boolean(opts.standalone);
      const requireSession = Boolean(launch.refs.taskId) && !standalone;
      const actor = await resolvePickupActor(
        deps,
        opts,
        command.parent?.opts(),
        { requireSession, fallbackAgent: launch.agent },
      );
      const result = await pickupHandoff(
        {
          handoffStore: services.handoffStore,
          taskStore: services.taskStore,
          contracts: services.contracts,
          continuationStore: services.taskContinuationStore,
          continuationHistory: services.taskContinuationHistory,
        },
        {
          id: handoffId,
          actorAgent: actor.agent,
          ...(actor.sessionId ? { actorSessionId: actor.sessionId } : {}),
          ...(actor.ownerId ? { ownerId: actor.ownerId } : {}),
          currentProjectRoot,
          standalone,
        },
      );

      if (result.contractTransferWarning) {
        warn(result.contractTransferWarning);
      }
      if (standalone && launch.refs.taskId) {
        warn(
          `Handoff ${handoffId} was picked up as standalone. Linked task ${launch.refs.taskId} was left unchanged.`,
        );
      }
      if (result.unlinkedTaskId) {
        warn(
          `Handoff ${handoffId} pointed at task ${result.unlinkedTaskId}, which no longer exists in this project. Packet was unlinked and picked up as standalone.`,
        );
      }

      output(isJson, result.record, (record) => formatPickupRecord(record, result.taskId, result.ownerId));
    });

  handoffCmd
    .command("list")
    .description("List handoff packets")
    .option("--open", "Only show packets that have not been consumed")
    .option("--limit <n>", "Maximum packets to return (default 20 for --json)")
    .option("--all", "Disable the default --json limit (return every match)")
    .option("--full", "Include all fields (refs, command, paths) in --json (default: lean summary)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const isFull = opts.full === true;
      const wantAll = opts.all === true;
      const explicitLimit = opts.limit !== undefined
        ? Number.parseInt(String(opts.limit), 10)
        : undefined;
      const effectiveLimit = explicitLimit ?? (isJson && !wantAll ? 20 : undefined);
      const currentProjectRoot = resolveMaestroProjectRoot(process.cwd());
      const records = await listProjectHandoffs(services.handoffStore, {
        openOnly: Boolean(opts.open),
        taskStore: services.taskStore,
        currentProjectRoot,
      });
      const sliced = effectiveLimit !== undefined && effectiveLimit > 0
        ? records.slice(0, effectiveLimit)
        : records;
      if (isJson && !isFull) {
        output(true, sliced.map(summarizeHandoff), () => []);
        return;
      }
      output(isJson, sliced, formatHandoffList);
    });

  handoffCmd
    .command("show <id>")
    .description("Show a handoff packet")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const currentProjectRoot = resolveMaestroProjectRoot(process.cwd());
      const record = await showProjectHandoff(services.handoffStore, id, {
        taskStore: services.taskStore,
        currentProjectRoot,
      });
      output(isJson, record, (r) => formatHandoffDetail(r));
    });
}

function parseAgent(value: unknown): HandoffAgent {
  if (value === undefined) {
    return "codex";
  }
  if (value === "codex" || value === "claude" || value === "hermes") {
    return value;
  }

  throw new MaestroError(`Invalid --agent '${String(value)}'`, [
    "Valid agents: codex, claude, hermes",
    `Defaults: codex=${DEFAULT_HANDOFF_MODELS.codex}, claude=${DEFAULT_HANDOFF_MODELS.claude}, hermes=${DEFAULT_HANDOFF_MODELS.hermes}`,
  ]);
}

async function resolveLinkedTask(
  deps: HandoffCommandDeps,
  explicitTaskId: string | undefined,
): Promise<{
  readonly taskId?: string;
  readonly summary?: TaskContinuationSummary;
  readonly recentEvents: readonly TaskContinuationEvent[];
}> {
  const services = deps.getServices();
  if (!explicitTaskId) {
    // Auto-link only fires for tasks actually in_progress. `listActive` can
    // surface continuations for pending tasks that were claimed-then-
    // unclaimed, which would otherwise cause a surprise project-store link
    // for a packet the user meant to be standalone.
    const allActive = await services.taskContinuationStore.listActive();
    const inProgress: TaskContinuationSummary[] = [];
    for (const summary of allActive) {
      const task = await services.taskStore.get(summary.taskId);
      if (task?.status === "in_progress") {
        inProgress.push(summary);
      }
    }
    if (inProgress.length !== 1) {
      // Zero, or multiple: safest to treat as standalone. Callers who want
      // task linkage pass --task-id explicitly.
      return { recentEvents: [] };
    }
    return {
      taskId: inProgress[0]!.taskId,
      summary: inProgress[0]!,
      recentEvents: await services.taskContinuationHistory.listRecent(inProgress[0]!.taskId, 5),
    };
  }

  const task = await services.taskStore.get(explicitTaskId);
  if (!task) {
    throw new MaestroError(`Task not found: ${explicitTaskId}`);
  }
  if (task.status === "completed") {
    throw new MaestroError(`Task ${explicitTaskId} is already completed and cannot anchor a new handoff`, [
      "Reopen the task first if you need to continue it",
    ]);
  }

  let summary = await loadTaskContinuationSummary(services.taskContinuationStore, explicitTaskId);
  if (!summary) {
    summary = buildTaskContinuationSummary(task);
    await services.taskContinuationStore.upsertActive(summary);
  }

  return {
    taskId: explicitTaskId,
    summary,
    recentEvents: await services.taskContinuationHistory.listRecent(explicitTaskId, 5),
  };
}

async function resolvePickupId(
  deps: HandoffCommandDeps,
  explicitId: string | undefined,
): Promise<string> {
  if (explicitId) {
    return explicitId;
  }

  const services = deps.getServices();
  const currentProjectRoot = resolveMaestroProjectRoot(process.cwd());
  const open = await listProjectHandoffs(services.handoffStore, {
    openOnly: true,
    taskStore: services.taskStore,
    currentProjectRoot,
  });
  if (open.length === 0) {
    throw new MaestroError("No open handoff packets are available to pick up");
  }
  if (open.length !== 1) {
    const preview = open.slice(0, 10).map((r) => {
      const task = r.task.length > 60 ? `${r.task.slice(0, 57)}...` : r.task;
      return `  ${r.id}  agent=${r.agent}  created=${r.createdAt}  task=${JSON.stringify(task)}`;
    });
    const hints = [
      `${open.length} open packets. Pass --id <handoff-id> to choose one:`,
      ...preview,
    ];
    if (open.length > preview.length) {
      hints.push(`  ...and ${open.length - preview.length} more`);
    }
    throw new MaestroError("Multiple open handoff packets exist; pickup is ambiguous", hints);
  }
  return open[0]!.id;
}

async function resolvePickupActor(
  deps: HandoffCommandDeps,
  opts: { agent?: unknown; session?: unknown },
  inherited: { agent?: unknown } | undefined,
  mode: { readonly requireSession: boolean; readonly fallbackAgent: HandoffAgent },
): Promise<{
  readonly agent: HandoffAgent;
  readonly sessionId?: string;
  readonly ownerId?: string;
}> {
  const rawAgent = opts.agent ?? inherited?.agent;
  const rawSession = opts.session;
  const explicitAgent = typeof rawAgent === "string" ? rawAgent.trim() : undefined;
  const explicitSession = typeof rawSession === "string" ? rawSession.trim() : undefined;

  if (mode.requireSession) {
    if ((explicitAgent && !explicitSession) || (!explicitAgent && explicitSession)) {
      throw new MaestroError("Pass both --agent and --session together when overriding pickup identity", [
        "Or set MAESTRO_AGENT/MAESTRO_SESSION_ID in your environment",
      ]);
    }

    if (explicitAgent && explicitSession) {
      const agent = parseAgent(explicitAgent);
      return {
        agent,
        sessionId: explicitSession,
        ownerId: buildTaskOwnerId(agent, explicitSession),
      };
    }

    const envSession = readEnvSession();
    if (envSession) {
      return envSession;
    }

    const fallbackSessionId = fallbackPickupSessionId();
    return {
      agent: mode.fallbackAgent,
      sessionId: fallbackSessionId,
      ownerId: buildTaskOwnerId("local", fallbackSessionId),
    };
  }

  if (!explicitAgent && explicitSession) {
    throw new MaestroError("Pass both --agent and --session together when overriding pickup identity", [
      "Or set MAESTRO_AGENT/MAESTRO_SESSION_ID in your environment",
    ]);
  }

  if (explicitAgent && explicitSession) {
    const agent = parseAgent(explicitAgent);
    return {
      agent,
      sessionId: explicitSession,
      ownerId: buildTaskOwnerId(agent, explicitSession),
    };
  }

  if (explicitAgent) {
    return { agent: parseAgent(explicitAgent) };
  }

  const envSession = readEnvSession();
  if (envSession) {
    return envSession;
  }
  return { agent: mode.fallbackAgent };
}

function readEnvSession(): { agent: HandoffAgent; sessionId: string; ownerId: string } | undefined {
  const envAgent = (process.env.MAESTRO_AGENT ?? "").trim();
  const envSessionId = (process.env.MAESTRO_SESSION_ID ?? "").trim();
  if (envAgent.length === 0 || envSessionId.length === 0) return undefined;
  try {
    const agent = parseAgent(envAgent);
    return {
      agent,
      sessionId: envSessionId,
      ownerId: buildTaskOwnerId(agent, envSessionId),
    };
  } catch {
    return undefined;
  }
}

function fallbackPickupSessionId(): string {
  const envUser = (process.env.USER ?? process.env.USERNAME ?? "").trim();
  if (envUser.length > 0) return envUser;
  // Bun's `userInfo().username` returns the literal string "unknown" when
  // USER/USERNAME are unset, unlike Node which falls back to getpwuid. Treat
  // that literal as a miss and reach for homedir's basename, which is the
  // user's real account name on every platform we run on.
  try {
    const name = userInfo().username.trim();
    if (name.length > 0 && name !== "unknown") return name;
  } catch {
    // fall through
  }
  try {
    const home = homedir().trim();
    if (home.length > 0) {
      const base = basename(home);
      if (base.length > 0 && base !== "root") return base;
    }
  } catch {
    // fall through
  }
  return "default";
}

function formatHandoffRecord(record: {
  readonly id: string;
  readonly agent: HandoffAgent;
  readonly model: string;
  readonly status: string;
  readonly targetDir: string;
  readonly promptPath: string;
  readonly outputPath: string;
  readonly refs: { readonly taskId?: string };
  readonly worktree?: { readonly branch: string; readonly baseBranch: string; readonly path: string };
  readonly pid?: number;
  readonly exitCode?: number;
}): string[] {
  const lines = [
    `[ok] Handoff launched: ${record.id}`,
    `  Agent: ${record.agent}/${record.model}`,
    `  Status: ${record.status}`,
    ...(record.refs.taskId ? [`  Task: ${record.refs.taskId}`] : []),
    `  Target: ${record.targetDir}`,
    `  Prompt: ${record.promptPath}`,
    `  Log: ${record.outputPath}`,
  ];

  if (record.worktree) {
    lines.push(`  Worktree: ${record.worktree.path} (${record.worktree.branch} from ${record.worktree.baseBranch})`);
  }

  if (record.pid !== undefined) {
    lines.push(`  PID: ${record.pid}`);
  }

  if (record.exitCode !== undefined) {
    lines.push(`  Exit code: ${record.exitCode}`);
  }

  return lines;
}

function formatPickupRecord(
  record: {
    readonly id: string;
    readonly pickedUpByAgent?: string;
    readonly pickedUpBySessionId?: string;
    readonly consumedAt?: string;
    readonly promptPath: string;
  },
  taskId: string | undefined,
  ownerId: string | undefined,
): string[] {
  return [
    `[ok] Handoff picked up: ${record.id}`,
    ...(taskId ? [`  Task: ${taskId}`] : []),
    ...(taskId && ownerId ? [`  Owner: ${ownerId}`] : []),
    ...(record.pickedUpByAgent ? [`  Picked up by: ${record.pickedUpByAgent}${record.pickedUpBySessionId ? `/${record.pickedUpBySessionId}` : ""}`] : []),
    ...(record.consumedAt ? [`  Consumed at: ${record.consumedAt}`] : []),
    `  Prompt: ${record.promptPath}`,
  ];
}

function formatHandoffList(records: readonly HandoffRecord[]): string[] {
  if (records.length === 0) {
    return ["No handoff packets"];
  }
  const lines = [`[ok] ${records.length} packet(s)`];
  for (const r of records) {
    const state = getHandoffDisplayState(r);
    const task = r.refs.taskId ? ` task=${r.refs.taskId}` : "";
    const short = r.task.length > 60 ? `${r.task.slice(0, 57)}...` : r.task;
    lines.push(`  ${r.id}  ${state}  agent=${r.agent}  created=${r.createdAt}${task}  ${JSON.stringify(short)}`);
  }
  return lines;
}

function formatHandoffDetail(record: HandoffRecord): string[] {
  const lines = [
    `[ok] ${record.id}`,
    `  State: ${getHandoffDisplayState(record)}`,
    `  Agent: ${record.agent}/${record.model}`,
    `  Status: ${record.status}`,
    `  Created: ${record.createdAt}`,
    `  Task: ${JSON.stringify(record.task)}`,
    ...(record.refs.taskId ? [`  Linked task: ${record.refs.taskId}`] : []),
    ...(record.createdByAgent
      ? [`  Created by: ${record.createdByAgent}${record.createdBySessionId ? `/${record.createdBySessionId}` : ""}`]
      : []),
    ...(record.pickedUpByAgent
      ? [`  Picked up by: ${record.pickedUpByAgent}${record.pickedUpBySessionId ? `/${record.pickedUpBySessionId}` : ""}`]
      : []),
    ...(record.consumedAt ? [`  Consumed at: ${record.consumedAt}`] : []),
    `  Target: ${record.targetDir}`,
    `  Prompt: ${record.promptPath}`,
    `  Log: ${record.outputPath}`,
  ];
  if (record.worktree) {
    lines.push(`  Worktree: ${record.worktree.path} (${record.worktree.branch} from ${record.worktree.baseBranch})`);
  }
  return lines;
}

