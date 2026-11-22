# PLAN_BRANCH_CONSOLIDATION — one master, two remotes, the archives

Status: **local merge done; push and branch cleanup proposed.** The 13-commit
merge is `4e830dd` (2026-11-22), with this plan as the docs commit on top
(`0840e27`); `wizard-options` is the merge tip, one commit behind `master`.
Both remotes are still at `ccad6f4`. This plan covers the remaining push, the
archive tags and the safe deletion of the branches that are already contained
in `master`.

## 1. The scan

| branch | status (tip, vs `master`) | action |
|---|---|---|
| `master` | `4e830dd` + this plan | push to both remotes |
| `wizard-options` | `4e830dd`, 0 ahead / 1 behind | merged ancestor; delete after the push |
| `syntax` | `be4e13a`, 0 ahead / 29 behind | contained; archive tag, then delete |
| `form-table` | `0f3421b`, 7 ahead / 29 behind | rejected experiment; keep, no merge |
| `backup/master-pre-coauthor-cleanup` | `4273fed`, 46 ahead, all patch-equivalent | keep as a backup |
| `backup/syntax-pre-coauthor-cleanup` | `7132edc`, 62 ahead, all patch-equivalent | keep as a backup |
| `backup/wip-pre-coauthor-cleanup` | `a3dea6d`, unrelated root | Oct 23 snapshot; keep, no merge |
| `wip/2026-10-12-snapshot` | `8e2bb29`, unrelated root | same snapshot; keep, no merge |
| `maps` | `563ef32`, orphan | binary-map storage; never touch |

Remote tips: `origin/master` and `github/master` are `ccad6f4`;
`origin/wizard-options` and `github/wizard-options` are the stale `ccad6f4`;
`origin/syntax` and `github/syntax` are `be4e13a`; the `form-table` and
`maps` remotes are at their branch tips.

The 13 commits that landed on `master` locally are the class-residue
fix, the typography surfaces, the closed-spelling audit, the sentence-cache
parity, the follow-up plans and the benchmark suite. There are no merge
commits and `git diff --check` is clean.

## 2. Goals

1. `master` on both remotes is `4e830dd` and CI is green on it.
2. The merged/stale branch names are gone, with their tips preserved as
   archive tags.
3. The rejected experiment (`form-table`), the pre-rewrite backups, the old
   snapshots and the `maps` orphan stay available and are never merged.
4. Nothing rewrites history and nothing force-pushes a shared branch.

## 3. Non-goals

- Merging `form-table`. `PLAN_PROBE_PERF.md` rejects it on size (3.59 MB
  gzip / 22.2 MB raw) and says “No merging of the `form-table` branch”.
- Deleting the backup or snapshot branches. They are the only
  pre-rewrite copies of that history.
- Touching `maps`. It is the orphan branch the build fetch reads; a merge
  would put binary blobs on `master`.
- Rewriting `master` or rebasing the 13 commits. They are already linear and
  contained.

## 4. Preconditions and gates

Before the push:

- [ ] `git status` is clean apart from the user's untracked source zip (move it
      out of the repository or leave it untracked; it is not part of history).
- [ ] `git rev-parse master` is `wizard-options` plus this plan's commit
      (`git merge-base --is-ancestor wizard-options master`).
- [ ] `git merge-base --is-ancestor ccad6f4 master` is true (the ff is real).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --release --workspace`
- [ ] The extension typecheck/unit tests and `python3 tools/check_docs.py`
- [ ] `python3 tools/bench.py selftest` (the benchmark driver still runs)

Record the tips before any deletion:

```bash
git rev-parse master wizard-options syntax form-table > /tmp/branch-tips.txt
git rev-parse origin/master github/master >> /tmp/branch-tips.txt
```

## 5. Phase 1 — push `master`

The two remotes are independent; push and verify each:

```bash
git push origin master
git push github master
git fetch origin
git fetch github
git rev-parse origin/master github/master   # both must equal local master
```

Both pushes are fast-forwards from `ccad6f4`, so no force is needed. If one
remote rejects (credentials, protected branch), fix that remote before
touching any branch; do not rewrite the other.

Gate: both remote `master` refs are `4e830dd` and the CI workflow on the
pushed commit is green (the PR job runs on `push`).

## 6. Phase 2 — archive tags before deletion

Tags preserve the tips if a branch is deleted. Use annotated tags so the date
and reason survive:

```bash
git tag -a archive/syntax-2026-11-19 be4e13a -m "syntax line, contained in master"
git tag -a archive/form-table-2026-11-20 0f3421b -m "rejected exact form table"
git tag -a archive/pre-coauthor-master-2026-11-11 4273fed -m "pre-rewrite master backup"
git tag -a archive/pre-coauthor-syntax-2026-11-12 7132edc -m "pre-rewrite syntax backup"
```

Push only the tags that should be public (`origin` and `github`); the backup
tags can stay local if the history is private. `archive/form-table` is worth
pushing so the rejection is traceable from the remote.

## 7. Phase 3 — delete only what is contained

`syntax` is an ancestor of `master`; `wizard-options` is the 13-commit merge
tip, with only this plan's docs commit on top. Both are safe to delete after
the archive tags and the push:

```bash
git branch -d wizard-options syntax
git push origin --delete wizard-options syntax
git push github --delete wizard-options syntax
```

The remote deletions are optional; keeping `origin/syntax` is harmless. If
there is any doubt, delete local first, keep the remote branches until the
archive tags are pushed and visible.

Keep:

- `form-table` local and remote (rejected experiment, plan reference);
- `backup/*` and `wip/*` (the only pre-rewrite copies; local only);
- `maps` local and remote.

## 8. Phase 4 — verify the final state

```bash
git fetch --all --prune
git branch -vv
git ls-remote --heads origin
git ls-remote --heads github
```

Expected: `master` is the same commit on both remotes; `wizard-options` and
`syntax` are gone; `form-table` and `maps` remain; the archive tags resolve. A
fresh clone must build (the first build fetches the `maps` branch) and pass
the release test gate.

## 9. Rollback

- The pre-push commit is `ccad6f4`; the local reflog keeps it.
- If a remote push was wrong, reset that remote to `ccad6f4` only with an
  explicit decision; a force-push to `master` is a history rewrite.
- A deleted branch is recoverable from its archive tag or the reflog.
- `maps` is never reset; it is the only copy of the binary maps.

## 10. Risks

- **Deleting a remote branch before its tag is pushed.** Tag first, verify
  the tag on the remote, then delete.
- **Two remotes diverging.** Push and verify one at a time; a failure on one
  does not justify rewriting the other.
- **The untracked zip.** It is not in history; do not accidentally commit it
  while cleaning the tree.
- **`form-table` temptation.** It is a measured rejection, not unfinished
  work; the branch stays as an archive.
- **`maps` merge.** Never merge the orphan into `master`; the build fetches it
  as a separate ref.

## 11. Files

- This plan.
- `misc_md/PLAN_PROBE_PERF.md` (the form-table rejection reference).
- No code changes; the branch tips and archive tags are repository metadata.
