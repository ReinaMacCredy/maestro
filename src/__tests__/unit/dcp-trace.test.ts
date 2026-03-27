import { describe, test, expect, beforeEach, afterEach } from 'bun:test';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { appendDcpTrace, readDcpTrace, collectMemoryNames } from '../../app/dcp/trace.ts';

let tmpDir: string;

beforeEach(() => {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'dcp-trace-'));
  // Create task directory structure
  const taskDir = path.join(tmpDir, '.maestro', 'features', 'test-feature', 'tasks', '01-test-task');
  fs.mkdirSync(taskDir, { recursive: true });
});

afterEach(() => {
  fs.rmSync(tmpDir, { recursive: true, force: true });
});

describe('DCP injection trace', () => {
  test('writes and reads trace file', () => {
    appendDcpTrace(tmpDir, 'test-feature', '01-test-task', 0, [
      { name: 'arch-decision', score: 0.85 },
      { name: 'api-convention', score: 0.72 },
    ]);

    const trace = readDcpTrace(tmpDir, 'test-feature', '01-test-task');
    expect(trace).not.toBeNull();
    expect(trace!.entries.length).toBe(1);
    expect(trace!.entries[0].memories).toEqual([
      { name: 'arch-decision', score: 0.85 },
      { name: 'api-convention', score: 0.72 },
    ]);
    expect(trace!.entries[0].revision).toBe(0);
    expect(trace!.entries[0].injectedAt).toBeTruthy();
  });

  test('idempotent: same revision is not written twice', () => {
    appendDcpTrace(tmpDir, 'test-feature', '01-test-task', 0, [
      { name: 'mem-a', score: 0.9 },
    ]);
    appendDcpTrace(tmpDir, 'test-feature', '01-test-task', 0, [
      { name: 'mem-b', score: 0.8 },
    ]);

    const trace = readDcpTrace(tmpDir, 'test-feature', '01-test-task');
    expect(trace!.entries.length).toBe(1);
    expect(trace!.entries[0].memories[0].name).toBe('mem-a');
  });

  test('accumulates entries across revisions', () => {
    appendDcpTrace(tmpDir, 'test-feature', '01-test-task', 0, [
      { name: 'mem-a', score: 0.9 },
    ]);
    appendDcpTrace(tmpDir, 'test-feature', '01-test-task', 1, [
      { name: 'mem-a', score: 0.85 },
      { name: 'mem-b', score: 0.6 },
    ]);

    const trace = readDcpTrace(tmpDir, 'test-feature', '01-test-task');
    expect(trace!.entries.length).toBe(2);
    expect(trace!.entries[0].revision).toBe(0);
    expect(trace!.entries[1].revision).toBe(1);
  });

  test('empty memories list is a no-op', () => {
    appendDcpTrace(tmpDir, 'test-feature', '01-test-task', 0, []);

    const trace = readDcpTrace(tmpDir, 'test-feature', '01-test-task');
    expect(trace).toBeNull();
  });

  test('readDcpTrace returns null when no trace exists', () => {
    const trace = readDcpTrace(tmpDir, 'test-feature', '01-test-task');
    expect(trace).toBeNull();
  });

  test('best-effort: invalid path does not throw', () => {
    // Should not throw even with a non-existent base path
    expect(() => {
      appendDcpTrace('/nonexistent/path', 'feature', 'task', 0, [
        { name: 'mem', score: 0.5 },
      ]);
    }).not.toThrow();
  });
});

describe('collectMemoryNames', () => {
  test('deduplicates names across revisions', () => {
    appendDcpTrace(tmpDir, 'test-feature', '01-test-task', 0, [
      { name: 'mem-a', score: 0.9 },
      { name: 'mem-b', score: 0.8 },
    ]);
    appendDcpTrace(tmpDir, 'test-feature', '01-test-task', 1, [
      { name: 'mem-a', score: 0.85 },
      { name: 'mem-c', score: 0.7 },
    ]);

    const trace = readDcpTrace(tmpDir, 'test-feature', '01-test-task')!;
    const names = collectMemoryNames(trace);
    expect(names.sort()).toEqual(['mem-a', 'mem-b', 'mem-c']);
  });

  test('returns empty array for trace with no entries', () => {
    const names = collectMemoryNames({ entries: [] });
    expect(names).toEqual([]);
  });
});
