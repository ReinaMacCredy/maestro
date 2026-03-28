import { describe, test, expect, beforeEach, afterEach } from 'bun:test';
import { detectHost, isHosted, _resetHostDetection } from '../../infra/utils/host-detect.ts';

function clearHostEnv() {
  _resetHostDetection();
  delete process.env.CLAUDE_PROJECT_DIR;
  delete process.env.CLAUDE_SESSION_ID;
  delete process.env.CLAUDECODE;
  delete process.env.CODEX_CI;
  delete process.env.CODEX_THREAD_ID;
}

beforeEach(clearHostEnv);
afterEach(clearHostEnv);

describe('detectHost', () => {
  test('returns standalone when no host env vars', () => {
    expect(detectHost()).toBe('standalone');
  });

  test('detects claude-code from CLAUDE_PROJECT_DIR', () => {
    process.env.CLAUDE_PROJECT_DIR = '/tmp/project';
    expect(detectHost()).toBe('claude-code');
  });

  test('detects claude-code from CLAUDE_SESSION_ID', () => {
    process.env.CLAUDE_SESSION_ID = 'abc123';
    expect(detectHost()).toBe('claude-code');
  });

  test('detects claude-code from CLAUDECODE', () => {
    process.env.CLAUDECODE = '1';
    expect(detectHost()).toBe('claude-code');
  });

  test('detects codex from CODEX_CI', () => {
    process.env.CODEX_CI = 'true';
    expect(detectHost()).toBe('codex');
  });

  test('detects codex from CODEX_THREAD_ID', () => {
    process.env.CODEX_THREAD_ID = 'thread-1';
    expect(detectHost()).toBe('codex');
  });

  test('claude-code takes priority over codex', () => {
    process.env.CLAUDE_PROJECT_DIR = '/tmp';
    process.env.CODEX_CI = 'true';
    expect(detectHost()).toBe('claude-code');
  });

  test('caches result', () => {
    expect(detectHost()).toBe('standalone');
    process.env.CLAUDE_PROJECT_DIR = '/tmp';
    // Still standalone because cached
    expect(detectHost()).toBe('standalone');
  });

  test('_resetHostDetection clears cache', () => {
    expect(detectHost()).toBe('standalone');
    _resetHostDetection();
    process.env.CLAUDE_PROJECT_DIR = '/tmp';
    expect(detectHost()).toBe('claude-code');
  });
});

describe('isHosted', () => {
  test('returns false for standalone', () => {
    expect(isHosted()).toBe(false);
  });

  test('returns true for claude-code', () => {
    process.env.CLAUDE_SESSION_ID = 'abc';
    expect(isHosted()).toBe(true);
  });

  test('returns true for codex', () => {
    process.env.CODEX_THREAD_ID = 'thread';
    expect(isHosted()).toBe(true);
  });
});
