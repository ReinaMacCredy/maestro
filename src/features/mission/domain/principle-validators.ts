import { z } from "zod";
import { MilestoneProfileSchema } from "./mission-validators.js";
import type { Principle, CreatePrincipleInput } from "./principle-types.js";

const PRINCIPLE_ID_PATTERN = /^[a-z0-9][a-z0-9-]*$/;

const GateCheckTypeSchema = z.string().refine(
  (value) => {
    if (value === "object_non_empty" || value === "array_all_passed") return true;
    const match = value.match(/^array_min_length:(\d+)$/);
    return match !== null && Number(match[1]) >= 1;
  },
  { message: "Invalid gate check type. Expected: array_min_length:N, object_non_empty, or array_all_passed" },
);

export const PrincipleSchema = z.object({
  id: z.string().regex(PRINCIPLE_ID_PATTERN, "Principle id must be lowercase alphanumeric with dashes"),
  name: z.string().min(1),
  source: z.enum(["karpathy", "custom"]),
  rule: z.string().min(1),
  profiles: z.array(MilestoneProfileSchema).min(1),
  mode: z.enum(["advisory", "gate"]),
  gateField: z.string().min(1).optional(),
  gateCheck: GateCheckTypeSchema.optional(),
}).strict().refine(
  (data) => {
    if (data.mode === "gate") {
      return data.gateField !== undefined && data.gateCheck !== undefined;
    }
    return true;
  },
  { message: "Gate-mode principles require gateField and gateCheck" },
);

export const CreatePrincipleInputSchema = z.object({
  id: z.string().regex(PRINCIPLE_ID_PATTERN, "Principle id must be lowercase alphanumeric with dashes"),
  name: z.string().min(1),
  source: z.enum(["karpathy", "custom"]).default("custom"),
  rule: z.string().min(1),
  profiles: z.array(z.string().min(1)).min(1),
  mode: z.enum(["advisory", "gate"]),
  gateField: z.string().min(1).optional(),
  gateCheck: z.string().min(1).optional(),
}).strict().refine(
  (data) => {
    if (data.mode === "gate") {
      return data.gateField !== undefined && data.gateCheck !== undefined;
    }
    return true;
  },
  { message: "Gate-mode principles require --gate-field and --gate-check" },
);

export function validatePrinciple(data: unknown): Principle {
  return PrincipleSchema.parse(data) as Principle;
}

export function validateCreatePrincipleInput(data: unknown): CreatePrincipleInput {
  return CreatePrincipleInputSchema.parse(data) as CreatePrincipleInput;
}
