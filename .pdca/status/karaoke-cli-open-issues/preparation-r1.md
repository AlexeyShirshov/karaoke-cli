# Campaign preparation: karaoke-cli-open-issues

Preparation revision: `r1`  
Preparation state: `ready`  
Campaign status: `.pdca/status/karaoke-cli-open-issues.md`  
Preparation reference: `.pdca/status/karaoke-cli-open-issues/preparation-r1.md@r1`  
Original task: `.pdca/status/karaoke-cli-open-issues.md` → `## Original task` (verbatim)  
Complete PLAN source: `.pdca/status/karaoke-cli-open-issues/planner-plan-r1.md` (verbatim, immutable)

## Campaign setup

- Form: `incremental` (`pdca-incremental`); overlay: `pdca-coder`.
- Goal predicate: `satisfied_criteria_count == 7` with no regressions; global acceptance AC-1..AC-7 remains fixed and is reproduced in the linked PLAN source.
- Measurement: count fully completed open-ticket criteria AC-1..AC-7; denominator 7; floor +1 completed ticket (or decision-relevant knowledge); target 7/7. Inclusion/exclusion and comparability conditions are fixed in PLAN r1 §3.
- PLAN-supplied baseline: 0/7 at HEAD `a1ea0a4`, based on the source anchors in PLAN r1 §§1, 3, 5, 10. This preparation did not measure or independently verify it.
- First-step boundary: use the PLAN-supplied baseline as stated; first candidate is a change-producing step using the initial PLAN r1 seed/brief. This handoff does not start DO.
- Allowed surface and protected inputs: karaoke-cli product/tests/docs needed for AC-1..AC-7; existing user changes in Cargo.toml, Cargo.lock and README.md are preserved and not attributed without evidence. The full restrictions are in PLAN r1 §5.
- Commit policy: `auto`, authorized by the explicit request; after acceptance, only accepted step paths are eligible for local commit. No push. No commit was made at preparation.
- Preparation barrier: reached. No runtime status/step, product edit, test run, build or other DO action was started.

## PLAN report reference

The following artifact is the complete PLAN r1 report exactly as returned by `pdca-planner`; it is persisted without translation, summary substitution, or edits. The campaign status stores the immutable original user task in its `## Original task` section.

Exact artifact: `.pdca/status/karaoke-cli-open-issues/planner-plan-r1.md`
