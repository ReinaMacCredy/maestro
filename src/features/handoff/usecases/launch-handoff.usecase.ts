import { isAbsolute, resolve } from "node:path";
import type {
  AssertionStorePort,
  FeatureStorePort,
  MissionStorePort,
} from "@/features/mission";
import type {
  HandoffAgent,
  HandoffLaunchPort,
  HandoffLaunchRecord,
  HandoffPromptContext,
  LaunchStorePort,
} from "@/features/handoff";
import { DEFAULT_HANDOFF_MODELS } from "@/features/handoff";
import type { TaskContinuationEvent, TaskContinuationSummary } from "@/features/task";
import type { GitPort } from "@/infra/ports/git.port.js";
import { MaestroError } from "@/shared/errors.js";
import { readText } from "@/shared/lib/fs.js";
import { buildHandoffPrompt } from "./build-handoff-prompt.usecase.js";

const PROMPT_FILE_SIZE_WARN_BYTES = 500_000;

export interface LaunchHandoffDeps {
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly git: GitPort;
  readonly launchStore: LaunchStorePort;
  readonly launchers: Readonly<Record<HandoffAgent, HandoffLaunchPort>>;
}

export interface LaunchHandoffResult {
  readonly record: HandoffLaunchRecord;
  readonly prompt: string;
}

export async function launchHandoff(
  deps: LaunchHandoffDeps,
  input: {
    readonly cwd: string;
    readonly task: string;
    readonly agent: HandoffAgent;
    readonly model?: string;
    readonly name?: string;
    readonly wait: boolean;
    readonly worktree?: string | boolean;
    readonly baseBranch?: string;
    readonly promptFile?: string;
    readonly refs?: {
      readonly taskId?: string;
      readonly createdByAgent?: string;
      readonly createdBySessionId?: string;
    };
    readonly continuation?: {
      readonly summary: TaskContinuationSummary;
      readonly recentEvents: readonly TaskContinuationEvent[];
    };
  },
): Promise<LaunchHandoffResult> {
  if (input.baseBranch && !input.worktree) {
    throw new MaestroError("--base can only be used with --worktree", [
      "Usage: maestro handoff \"task\" --worktree [slug] --base <branch>",
    ]);
  }

  const handoffLauncher = deps.launchers[input.agent];
  if (!handoffLauncher) {
    throw new MaestroError(`Unsupported agent '${input.agent}'`, [
      "Valid agents: codex, claude",
    ]);
  }

  const worktree = input.worktree
    ? await createHandoffWorktree(deps.git, input.cwd, input.agent, input.worktree, input.baseBranch, input.task)
    : undefined;
  const targetDir = worktree?.path ?? input.cwd;
  const model = input.model ?? DEFAULT_HANDOFF_MODELS[input.agent];
  const name = input.name?.trim().length
    ? input.name.trim()
    : `[Handoff] ${truncateTask(input.task)}`;
  const extraConstraints = [
    worktree
      ? `This handoff runs in a fresh worktree at ${worktree.path} on branch ${worktree.branch} from base ${worktree.baseBranch}.`
      : undefined,
  ].filter((line): line is string => line !== undefined);

  const { prompt, context } = input.promptFile !== undefined
    ? {
        prompt: await readPromptFromFile(input.promptFile, input.cwd),
        context: buildMinimalContext(input.task, extraConstraints, input.refs?.taskId),
      }
    : await buildHandoffPrompt(deps, {
        cwd: input.cwd,
        task: input.task,
        extraConstraints,
        continuation: input.continuation,
        taskId: input.refs?.taskId,
      });

  const initialRecord = await deps.launchStore.create({
    task: input.task,
    name,
    agent: input.agent,
    model,
    wait: input.wait,
    sourceDir: input.cwd,
    targetDir,
    refs: {
      ...context.refs,
      ...(input.refs?.taskId ? { taskId: input.refs.taskId } : {}),
    },
    ...(input.refs?.createdByAgent ? { createdByAgent: input.refs.createdByAgent } : {}),
    ...(input.refs?.createdBySessionId ? { createdBySessionId: input.refs.createdBySessionId } : {}),
    ...(worktree ? { worktree } : {}),
    prompt,
  });

  try {
    const launchResult = await handoffLauncher.launch({
      prompt,
      targetDir,
      model,
      name,
      wait: input.wait,
      logPath: deps.launchStore.resolveArtifactPath(initialRecord.outputPath, initialRecord.refs),
    });
    const waitedExitCode = input.wait ? launchResult.exitCode : undefined;
    const finalRecord = await deps.launchStore.update({
      ...initialRecord,
      status: input.wait
        ? (waitedExitCode === 0 ? "completed" : "failed")
        : "launched",
      command: launchResult.command,
      ...(launchResult.pid !== undefined ? { pid: launchResult.pid } : {}),
      ...(launchResult.exitCode !== undefined ? { exitCode: launchResult.exitCode } : {}),
    });

    if (input.wait && waitedExitCode !== 0) {
      const message = waitedExitCode === undefined
        ? `${input.agent} handoff did not report an exit code`
        : `${input.agent} handoff exited with code ${waitedExitCode}`;
      throw new MaestroError(message, [
        `Launch record: ${finalRecord.id}`,
        `Prompt: ${finalRecord.promptPath}`,
        `Log: ${finalRecord.outputPath}`,
      ]);
    }

    return {
      record: finalRecord,
      prompt,
    };
  } catch (error) {
    if (error instanceof MaestroError) {
      throw error;
    }
    const message = error instanceof Error ? error.message : String(error);
    const failedRecord = await deps.launchStore.update({
      ...initialRecord,
      status: "failed",
      errorMessage: message,
    });
    throw new MaestroError(`Failed to launch ${input.agent} handoff: ${message}`, [
      `Launch record: ${failedRecord.id}`,
      `Prompt: ${failedRecord.promptPath}`,
      `Log: ${failedRecord.outputPath}`,
    ]);
  }
}

async function createHandoffWorktree(
  git: GitPort,
  cwd: string,
  agent: HandoffAgent,
  worktree: string | boolean,
  baseBranch: string | undefined,
  task: string,
) {
  const slug = normalizeWorktreeSlug(typeof worktree === "string" ? worktree : task);
  const resolvedBaseBranch = baseBranch ?? await git.getCurrentBranch(cwd);
  return git.createWorktree(cwd, {
    slug,
    baseBranch: resolvedBaseBranch,
    branchPrefix: agent,
  });
}

function normalizeWorktreeSlug(value: string): string {
  const slug = value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40);
  return slug.length > 0 ? slug : "handoff";
}

function truncateTask(task: string): string {
  const normalized = task.replace(/\s+/g, " ").trim();
  return normalized.length > 72 ? `${normalized.slice(0, 69)}...` : normalized;
}

async function readPromptFromFile(promptFile: string, cwd: string): Promise<string> {
  const absolute = isAbsolute(promptFile) ? promptFile : resolve(cwd, promptFile);
  let content: string | undefined;
  try {
    content = await readText(absolute);
  } catch (err: unknown) {
    const code = (err as NodeJS.ErrnoException | undefined)?.code;
    if (code === "EISDIR") {
      throw new MaestroError(`--prompt-file is a directory, not a file: ${absolute}`, [
        "Pass a path to a readable file containing the brief",
      ]);
    }
    throw err;
  }
  if (content === undefined) {
    throw new MaestroError(`--prompt-file not found: ${absolute}`, [
      "Check the path is correct and readable",
      "Use an absolute path or a path relative to the current working directory",
    ]);
  }
  if (content.trim().length === 0) {
    throw new MaestroError(`--prompt-file is empty: ${absolute}`, [
      "Write the handoff brief to the file before launching",
      "An empty prompt produces a useless launch",
    ]);
  }
  if (Buffer.byteLength(content, "utf8") > PROMPT_FILE_SIZE_WARN_BYTES) {
    // Warn but do not hard-fail. Brief files this large are unusual and
    // probably indicate something went wrong upstream (wrong file, runaway
    // template, log dump). Keep launching so the agent can still use it.
    process.stderr.write(
      `[warn] --prompt-file is larger than ${PROMPT_FILE_SIZE_WARN_BYTES} bytes: ${absolute}\n`,
    );
  }
  return content;
}

function buildMinimalContext(
  task: string,
  extraConstraints: readonly string[],
  taskId: string | undefined,
): HandoffPromptContext {
  return {
    task,
    context: [],
    relevantFiles: [],
    currentState: [],
    whatWasTried: [],
    decisions: [],
    acceptanceCriteria: [],
    constraints: extraConstraints,
    refs: taskId ? { taskId } : {},
  };
}
