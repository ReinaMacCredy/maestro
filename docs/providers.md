# Provider Registry and Skills

Maestro treats agent integrations as providers. Runtime providers can launch handoffs. Skill target providers can receive Maestro-managed skills.

## Providers

Current providers:

| Provider | Runtime | Skill target | Root |
|---|---:|---:|---|
| Codex | yes | yes | `$CODEX_HOME/skills` or `~/.codex/skills` |
| Claude Code | yes | yes | `~/.claude/skills` |
| Hermes | yes | yes | `~/.hermes/skills/maestro` |
| AgentSkills | no | yes | `~/.agents/skills` |

Inspect providers with:

```bash
maestro providers list
maestro providers list --json
maestro providers doctor
maestro providers doctor hermes --json
```

`doctor` checks config paths, skills roots, provider binaries when applicable, and Hermes shared-skill configuration.

## Skills

Maestro discovers AgentSkills-compatible directories that contain a `SKILL.md` file with YAML frontmatter.

Required frontmatter:

```yaml
---
name: my-skill
description: Use this for a specific workflow.
---
```

Unknown frontmatter fields are preserved in the discovered metadata. Skill discovery precedence is deterministic:

1. project `.maestro/skills`
2. project `.agents/skills`
3. repo bundled skills
4. `~/.maestro/external-skills`
5. `~/.agents/skills`
6. provider roots

Collisions are warnings. The first skill in the precedence order wins.

Common commands:

```bash
maestro skills list --scope all
maestro skills inspect my-skill
maestro skills install ./my-skill --scope user --targets all
maestro skills install owner/repo/path/to/skill --scope user --targets codex,hermes
maestro skills remove my-skill --scope user
maestro skills sync --targets all
```

Supported install sources:

- local skill directory
- local directory containing one or more skill directories
- Git URL
- GitHub shorthand: `owner/repo` or `owner/repo/path`
- HTTP `zip`, `tar`, `tgz`, or `tar.gz` archive URL

Marketplace slug lookup is intentionally not implemented until a stable documented AgentSkills registry API exists.

## Managed Storage

Maestro keeps bundled skills and external skills separate:

- bundled source of truth: `~/.maestro/skills`
- external managed skills: `~/.maestro/external-skills`
- shared AgentSkills root: `~/.agents/skills`

External installs write `.maestro-external-skill.json` with the original source, resolved commit or archive URL when available, file hashes, installed target roots, and install timestamp. Target roots receive links to the managed copy where possible. `remove` and `sync` only remove or replace Maestro-managed links/directories and leave foreign skill directories in place.

## Hermes

`maestro install` and `maestro update --agents-only` ensure `~/.hermes/config.yaml` contains:

```yaml
skills:
  external_dirs:
    - ~/.agents/skills
```

When Maestro rewrites an existing Hermes config, it creates a timestamped backup first.

## Security Model

Skill install copies files and writes manifests. It does not execute scripts from the skill source during install. Git and HTTP sources are fetched and unpacked as data, validated for `SKILL.md`, and then copied into Maestro-managed storage.
