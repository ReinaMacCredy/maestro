# Codex Runtime Notes

This repository supports Codex as an optional runtime alongside Claude Code and Amp.

## Install

```bash
bash scripts/install-codex.sh
```

## Optional ECC Skillpack

Enable the optional ECC skillpack only when you need the extra ECC-specific hooks/rules:

```bash
bash scripts/install-codex.sh --with-ecc-skillpack
```

If the activation helper is not present in your checkout yet, enable it later when available:

```bash
bash ~/.codex/skills/maestro/scripts/enable-ecc-skillpack.sh all
```

## Command Surface

Maestro command names stay unchanged. Core workflow commands remain:

- `/maestro:setup`
- `/maestro:new-track`
- `/maestro:implement`
- `/maestro:review`
- `/maestro:status`
- `/maestro:revert`
- `/maestro:note`

## Validation

Before shipping changes touching hooks/rules, run:

```bash
bash -n scripts/*.sh .claude/scripts/*.sh
bash scripts/validate-hook-config.sh
find scripts/hooks/ecc -type f -name '*.js' -print0 | xargs -0 -n1 node --check
bash scripts/test-hooks.sh
```
