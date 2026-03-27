import { describe, test, expect, beforeEach, afterEach } from 'bun:test';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import {
  recordTelemetry,
  loadTelemetry,
  buildEffectivenessMap,
  type TelemetryRecord,
} from '../../app/dcp/telemetry.ts';

let tmpDir: string;

function makeRecord(overrides: Partial<TelemetryRecord> = {}): TelemetryRecord {
  return {
    taskId: '01-task',
    featureName: 'test-feature',
    timestamp: new Date().toISOString(),
    injectedMemories: ['mem-a', 'mem-b'],
    outcome: 'success',
    revisionCount: 0,
    verificationPassed: true,
    ...overrides,
  };
}

beforeEach(() => {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'dcp-telemetry-'));
  fs.mkdirSync(path.join(tmpDir, '.maestro'), { recursive: true });
});

afterEach(() => {
  fs.rmSync(tmpDir, { recursive: true, force: true });
});

describe('recordTelemetry', () => {
  test('appends valid JSONL', () => {
    recordTelemetry(tmpDir, makeRecord({ taskId: '01-task' }));
    recordTelemetry(tmpDir, makeRecord({ taskId: '02-task' }));

    const content = fs.readFileSync(path.join(tmpDir, '.maestro', 'telemetry.jsonl'), 'utf-8');
    const lines = content.trim().split('\n');
    expect(lines.length).toBe(2);
    expect(JSON.parse(lines[0]).taskId).toBe('01-task');
    expect(JSON.parse(lines[1]).taskId).toBe('02-task');
  });

  test('best-effort: invalid path does not throw', () => {
    expect(() => {
      recordTelemetry('/nonexistent/path', makeRecord());
    }).not.toThrow();
  });
});

describe('loadTelemetry', () => {
  test('reads back recorded entries', () => {
    recordTelemetry(tmpDir, makeRecord({ taskId: '01-task' }));
    recordTelemetry(tmpDir, makeRecord({ taskId: '02-task' }));

    const records = loadTelemetry(tmpDir);
    expect(records.length).toBe(2);
    expect(records[0].taskId).toBe('01-task');
    expect(records[1].taskId).toBe('02-task');
  });

  test('returns [] on missing file', () => {
    const records = loadTelemetry(tmpDir);
    expect(records).toEqual([]);
  });

  test('caps at maxRecords (returns tail)', () => {
    for (let i = 0; i < 10; i++) {
      recordTelemetry(tmpDir, makeRecord({ taskId: `task-${i}` }));
    }

    const records = loadTelemetry(tmpDir, 3);
    expect(records.length).toBe(3);
    expect(records[0].taskId).toBe('task-7');
    expect(records[2].taskId).toBe('task-9');
  });

  test('skips malformed lines gracefully', () => {
    const filePath = path.join(tmpDir, '.maestro', 'telemetry.jsonl');
    fs.writeFileSync(filePath, JSON.stringify(makeRecord({ taskId: 'good' })) + '\n');
    fs.appendFileSync(filePath, 'NOT VALID JSON\n');
    fs.appendFileSync(filePath, JSON.stringify(makeRecord({ taskId: 'also-good' })) + '\n');

    const records = loadTelemetry(tmpDir);
    expect(records.length).toBe(2);
    expect(records[0].taskId).toBe('good');
    expect(records[1].taskId).toBe('also-good');
  });
});

describe('buildEffectivenessMap', () => {
  test('returns empty map for empty records', () => {
    const result = buildEffectivenessMap([]);
    expect(result.size).toBe(0);
  });

  test('memories below minSamples are omitted', () => {
    const records = [
      makeRecord({ injectedMemories: ['mem-a'], outcome: 'success' }),
      makeRecord({ injectedMemories: ['mem-a'], outcome: 'success' }),
    ];
    // minSamples=3, only 2 records for mem-a
    const result = buildEffectivenessMap(records, 3);
    expect(result.has('mem-a')).toBe(false);
  });

  test('success-only memories score near 1.0', () => {
    const now = Date.now();
    const records = Array.from({ length: 5 }, (_, i) =>
      makeRecord({
        taskId: `task-${i}`,
        injectedMemories: ['mem-a'],
        outcome: 'success',
        timestamp: new Date(now - i * 1000).toISOString(), // recent
      }),
    );
    const result = buildEffectivenessMap(records, 3);
    expect(result.get('mem-a')!).toBeGreaterThan(0.95);
  });

  test('revision-heavy memories score low', () => {
    const now = Date.now();
    const records = Array.from({ length: 5 }, (_, i) =>
      makeRecord({
        taskId: `task-${i}`,
        injectedMemories: ['mem-a'],
        outcome: 'revision',
        revisionCount: 3,
        timestamp: new Date(now - i * 1000).toISOString(),
      }),
    );
    const result = buildEffectivenessMap(records, 3);
    expect(result.get('mem-a')!).toBeLessThan(0.2);
  });

  test('blocked outcome scores 0.0', () => {
    const now = Date.now();
    const records = Array.from({ length: 3 }, (_, i) =>
      makeRecord({
        taskId: `task-${i}`,
        injectedMemories: ['mem-a'],
        outcome: 'blocked',
        timestamp: new Date(now - i * 1000).toISOString(),
      }),
    );
    const result = buildEffectivenessMap(records, 3);
    expect(result.get('mem-a')!).toBeLessThan(0.01);
  });

  test('mixed outcomes produce intermediate score', () => {
    const now = Date.now();
    const records = [
      makeRecord({ taskId: 't1', injectedMemories: ['mem-a'], outcome: 'success', timestamp: new Date(now).toISOString() }),
      makeRecord({ taskId: 't2', injectedMemories: ['mem-a'], outcome: 'success', timestamp: new Date(now - 1000).toISOString() }),
      makeRecord({ taskId: 't3', injectedMemories: ['mem-a'], outcome: 'revision', revisionCount: 2, timestamp: new Date(now - 2000).toISOString() }),
    ];
    const result = buildEffectivenessMap(records, 3);
    const score = result.get('mem-a')!;
    // 2 successes (1.0) + 1 heavy revision (0.1) -> somewhere between
    expect(score).toBeGreaterThan(0.5);
    expect(score).toBeLessThan(1.0);
  });

  test('recency weighting: recent records matter more', () => {
    const now = Date.now();
    const recentGood = [
      // 3 recent successes, 3 old failures
      makeRecord({ taskId: 't1', injectedMemories: ['mem-a'], outcome: 'success', timestamp: new Date(now).toISOString() }),
      makeRecord({ taskId: 't2', injectedMemories: ['mem-a'], outcome: 'success', timestamp: new Date(now - 1000).toISOString() }),
      makeRecord({ taskId: 't3', injectedMemories: ['mem-a'], outcome: 'success', timestamp: new Date(now - 2000).toISOString() }),
      makeRecord({ taskId: 't4', injectedMemories: ['mem-a'], outcome: 'revision', revisionCount: 3, timestamp: new Date(now - 90 * 24 * 60 * 60 * 1000).toISOString() }),
      makeRecord({ taskId: 't5', injectedMemories: ['mem-a'], outcome: 'revision', revisionCount: 3, timestamp: new Date(now - 91 * 24 * 60 * 60 * 1000).toISOString() }),
      makeRecord({ taskId: 't6', injectedMemories: ['mem-a'], outcome: 'revision', revisionCount: 3, timestamp: new Date(now - 92 * 24 * 60 * 60 * 1000).toISOString() }),
    ];
    const result = buildEffectivenessMap(recentGood, 3);
    // Recent successes should dominate over old failures
    expect(result.get('mem-a')!).toBeGreaterThan(0.7);
  });

  test('handles multiple memories per record', () => {
    const now = Date.now();
    const records = [
      makeRecord({ taskId: 't1', injectedMemories: ['mem-a', 'mem-b'], outcome: 'success', timestamp: new Date(now).toISOString() }),
      makeRecord({ taskId: 't2', injectedMemories: ['mem-a', 'mem-b'], outcome: 'success', timestamp: new Date(now - 1000).toISOString() }),
      makeRecord({ taskId: 't3', injectedMemories: ['mem-a', 'mem-b'], outcome: 'success', timestamp: new Date(now - 2000).toISOString() }),
    ];
    const result = buildEffectivenessMap(records, 3);
    expect(result.has('mem-a')).toBe(true);
    expect(result.has('mem-b')).toBe(true);
    expect(result.get('mem-a')!).toBeGreaterThan(0.95);
    expect(result.get('mem-b')!).toBeGreaterThan(0.95);
  });
});
