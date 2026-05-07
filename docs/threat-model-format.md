# Threat Model Evidence Format

Maestro records threat-model evidence as a structured payload via:

    maestro evidence record --kind threat-model --threat-model-file <path>

The file may be JSON or YAML. The schema:

| Field            | Type                                            | Required | Description                                |
| ---------------- | ----------------------------------------------- | -------- | ------------------------------------------ |
| assets           | string[]                                        | yes      | Resources to protect.                      |
| threatCategories | string[]                                        | yes      | STRIDE-style categories or domain-specific |
| mitigations      | { threat: string; mitigation: string }[]        | yes      | Pairs of (threat name, mitigation summary) |
| residualRisk     | "low" \| "medium" \| "high"                     | yes      | Remaining risk after mitigations           |
| criterion_id     | string                                          | no       | Acceptance-criterion this row covers       |
| source_file      | string                                          | no       | Set automatically by `evidence record`     |

## Example A — JSON

    {
      "assets": ["session tokens", "password hashes"],
      "threatCategories": ["spoofing", "tampering", "info-disclosure"],
      "mitigations": [
        { "threat": "session-fixation", "mitigation": "rotate token on login" },
        { "threat": "weak-hashing",     "mitigation": "argon2id with workfactor 3" }
      ],
      "residualRisk": "low"
    }

## Example B — YAML

    assets:
      - session tokens
      - password hashes
    threatCategories:
      - spoofing
      - tampering
      - info-disclosure
    mitigations:
      - threat: session-fixation
        mitigation: rotate token on login
      - threat: weak-hashing
        mitigation: argon2id with workfactor 3
    residualRisk: low

## Risk Engine semantics

When the diff intersects security-relevant sensitive paths AND the
derived risk class is `critical`, the Verdict is `HUMAN` with reason
code `threat-model-required` unless a `threat-model` Evidence row is
present.

Per Rule 1 (LLM veto-only), **presence is necessary but not sufficient**:
a schema-valid empty-content threat-model row clears the
`threat-model-required` predicate, but other gates may still hold.
Substantive correctness (whether the threat model meaningfully covers
the change) is reviewed at L6 when the PR is opened.
