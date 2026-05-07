import type { TaskStorePort } from "./ports/task-store.port.js";
import type { CandidateStorePort } from "./ports/candidate-store.port.js";
import type { TaskContinuationStorePort } from "./ports/task-continuation-store.port.js";
import type { TaskContinuationHistoryPort } from "./ports/task-continuation-history.port.js";
import type { ContractStorePort } from "./ports/contract-store.port.js";
import type { ContractVersionStorePort } from "./ports/contract-version-store.port.js";
import type { GitAnchorPort } from "./ports/git-anchor.port.js";
import type { RunStateStorePort } from "./ports/run-state-store.port.js";
import {
  buildContractWorkflows,
  type ContractWorkflows,
} from "./usecases/contract-workflows.usecase.js";
import { JsonlTaskStoreAdapter } from "./adapters/jsonl-task-store.adapter.js";
import { FsCandidateStoreAdapter } from "./adapters/fs-candidate-store.adapter.js";
import { FsTaskContinuationStoreAdapter } from "./adapters/fs-task-continuation-store.adapter.js";
import { FsTaskContinuationHistoryStoreAdapter } from "./adapters/fs-task-continuation-history-store.adapter.js";
import { FsNowMdWriterAdapter } from "./adapters/now-md-writer.adapter.js";
import { FsContractStoreAdapter } from "./adapters/fs-contract-store.adapter.js";
import { FsContractVersionStoreAdapter } from "./adapters/fs-contract-version-store.adapter.js";
import { ShellGitAnchorAdapter } from "./adapters/git-anchor.adapter.js";
import { FsRunStateStoreAdapter } from "./adapters/fs-run-state-store.adapter.js";
import { FsEvidenceStoreAdapter } from "@/features/evidence";

export interface TaskServices {
  readonly taskStore: TaskStorePort;
  readonly contractStore: ContractStorePort;
  readonly contractVersionStore: ContractVersionStorePort;
  readonly contracts: ContractWorkflows;
  readonly gitAnchor: GitAnchorPort;
  readonly taskCandidateStore: CandidateStorePort;
  readonly taskContinuationStore: TaskContinuationStorePort;
  readonly taskContinuationHistory: TaskContinuationHistoryPort;
  readonly taskNowMdWriter: FsNowMdWriterAdapter;
  readonly runStateStore: RunStateStorePort;
}

export function buildTaskServices(projectDir: string): TaskServices {
  const taskStore = new JsonlTaskStoreAdapter(projectDir);
  const contractStore = new FsContractStoreAdapter(projectDir);
  const contractVersionStore = new FsContractVersionStoreAdapter(projectDir);
  const gitAnchor = new ShellGitAnchorAdapter();
  const evidenceStore = new FsEvidenceStoreAdapter(projectDir);
  const contracts = buildContractWorkflows(
    contractStore,
    taskStore,
    gitAnchor,
    contractVersionStore,
    evidenceStore,
  );
  return {
    taskStore,
    contractStore,
    contractVersionStore,
    contracts,
    gitAnchor,
    taskCandidateStore: new FsCandidateStoreAdapter(projectDir),
    taskContinuationStore: new FsTaskContinuationStoreAdapter(projectDir),
    taskContinuationHistory: new FsTaskContinuationHistoryStoreAdapter(projectDir),
    taskNowMdWriter: new FsNowMdWriterAdapter(projectDir, contractStore),
    runStateStore: new FsRunStateStoreAdapter(projectDir),
  };
}
