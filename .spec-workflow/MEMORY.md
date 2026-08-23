# Repo memory — maestro (rewrite-maestro-in-typescript branch)

- [ADR-0001](adr/0001-typescript-rewrite.md) — TS on bun, bun-shim distribution, immediate `maestro` name takeover, Rust kept as `maestro-legacy`; compiled-binary option rejected (unproven external `.ts` import, spike SIGKILL).
- [ADR-0002](adr/0002-kernel-policy-recipes-separation.md) — three layers: mechanism-only kernel, policy as removable plugins, prompt-first recipes; self-written Cordis-inspired runtime (no cordis dep); no lean mode ever — disable the plugin instead.
- [ADR-0003](adr/0003-single-work-entity.md) — one `work` entity (tree+DAG+kind-as-data+lease) + minimal `decision` entity; supersedes the four-entity clause of ADR-0002; feature lifecycle becomes gates on `work done`.
- [ADR-0004](adr/0004-hook-first-delivery.md) — guidance brief + store mailbox delivered via harness hooks (SendMessage model, own transport); thin mirror pointer lines; recipes on demand; rejected thick HARNESS.md and polled inbox.
- [Bundle 2 recipes](archive/maestro-ts-recipes-policies/) — built-in `recipe` serves 18 source-owned markdown methods on demand and contributes only a brief pointer; `recipe show` never extracts content into repos.
- [Bundle 2 policies](archive/maestro-ts-recipes-policies/) — tdd/qa/research/witness ship disabled and gate through `test:`/`qa:` claim-proof pairs or `research:`/independent-session `witness:` notes; mutation coverage proves witness identity matters.
- [ADR-0005](adr/0005-shared-store-anchor.md) — one store per repo at the git common root; worktrees share sessions/mailbox/leases
