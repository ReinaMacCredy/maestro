# Triage

Diagnose in this order. Stop at the first step whose answer is unknown, gather
the smallest evidence that settles it, then continue.

1. **Problem** - What observable outcome differs from the intended one?
2. **Authority** - Who may decide, write, reclaim, install, or publish here?
3. **Topology** - Which sessions, processes, stores, worktrees, and consumers are involved?
4. **Attention** - Who can currently see the evidence and who is silent?
5. **Capability** - Does the responsible actor have the required tool and permission?
6. **State** - What branch, commit, lease, decision, cursor, pid, and dirty path is current?
7. **Evidence** - Which source, artifact, installed, live, or journey link is actually proven?
8. **Owning layer** - What is the smallest layer that can correct the common mechanism?
9. **Learning** - What should be retained, promoted, reviewed later, or deleted?

| Symptom | First question |
|---|---|
| Wrong result | Problem: what exact observable differs? |
| Work cannot proceed | Authority: who owns the decision or mutation? |
| Two agents conflict | Topology: which leases and paths overlap? |
| A holder is silent | Attention: who has seen its latest activity? |
| A command cannot run | Capability: is the tool or permission present? |
| Behavior changed between sessions | State: which commit, runtime, and store is each using? |
| Tests pass but users still fail | Evidence: which downstream layer is NOT TESTED? |
| The same defect returns | Owning layer: where does the shared mechanism live? |
| A workaround keeps growing | Learning: what correction earned promotion, and until when? |
