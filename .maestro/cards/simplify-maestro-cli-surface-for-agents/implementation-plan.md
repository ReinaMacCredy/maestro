## Task T1: Add canonical maestro next modes
check: `maestro next` reports one best action read-only, `maestro next --run` claims only an auto-safe ready card, and `maestro next --loop --max-steps N` stops at input/risk/blocker with a transcript
covers: ac-1

## Task T2: Guard generic card status lifecycle transitions
after: T1
check: generic `maestro card update <task> --status verified` and `--status needs_verification` fail with typed task remedies while compatible low-risk status updates still work
covers: ac-2

## Task T3: Add validated feature-flow helpers
after: T2
check: `maestro qa baseline`, `maestro qa slice`, `maestro feature proof add`, `maestro feature proof waive`, and `maestro feature prepare --task` write validated durable artifacts from explicit input and reject empty or ambiguous input
covers: ac-3

## Task T4: Add coordination advisories and sender assertions
after: T3
check: `maestro active --connect` prints exact advisory link/message/conflict commands, and `maestro msg send --from <card>` fails when the asserted sender differs from the current card without writing a message
covers: ac-4

## Task T5: Hide dead-end retired task archive verbs and run full validation
after: T4
check: retired task archive/unarchive are not visible as normal help paths, compatibility errors remain instructional, and `cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass
covers: ac-1, ac-2, ac-3, ac-4
