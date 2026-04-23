# Security Context

<!-- maestro-setup:generated:start -->
## Security-Sensitive Areas

- Auth and permissions: TODO(maestro-setup)
- Secrets and tokens: TODO(maestro-setup)
- Personally identifiable information: TODO(maestro-setup)
- Payments or financial state: TODO(maestro-setup)
- Deployment and infrastructure: TODO(maestro-setup)
- Destructive data behavior: TODO(maestro-setup)

## Default Security Posture

- Parse untrusted input at boundaries.
- Prefer allowlists over implicit trust.
- Fail closed when permission checks are unclear.
- Do not log secrets or sensitive payloads.
- Ask for human approval before widening access or deleting data.

## Review Questions

- What trust boundary changes?
- What happens on malformed or hostile input?
- Can this leak sensitive data into logs or prompts?
- Is rollback safe if behavior is wrong?
<!-- maestro-setup:generated:end -->

## User Notes

Add security notes here. This section is outside the managed block.
