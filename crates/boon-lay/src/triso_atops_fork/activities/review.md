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
modified = "2026-10-29T15:00:54+08:00"
target = "fn:e6c95a55e6568166"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/mod.rs::Activity"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-29"
signed_at = "2026-10-29T15:00:54+08:00"
commit = "58fc040a0b0b71200b9427aee1f74d65bf986b00"
hash = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"
doc_hash = "sha256:f6248642edc573217e7d25f7f668e2940a034f99f8e7b6305ef26a68d7da0440"
cargo_lock = "sha256:f98236dd033e5a874219475b1066e85b1068ae8fb68882c2c9732d8f6bc0f51e"
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

[review.signature]
key = "k1"
alg = "ed25519"
value = "c977fMH5hq8DjfFpBc3CT9Xj+2KS6ysIfltTGjgPbP5IeDDCznnxXWt4MzW4JDROVlC1ra1wiwLHYsVfn4KKBg=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-29, commit 58fc040a0b.

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

# Review: activity_from_atom_count (github:theodoreOnzGit)

```toml
[kovan]
id = "review-activity-from-atom-count-theodoreonzgit-19e5783b"
kind = "review"
origin = "human"
created = "2026-10-29T15:14:07+08:00"
modified = "2026-10-29T15:14:07+08:00"
target = "fn:19e5783b37ef814f"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/mod.rs::activity_from_atom_count"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-29"
signed_at = "2026-10-29T15:14:07+08:00"
commit = "ead303e96d468032a5d9d350add9da2e95667838"
hash = "sha256:ad68db54587104e69bd653cfadd3f834f3d65ded6a28ca0be769331eadbc74e2"
doc_hash = "sha256:b5bf22b5fd7181c754fecaca6efd9b7c08fa418127ded5124eed314f36d2f596"
cargo_lock = "sha256:f98236dd033e5a874219475b1066e85b1068ae8fb68882c2c9732d8f6bc0f51e"

[review.callees]
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:e6c95a55e6568166" = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: wrapper, follows upstream behaviour"
independence = "someone_else"
limits_and_guards = "no_limits: wrapper, follows upstream behaviour"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "s0eIHqyuU2DwjfsTVVYcWIvDwATTTUijF6ZLe1twKQK2+x5dz9O1m9npZU5UQedLSTSiLdKhowAhoD/9LYeeBw=="

[[relation]]
target = "concept:16-nuclear-fuel-cycle/fuel-depletion/decay-data"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-29, commit ead303e96d.

# Review: atom_count_from_activity (github:theodoreOnzGit)

```toml
[kovan]
id = "review-atom-count-from-activity-theodoreonzgit-cef7daa7"
kind = "review"
origin = "human"
created = "2026-10-29T15:15:08+08:00"
modified = "2026-10-29T15:15:08+08:00"
target = "fn:cef7daa77e029e71"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/mod.rs::atom_count_from_activity"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-29"
signed_at = "2026-10-29T15:15:08+08:00"
commit = "a1d3a93e1aa5f7098a210aaef1383ed5999126bd"
hash = "sha256:ac2db408ddbe78a00dbef118ac3cf96734e5998abd38409cd602473c3a347010"
doc_hash = "sha256:60e2c9e7ecab0d92480008c5ef4a399aff576371f0eda497f14a3b9f8ca0f0bc"
cargo_lock = "sha256:f98236dd033e5a874219475b1066e85b1068ae8fb68882c2c9732d8f6bc0f51e"

[review.callees]
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:e6c95a55e6568166" = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: wrapper, follows upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: wrapper, follows upstream behaviour"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "E9Z1QPT/A0734nxlYV759ymh5xrivOwSk+4wHopnoPhN+C+AAMaAs3Sk1HShM0HLhn3r08cLV9ouBU6sFDm6BA=="

[[relation]]
target = "concept:16-nuclear-fuel-cycle/fuel-depletion/decay-data"
kind = "implements"
```

basic equations, make sense. 
Numbers are unitless, f64 is ok

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-29, commit a1d3a93e1a.

# Review: BQ_PER_CI (github:theodoreOnzGit)

```toml
[kovan]
id = "review-bq-per-ci-theodoreonzgit-672b140f"
kind = "review"
origin = "human"
created = "2026-10-29T15:15:58+08:00"
modified = "2026-10-29T15:15:58+08:00"
target = "fn:672b140fc503762f"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/mod.rs::BQ_PER_CI"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-29"
signed_at = "2026-10-29T15:15:58+08:00"
commit = "8e840bfb8d3036e5587075c298ce569c4a299f8f"
hash = "sha256:7cdec66678905becb105d95cc0922974e31363d078e8e18357228304ad8bce5f"
doc_hash = "sha256:3a052dbf0d18b2543d7c5ab9c84831a0dc9c733134382d392f79450c31a25b3e"
cargo_lock = "sha256:f98236dd033e5a874219475b1066e85b1068ae8fb68882c2c9732d8f6bc0f51e"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "original_code"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "amgIJcPeOafXDg52bBMPmY9ryCoEdWv1BhJET22kGaEsXzLEF0T+IsUHxUmbdycaL/XbaadhVLdUHsSVoDdYCQ=="

[[relation]]
target = "concept:16-nuclear-fuel-cycle/fuel-depletion/decay-data"
kind = "implements"
```

ported from TRISO-ATOPS, also from memory, this is correct

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-29, commit 8e840bfb8d.
