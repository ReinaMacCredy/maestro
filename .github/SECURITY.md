# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 2.x.x   | :white_check_mark: |
| < 2.0   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in this project, please report it responsibly.

### How to Report

1. **Do not** open a public GitHub issue for security vulnerabilities
2. Email the maintainer directly or use GitHub's private vulnerability reporting feature
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### What to Expect

- Acknowledgment within 48 hours
- Status update within 7 days
- We aim to resolve critical issues within 30 days

### Scope

This security policy covers:

- Skills and their execution context
- Hooks and lifecycle scripts
- Plugin configuration files
- Any code that runs in the Claude Code environment

### Out of Scope

- Issues in Claude Code itself (report to Anthropic)
- Issues in third-party dependencies (report upstream)
- Theoretical attacks without proof of concept

## Security Best Practices

When contributing to this project:

1. Never commit secrets, API keys, or credentials
2. Validate all external input in hooks
3. Use pinned versions for GitHub Actions
4. Review skill instructions for injection risks
