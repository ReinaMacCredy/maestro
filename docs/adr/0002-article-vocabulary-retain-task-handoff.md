# Adopt article vocabulary, retain task and handoff

Primitives are renamed to match OpenAI's harness-engineering vocabulary (`design-docs`, `exec-plans`, `product-specs`, `references`, `generated`, `architecture`, `quality-score`, `principles`, `worktree`, `loop`) so the maestro documentation can cite the article directly and consumer projects find familiar terminology. `task` and `handoff` are kept as-is because they are the well-shaped execution primitives in maestro today: task is the unit of PR-shaped work; handoff is the artifact emitted at cross-session state transitions.

Previous maestro names that disappear: `mission` (now `exec-plan`), `spec` (now `product-spec`), `intake` / `brainstorm` (folded into `design-docs` reading and `product-spec` authoring), `session` / `notes` (folded into `handoff`). This breaks existing users; deprecation strategy is open (see future ADR).
