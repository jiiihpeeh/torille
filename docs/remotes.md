# Remotes and keeping GitLab and GitHub in sync

The repository has two remotes:

| remote | URL | role |
|---|---|---|
| `origin` | `git@gitlab.com:jiiihpeeh/torille.git` | primary; `build.rs` and `torille-maps` fetch from it by default |
| `github` | `https://github.com/jiiihpeeh/torille.git` | mirror |

Both remotes must hold the same commit on the **normal branch** and on the
**`maps` orphan branch**. They are updated separately because `maps` is a
single-commit branch that is force-pushed, not merged.

`build.rs` follows `MAPS_REMOTE` (the `origin` remote by default), and falls
back to the package's `repository` URL — the public GitHub mirror — when
`origin` has no `maps` branch. That is the case for a Cargo git dependency,
whose checkout is a cache without the branch.

`master` is the normal branch. `form-table` holds the rejected exact-table
experiment and is not merged; the `backup/*` and `wip/*` branches are local
pre-rewrite copies that are never pushed; deleted branches are kept as
`archive/*` tags.

## Normal branch

Push `master` to both remotes:

```bash
git push origin master
git push github master
```

A one-liner for the common case:

```bash
git push origin master && git push github master
```

Do not use `git push --all github`: it would treat `maps` as an ordinary branch
and would not force-update the single-commit orphan branch.

## `maps` branch

`cargo run -p torille-maps -- publish` force-pushes the regenerated blobs to
`origin` only (`MAPS_REMOTE` selects another named remote, but the tool still
updates one remote per run). Mirror the exact commit to GitHub:

```bash
cargo run --release -p torille-maps -- publish
maps_commit=$(git rev-parse origin/maps)
git push --force github "${maps_commit}:refs/heads/maps"
```

The force is expected: `maps` always has exactly one parentless commit, so the
new commit intentionally replaces the old one.

## Verify both remotes

```bash
git ls-remote --heads origin master maps
git ls-remote --heads github master maps
```

The four commit IDs should form two equal pairs: `origin/master` ==
`github/master`, and `origin/maps` == `github/maps`.

## After a release or a large engine change

1. Run the repository gates.
2. Push the normal branch to both remotes.
3. If any `maps/*.bin.gz` changed, publish and mirror the `maps` branch.
4. Run the `git ls-remote` check above.
