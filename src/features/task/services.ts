import type { TaskStorePort } from "./ports/task-store.port.js";
import type { CandidateStorePort } from "./ports/candidate-store.port.js";
import type { TaskContinuationStorePort } from "./ports/task-continuation-store.port.js";
import type { TaskContinuationHistoryPort } from "./ports/task-continuation-history.port.js";
import type { NowMdWriterPort } from "./ports/now-md-writer.port.js";
import type { ContractStorePort } from "./ports/contract-store.port.js";
import type { GitAnchorPort } from "./ports/git-anchor.port.js";
import { JsonlTaskStoreAdapter } from "./adapters/jsonl-task-store.adapter.js";
import { FsCandidateStoreAdapter } from "./adapters/fs-candidate-store.adapter.js";
import { FsTaskContinuationStoreAdapter } from "./adapters/fs-task-continuation-store.adapter.js";
import { FsTaskContinuationHistoryStoreAdapter } from "./adapters/fs-task-continuation-history-store.adapter.js";
import { FsNowMdWriterAdapter } from "./adapters/now-md-writer.adapter.js";
import { FsContractStoreAdapter } from "./adapters/fs-contract-store.adapter.js";
import { ShellGitAnchorAdapter } from "./adapters/git-anchor.adapter.js";

export interface TaskServices {
  readonly taskStore: TaskStorePort;
  readonly contractStore: ContractStorePort;
  readonly gitAnchor: GitAnchorPort;
  readonly taskCandidateStore: CandidateStorePort;
  readonly taskContinuationStore: TaskContinuationStorePort;
  readonly taskContinuationHistory: TaskContinuationHistoryPort;
  readonly taskNowMdWriter: NowMdWriterPort;
}

export function buildTaskServices(projectDir: string): TaskServices {
  const contractStore = new FsContractStoreAdapter(projectDir);
  return {
    taskStore: new JsonlTaskStoreAdapter(projectDir),
    contractStore,
    gitAnchor: new ShellGitAnchorAdapter(),
    taskCandidateStore: new FsCandidateStoreAdapter(projectDir),
    taskContinuationStore: new FsTaskContinuationStoreAdapter(projectDir),
    taskContinuationHistory: new FsTaskContinuationHistoryStoreAdapter(projectDir),
    taskNowMdWriter: new FsNowMdWriterAdapter(projectDir, contractStore),
  };
}
