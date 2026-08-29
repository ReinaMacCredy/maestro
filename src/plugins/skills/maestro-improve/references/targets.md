# Targets

A lesson names one target. The target decides where the edit lands, who reads
it afterwards, and how far it travels.

| target | lives in | who reads it | cost of an edit |
|---|---|---|---|
| a recipe section | `maestro recipe show <name>` (source: `src/plugins/recipes/*.md`) | every session that opens that recipe | a repository change, released and installed |
| a rule in `lane.md` or `lead.md` | the room, scaffolded from the runtime | the room and every lane it opens | a repository change; it reaches the live room only through `maestro update` |
| a room template | the room's `IDENTITY.md`, `AGENTS.md`, `OWNER.md` and their sources | the room itself | same as above; `OWNER.md` is the owner's and is never overwritten |
| a `skills/maestro-*` file | `src/plugins/skills/<name>/` | any session that loads the skill | a repository change; the version stamp is what makes an installed copy replaceable |
| a repository's Workspace Protocol | that repository's `AGENTS.md` / `CLAUDE.md` | every agent working in that repository | a commit in that repository, nothing else |

Two consequences worth holding on to.

The room is never hand-edited. A lesson whose target is `lane.md`, `lead.md`,
or a room template is an edit to the runtime's source of that file; the live
room picks it up on the next `maestro update`, which is the room's own gate.
Editing `~/maestro` directly makes the change disappear at that update.

A repository's Workspace Protocol is the cheapest target and the narrowest. A
correction that only applies to one repository belongs there, not in a recipe:
doctrine that every session reads is the most expensive text in the system, and
it earns its place by applying everywhere.
