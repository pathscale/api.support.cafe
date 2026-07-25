# WorkTable schema changes and deployment

> **A WorkTable schema change is a deployment event, not just a code change.**
> If the schema moves, upgrading versions and deploying becomes significantly
> more complicated, and the change **must** ship with a WT data migration script.

This repo has **7 `worktable!` definitions**, **6 of them with
`persist: true`** — those are the ones holding on-disk data, and the ones a
schema change can break.

## The rule

**Before any merge → deploy, double-check whether the WorkTable schema changed.**

This check is mandatory and belongs *before* the merge, not during the deploy.
Discovering it at deploy time is the failure mode this document exists to
prevent.

If the schema did change, write the WT data migration script and ship it with
the change. Do not deploy without it.

## What counts as a schema change

Any of these, on a table with `persist: true`:

- adding, removing or renaming a column
- changing a column's type, or its optionality (`optional`)
- adding, removing or altering an index, or changing the primary key
- **bumping the `worktable` crate version** — this can change the on-disk
  format with no source edit at all, which is the easiest case to miss

## How to check

Run these against the branch you are about to merge:

```bash
# 1. Did any worktable! definition change?
git diff <base>..HEAD -- 'src/db/**'

# 2. Did the worktable crate version move? (source-free schema change)
git diff <base>..HEAD -- Cargo.toml Cargo.lock | grep -i worktable

# 3. Which tables actually persist? Only these need a migration.
grep -rn "persist: *true" src/
```

If (1) touches a `persist: true` table, or (2) shows **any** movement, a
migration script is required.

## Precedent — endpoint-libs 2.0 port, 2026-07-25

Checked, and clean: no migration was needed. That port changed only the
`endpoint-libs` and `honey_id-types` versions. `worktable` was verified
untouched in `Cargo.toml`, `Cargo.lock` and all of `src/`.

The point of recording this is not the result but the check. **Do not assume a
dependency upgrade leaves WorkTable alone because it looks unrelated** — a
transitive bump of the `worktable` crate is exactly the kind of thing that
slips through, and it changes on-disk format without touching a single line of
this repo's source.
