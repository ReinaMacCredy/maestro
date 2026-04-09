import type { RawLearningEntry } from "../domain/memory-types.js";
import type { GitPort } from "../ports/git.port.js";
import type { LearningStorePort } from "../ports/learning-store.port.js";

export interface AppendLearningOpts {
  readonly content: string;
  readonly dir: string;
}

export async function appendLearning(
  git: GitPort,
  store: LearningStorePort,
  opts: AppendLearningOpts,
): Promise<RawLearningEntry> {
  const isRepo = await git.isRepo(opts.dir);
  const branch = isRepo ? (await git.getState(opts.dir)).branch : undefined;

  const entry: RawLearningEntry = {
    sessionDate: new Date().toISOString().slice(0, 10),
    content: opts.content,
    branch,
  };

  await store.appendRaw(entry);
  return entry;
}
