# ECC Rule Precedence in Maestro

Default precedence for imported ECC rule content:

1. ECC rule set under `rules/ecc/**`
2. Existing Maestro defaults

Exception:
- BR/BV task lifecycle requirements remain mandatory and are not overridden.

Runtime toggles:
- `MAESTRO_ENABLE_ECC_QUALITY_GATES=1` enables quality-gate hooks (default on).
- `MAESTRO_ENABLE_LEARNING_HOOKS=0` keeps learning capture disabled (default off).
