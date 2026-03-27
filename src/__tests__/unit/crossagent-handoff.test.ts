import { describe, it, expect } from 'bun:test';
import { formatCrossAgentHandoff, buildQuickstart, formatCrossAgentReport } from '../../app/handoff/crossagent-format.ts';
import type { CrossAgentDocument, CrossAgentTask } from '../../app/handoff/crossagent.ts';

// ============================================================================
// formatCrossAgentHandoff
// ============================================================================

describe('formatCrossAgentHandoff', () => {
  function makeDoc(overrides?: Partial<CrossAgentDocument>): CrossAgentDocument {
    return {
      feature: 'test-feat',
      fromHost: 'claude-code',
      toAgent: 'codex',
      maestroVersion: '0.2.0',
      createdAt: '2026-03-27T10:00:00Z',
      plan: '## Discovery\nResearch findings here.\n\n### 1. Setup\nDo setup.',
      tasks: [
        { id: '01-setup', name: 'setup', status: 'pending' },
        { id: '02-api', name: 'api', status: 'pending', deps: ['01-setup'] },
      ],
      memories: [],
      doctrine: [],
      modifiedFiles: [],
      ...overrides,
    };
  }

  it('produces valid markdown with all sections', () => {
    const doc = makeDoc();
    const quickstart = buildQuickstart('test-feat', doc.tasks);
    const md = formatCrossAgentHandoff(doc, quickstart);

    expect(md).toContain('# Cross-Agent Handoff: test-feat');
    expect(md).toContain('| From | claude-code |');
    expect(md).toContain('| To | codex |');
    expect(md).toContain('## Plan');
    expect(md).toContain('## Tasks');
    expect(md).toContain('| 1 | 01-setup | setup | pending | - |');
    expect(md).toContain('| 2 | 02-api | api | pending | 01-setup |');
    expect(md).toContain('## Quickstart');
  });

  it('includes memories when present', () => {
    const doc = makeDoc({
      memories: [{ name: 'auth-decision', category: 'decision', content: 'Use JWT tokens.' }],
    });
    const md = formatCrossAgentHandoff(doc, '');
    expect(md).toContain('## Key Decisions');
    expect(md).toContain('### auth-decision (decision)');
    expect(md).toContain('Use JWT tokens.');
  });

  it('includes doctrine when present', () => {
    const doc = makeDoc({
      doctrine: [{ name: 'no-mocks', rule: 'Always use real DB in integration tests' }],
    });
    const md = formatCrossAgentHandoff(doc, '');
    expect(md).toContain('## Doctrine');
    expect(md).toContain('**no-mocks**');
  });

  it('includes modified files', () => {
    const doc = makeDoc({ modifiedFiles: ['src/auth.ts', 'src/db.ts'] });
    const md = formatCrossAgentHandoff(doc, '');
    expect(md).toContain('## Modified Files');
    expect(md).toContain('`src/auth.ts`');
  });

  it('includes additional context', () => {
    const doc = makeDoc({ additionalContext: 'Focus on performance.' });
    const md = formatCrossAgentHandoff(doc, '');
    expect(md).toContain('## Additional Context');
    expect(md).toContain('Focus on performance.');
  });

  it('omits empty optional sections', () => {
    const doc = makeDoc();
    const md = formatCrossAgentHandoff(doc, '');
    expect(md).not.toContain('## Key Decisions');
    expect(md).not.toContain('## Doctrine');
    expect(md).not.toContain('## Modified Files');
    expect(md).not.toContain('## Additional Context');
  });

  it('defaults "to" to "any" when no target agent', () => {
    const doc = makeDoc({ toAgent: undefined });
    const md = formatCrossAgentHandoff(doc, '');
    expect(md).toContain('| To | any |');
  });
});

// ============================================================================
// buildQuickstart
// ============================================================================

describe('buildQuickstart', () => {
  it('produces 5 numbered sections', () => {
    const tasks: CrossAgentTask[] = [
      { id: '01-setup', name: 'setup', status: 'pending' },
    ];
    const qs = buildQuickstart('my-feat', tasks);

    expect(qs).toContain('### 1. Find the next runnable task');
    expect(qs).toContain('### 2. Claim and implement');
    expect(qs).toContain('### 3. Mark done');
    expect(qs).toContain('### 4. Repeat until all tasks done');
    expect(qs).toContain('### 5. Report completion');
  });

  it('uses first runnable task ID as example', () => {
    const tasks: CrossAgentTask[] = [
      { id: '01-done', name: 'done', status: 'done' },
      { id: '02-next', name: 'next', status: 'pending' },
    ];
    const qs = buildQuickstart('my-feat', tasks);
    expect(qs).toContain('maestro task-claim --feature my-feat --task 02-next');
  });

  it('picks first task with no unmet deps', () => {
    const tasks: CrossAgentTask[] = [
      { id: 'aaa-blocked', name: 'blocked', status: 'pending', deps: ['zzz-first'] },
      { id: 'zzz-first', name: 'first', status: 'pending' },
    ];
    const qs = buildQuickstart('my-feat', tasks);
    expect(qs).toContain('--task zzz-first');
    expect(qs).not.toContain('--task aaa-blocked');
  });

  it('uses placeholder when no pending tasks', () => {
    const tasks: CrossAgentTask[] = [
      { id: '01-done', name: 'done', status: 'done' },
    ];
    const qs = buildQuickstart('my-feat', tasks);
    expect(qs).toContain('maestro task-claim --feature my-feat --task <task-id>');
  });

  it('includes --json on all commands', () => {
    const qs = buildQuickstart('my-feat', []);
    const lines = qs.split('\n').filter((l) => l.includes('maestro '));
    for (const line of lines) {
      expect(line).toContain('--json');
    }
  });

  it('includes feature name in task-next and handoff-report', () => {
    const qs = buildQuickstart('my-feat', []);
    expect(qs).toContain('--feature my-feat');
  });

  it('recommends task-next for dependency ordering', () => {
    const qs = buildQuickstart('my-feat', []);
    expect(qs).toContain('task-next');
    expect(qs).toContain('dependency order');
  });
});

// ============================================================================
// formatCrossAgentReport
// ============================================================================

describe('formatCrossAgentReport', () => {
  it('produces valid report markdown', () => {
    const md = formatCrossAgentReport('test-feat', 'All done!', 3, 1, 'codex');
    expect(md).toContain('# Handoff Report: test-feat');
    expect(md).toContain('| Reporter | codex |');
    expect(md).toContain('| Tasks Completed | 3 |');
    expect(md).toContain('| Tasks Pending | 1 |');
    expect(md).toContain('All done!');
  });
});
