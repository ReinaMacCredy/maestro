import * as fs from 'fs';
import { defineCommand } from 'citty';
import { debugVisualize } from '../../../app/visual/debug-visualize.ts';
import type { DebugVisualType, VisualResult } from '../../../app/visual/types.ts';
import { DEBUG_VISUAL_TYPES } from '../../../app/visual/types.ts';
import { output } from '../../../infra/utils/output.ts';
import { handleCommandError, MaestroError } from '../../../domain/errors.ts';

function formatResult(result: VisualResult): string {
  const lines: string[] = [];
  lines.push(`[ok] Generated ${result.type} debug visualization`);
  lines.push(`Path: ${result.path}`);
  lines.push(result.opened ? 'Opened in browser.' : 'Browser not opened (use without --no-open to auto-open).');
  return lines.join('\n');
}

function parseData(raw: string): unknown {
  if (!raw || !raw.trim()) {
    throw new MaestroError('--data is required', ['Pass a JSON string or file path.']);
  }

  if (raw.startsWith('{') || raw.startsWith('[')) {
    try {
      return JSON.parse(raw);
    } catch (e) {
      throw new MaestroError(
        `Invalid JSON in --data: ${(e as Error).message}`,
        ['Check JSON syntax. For large payloads, pass a file path instead.'],
      );
    }
  }

  // File path
  let content: string;
  try {
    content = fs.readFileSync(raw, 'utf-8');
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code === 'ENOENT') {
      throw new MaestroError(`Cannot read data file: ${raw}`, ['Check the file path exists.']);
    }
    if (code === 'EACCES') {
      throw new MaestroError(`Permission denied reading: ${raw}`);
    }
    if (code === 'EISDIR') {
      throw new MaestroError(`Path is a directory, not a file: ${raw}`);
    }
    throw e;
  }

  try {
    return JSON.parse(content);
  } catch (e) {
    throw new MaestroError(
      `Invalid JSON in file ${raw}: ${(e as Error).message}`,
      ['Check the file contains valid JSON.'],
    );
  }
}

export default defineCommand({
  meta: { name: 'debug-visual', description: 'Render debug data as interactive HTML\n\nExamples:\n  maestro debug-visual --type component-tree --data \'{"root": "App"}\'\n  maestro debug-visual --type flame-chart --data ./profile.json --no-open' },
  args: {
    type: {
      type: 'string',
      required: true,
      description: `Debug type: ${DEBUG_VISUAL_TYPES.join(', ')}`,
    },
    data: {
      type: 'string',
      required: true,
      description: 'JSON string or path to JSON file',
    },
    title: {
      type: 'string',
      description: 'Page title (defaults to type name)',
    },
    'no-open': {
      type: 'boolean',
      default: false,
      description: 'Do not open browser automatically',
    },
  },
  async run({ args }) {
    try {
      if (!DEBUG_VISUAL_TYPES.includes(args.type as DebugVisualType)) {
        throw new MaestroError(
          `Invalid type: ${args.type}`,
          [`Valid types: ${DEBUG_VISUAL_TYPES.join(', ')}`],
        );
      }

      const data = parseData(args.data);
      const result = await debugVisualize(
        args.type as DebugVisualType,
        data,
        args.title,
        !args['no-open'],
      );
      output(result, formatResult);
    } catch (err) {
      handleCommandError('debug-visual', err);
    }
  },
});
