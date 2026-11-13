# Git facts

Captured before evidence commit.

## git rev-parse HEAD

```
ebb145264999aae589be3d6c8f79ee68b2afae90
```

## git rev-parse origin/main

```
a1ea0a4cea0d7d19142531d91e91469e731af2e8
```

## git status --porcelain

```
?? .pdca/open-issues/rv1/check-r1/
```

## git log --oneline a1ea0a4..HEAD

```
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

## git log --name-status --format='%h %s' a1ea0a4..HEAD

```
ebb1452 docs: persist PDCA cycle evidence and superseded preparation records

A	.pdca/open-issues/rv1/attempts/cargo-build-ticket4-cli.log
A	.pdca/open-issues/rv1/attempts/cargo-fetch-1.0.0.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-after-env-info-fix.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-check-final.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-check-ticket6.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-check-ticket7-precommit.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket1-preflight.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket1.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket2-assertion-fix.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket2.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket3.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket4.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket5-residual-api.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket5.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket6-final.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket6-flac-blocksize-fix.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket6.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt-ticket7.log
A	.pdca/open-issues/rv1/attempts/cargo-fmt.log
A	.pdca/open-issues/rv1/attempts/cargo-info-flacenc.log
A	.pdca/open-issues/rv1/attempts/cargo-info-mp3lame-encoder.log
A	.pdca/open-issues/rv1/attempts/cargo-info-mp3lame-sys.log
A	.pdca/open-issues/rv1/attempts/cargo-search-flacenc.log
A	.pdca/open-issues/rv1/attempts/cargo-search-mp3lame.log
A	.pdca/open-issues/rv1/attempts/cargo-test-full-final.log
A	.pdca/open-issues/rv1/attempts/cargo-test-no-run-after-env-info-fix.log
A	.pdca/open-issues/rv1/attempts/cargo-test-no-run-after-flac-debug-fix.log
A	.pdca/open-issues/rv1/attempts/cargo-test-no-run-api-fixture-fixed.log
A	.pdca/open-issues/rv1/attempts/cargo-test-no-run-cmake-policy-min-3.5.log
A	.pdca/open-issues/rv1/attempts/cargo-test-no-run-first-implementation.log
A	.pdca/open-issues/rv1/attempts/cargo-test-no-run-upstream-v1.0-corrected.log
A	.pdca/open-issues/rv1/attempts/cargo-test-no-run-upstream-v1.0.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket1-batch.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket1-preflight-continue.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket2-custom-names-after-assertion-fix.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket2-custom-names.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket2-json-cli-qualified.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket2-json-cli.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket3-catalog-fixture.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket4-cli-unit.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket5-four-stem-and-residual.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket5-four-stem.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-codecs-after-blocksize-fix.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-codecs.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-flags.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket6-output-tests-after-blocksize-fix.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket6-output-tests.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket7-cli-level-flags-qualified.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket7-cli-level-flags.log
A	.pdca/open-issues/rv1/attempts/cargo-test-ticket7-level-semantics.log
A	.pdca/open-issues/rv1/attempts/cargo-update-candle-roformer-1.0.0.log
A	.pdca/open-issues/rv1/attempts/cargo-update-candle-roformer.log
A	.pdca/open-issues/rv1/attempts/cargo-update-direct-codecs.log
A	.pdca/open-issues/rv1/attempts/cli-ticket1-mixed-batch.log
A	.pdca/open-issues/rv1/attempts/cli-ticket1-per-file-preflight.log
A	.pdca/open-issues/rv1/attempts/cli-ticket1-preflight-batch.log
A	.pdca/open-issues/rv1/attempts/cli-ticket2-custom-name.log
A	.pdca/open-issues/rv1/attempts/cli-ticket2-custom-sidecars-after-asset-order-fix.log
A	.pdca/open-issues/rv1/attempts/cli-ticket2-custom-sidecars.log
A	.pdca/open-issues/rv1/attempts/cli-ticket2-reject-json.log
A	.pdca/open-issues/rv1/attempts/cli-ticket2-reject-traversal.log
A	.pdca/open-issues/rv1/attempts/cli-ticket3-builtin-pretty.log
A	.pdca/open-issues/rv1/attempts/cli-ticket3-manifest-filter-limit-json.log
A	.pdca/open-issues/rv1/attempts/cli-ticket4-debug-stage-timings.log
A	.pdca/open-issues/rv1/attempts/cli-ticket4-default-output.log
A	.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-no-weights-after-rebuild.log
A	.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-no-weights.log
A	.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-resolved-paths-after-rebuild.log
A	.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-resolved-paths.log
A	.pdca/open-issues/rv1/attempts/cli-ticket4-error-path-cause.log
A	.pdca/open-issues/rv1/attempts/cli-ticket6-flac-file-identification.log
A	.pdca/open-issues/rv1/attempts/cli-ticket6-output-artifact-sizes.log
A	.pdca/open-issues/rv1/attempts/dependency-direct-versions.log
A	.pdca/open-issues/rv1/attempts/dependency-lock-delta.log
A	.pdca/open-issues/rv1/attempts/git-commit-cycle-status-final.log
A	.pdca/open-issues/rv1/attempts/git-commit-cycle-status.log
A	.pdca/open-issues/rv1/attempts/git-commit-ticket6.log
A	.pdca/open-issues/rv1/attempts/git-commit-ticket7.log
A	.pdca/open-issues/rv1/attempts/git-diff-check-all-pdca.log
A	.pdca/open-issues/rv1/attempts/git-diff-check-before-ticket1-commit.log
A	.pdca/open-issues/rv1/attempts/git-diff-check-ticket2.log
A	.pdca/open-issues/rv1/attempts/git-diff-check-ticket3.log
A	.pdca/open-issues/rv1/attempts/git-diff-check-ticket4.log
A	.pdca/open-issues/rv1/attempts/git-diff-check-ticket5.log
A	.pdca/open-issues/rv1/attempts/git-diff-check-ticket6.log
A	.pdca/open-issues/rv1/attempts/git-diff-check-ticket7-precommit.log
A	.pdca/open-issues/rv1/attempts/git-status-before-status-commit.log
A	.pdca/open-issues/rv1/attempts/git-status-final-metadata.log
A	.pdca/open-issues/rv1/attempts/git-status-final.log
A	.pdca/open-issues/rv1/attempts/git-status-porcelain-final.log
A	.pdca/open-issues/rv1/source/candle-roformer-1.0.0/Cargo.toml
A	.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/config.rs
A	.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/dsp.rs
A	.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/lib.rs
A	.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/resample.rs
A	.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/separator.rs
A	.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/wav.rs
A	.pdca/open-issues/rv1/source/flacenc-0.5.1/Cargo.toml
A	.pdca/open-issues/rv1/source/flacenc-0.5.1/README.md
A	.pdca/open-issues/rv1/source/flacenc-0.5.1/src/config.rs
A	.pdca/open-issues/rv1/source/flacenc-0.5.1/src/lib.rs
A	.pdca/open-issues/rv1/source/flacenc-0.5.1/src/source.rs
A	.pdca/open-issues/rv1/source/mp3lame-encoder-0.3.0/Cargo.toml
A	.pdca/open-issues/rv1/source/mp3lame-encoder-0.3.0/README.md
A	.pdca/open-issues/rv1/source/mp3lame-encoder-0.3.0/src/input.rs
A	.pdca/open-issues/rv1/source/mp3lame-encoder-0.3.0/src/lib.rs
A	.pdca/open-issues/rv1/source/mp3lame-sys-0.2.0/Cargo.toml
A	.pdca/open-issues/rv1/source/mp3lame-sys-0.2.0/README.md
A	.pdca/open-issues/rv1/source/mp3lame-sys-0.2.0/build.rs
A	.pdca/status/karaoke-cli-open-issues.md
A	.pdca/status/karaoke-cli-open-issues/planner-plan-r1.md
A	.pdca/status/karaoke-cli-open-issues/preparation-r1.md
M	.pdca/status/open-issues-1.md
92150b8 docs: finalize open-issues cycle status

M	.pdca/status/open-issues-1.md
9a218e2 docs: record open-issues PDCA candidate

A	.pdca/status/open-issues-1.md
e4fe915 feat: specify level processing and residual semantics

M	README.md
M	docs/dependency-audit.md
M	src/main.rs
M	tests/flow.rs
1f98eb4 feat: support FLAC and MP3 output formats

M	README.md
A	docs/dependency-audit.md
M	src/main.rs
M	src/pipeline.rs
M	tests/flow.rs
253e07a feat: expose named multi-stem separation

M	src/lib.rs
M	src/pipeline.rs
M	tests/flow.rs
8345e3b feat: report runtime diagnostics and verbose timings

M	src/main.rs
74b3e76 test: verify local model catalogue listing

M	src/catalog.rs
A	tests/fixtures/model-catalog.json
8920434 fix: preserve custom names for transcript sidecars

M	src/main.rs
M	tests/flow.rs
1ed3cb9 feat: implement karaoke CLI ticket support

M	Cargo.lock
M	Cargo.toml
M	README.md
A	build.rs
A	data/models.json
A	src/catalog.rs
M	src/lib.rs
M	src/main.rs
M	src/pipeline.rs
M	tests/flow.rs
```

## git reflog -n 15

```
ebb1452 HEAD@{0}: commit (amend): docs: persist PDCA cycle evidence and superseded preparation records
bd9a95e HEAD@{1}: commit: docs: persist PDCA cycle evidence and superseded preparation records
92150b8 HEAD@{2}: commit: docs: finalize open-issues cycle status
9a218e2 HEAD@{3}: commit: docs: record open-issues PDCA candidate
e4fe915 HEAD@{4}: commit: feat: specify level processing and residual semantics
1f98eb4 HEAD@{5}: commit: feat: support FLAC and MP3 output formats
253e07a HEAD@{6}: commit: feat: expose named multi-stem separation
8345e3b HEAD@{7}: commit: feat: report runtime diagnostics and verbose timings
74b3e76 HEAD@{8}: commit: test: verify local model catalogue listing
8920434 HEAD@{9}: commit: fix: preserve custom names for transcript sidecars
1ed3cb9 HEAD@{10}: commit: feat: implement karaoke CLI ticket support
a1ea0a4 HEAD@{11}: commit: chore: follow the crate to registry Candle 0.11.1
9ec3ed1 HEAD@{12}: commit (initial): feat: karaoke-cli, the file-level front end of candle-roformer
```

## Push status

origin/main still equals a1ea0a4 (a1ea0a4cea0d7d19142531d91e91469e731af2e8); therefore no push of the candidate commits to origin/main occurred.

## Issue-action auditability

Git history can audit repository commits and file changes, but it does not record GitHub issue actions. No shell-history or prior command audit log covering those actions was available to this evidence-gathering run; therefore earlier issue actions cannot be verified here. issues.json/issues.md are current-state snapshots from gh issue view at their recorded UTC retrieval time, not an audit trail of past actions.
