import type { MemoryStats } from "../domain/memory-types.js";
import type { CorrectionStorePort } from "../ports/correction-store.port.js";
import type { LearningStorePort } from "../ports/learning-store.port.js";
import type { RatchetStorePort } from "../ports/ratchet-store.port.js";
import type { ProjectGraphStorePort } from "../ports/project-graph-store.port.js";

export async function getMemoryStats(
  corrStore: CorrectionStorePort,
  learnStore: LearningStorePort,
  ratchetStore: RatchetStorePort,
  graphStore?: ProjectGraphStorePort,
): Promise<MemoryStats> {
  const corrections = await corrStore.list();
  const hard = corrections.filter((c) => c.severity === "hard").length;
  const soft = corrections.length - hard;

  const rawCount = await learnStore.rawCount();
  const compiled = await learnStore.readCompiled();
  let staleDays: number | undefined;
  if (compiled) {
    const compiledDate = new Date(compiled.compiledAt);
    const now = new Date();
    staleDays = Math.floor((now.getTime() - compiledDate.getTime()) / (1000 * 60 * 60 * 24));
  }

  const suite = await ratchetStore.getSuite();
  const baseline = await ratchetStore.getBaseline();
  let lastResult: "pass" | "fail" | undefined;
  if (baseline && suite.assertions.length > 0) {
    lastResult = baseline.passCount === suite.assertions.length ? "pass" : "fail";
  }

  return {
    corrections: { total: corrections.length, hard, soft },
    learnings: { rawCount, compiledAt: compiled?.compiledAt, staleDays },
    ratchet: { assertions: suite.assertions.length, lastResult },
    graph: {
      projects: graphStore ? (await graphStore.load()).nodes.length : 0,
      links: graphStore ? (await graphStore.load()).edges.length : 0,
    },
  };
}
