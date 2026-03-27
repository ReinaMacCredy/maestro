/**
 * E2E tests for cross-agent handoff lifecycle.
 * handoff-plan --> handoff-pickup --> task-claim --> task-done --> handoff-report
 */

import { describe, test, expect, afterEach } from 'bun:test';
import { createTestHarness, getErrorText, type TestHarness } from '../mocks/test-harness.ts';

let harness: TestHarness;

afterEach(async () => {
  if (harness) await harness.cleanup();
});

const VALID_PLAN = [
  '## Discovery',
  'We investigated the codebase thoroughly and found that the authentication module needs a complete rewrite.',
  'The current implementation uses session cookies but we need JWT tokens for the API.',
  '',
  '### 1. Setup auth module',
  'Create the JWT authentication infrastructure.',
  '',
  '### 2. Add API endpoints',
  'Implement the login and refresh token endpoints.',
].join('\n');

describe('cross-agent handoff lifecycle', () => {
  test('handoff-plan requires approved plan with tasks', async () => {
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('feature-create', 'cross-test');

    // No plan yet
    const r1 = await harness.run('handoff-plan', '--to', 'codex');
    expect(r1.exitCode).not.toBe(0);
    expect(getErrorText(r1)).toContain('approved');
  });

  test('handoff-plan requires synced tasks', async () => {
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('feature-create', 'cross-test');
    await harness.run('plan-write', '--content', VALID_PLAN);
    await harness.run('plan-approve');

    // Plan approved but no tasks synced
    const r = await harness.run('handoff-plan', '--to', 'codex');
    expect(r.exitCode).not.toBe(0);
    expect(getErrorText(r)).toContain('task');
  });

  test('full lifecycle: plan --> pickup --> claim --> done --> report', async () => {
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('feature-create', 'cross-test');
    await harness.run('plan-write', '--content', VALID_PLAN);
    await harness.run('plan-approve');
    await harness.run('task-sync');

    // 1. Export handoff
    const planResult = await harness.run('handoff-plan', '--to', 'codex');
    expect(planResult.exitCode).toBe(0);
    const planParsed = JSON.parse(planResult.stdout);
    expect(planParsed.feature).toBe('cross-test');
    expect(planParsed.taskCount).toBe(2);
    expect(planParsed.to).toBe('codex');
    expect(planParsed.handoffPath).toContain('crossagent');

    // Feature status should be handed-off
    const statusAfterPlan = await harness.run('feature-info', '--feature', 'cross-test');
    const featureInfo = JSON.parse(statusAfterPlan.stdout);
    expect(featureInfo.status).toBe('handed-off');

    // 2. Pickup handoff
    const pickupResult = await harness.run('handoff-pickup', '--feature', 'cross-test');
    expect(pickupResult.exitCode).toBe(0);
    const pickupParsed = JSON.parse(pickupResult.stdout);
    expect(pickupParsed.feature).toBe('cross-test');
    expect(pickupParsed.tasks.length).toBe(2);
    expect(pickupParsed.quickstart).toContain('maestro task-claim');
    expect(pickupParsed.state.status).toBe('picked-up');

    // 3. Pickup is idempotent
    const pickup2 = await harness.run('handoff-pickup', '--feature', 'cross-test');
    expect(pickup2.exitCode).toBe(0);
    const pickup2Parsed = JSON.parse(pickup2.stdout);
    expect(pickup2Parsed.state.status).toBe('picked-up');

    // 4. Claim and complete a task
    const taskList = await harness.run('task-list', '--feature', 'cross-test');
    const tasks = JSON.parse(taskList.stdout);
    const firstTask = tasks[0].id;

    await harness.run('task-claim', '--task', firstTask, '--agent-id', 'codex');
    await harness.run('task-done', '--task', firstTask, '--content', 'Setup complete');

    // 5. Report completion
    const reportResult = await harness.run('handoff-report', '--feature', 'cross-test', '--content', 'Auth module implemented');
    expect(reportResult.exitCode).toBe(0);
    const reportParsed = JSON.parse(reportResult.stdout);
    expect(reportParsed.feature).toBe('cross-test');
    expect(reportParsed.tasksCompleted).toBe(1);
    expect(reportParsed.tasksPending).toBe(1);
    expect(reportParsed.state.status).toBe('completed');
    expect(reportParsed.reportPath).toContain('report.md');

    // Feature status should be review-pending
    const statusAfterReport = await harness.run('feature-info', '--feature', 'cross-test');
    const featureInfo2 = JSON.parse(statusAfterReport.stdout);
    expect(featureInfo2.status).toBe('review-pending');
  });

  test('handoff-pickup auto-discovers pending handoff', async () => {
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('feature-create', 'auto-test');
    await harness.run('plan-write', '--content', VALID_PLAN);
    await harness.run('plan-approve');
    await harness.run('task-sync');
    await harness.run('handoff-plan', '--to', 'codex');

    // Pickup without --feature should find it
    const result = await harness.run('handoff-pickup');
    expect(result.exitCode).toBe(0);
    const parsed = JSON.parse(result.stdout);
    expect(parsed.feature).toBe('auto-test');
  });

  test('handoff-report requires content', async () => {
    harness = await createTestHarness();
    await harness.run('init');
    await harness.run('feature-create', 'content-test');

    const result = await harness.run('handoff-report', '--feature', 'content-test');
    expect(result.exitCode).not.toBe(0);
    expect(getErrorText(result)).toContain('No summary provided');
  });
});
