# Council brief template

One brief, identical for every core seat. Fill every line; write `none` rather
than dropping a heading. The seat prompt is the brief, the case output
contract, and exactly one role line.

```text
CASE_ID: <stable, URL-safe>
ORIGINAL REQUEST: <the user's words, verbatim>
DECISION QUESTION: <may clarify the request; never narrows or replaces it>
OBSERVABLE OUTCOME: <what changes when the decision is right>
AUTHORITATIVE FACTS: <owner rulings and verified facts, each with provenance>
DIRECT OBSERVATIONS: <source-backed observations with exact locations>
UNVERIFIED CLAIMS: <every other premise, labelled as a claim>
UNKNOWNS: <what nobody has checked>
HARD CONSTRAINTS: <what a verdict may not violate>
PREFERENCES / PRIORITY ORDER: <what the owner wants, ranked; not constraints>
AUTHORIZED SCOPE AND SOURCES: <paths, repos, documents a seat may read>
SNAPSHOT: <branch, commit, dirty paths pinned at brief time>
REQUESTED OUTPUT: <the natural units this case adjudicates>
CASE OUTPUT CONTRACT: <the sections or fields every seat returns>
```

Every seat prompt opens with a plain lowercase sentence, then:

```text
seat execution mode: work as a fully autonomous reviewer with independent
judgment inside the authorized scope. This assignment asks for your own
analysis, not orchestration: do not load the council skill, open work, spawn
or contact agents, or read other seats' work items. Begin directly.
```

and closes with:

```text
This is analysis only. Do not edit, create, rename, or delete files. Do not
write code. Do not spawn or contact agents. Do not optimize for agreement.
Distinguish direct observations from inference and state what evidence would
prove your position wrong.
```

## Snapshot

Pin the branch, commit, and dirty paths before opening seats. Unrelated
existing changes are not council writes, and a previously dirty checkout need
not be clean. When decision-relevant source can change during review, capture
a reconstructable patch or archive outside the repository; a commit hash
detects drift but does not preserve bytes.
