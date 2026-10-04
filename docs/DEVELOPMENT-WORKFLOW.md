# Development workflow: Antigravity and Codex

## Roles

| Role | Responsibility | Handoff |
|---|---|---|
| Maintainer | Scope, product decisions, merging and publication | Accepted task and branch |
| Antigravity | Implement one task, tests and relevant docs | Exact commit/diff, evidence, dependencies and limitations |
| Codex | Review correctness, simplify, measure footprint and perform agreed cleanup | Findings, verification and disposition |

Cleanup is part of development; it does not authorize publishing, merging or
unrelated edits. Resolve disagreements using reproductions and task acceptance
criteria, not stylistic preference.

## One writer per checkout

Use one branch per task, for example `feat/fs-001-image-guards`. This is a proposed
branch name, not a branch already created. If both tools work simultaneously, use
separate worktrees/clones. Switching a branch in a shared folder changes both
tools' view. In one folder, stop the implementation agent and save/commit its
work before review/cleanup edits begin. Preserve all unrelated uncommitted work.

Confirm branch ownership at each writing handoff. Read-only review can occur
while another checkout is edited, but final approval applies to one exact commit.

## Lifecycle

`Ready -> Implementing -> Review -> Changes requested / Cleanup -> Verified -> Merged`

Record task ID, owner, branch, base commit, implementation commit and reviewed
commit in the issue/PR. Update roadmap status when work is actually assigned.
A passing test run is evidence, not an automatic merge.

## Implementation handoff template

```text
Task ID:
Branch and commit:
Base commit:
What changed and why:
Files changed:
Tests/commands and results:
Dependencies added/removed:
Binary size/memory impact (measured / not measured):
Limitations and decisions needed:
Files safe for Codex to edit:
Implementation agent stopped writing: yes/no
```

No automatic cross-tool messaging is configured. The maintainer passes the
handoff to this chat or supplies a PR link.

## Review and cleanup

- Begin with the diff and acceptance criteria; separate blockers from preferences.
- Block on reproducible data loss, corruption, security failures or broken
  supported behavior. Track less urgent improvements as follow-ups.
- Prefer existing library APIs and deleting unnecessary code. Measure before
  optimizing. Keep cleanup in a small attributable follow-up commit.
- Test behavior at risk, not every implementation detail.
- Recheck the combined diff and relevant tests after cleanup.
- Create a new task when scope grows rather than silently widening the PR.

For every added dependency record purpose, enabled features, license, platforms,
alternatives and size effect. Useful large engines can be optional packs. Avoid
parallel engines for one operation unless measurements justify them.

## Source of truth

- [ROADMAP.md](ROADMAP.md): ordering, task IDs and milestone gates.
- [CAPABILITIES-AND-DOWNLOADS.md](CAPABILITIES-AND-DOWNLOADS.md): install design.
- [ANTIGRAVITY-HANDOFF.md](ANTIGRAVITY-HANDOFF.md): first implementation brief.
- [REVIEW-CHECKLIST.md](REVIEW-CHECKLIST.md): review and cleanup queue.

Update these documents when decisions change. Keep proposed commands labelled
as proposed until released code implements them.
