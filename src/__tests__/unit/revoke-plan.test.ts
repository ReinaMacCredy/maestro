import { describe, test, expect } from 'bun:test';
import { revokePlan, type RevokePlanDeps } from '../../app/plans/revoke-plan.ts';

function makeDeps(overrides: {
  isApproved?: boolean;
  tasks?: Array<{ id: string; status: string }>;
} = {}): RevokePlanDeps {
  const { isApproved = true, tasks = [] } = overrides;
  let revokedCalled = false;
  let statusUpdated: string | null = null;

  return {
    planAdapter: {
      isApproved: () => isApproved,
      revokeApproval: () => { revokedCalled = true; },
      // Unused methods
      write: () => {},
      read: () => null,
      approve: () => {},
      getComments: () => [],
      addComment: () => ({ body: '', line: 0, author: '', createdAt: '' }),
      clearComments: () => {},
    },
    featureAdapter: {
      updateStatus: (_name: string, status: string) => { statusUpdated = status; },
      // Unused methods
      create: () => ({ name: '', status: 'discovery', createdAt: '' }),
      get: () => null,
      list: () => [],
      requireActive: () => ({ name: '', status: 'discovery', createdAt: '' }),
      getActive: () => null,
      getInfo: () => ({ name: '', status: 'discovery', createdAt: '', plan: false, tasks: 0, memories: 0 }),
      complete: () => {},
      setSession: () => {},
      getSession: () => null,
    },
    taskPort: {
      list: async () => tasks as any[],
      get: async () => null,
      create: async () => ({} as any),
      remove: async () => {},
      claim: async () => ({} as any),
      done: async () => ({} as any),
      block: async () => ({} as any),
      unblock: async () => ({} as any),
      getRunnable: async () => [],
      review: async () => ({} as any),
      revision: async () => ({} as any),
      readSpec: async () => null,
      writeSpec: async () => {},
      readReport: async () => null,
      writeReport: async () => {},
      readVerification: async () => null,
      writeVerification: async () => {},
    },
    // Expose test state
    get _revokedCalled() { return revokedCalled; },
    get _statusUpdated() { return statusUpdated; },
  } as RevokePlanDeps & { _revokedCalled: boolean; _statusUpdated: string | null };
}

describe('revokePlan', () => {
  test('revokes when no active tasks', async () => {
    const deps = makeDeps({ isApproved: true, tasks: [] });
    const result = await revokePlan(deps, 'test-feature');
    expect(result).toEqual({ feature: 'test-feature', revoked: true });
    expect((deps as any)._revokedCalled).toBe(true);
    expect((deps as any)._statusUpdated).toBe('planning');
  });

  test('throws when plan not approved', async () => {
    const deps = makeDeps({ isApproved: false });
    await expect(revokePlan(deps, 'test-feature')).rejects.toThrow('not approved');
  });

  test('throws when claimed tasks exist', async () => {
    const deps = makeDeps({
      isApproved: true,
      tasks: [{ id: '01-task', status: 'claimed' }],
    });
    await expect(revokePlan(deps, 'test-feature')).rejects.toThrow('actively being worked');
  });

  test('throws when review tasks exist', async () => {
    const deps = makeDeps({
      isApproved: true,
      tasks: [{ id: '01-task', status: 'review' }],
    });
    await expect(revokePlan(deps, 'test-feature')).rejects.toThrow('actively being worked');
  });

  test('allows revoke when only done/pending tasks exist', async () => {
    const deps = makeDeps({
      isApproved: true,
      tasks: [
        { id: '01-task', status: 'done' },
        { id: '02-task', status: 'pending' },
      ],
    });
    const result = await revokePlan(deps, 'test-feature');
    expect(result.revoked).toBe(true);
  });
});
