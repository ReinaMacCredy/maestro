# Loop Recipes

Orchestration recipes for the work loop, served on demand by `maestro loop`.
Each recipe is the HOW of running one fan-out or loop pattern: how to dispatch
the agents, collect their results through the verbs, and stop. The WHEN -- the
judgment to reach for a pattern -- lives in the skills (maestro-card,
maestro-design); this catalog carries the mechanics.

## How to use this

Run `maestro loop` (or `maestro loop list`) for the recipes with a one-line
when-to-use, then `maestro loop show <name>` for one recipe. Serving from the
binary means the recipes need no `.maestro` repo and never drift from what this
binary ships.

Two ground rules run through every recipe:

- Results land through the verbs (task / decision / event), never only in
  conversation. A fan-out that leaves its findings in chat is lost on the next
  context.
- Claude Code authors a `Workflow` script; Codex dispatches parallel sub-agents
  directly, with worktree threads when files overlap.
