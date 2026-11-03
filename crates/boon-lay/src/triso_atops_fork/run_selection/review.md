# Review record: crates/boon-lay/src/triso_atops_fork/run_selection

```toml
[kovan]
id = "review-record-crates-boon-lay-src-triso-atops-fork-run-selection"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: normalise_nuclide_name (github:theodoreOnzGit)

```toml
[kovan]
id = "review-normalise-nuclide-name-theodoreonzgit-3292f77e"
kind = "review"
origin = "human"
created = "2026-11-03T14:51:31+08:00"
modified = "2026-11-03T14:51:31+08:00"
target = "fn:3292f77e069a4060"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_selection/mod.rs::normalise_nuclide_name"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T14:51:32+08:00"
commit = "e6f0fcff35aee1393a8342d6b278fdbbce2581de"
hash = "sha256:6f1cce1318935d762a5e95055133dcd60b18d8b2d565afcb4649635a43a0d0db"
doc_hash = "sha256:8ba7cbafe9285b5b5618d182a98ed1776e881434530ffd03e58e7c2924bf6cc6"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "plumbing"

[review.callees]
"fn:4ef295f5b3d28b64" = "sha256:48e95438f69cba28a4bf0a211b732c7dbe4fabe11f86934ecbc07520522f3496"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "returns_result"
independence = "someone_else"
limits_and_guards = "guarded_returns_result"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "AgqgGEShI38dggt1RZYq6H1f/orz4S2I3eqRj9oqH8ghWfnTDU22WIzggDumCaM0yuJFb9ttDVU/JVhAqx3PAg=="
```

this is data utility, parser

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit e6f0fcff35.
