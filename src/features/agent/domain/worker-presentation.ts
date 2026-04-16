export interface WorkerGuidance {
  readonly summary: string;
  readonly bestFor: string;
  readonly tradeoffs: string;
}

export function getWorkerGuidance(slug: string): WorkerGuidance {
  switch (slug) {
    case "claude-code":
      return {
        summary: "Highest quality, slower and pricier.",
        bestFor: "hard bugs; risky refactors; architecture-heavy work; tasks where correctness matters",
        tradeoffs: "slower; highest cost",
      };
    case "gemini":
      return {
        summary: "Fast and low cost, lighter reasoning.",
        bestFor: "low-risk tasks; drafting and support work; simple follow-up tasks; cheap retries",
        tradeoffs: "weaker on complex tasks; may need more retries",
      };
    case "codex":
    default:
      return {
        summary: "Fast, strong general-purpose coding.",
        bestFor: "everyday implementation; debugging and iteration; medium to high complexity tasks",
        tradeoffs: "less exhaustive than Claude Code; higher cost than Gemini",
      };
  }
}

export function formatWorkerLabel(slug: string): string {
  return slug
    .split("-")
    .map((part) => part.length === 0 ? part : part[0]!.toUpperCase() + part.slice(1))
    .join(" ");
}
