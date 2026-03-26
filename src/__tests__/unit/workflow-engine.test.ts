import { describe, test, expect } from 'bun:test';
import { createWorkflowEngine, type StatusContext } from '../../app/workflow/engine.ts';
import { WorkflowRegistry } from '../../app/workflow/registry.ts';
import { declareAllTools } from '../../app/workflow/tool-declarations.ts';

function makeRegistry(): WorkflowRegistry {
  const reg = new WorkflowRegistry();
  declareAllTools(reg);
  return reg;
}

function makeStatus(overrides: Partial<StatusContext> = {}): StatusContext {
  return {
    plan: { exists: true, approved: true },
    tasks: { total: 5, done: 2, pending: 2, inProgress: 1 },
    context: { count: 3 },
    ...overrides,
  };
}

describe('WorkflowEngine', () => {
  test('getRecommendation returns valid shape', () => {
    const { engine } = createWorkflowEngine(makeRegistry());
    const rec = engine.getRecommendation('execution', makeStatus());

    expect(Array.isArray(rec.primary)).toBe(true);
    expect(Array.isArray(rec.secondary)).toBe(true);
    expect(Array.isArray(rec.urgent)).toBe(true);
    expect(rec.stage).toBe('execution');
  });

  test('getRecommendation surfaces urgent tasks in review', () => {
    const { engine } = createWorkflowEngine(makeRegistry());
    const rec = engine.getRecommendation('execution', makeStatus({
      tasks: { total: 5, done: 2, pending: 1, inProgress: 1, review: 1 },
    }));

    expect(rec.urgent).toContain('maestro_task(action: accept)');
  });

  test('emit dispatches to event bus', () => {
    const { engine, eventBus } = createWorkflowEngine(makeRegistry());
    let received = false;
    eventBus.on('task-done', () => { received = true; });

    engine.emit('task-done', 'my-feature', 'setup-auth');

    expect(received).toBe(true);
  });

  test('events getter returns the event bus', () => {
    const { engine, eventBus } = createWorkflowEngine(makeRegistry());
    expect(engine.events).toBe(eventBus);
  });

  test('works for all stages', () => {
    const { engine } = createWorkflowEngine(makeRegistry());
    const stages = ['discovery', 'research', 'planning', 'approval', 'execution', 'done'] as const;
    for (const stage of stages) {
      const rec = engine.getRecommendation(stage, makeStatus());
      expect(rec.stage).toBe(stage);
    }
  });
});
