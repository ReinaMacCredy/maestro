import type {
  AssertionStorePort,
  FeatureStorePort,
  MissionStorePort,
} from "@/features/mission";
import type { HandoffLaunchPort, HandoffLaunchRecord, HandoffProvider, LaunchStorePort } from "@/features/handoff";
import { DEFAULT_HANDOFF_MODELS } from "@/features/handoff";
import type { GitPort } from "@/infra/ports/git.port.js";
import { MaestroError } from "@/shared/errors.js";
import { buildHandoffPrompt } from "./build-handoff-prompt.usecase.js";

export interface LaunchHandoffDeps {
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly git: GitPort;
  readonly launchStore: LaunchStorePort;
  readonly launchers: Readonly<Record<HandoffProvider, HandoffLaunchPort>>;
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
    readonly provider: HandoffProvider;
    readonly model?: string;
    readonly name?: string;
    readonly wait: boolean;
    readonly worktree?: string | boolean;
    readonly baseBranch?: string;
  },
): Promise<LaunchHandoffResult> {
  if (input.baseBranch && !input.worktree) {
    throw new MaestroError("--base can only be used with --worktree", [
      "Usage: maestro handoff \"task\" --worktree [slug] --base <branch>",
    ]);
  }

  const providerLauncher = deps.launchers[input.provider];
  if (!providerLauncher) {
    throw new MaestroError(`Unsupported provider '${input.provider}'`, [
      "Valid providers: codex, claude",
    ]);
  }

  const worktree = input.worktree
    ? await createHandoffWorktree(deps.git, input.cwd, input.worktree, input.baseBranch, input.task)
    : undefined;
  const targetDir = worktree?.path ?? input.cwd;
  const model = input.model ?? DEFAULT_HANDOFF_MODELS[input.provider];
  const name = input.name?.trim().length
    ? input.name.trim()
    : `[Handoff] ${truncateTask(input.task)}`;
  const extraConstraints = [
    worktree
      ? `This handoff runs in a fresh worktree at ${worktree.path} on branch ${worktree.branch} from base ${worktree.baseBranch}.`
      : undefined,
  ].filter((line): line is string => line !== undefined);

  const { prompt, context } = await buildHandoffPrompt(deps, {
    cwd: input.cwd,
    task: input.task,
    extraConstraints,
  });

  const initialRecord = await deps.launchStore.create({
    task: input.task,
    name,
    provider: input.provider,
    model,
    wait: input.wait,
    sourceDir: input.cwd,
    targetDir,
    refs: context.refs,
    ...(worktree ? { worktree } : {}),
    prompt,
  });

  try {
    const launchResult = await providerLauncher.launch({
      prompt,
      targetDir,
      model,
      name,
      wait: input.wait,
      logPath: deps.launchStore.resolveArtifactPath(initialRecord.outputPath),
    });
    const finalRecord = await deps.launchStore.update({
      ...initialRecord,
      status: input.wait
        ? (launchResult.exitCode === 0 ? "completed" : "failed")
        : "launched",
      command: launchResult.command,
      ...(launchResult.pid !== undefined ? { pid: launchResult.pid } : {}),
      ...(launchResult.exitCode !== undefined ? { exitCode: launchResult.exitCode } : {}),
    });

    return {
      record: finalRecord,
      prompt,
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const failedRecord = await deps.launchStore.update({
      ...initialRecord,
      status: "failed",
      errorMessage: message,
    });
    throw new MaestroError(`Failed to launch ${input.provider} handoff: ${message}`, [
      `Launch record: ${failedRecord.id}`,
      `Prompt: ${failedRecord.promptPath}`,
      `Log: ${failedRecord.outputPath}`,
    ]);
  }
}

async function createHandoffWorktree(
  git: GitPort,
  cwd: string,
  worktree: string | boolean,
  baseBranch: string | undefined,
  task: string,
) {
  const slug = normalizeWorktreeSlug(typeof worktree === "string" ? worktree : task);
  const resolvedBaseBranch = baseBranch ?? await git.getCurrentBranch(cwd);
  return git.createWorktree(cwd, {
    slug,
    baseBranch: resolvedBaseBranch,
    branchPrefix: "codex",
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
