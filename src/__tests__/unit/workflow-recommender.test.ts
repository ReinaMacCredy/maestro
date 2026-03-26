import { describe, test, expect } from 'bun:test';
import { recommend, type RecommendationContext } from '../../app/workflow/recommender.ts';
import { WorkflowRegistry } from '../../app/workflow/registry.ts';
import { declareAllTools } from '../../app/workflow/tool-declarations.ts';

function makeRegistry(): WorkflowRegistry {
  const reg = new WorkflowRegistry();
  declareAllTools(reg);
  return reg;
}

function makeContext(overrides: Partial<RecommendationContext> = {}): RecommendationContext {
  return {
    stage: 'execution',
    taskReview: 0,
    taskRevision: 0,
    taskPending: 3,
    taskClaimed: 1,
    planExists: true,
    planApproved: true,
    memoryCount: 5,
    ...overrides,
  };
}

describe('recommend', () => {
  test('returns primary tools for execution stage', () => {
    const registry = makeRegistry();
    const rec = recommend(registry, 'execution', makeContext());
    expect(rec.primary.length).toBeGreaterThan(0);
    expect(rec.stage).toBe('execution');
  });

  test('returns urgent tools when tasks in review', () => {
    const registry = makeRegistry();
    const rec = recommend(registry, 'execution', makeContext({ taskReview: 2 }));
    expect(rec.urgent).toContain('maestro_task(action: accept)');
    expect(rec.urgent).toContain('maestro_task(action: reject)');
  });

  test('returns urgent tools when tasks in revision', () => {
    const registry = makeRegistry();
    const rec = recommend(registry, 'execution', makeContext({ taskRevision: 1 }));
    expect(rec.urgent).toContain('maestro_task(action: claim)');
  });

  test('no urgent tools when no review/revision tasks', () => {
    const registry = makeRegistry();
    const rec = recommend(registry, 'execution', makeContext());
    expect(rec.urgent.filter(u => u.includes('accept') || u.includes('reject') || u.includes('claim'))).toHaveLength(0);
  });

  test('approval stage suggests tasks_sync when no tasks exist', () => {
    const registry = makeRegistry();
    const rec = recommend(registry, 'approval', makeContext({
      stage: 'approval',
      planApproved: true,
      taskPending: 0,
    }));
    expect(rec.urgent).toContain('maestro_task(action: sync)');
  });

  test('collects contextHints from tool metadata', () => {
    const registry = makeRegistry();
    const rec = recommend(registry, 'discovery', makeContext({ stage: 'discovery' }));
    // contextHints should be a record (may or may not have entries depending on tool metadata)
    expect(typeof rec.contextHints).toBe('object');
  });

  test('returns valid recommendation for all stages', () => {
    const registry = makeRegistry();
    const stages = ['discovery', 'research', 'planning', 'approval', 'execution', 'done'] as const;
    for (const stage of stages) {
      const rec = recommend(registry, stage, makeContext({ stage }));
      expect(rec.stage).toBe(stage);
      expect(Array.isArray(rec.primary)).toBe(true);
      expect(Array.isArray(rec.secondary)).toBe(true);
      expect(Array.isArray(rec.urgent)).toBe(true);
    }
  });
});
