# Post-commit Git facts

Captured after the implementation and attempt-2 evidence commit, at HEAD `624d456f711e3c235a40483338df9196777f88c3`, while the working tree was clean. Working directory: `/home/alex/sources/karaoke-cli`.

## `git rev-parse HEAD` — exit 0

```
624d456f711e3c235a40483338df9196777f88c3
```

## `git status --porcelain` — exit 0; empty stdout

```
```

## `git rev-parse origin/main` — exit 0

```
a1ea0a4cea0d7d19142531d91e91469e731af2e8
```

## `git log --oneline a1ea0a4..HEAD` — exit 0

```
624d456 fix: report candle dependency provenance from lockfile
207037a docs: correct fmt check evidence
a57d256 docs: add CHECK evidence pack r1
ebb1452 docs: persist PDCA cycle evidence and superseded preparation records
92150b8 docs: finalize open-issues cycle status
9a218e2 docs: record open-issues PDCA candidate
e4fe915 feat: specify level processing and residual semantics
1f98eb4 feat: support FLAC and MP3 output formats
253e07a feat: expose named multi-stem separation
8345e3b feat: report runtime diagnostics and verbose timings
74b3e76 test: verify local model catalogue listing
8920434 fix: preserve custom names for transcript sidecars
1ed3cb9 feat: implement karaoke CLI ticket support
```

## `git diff --stat a1ea0a4..HEAD -- . ':(exclude).pdca'` — exit 0

```
 Cargo.lock                        | 843 ++++++++++++++++++++++++++++++++--
 Cargo.toml                        |   8 +-
 README.md                         | 144 +++++-
 build.rs                          |  62 +++
 data/models.json                  |  29 ++
 docs/dependency-audit.md          |  57 +++
 src/catalog.rs                    | 180 ++++++++
 src/lib.rs                        |   7 +
 src/main.rs                       | 875 ++++++++++++++++++++++++++---------
 src/pipeline.rs                   | 937 ++++++++++++++++++++++++++++++++++++++
 tests/fixtures/model-catalog.json |  27 ++
 tests/flow.rs                     | 562 ++++++++++++++++++++++-
 12 files changed, 3453 insertions(+), 278 deletions(-)
```

## `git status` — exit 0

```
On branch main
Your branch is ahead of 'origin/main' by 13 commits.
  (use "git push" to publish your local commits)

nothing to commit, working tree clean
```

## Auditability limits

Git history can audit repository commits and file changes, but it cannot establish historical GitHub issue actions. The available issue JSON/Markdown files are state snapshots, not an event audit trail, and no prior shell/GitHub command audit log was available. No GitHub issue actions or push were performed in this DO.
