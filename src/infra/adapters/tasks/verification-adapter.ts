/**
 * FsVerificationAdapter -- deterministic task verification checks.
 *
 * Four criteria:
 *   1. Build check: run build command with timeout
 *   2. Git diff check: verify changes exist since claimedAt
 *   3. Summary quality: minimum length + spec term overlap
 *   4. AC match: acceptance criteria keyword presence in summary
 */

import type { VerificationPort, VerifyParams, VerificationReport, VerificationCriterion } from '../../../domain/ports/verification.ts';
import type { ResolvedVerificationConfig } from './verification-config.ts';
import { spawn } from 'child_process';
import simpleGit from 'simple-git';
import * as path from 'path';
import { readJson } from '../../utils/fs-io.ts';
import { extractKeywords } from '../../../domain/text-utils.ts';
import { MAX_BUILD_OUTPUT_BYTES, MIN_SUMMARY_LENGTH } from '../../../domain/constants.ts';

export class FsVerificationAdapter implements VerificationPort {
  private config: ResolvedVerificationConfig;

  constructor(config: ResolvedVerificationConfig) {
    this.config = config;
  }

  async verify(params: VerifyParams): Promise<VerificationReport> {
    const criteria: VerificationCriterion[] = [];
    const suggestions: string[] = [];
    let buildOutput: string | undefined;

    // 1. Build check
    const buildCmd = this.config.buildCommand ?? this.detectBuildCommand(params.projectRoot);
    if (buildCmd) {
      const buildResult = await this.runBuildCheck(buildCmd, params.projectRoot);
      criteria.push(buildResult.criterion);
      if (!buildResult.criterion.passed) {
        buildOutput = buildResult.output;
        suggestions.push('Fix build errors before marking task done');
      }
    }

    // 2. Git diff check
    const diffCriterion = await this.checkGitDiff(params.projectRoot, params.claimedAt);
    criteria.push(diffCriterion);
    if (!diffCriterion.passed) {
      suggestions.push('No code changes detected -- verify work was committed');
    }

    // 3. Summary quality
    const summaryCriterion = this.checkSummaryQuality(params.summary, params.specContent);
    criteria.push(summaryCriterion);
    if (!summaryCriterion.passed) {
      suggestions.push('Provide a more detailed summary referencing the spec');
    }

    // 4. AC match
    if (params.acceptanceCriteria) {
      const acCriterion = this.checkAcceptanceCriteria(params.summary, params.acceptanceCriteria);
      criteria.push(acCriterion);
      if (!acCriterion.passed) {
        suggestions.push('Summary does not reference all acceptance criteria');
      }
    }

    const passed = criteria.filter(c => c.passed).length;
    const total = criteria.length;
    const score = total > 0 ? passed / total : 1;

    return {
      passed: score >= this.config.scoreThreshold,
      score,
      criteria,
      buildOutput,
      suggestions,
      timestamp: new Date().toISOString(),
    };
  }

  // --------------------------------------------------------------------------
  // Individual checks
  // --------------------------------------------------------------------------

  private async runBuildCheck(
    command: string,
    cwd: string,
  ): Promise<{ criterion: VerificationCriterion; output?: string }> {
    return new Promise((resolve) => {
      const [cmd, ...args] = command.split(/\s+/);
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), this.config.buildTimeoutMs);

      let stderr = '';

      try {
        const proc = spawn(cmd, args, {
          cwd,
          signal: controller.signal,
          stdio: ['ignore', 'ignore', 'pipe'],
        });

        proc.stderr.on('data', (chunk: Buffer) => {
          if (stderr.length < MAX_BUILD_OUTPUT_BYTES) {
            stderr += chunk.toString();
          }
        });

        proc.on('close', (code) => {
          clearTimeout(timeout);
          const passed = code === 0;
          resolve({
            criterion: {
              name: 'build',
              passed,
              detail: passed ? `Build passed (${command})` : `Build failed with exit code ${code}`,
            },
            output: passed ? undefined : stderr.slice(0, MAX_BUILD_OUTPUT_BYTES),
          });
        });

        proc.on('error', (err) => {
          clearTimeout(timeout);
          const aborted = err.name === 'AbortError' || controller.signal.aborted;
          resolve({
            criterion: {
              name: 'build',
              passed: false,
              detail: aborted
                ? `Build timed out after ${this.config.buildTimeoutMs}ms`
                : `Build error: ${err.message}`,
            },
            output: aborted ? undefined : stderr.slice(0, MAX_BUILD_OUTPUT_BYTES),
          });
        });
      } catch (err) {
        clearTimeout(timeout);
        resolve({
          criterion: {
            name: 'build',
            passed: false,
            detail: `Build spawn error: ${(err as Error).message}`,
          },
        });
      }
    });
  }

  private async checkGitDiff(projectRoot: string, claimedAt?: string): Promise<VerificationCriterion> {
    try {
      const git = simpleGit(projectRoot);
      const status = await git.status();

      // Check for uncommitted changes
      if (!status.isClean()) {
        return { name: 'git-diff', passed: true, detail: 'Uncommitted changes detected' };
      }

      // Check for commits since claimedAt
      if (claimedAt) {
        const log = await git.log({ '--since': claimedAt });
        if (log.total > 0) {
          return { name: 'git-diff', passed: true, detail: `${log.total} commit(s) since task claimed` };
        }
      }

      return { name: 'git-diff', passed: false, detail: 'No changes detected since task was claimed' };
    } catch {
      // Git not available or not a repo -- skip gracefully
      return { name: 'git-diff', passed: true, detail: 'Git check skipped (not a git repo or git unavailable)' };
    }
  }

  private checkSummaryQuality(summary: string, specContent?: string): VerificationCriterion {
    if (summary.length < MIN_SUMMARY_LENGTH) {
      return {
        name: 'summary-quality',
        passed: false,
        detail: `Summary too short (${summary.length} chars, minimum ${MIN_SUMMARY_LENGTH})`,
      };
    }

    if (specContent) {
      const uniqueTerms = [...extractKeywords(specContent)];

      if (uniqueTerms.length > 0) {
        const summaryLower = summary.toLowerCase();
        const matched = uniqueTerms.filter(t => summaryLower.includes(t));
        if (matched.length === 0) {
          return {
            name: 'summary-quality',
            passed: false,
            detail: 'Summary does not reference any key terms from the spec',
          };
        }
      }
    }

    return { name: 'summary-quality', passed: true, detail: 'Summary meets quality threshold' };
  }

  private checkAcceptanceCriteria(summary: string, acceptanceCriteria: string): VerificationCriterion {
    // Extract AC bullet points
    const bullets = acceptanceCriteria
      .split('\n')
      .map(line => line.replace(/^[-*\[\]x ]+/i, '').trim())
      .filter(line => line.length > 0);

    if (bullets.length === 0) {
      return { name: 'ac-match', passed: true, detail: 'No acceptance criteria to check' };
    }

    const summaryLower = summary.toLowerCase();
    const matchedCount = bullets.filter(bullet => {
      const words = [...extractKeywords(bullet)];
      return words.some(w => summaryLower.includes(w));
    }).length;

    const ratio = matchedCount / bullets.length;
    const passed = ratio >= 0.5; // At least half of AC referenced

    return {
      name: 'ac-match',
      passed,
      detail: `${matchedCount}/${bullets.length} acceptance criteria referenced in summary`,
    };
  }

  // --------------------------------------------------------------------------
  // Build command detection
  // --------------------------------------------------------------------------

  private detectBuildCommand(projectRoot: string): string | undefined {
    try {
      const pkgPath = path.join(projectRoot, 'package.json');
      const pkg = readJson<{ scripts?: Record<string, string> }>(pkgPath);
      if (!pkg) return undefined;
      const scripts = pkg.scripts ?? {};

      // Priority: typecheck > build (not 'check' -- often includes full test suite)
      if (scripts.typecheck) return 'bun run typecheck';
      if (scripts.build) return 'bun run build';
    } catch {
      // No package.json or parse error
    }
    return undefined;
  }
}
