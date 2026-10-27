# Review record: crates/boon-lay/src/triso_atops_fork/activities

```toml
[kovan]
id = "review-record-crates-boon-lay-src-triso-atops-fork-activities"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: Activity (github:theodoreOnzGit)

```toml
[kovan]
id = "review-activity-theodoreonzgit-e6c95a55"
kind = "review"
origin = "human"
created = "2026-10-23T13:31:59+08:00"
modified = "2026-10-23T13:31:59+08:00"
target = "fn:e6c95a55e6568166"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/mod.rs::Activity"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-23"
signed_at = "2026-10-23T13:32:00+08:00"
commit = "1e1bc4a6465507676a8cb27c94b5f2190f9cde82"
hash = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"
doc_hash = "sha256:f6248642edc573217e7d25f7f668e2940a034f99f8e7b6305ef26a68d7da0440"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
rung4_test_catches_wrong_physics = "compared_with_tolerance"
rung4_tests_pass = "all_pass"
rung4_tolerance_justified = "from_outside"
rung4_usable_without_source = "from_name_and_description"
rung4_wrong_quantity_by_accident = "no_named_and_unit_carrying"
vv_case_author = "human_wrote_and_verified"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "Ub/efGCZas6U7tEvBkKDs9Ht0ePyIgCowgpq0CIN2CBBazLp0AyuVTQFT1VCaI87J46/zFyYJYu71cXdb8RfAA=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-23, commit 1e1bc4a646.

# Review: OTHER_ATTENUATION_FACTOR (github:theodoreOnzGit)

```toml
[kovan]
id = "review-other-attenuation-factor-theodoreonzgit-a3654ef9"
kind = "review"
origin = "human"
created = "2026-10-27T16:41:14+08:00"
modified = "2026-10-27T16:41:14+08:00"
target = "fn:a3654ef9ead0b08e"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::OTHER_ATTENUATION_FACTOR"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T16:41:15+08:00"
commit = "6d57915a171dd6bbe3a9578daa7f343b945048cc"
hash = "sha256:592d21c9d20158f91f15be8cb7044e39664a0b092598e6c8f8976a45bc511129"
doc_hash = "sha256:34bb960b406d9eaa334e3b1389de2fad1ae2d5dad2e5df14fe7ee9c39096a2a6"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "original_code"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "+wjoTn79ieWzKQX6chC7pi/xr/JFGG2gbrRV44El9ONMQZfZ3E1upuUM/T7rBQN4eXdS3syg4cy+RKd4MQuIAg=="
```

attenuation factor is defined inline for base_activities upstream, checked to be 1e8

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 6d57915a17.

# Review: SourceAndGraphite (github:theodoreOnzGit)

```toml
[kovan]
id = "review-sourceandgraphite-theodoreonzgit-ee87ae97"
kind = "review"
origin = "human"
created = "2026-10-27T16:43:35+08:00"
modified = "2026-10-27T16:43:35+08:00"
target = "fn:ee87ae9710c2fa05"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::SourceAndGraphite"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T16:43:36+08:00"
commit = "c1bf1b3cbe7b7a743f4b5c8e0c48ed521648d9db"
hash = "sha256:9d7ee874d9552f077202594f2437fd8a72a3d2c0b37446ecad11026af97edd94"
doc_hash = "sha256:c9fdcb596cfd2b0a8a12bca63109756297b74ed9cda0b60da56ceedc304671dd"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_holds_extra = "no"
item_impossible_values = "other: just datastructure, not guarding cos it copies upstream"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "other: data structure utility"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "5zrHmidghv4ppuYuaAahhE43xv9gt4tWHhkvbXgu3E07BuKpi9ol7PXUGru9GwL52pAOSKyUp9rpNS40MQYVDQ=="
```

python returns (S,G) for source rate and graphite activity. 
We are returning a struct rather than a tuple in Python. Acceptable

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit c1bf1b3cbe.
