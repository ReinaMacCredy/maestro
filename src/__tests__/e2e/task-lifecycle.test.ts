/**
 * E2E tests for the full task lifecycle.
 * sync --> list --> claim --> done/block/unblock --> feature-complete
 *
 * Covers: task-sync, task-list, task-claim, task-done, task-block, task-unblock,
 *         task-info, dependency ordering, state machine enforcement, feature-complete.
 */

import { describe, test, expect, afterEach } from 'bun:test';
import { createTestHarness, type TestHarness } from '../mocks/test-harness.ts';

let harness: TestHarness;

afterEach(async () => {
  if (harness) await harness.cleanup();
});

/** Plan with 3 sequential tasks (implicit deps: 2->1, 3->2). Task 1 explicitly has no deps. */
const SEQUENTIAL_PLAN = [
  '## Discovery',
  'We investigated the codebase thoroughly and found that the current implementation needs significant refactoring to support the new feature requirements.',
  '',
  '### 1. Setup database',
  '- **Depends on**: none',
  'Create the database schema and migrations',
  '',
  '### 2. Build API',
  'Implement REST endpoints for the new resource',
  '',
  '### 3. Add tests',
  'Write integration tests for the API layer',
].join('\n');

/** Plan with parallel tasks: tasks 1 and 2 have no deps, task 3 depends on both */
const PARALLEL_PLAN = [
  '## Discovery',
  'We investigated the codebase thoroughly and found that the current implementation needs significant refactoring to support the new feature requirements.',
  '',
  '### 1. Frontend scaffold',
  '- **Depends on**: none',
  'Create React components for the feature',
  '',
  '### 2. Backend scaffold',
  '- **Depends on**: none',
  'Create API routes and handlers',
  '',
  '### 3. Integration',
  '- **Depends on**: 1, 2',
  'Connect frontend to backend via API client',
].join('\n');

/** Helper: init + create feature + write plan + approve + sync. Returns { created, removed, kept } */
async function setupWithTasks(h: TestHarness, plan: string, featureName = 'test-feature') {
  // Force fs backend so folder names are deterministic (01-xxx, 02-xxx, ...)
  await h.run('init');
  await h.run('config-set', '--key', 'tasks.backend', '--value', 'fs');
  await h.run('feature-create', featureName);
  await h.run('plan-write', '--feature', featureName, '--content', plan);
  await h.run('plan-approve', '--feature', featureName);
  const syncResult = await h.run('task-sync', '--feature', featureName);
  return JSON.parse(syncResult.stdout);
}

describe('task sync', () => {
  test('generates tasks from approved plan', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);

    expect(sync.created).toHaveLength(3);
    // Folder names follow the pattern: NN-slugified-name
    const folders = sync.created as string[];
    expect(folders.some((f: string) => f.includes('setup-database'))).toBe(true);
    expect(folders.some((f: string) => f.includes('build-api'))).toBe(true);
    expect(folders.some((f: string) => f.includes('add-tests'))).toBe(true);
    expect(sync.removed).toHaveLength(0);
    expect(sync.kept).toHaveLength(0);
  });

  test('re-sync keeps done tasks, updates pending', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    // Claim and complete first task
    await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'agent-1');
    await harness.run('task-done', '--feature', 'test-feature', '--task', firstTask, '--summary', 'Done');

    // Re-sync should keep done task
    const resync = await harness.run('task-sync', '--feature', 'test-feature');
    expect(resync.exitCode).toBe(0);
    const parsed = JSON.parse(resync.stdout);
    expect(parsed.kept).toContain(firstTask);
  });

  test('rejects sync without approved plan', async () => {
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('config-set', '--key', 'tasks.backend', '--value', 'fs');
    await harness.run('feature-create', 'test-feature');
    const plan = '## Discovery\nWe investigated the codebase thoroughly and found that the current implementation needs significant refactoring to support the new feature requirements.\n\n### 1. Setup\nSetup things';
    await harness.run('plan-write', '--feature', 'test-feature', '--content', plan);

    // Plan not approved
    const result = await harness.run('task-sync', '--feature', 'test-feature');
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain('approv');
  });
});

describe('task list', () => {
  test('lists all tasks after sync', async () => {
    harness = await createTestHarness();
    await setupWithTasks(harness, SEQUENTIAL_PLAN);

    const result = await harness.run('task-list', '--feature', 'test-feature', '--all');
    expect(result.exitCode).toBe(0);
    const tasks = JSON.parse(result.stdout);
    expect(tasks).toHaveLength(3);
  });

  test('filters by status', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    // Claim first task
    await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'a1');

    const claimed = await harness.run('task-list', '--feature', 'test-feature', '--status', 'claimed');
    const claimedTasks = JSON.parse(claimed.stdout);
    expect(claimedTasks).toHaveLength(1);
    expect(claimedTasks[0].folder).toBe(firstTask);

    const pending = await harness.run('task-list', '--feature', 'test-feature', '--status', 'pending');
    const pendingTasks = JSON.parse(pending.stdout);
    expect(pendingTasks).toHaveLength(2);
  });

  test('rejects invalid status filter', async () => {
    harness = await createTestHarness();
    await setupWithTasks(harness, SEQUENTIAL_PLAN);

    const result = await harness.run('task-list', '--feature', 'test-feature', '--status', 'invalid');
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain('Invalid status');
  });
});

describe('task claim', () => {
  test('claims a pending task', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    const result = await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'agent-1');
    expect(result.exitCode).toBe(0);
    const task = JSON.parse(result.stdout);
    expect(task.status).toBe('claimed');
    expect(task.claimedBy).toBe('agent-1');
  });

  test('rejects claiming an already-claimed task', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'agent-1');

    const result = await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'agent-2');
    expect(result.exitCode).toBe(1);
  });

  test('rejects claiming a done task', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'a1');
    await harness.run('task-done', '--feature', 'test-feature', '--task', firstTask, '--summary', 'Done');

    const result = await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'a2');
    expect(result.exitCode).toBe(1);
  });
});

describe('task done', () => {
  test('marks claimed task as done', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'a1');

    const result = await harness.run('task-done', '--feature', 'test-feature', '--task', firstTask, '--summary', 'Created schema and migrations');
    expect(result.exitCode).toBe(0);
    const task = JSON.parse(result.stdout);
    expect(task.status).toBe('done');
    expect(task.summary).toBe('Created schema and migrations');
  });

  test('rejects done on pending task (not claimed)', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    const result = await harness.run('task-done', '--feature', 'test-feature', '--task', firstTask, '--summary', 'Done');
    expect(result.exitCode).toBe(1);
  });
});

describe('task block and unblock', () => {
  test('blocks a claimed task and unblocks it', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    // Claim
    await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'a1');

    // Block
    const blockResult = await harness.run('task-block', '--feature', 'test-feature', '--task', firstTask, '--reason', 'Need DB credentials');
    expect(blockResult.exitCode).toBe(0);
    const blocked = JSON.parse(blockResult.stdout);
    expect(blocked.status).toBe('blocked');
    expect(blocked.blockerReason).toBe('Need DB credentials');

    // Unblock
    const unblockResult = await harness.run('task-unblock', '--feature', 'test-feature', '--task', firstTask, '--decision', 'Credentials added to vault');
    expect(unblockResult.exitCode).toBe(0);
    const unblocked = JSON.parse(unblockResult.stdout);
    expect(unblocked.status).toBe('pending');
    expect(unblocked.blockerDecision).toBe('Credentials added to vault');
  });

  test('blocks a pending task', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    const result = await harness.run('task-block', '--feature', 'test-feature', '--task', firstTask, '--reason', 'Waiting on design');
    expect(result.exitCode).toBe(0);
    const task = JSON.parse(result.stdout);
    expect(task.status).toBe('blocked');
  });

  test('unblock on non-blocked task returns idempotent success', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    // Pending is the target state of unblock -- idempotent return with already: true
    const result = await harness.run('task-unblock', '--feature', 'test-feature', '--task', firstTask, '--decision', 'Not blocked', '--json');
    expect(result.exitCode).toBe(0);
    const parsed = JSON.parse(result.stdout);
    expect(parsed.already).toBe(true);
  });
});

describe('dependency ordering', () => {
  test('sequential deps: task-info shows dependency chain', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const [task1, task2, task3] = sync.created;

    // Task 1 has Depends on: none --> no deps
    const info1 = await harness.run('task-info', '--feature', 'test-feature', '--task', task1);
    expect(info1.exitCode).toBe(0);
    const t1 = JSON.parse(info1.stdout);
    expect(t1.dependsOn ?? []).toHaveLength(0);

    // Task 2 depends on task 1 -- dependsOn stores task ids (not folder names)
    const info2 = await harness.run('task-info', '--feature', 'test-feature', '--task', task2);
    expect(info2.exitCode).toBe(0);
    const t2 = JSON.parse(info2.stdout);
    expect(t2.dependsOn).toHaveLength(1);
    expect(t2.dependsOn).toContain(t1.id);

    // Task 3 depends on task 2
    const info3 = await harness.run('task-info', '--feature', 'test-feature', '--task', task3);
    expect(info3.exitCode).toBe(0);
    const t3 = JSON.parse(info3.stdout);
    expect(t3.dependsOn).toHaveLength(1);
    expect(t3.dependsOn).toContain(t2.id);
  });

  test('parallel deps: task 3 depends on both 1 and 2', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, PARALLEL_PLAN);
    const [task1, task2, task3] = sync.created;

    // Fetch ids for task1 and task2 to check against dependsOn (which stores ids)
    const info1 = await harness.run('task-info', '--feature', 'test-feature', '--task', task1);
    const t1 = JSON.parse(info1.stdout);
    const info2 = await harness.run('task-info', '--feature', 'test-feature', '--task', task2);
    const t2 = JSON.parse(info2.stdout);

    const info3 = await harness.run('task-info', '--feature', 'test-feature', '--task', task3);
    expect(info3.exitCode).toBe(0);
    const t3 = JSON.parse(info3.stdout);
    expect(t3.dependsOn).toContain(t1.id);
    expect(t3.dependsOn).toContain(t2.id);
  });

  test('completing a task allows dependent to be claimed', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const [task1, task2] = sync.created;

    // Complete task 1
    await harness.run('task-claim', '--feature', 'test-feature', '--task', task1, '--agent-id', 'a1');
    await harness.run('task-done', '--feature', 'test-feature', '--task', task1, '--summary', 'Done');

    // Task 2 should be claimable now
    const claim2 = await harness.run('task-claim', '--feature', 'test-feature', '--task', task2, '--agent-id', 'a1');
    expect(claim2.exitCode).toBe(0);
    const t2 = JSON.parse(claim2.stdout);
    expect(t2.status).toBe('claimed');
  });
});

describe('full task lifecycle', () => {
  test('sync -> claim -> done all tasks -> feature-complete', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, PARALLEL_PLAN);
    const [task1, task2, task3] = sync.created;

    // Complete task 1
    await harness.run('task-claim', '--feature', 'test-feature', '--task', task1, '--agent-id', 'a1');
    await harness.run('task-done', '--feature', 'test-feature', '--task', task1, '--summary', 'Components built');

    // Complete task 2
    await harness.run('task-claim', '--feature', 'test-feature', '--task', task2, '--agent-id', 'a2');
    await harness.run('task-done', '--feature', 'test-feature', '--task', task2, '--summary', 'Routes built');

    // Complete task 3
    await harness.run('task-claim', '--feature', 'test-feature', '--task', task3, '--agent-id', 'a1');
    await harness.run('task-done', '--feature', 'test-feature', '--task', task3, '--summary', 'Integrated');

    // Feature complete
    const completeResult = await harness.run('feature-complete', '--feature', 'test-feature');
    expect(completeResult.exitCode).toBe(0);
    const completed = JSON.parse(completeResult.stdout);
    expect(completed.feature.status).toBe('completed');
    expect(completed.tasksSummary.total).toBe(3);
    expect(completed.tasksSummary.done).toBe(3);
  });

  test('feature-complete rejects when tasks are not all done', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    // Only complete first task
    await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'a1');
    await harness.run('task-done', '--feature', 'test-feature', '--task', firstTask, '--summary', 'Done');

    const result = await harness.run('feature-complete', '--feature', 'test-feature');
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain('not done');
  });

  test('feature-complete rejects when no tasks exist', async () => {
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('feature-create', 'test-feature');

    const result = await harness.run('feature-complete', '--feature', 'test-feature');
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain('no tasks');
  });

  test('block-unblock-reclaim flow', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    // Claim task
    await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'a1');

    // Block it
    await harness.run('task-block', '--feature', 'test-feature', '--task', firstTask, '--reason', 'Need credentials');

    // Unblock it
    await harness.run('task-unblock', '--feature', 'test-feature', '--task', firstTask, '--decision', 'Got them');

    // Re-claim (should be pending again)
    const reclaim = await harness.run('task-claim', '--feature', 'test-feature', '--task', firstTask, '--agent-id', 'a2');
    expect(reclaim.exitCode).toBe(0);
    const task = JSON.parse(reclaim.stdout);
    expect(task.status).toBe('claimed');
    expect(task.claimedBy).toBe('a2');
  });
});

describe('task info', () => {
  test('shows task details', async () => {
    harness = await createTestHarness();
    const sync = await setupWithTasks(harness, SEQUENTIAL_PLAN);
    const firstTask = sync.created[0];

    const result = await harness.run('task-info', '--feature', 'test-feature', '--task', firstTask);
    expect(result.exitCode).toBe(0);
    const info = JSON.parse(result.stdout);
    expect(info.folder).toBe(firstTask);
    expect(info.name).toBeDefined();
    expect(info.status).toBe('pending');
    expect(info.origin).toBe('plan');
  });

  test('returns error for nonexistent task', async () => {
    harness = await createTestHarness();
    await setupWithTasks(harness, SEQUENTIAL_PLAN);

    const result = await harness.run('task-info', '--feature', 'test-feature', '--task', 'nonexistent');
    expect(result.exitCode).toBe(1);
  });
});
