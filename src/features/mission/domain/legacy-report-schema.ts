/**
 * Agent-report schemas used by `maestro reply` to parse untrusted user input.
 */

import { z } from "zod";

const CommandRunSchema = z.object({
  command: z.string().min(1),
  exitCode: z.number().int(),
  observation: z.string(),
}).strict();

const InteractiveCheckSchema = z.object({
  action: z.string().min(1),
  observed: z.string(),
}).strict();

const TestCaseSchema = z.object({
  name: z.string().min(1),
  verifies: z.string(),
}).strict();

const TestFileSchema = z.object({
  file: z.string().min(1),
  cases: z.array(TestCaseSchema),
}).strict();

const DiscoveredIssueSchema = z.object({
  severity: z.string().min(1),
  description: z.string().min(1),
  suggestedFix: z.string().optional(),
}).strict();

/** Rich agent report (plan spec) */
export const RichAgentReportSchema = z.object({
  salientSummary: z.string().min(1),
  whatWasImplemented: z.string(),
  whatWasLeftUndone: z.string(),
  verification: z.object({
    commandsRun: z.array(CommandRunSchema),
    interactiveChecks: z.array(InteractiveCheckSchema),
  }).strict(),
  tests: z.object({
    added: z.array(TestFileSchema),
  }).strict(),
  discoveredIssues: z.array(DiscoveredIssueSchema),
}).strict();

/** Legacy agent report (backward compat -- transforms to rich format) */
export const LegacyAgentReportSchema = z.object({
  content: z.string().min(1),
  timestamp: z.string().optional(),
  agent: z.string().optional(),
}).strict().transform((legacy) => ({
  salientSummary: legacy.content,
  whatWasImplemented: legacy.content,
  whatWasLeftUndone: "",
  verification: {
    commandsRun: [] as readonly z.infer<typeof CommandRunSchema>[],
    interactiveChecks: [] as readonly z.infer<typeof InteractiveCheckSchema>[],
  },
  tests: { added: [] as readonly z.infer<typeof TestFileSchema>[] },
  discoveredIssues: [] as readonly z.infer<typeof DiscoveredIssueSchema>[],
}));

/** Accepts rich or legacy agent report, normalizes to rich format */
export const AgentReportSchema = z.union([RichAgentReportSchema, LegacyAgentReportSchema]);

export type AgentReport = z.infer<typeof AgentReportSchema>;
