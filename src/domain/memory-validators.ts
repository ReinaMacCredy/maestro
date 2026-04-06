import { z } from "zod";

export const CorrectionTriggerSchema = z.object({
  keywords: z.array(z.string()),
  fileGlobs: z.array(z.string()),
});

export const CorrectionSchema = z.object({
  id: z.string().min(1),
  rule: z.string().min(1),
  source: z.string().min(1),
  trigger: CorrectionTriggerSchema,
  severity: z.enum(["soft", "hard"]),
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
  promotedToRatchet: z.string().optional(),
});

export const CreateCorrectionInputSchema = z.object({
  rule: z.string().min(1),
  source: z.string().min(1),
  trigger: CorrectionTriggerSchema,
  severity: z.enum(["soft", "hard"]),
});

export const CorrectionQuerySchema = z.object({
  keywords: z.array(z.string()).optional(),
  filePaths: z.array(z.string()).optional(),
  text: z.string().optional(),
});

export const RawLearningEntrySchema = z.object({
  sessionDate: z.string().min(1),
  content: z.string().min(1),
  branch: z.string().optional(),
});

export const CompiledLearningsSchema = z.object({
  compiledAt: z.string().min(1),
  summary: z.string().min(1),
  rawCount: z.number().int().nonnegative(),
});

export const RatchetAssertionSchema = z.object({
  id: z.string().min(1),
  correctionId: z.string().min(1),
  rule: z.string().min(1),
  check: z.string().min(1),
  createdAt: z.string().min(1),
});

export const RatchetSuiteSchema = z.object({
  assertions: z.array(RatchetAssertionSchema),
});

export const RatchetBaselineSchema = z.object({
  passCount: z.number().int().nonnegative(),
  lastRunAt: z.string().min(1),
});

export const ProjectNodeSchema = z.object({
  path: z.string().min(1),
  name: z.string().min(1),
  role: z.string().optional(),
});

export const ProjectEdgeSchema = z.object({
  from: z.string().min(1),
  to: z.string().min(1),
  relation: z.enum(["exposes", "consumes", "shared-types"]),
  detail: z.string().optional(),
});

export const ProjectGraphSchema = z.object({
  nodes: z.array(ProjectNodeSchema),
  edges: z.array(ProjectEdgeSchema),
});

export const MemoryConfigSchema = z.object({
  enabled: z.boolean(),
  corrections: z.object({
    enabled: z.boolean(),
    matching: z.enum(["keyword", "ast-grep", "both"]),
    auto_capture: z.enum(["prompt", "auto", "off"]),
    severity_default: z.enum(["soft", "hard"]),
  }),
  learnings: z.object({
    enabled: z.boolean(),
    compile_threshold: z.number().int().positive(),
    max_age_days: z.number().int().positive(),
  }),
  ratchet: z.object({
    enabled: z.boolean(),
    enforcement: z.enum(["warn", "block"]),
  }),
  graph: z.object({
    enabled: z.boolean(),
  }),
});
