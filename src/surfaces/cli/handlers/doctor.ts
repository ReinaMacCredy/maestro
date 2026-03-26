/**
 * maestro doctor -- health check for config, integrations, and project state.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../services.ts';
import { doctor, type DoctorReport } from '../../../app/workflow/doctor.ts';
import { output, renderTable } from '../../../infra/utils/output.ts';
import { handleCommandError } from '../../../domain/errors.ts';

const STATUS_MARKER: Record<string, string> = {
  ok: '[ok]',
  warn: '[!]',
  fail: '[x]',
};

function formatDoctor(report: DoctorReport): string {
  const lines: string[] = [];

  lines.push('maestro doctor');
  lines.push('');

  const headers = ['Check', 'Status', 'Message'];
  const rows = report.checks.map((c) => [
    c.name,
    STATUS_MARKER[c.status] ?? c.status,
    c.message,
  ]);
  lines.push(renderTable(headers, rows));

  lines.push('');
  const { ok, warn, fail } = report.summary;
  lines.push(`summary: ${ok} ok, ${warn} warn, ${fail} fail`);

  return lines.join('\n');
}

export default defineCommand({
  meta: { name: 'doctor', description: 'Health check for config, integrations, and project state\n\nExamples:\n  maestro doctor\n  maestro doctor --json' },
  args: {},
  async run() {
    try {
      const services = getServices();
      const report = await doctor(services);
      output(report, formatDoctor);
      if (report.summary.fail > 0) process.exitCode = 1;
    } catch (err) {
      handleCommandError('doctor', err);
    }
  },
});
