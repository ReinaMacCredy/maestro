#!/usr/bin/env bun

import { $, type BuildOutput } from 'bun';
import { openSync, readSync, closeSync, readFileSync, writeFileSync, chmodSync, rmSync } from 'node:fs';

function checkBuild(result: BuildOutput, label: string) {
  if (!result.success) {
    console.error(`[build] ${label} failed:`);
    for (const log of result.logs) console.error(log);
    process.exit(1);
  }
}

async function build() {
  // Clean leftover artifacts from previous builds
  for (const p of ['dist/.claude-plugin', 'dist/skills', 'dist/start.mjs', 'dist/hooks']) {
    try { rmSync(p, { recursive: true }); } catch {}
  }

  // Step 0a: Sync version from package.json to src/version.ts
  const pkg = JSON.parse(readFileSync('./package.json', 'utf-8'));
  writeFileSync('./src/version.ts', `export const VERSION = '${pkg.version}';\n`);

  // Step 0b: Run generators (MUST run before server bundle -- loadSkill depends on registry.generated.ts)
  console.log('[build] Generating skills registry...');
  await $`bun src/skills/generate.ts`;

  console.log('[build] Generating command registry...');
  await $`bun src/surfaces/cli/_generate.ts`;

  // Step 1: Server bundle (Node target, ESM)
  console.log('[build] Bundling MCP server...');
  checkBuild(await Bun.build({
    entrypoints: ['./src/surfaces/mcp/index.ts'],
    outdir: './dist',
    target: 'node',
    format: 'esm',
    minify: true,
    external: ['simple-git'],
    naming: { entry: 'server.bundle.mjs' },
  }), 'Server bundle');

  // Step 2: Hook scripts (Node target, ESM) -- built in parallel
  console.log('[build] Bundling hooks...');
  const hooks = ['sessionstart', 'pretooluse', 'posttooluse', 'precompact', 'pre-agent'];
  const hookResults = await Promise.all(hooks.map(hook => Bun.build({
    entrypoints: [`./src/surfaces/hooks/${hook}.ts`],
    outdir: './hooks',
    target: 'node',
    format: 'esm',
    minify: true,
    external: ['simple-git'],
    naming: { entry: `${hook}.mjs` },
  })));
  hooks.forEach((hook, i) => checkBuild(hookResults[i], `Hook: ${hook}`));

  // Step 3: CLI bundle for npm bin entry (Node target)
  console.log('[build] Bundling CLI for npm...');
  checkBuild(await Bun.build({
    entrypoints: ['./src/surfaces/cli/index.ts'],
    outdir: './dist',
    target: 'node',
    format: 'esm',
    external: ['simple-git'],
    naming: { entry: 'cli.js' },
  }), 'CLI bundle');

  // Step 4: Ensure CLI shebang + executable
  const cliPath = './dist/cli.js';
  const probe = Buffer.alloc(2);
  const probeFd = openSync(cliPath, 'r');
  readSync(probeFd, probe, 0, 2, 0);
  closeSync(probeFd);
  if (probe.toString('ascii') !== '#!') {
    const content = readFileSync(cliPath, 'utf-8');
    writeFileSync(cliPath, '#!/usr/bin/env node\n' + content);
  }
  chmodSync(cliPath, 0o755);

  // Step 5: Compile standalone binary (existing behavior)
  console.log('[build] Compiling to standalone binary...');
  await $`bun build --compile --minify ./src/surfaces/cli/index.ts --outfile ./dist/maestro`;

  console.log('[build] Done.');
  console.log('  dist/server.bundle.mjs  -- MCP server');
  console.log('  hooks/*.mjs             -- Hook scripts');
  console.log('  dist/cli.js             -- npm CLI entry');
  console.log('  dist/maestro            -- Standalone binary');
}

build().catch((err) => {
  console.error(err);
  process.exit(1);
});
