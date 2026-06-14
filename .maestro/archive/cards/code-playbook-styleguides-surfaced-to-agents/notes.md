# Code playbook / styleguides surfaced to agents

2026-06-14  dec-playbook-scope-general-agnostic-a3d8 locked -- Playbook scope: general-agnostic principles, not per-language
2026-06-14  dec-playbook-slot-a-code-style-section-6505 locked -- Playbook slot: a Code style section inside HARNESS.md (W1)
2026-06-14  dec-playbook-ownership-maestro-owned-b8dc locked -- Playbook ownership: maestro-owned principles; project style stays in AGENTS.md
2026-06-14  dec-playbook-content-the-universal-code-ea9d locked -- Playbook content: the universal code-style principles
2026-06-14  2026-06-14 build authorized by user; directive: 'skip qa and go' -> lean qa baseline (bl-001/002/003 covering ac-1/2/3), flow accept -> prepare -> work -> verify; no full qa ceremony.
2026-06-14  Impl notes: (1) ac-1 holds via 'maestro install --agent', NOT bare 'maestro init' (init only scaffolds .maestro; install writes both CLAUDE.md+AGENTS.md mirror blocks). (2) D2 kept narrow in mirrors.rs (resync_mirror_blocks/preview_mirror_block_resync), content-based + marker-gated, no install-lock coupling; backs up drift via op 'sync'. (3) No-op guard test: feature does not change block bodies (only HARNESS.md), so sync must be a pure no-op on healthy repos. (4) New install-facade exports required updating the architecture_imports allowlist guard. (5) Second version literal lived in src/domain/harness/extract.rs test (1.13.0->1.14.0).
