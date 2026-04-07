import type {
  CompiledLearnings,
  Correction,
  MemoryStats,
  RatchetBaseline,
  RatchetSuite,
} from "../domain/memory-types.js";
import type { CorrectionStorePort } from "../ports/correction-store.port.js";
import type { LearningStorePort } from "../ports/learning-store.port.js";
import type { RatchetStorePort } from "../ports/ratchet-store.port.js";
import type { ProjectGraphStorePort } from "../ports/project-graph-store.port.js";

export function buildMemoryStats(options: {
  readonly corrections: readonly Correction[];
  readonly rawLearningCount: number;
  readonly compiledLearnings?: CompiledLearnings;
  readonly ratchetSuite: RatchetSuite;
  readonly ratchetBaseline?: RatchetBaseline;
  readonly graphProjects: number;
  readonly graphLinks: number;
}): MemoryStats {
  const {
    corrections,
    rawLearningCount,
    compiledLearnings,
    ratchetSuite,
    ratchetBaseline,
    graphProjects,
    graphLinks,
  } = options;
  const hard = corrections.filter((correction) => correction.severity === "hard").length;
  const soft = corrections.length - hard;
  const staleDays = compiledLearnings
    ? getCompiledLearningStaleDays(compiledLearnings.compiledAt)
    : undefined;
  let lastResult: "pass" | "fail" | undefined;
  if (ratchetBaseline && ratchetSuite.assertions.length > 0) {
    lastResult = ratchetBaseline.passCount === ratchetSuite.assertions.length ? "pass" : "fail";
  }

  return {
    corrections: { total: corrections.length, hard, soft },
    learnings: { rawCount: rawLearningCount, compiledAt: compiledLearnings?.compiledAt, staleDays },
    ratchet: { assertions: ratchetSuite.assertions.length, lastResult },
    graph: { projects: graphProjects, links: graphLinks },
  };
}

export async function getMemoryStats(
  corrStore: CorrectionStorePort,
  learnStore: LearningStorePort,
  ratchetStore: RatchetStorePort,
  graphStore?: ProjectGraphStorePort,
): Promise<MemoryStats> {
  const [corrections, rawCount, compiledLearnings, ratchetSuite, ratchetBaseline, graph] = await Promise.all([
    corrStore.list(),
    learnStore.rawCount(),
    learnStore.readCompiled(),
    ratchetStore.getSuite(),
    ratchetStore.getBaseline(),
    graphStore?.load(),
  ]);

  return buildMemoryStats({
    corrections,
    rawLearningCount: rawCount,
    compiledLearnings,
    ratchetSuite,
    ratchetBaseline,
    graphProjects: graph?.nodes.length ?? 0,
    graphLinks: graph?.edges.length ?? 0,
  });
}

function getCompiledLearningStaleDays(compiledAt: string): number {
  const compiledDate = new Date(compiledAt);
  const now = new Date();
  return Math.floor((now.getTime() - compiledDate.getTime()) / (1000 * 60 * 60 * 24));
}
