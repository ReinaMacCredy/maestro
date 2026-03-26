import { describe, test, expect } from 'bun:test';
import { respond, textResponse, errorResponse } from '../../surfaces/mcp/respond.ts';

describe('respond', () => {
  test('wraps payload in MCP text content', () => {
    const result = respond({ foo: 'bar' });
    expect(result.content).toHaveLength(1);
    expect(result.content[0].type).toBe('text');
    const parsed = JSON.parse(result.content[0].text);
    expect(parsed.foo).toBe('bar');
  });

  test('handles circular references gracefully', () => {
    const obj: Record<string, unknown> = { a: 1 };
    obj.self = obj;
    const result = respond(obj);
    const parsed = JSON.parse(result.content[0].text);
    expect(parsed.success).toBe(false);
    expect(parsed.reason).toBe('serialization_error');
  });

  test('truncates oversized responses', () => {
    const large = { data: 'x'.repeat(200000) };
    const result = respond(large);
    const parsed = JSON.parse(result.content[0].text);
    expect(parsed.truncated).toBe(true);
    expect(parsed.original_keys).toContain('data');
  });
});

describe('textResponse', () => {
  test('wraps plain text', () => {
    const result = textResponse('hello world');
    expect(result.content[0].type).toBe('text');
    expect(result.content[0].text).toBe('hello world');
  });
});

describe('errorResponse', () => {
  test('produces error with success: false and isError: true', () => {
    const result = errorResponse({
      terminal: true,
      reason: 'not_found',
      error: 'Feature not found',
      hint: 'Create a feature first',
    });
    expect(result.isError).toBe(true);
    const parsed = JSON.parse(result.content[0].text);
    expect(parsed.success).toBe(false);
    expect(parsed.terminal).toBe(true);
    expect(parsed.reason).toBe('not_found');
    expect(parsed.error).toBe('Feature not found');
    expect(parsed.hint).toBe('Create a feature first');
  });

  test('includes suggestions when provided', () => {
    const result = errorResponse({
      terminal: false,
      reason: 'invalid',
      error: 'Bad input',
      suggestions: ['Try this', 'Or this'],
    });
    const parsed = JSON.parse(result.content[0].text);
    expect(parsed.suggestions).toEqual(['Try this', 'Or this']);
  });

  test('omits empty suggestions array', () => {
    const result = errorResponse({
      terminal: false,
      reason: 'err',
      error: 'msg',
      suggestions: [],
    });
    const parsed = JSON.parse(result.content[0].text);
    expect(parsed.suggestions).toBeUndefined();
  });
});
