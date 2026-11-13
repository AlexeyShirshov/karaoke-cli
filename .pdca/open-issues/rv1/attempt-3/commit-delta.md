# Commit delta and post-commit checks

The command outputs below were generated against the exact staged tree `e3ea8864e5d2043f542ead22d44fb4c70191da9d` (all 10 staged paths are under `.pdca/`). After the single evidence commit, the corresponding commands using `HEAD` were run and compared to these outputs. The evidence commit does not change any product path.

## `git diff --name-only 624d456 HEAD` (exit 0; stdout follows)

```text
.pdca/open-issues/rv1/attempt-2/post-commit-git-facts.md
.pdca/open-issues/rv1/attempt-3/README.txt
.pdca/open-issues/rv1/attempt-3/commit-delta.md
.pdca/open-issues/rv1/attempt-3/dependency-versions.txt
.pdca/open-issues/rv1/attempt-3/fix.diff
.pdca/open-issues/rv1/attempt-3/post-commit-status.porcelain
.pdca/open-issues/rv1/attempt-3/regression-assertions.md
.pdca/open-issues/rv1/attempt-3/snapshots/SHA256SUMS
.pdca/open-issues/rv1/attempt-3/snapshots/build.rs
.pdca/open-issues/rv1/attempt-3/snapshots/src/main.rs
.pdca/status/open-issues-1.md
```

## `git diff --name-only ebb1452 HEAD` (exit 0; stdout follows)

```text
.pdca/open-issues/rv1/attempt-2/cargo-fmt-apply.command
.pdca/open-issues/rv1/attempt-2/cargo-fmt-apply.environment
.pdca/open-issues/rv1/attempt-2/cargo-fmt-apply.exit
.pdca/open-issues/rv1/attempt-2/cargo-fmt-apply.result
.pdca/open-issues/rv1/attempt-2/cargo-fmt-apply.stderr
.pdca/open-issues/rv1/attempt-2/cargo-fmt-apply.stdout
.pdca/open-issues/rv1/attempt-2/cargo-fmt-check.command
.pdca/open-issues/rv1/attempt-2/cargo-fmt-check.environment
.pdca/open-issues/rv1/attempt-2/cargo-fmt-check.exit
.pdca/open-issues/rv1/attempt-2/cargo-fmt-check.result
.pdca/open-issues/rv1/attempt-2/cargo-fmt-check.stderr
.pdca/open-issues/rv1/attempt-2/cargo-fmt-check.stdout
.pdca/open-issues/rv1/attempt-2/cargo-fmt.command
.pdca/open-issues/rv1/attempt-2/cargo-fmt.environment
.pdca/open-issues/rv1/attempt-2/cargo-fmt.exit
.pdca/open-issues/rv1/attempt-2/cargo-fmt.result
.pdca/open-issues/rv1/attempt-2/cargo-fmt.stderr
.pdca/open-issues/rv1/attempt-2/cargo-fmt.stdout
.pdca/open-issues/rv1/attempt-2/cargo-test.command
.pdca/open-issues/rv1/attempt-2/cargo-test.environment
.pdca/open-issues/rv1/attempt-2/cargo-test.exit
.pdca/open-issues/rv1/attempt-2/cargo-test.result
.pdca/open-issues/rv1/attempt-2/cargo-test.stderr
.pdca/open-issues/rv1/attempt-2/cargo-test.stdout
.pdca/open-issues/rv1/attempt-2/env-info.command
.pdca/open-issues/rv1/attempt-2/env-info.environment
.pdca/open-issues/rv1/attempt-2/env-info.exit
.pdca/open-issues/rv1/attempt-2/env-info.result
.pdca/open-issues/rv1/attempt-2/env-info.stderr
.pdca/open-issues/rv1/attempt-2/env-info.stdout
.pdca/open-issues/rv1/attempt-2/git-diff-check.command
.pdca/open-issues/rv1/attempt-2/git-diff-check.environment
.pdca/open-issues/rv1/attempt-2/git-diff-check.exit
.pdca/open-issues/rv1/attempt-2/git-diff-check.result
.pdca/open-issues/rv1/attempt-2/git-diff-check.stderr
.pdca/open-issues/rv1/attempt-2/git-diff-check.stdout
.pdca/open-issues/rv1/attempt-2/post-commit-git-facts.md
.pdca/open-issues/rv1/attempt-3/README.txt
.pdca/open-issues/rv1/attempt-3/commit-delta.md
.pdca/open-issues/rv1/attempt-3/dependency-versions.txt
.pdca/open-issues/rv1/attempt-3/fix.diff
.pdca/open-issues/rv1/attempt-3/post-commit-status.porcelain
.pdca/open-issues/rv1/attempt-3/regression-assertions.md
.pdca/open-issues/rv1/attempt-3/snapshots/SHA256SUMS
.pdca/open-issues/rv1/attempt-3/snapshots/build.rs
.pdca/open-issues/rv1/attempt-3/snapshots/src/main.rs
.pdca/open-issues/rv1/check-r1/diff-stat.txt
.pdca/open-issues/rv1/check-r1/diff.patch
.pdca/open-issues/rv1/check-r1/fmt-check-status.txt
.pdca/open-issues/rv1/check-r1/fmt-check.log
.pdca/open-issues/rv1/check-r1/full-test-status.txt
.pdca/open-issues/rv1/check-r1/full-test.log
.pdca/open-issues/rv1/check-r1/git-facts.md
.pdca/open-issues/rv1/check-r1/issues.json
.pdca/open-issues/rv1/check-r1/issues.md
.pdca/open-issues/rv1/check-r1/snapshot/Cargo.lock
.pdca/open-issues/rv1/check-r1/snapshot/Cargo.toml
.pdca/open-issues/rv1/check-r1/snapshot/HEAD
.pdca/open-issues/rv1/check-r1/snapshot/README.md
.pdca/open-issues/rv1/check-r1/snapshot/SHA256SUMS
.pdca/open-issues/rv1/check-r1/snapshot/build.rs
.pdca/open-issues/rv1/check-r1/snapshot/data/models.json
.pdca/open-issues/rv1/check-r1/snapshot/docs/dependency-audit.md
.pdca/open-issues/rv1/check-r1/snapshot/src/catalog.rs
.pdca/open-issues/rv1/check-r1/snapshot/src/lib.rs
.pdca/open-issues/rv1/check-r1/snapshot/src/main.rs
.pdca/open-issues/rv1/check-r1/snapshot/src/pipeline.rs
.pdca/open-issues/rv1/check-r1/snapshot/tests/fixtures/model-catalog.json
.pdca/open-issues/rv1/check-r1/snapshot/tests/flow.rs
.pdca/status/open-issues-1.md
build.rs
src/main.rs
```

## `git diff --stat ebb1452..HEAD -- . ':(exclude).pdca'` (exit 0; stdout follows)

```text
 build.rs    | 86 +++++++++++++++++++++++++++++++++----------------------------
 src/main.rs | 47 +++++++++++++++++++++++++++++----
 2 files changed, 89 insertions(+), 44 deletions(-)
```

The first list is entirely `.pdca/**`; the second contains only the same PDCA paths plus product paths `build.rs` and `src/main.rs`. The non-PDCA diff stat therefore remains the fixed AC-5 delta only.

## `git status --porcelain` (exit 0; exact empty stdout)

The exact empty stdout is represented by the zero-byte file `post-commit-status.porcelain`; after commit the command was run and confirmed empty. The worktree was clean.
