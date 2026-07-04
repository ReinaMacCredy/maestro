## Task T1: Add recipe transition schema and validation
check: cargo test --test loop_recipes_integration
covers: ac-2

## Task T2: Build LoopChainFacts and deterministic matcher
after: T1
check: cargo test --test loop_recipes_integration
covers: ac-1
covers: ac-2

## Task T3: Add loop next --chain text and JSON output
after: T2
check: cargo test --test loop_recipes_integration
covers: ac-3

## Task T4: Add structured loop outcome transition receipts
after: T3
check: cargo test --test loop_recipes_integration
covers: ac-4

## Task T5: Add loop trace text and JSON readout
after: T4
check: cargo test --test loop_recipes_integration
covers: ac-5

## Task T6: Refresh docs, CLI reference, and resource guards
after: T5
check: cargo test --test cli_reference_freshness && cargo test --test resources_version_guard
covers: ac-6
