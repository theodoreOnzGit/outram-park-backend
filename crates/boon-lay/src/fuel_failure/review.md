# Review record: crates/boon-lay/src/fuel_failure

```toml
[kovan]
id = "review-record-crates-boon-lay-src-fuel-failure"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: weibull_failure_fraction (github:theodoreOnzGit)

```toml
[kovan]
id = "review-weibull-failure-fraction-theodoreonzgit-dd41ff29"
kind = "review"
origin = "human"
created = "2026-10-12T09:48:30+08:00"
modified = "2026-10-13T10:52:24+08:00"
target = "fn:dd41ff2953124783"

[review]
path = "crates/boon-lay/src/fuel_failure/weibull.rs::weibull_failure_fraction"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T10:52:25+08:00"
commit = "8b786d76b9e3ee1e984f2ede0df008ca592c2ec4"
hash = "sha256:909caa1fbab166b538d958e5778a9e4b2bbb0ac2b44faa8f50ef9c0624a91539"
doc_hash = "sha256:52c13390871b8991d976b786d6612153fdd5adfbc946875a3e173157e8b39949"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: For this weibull failure fraction, it will panic if induced stress is less than zero, or of the weibull thingy is less than zero. It is really up to the user to use it correctly... But can be improved in future"
independence = "someone_else"
limits_and_guards = "guard_present_range_undocumented"
maintainability = "yes"
numerical_hazards = "handled"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "not_documented"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.signature]
key = "k1"
alg = "ed25519"
value = "3w97DWVex+Quosm+ouZ1jBvb7KD4daZIe+sE9OQODUDscrUPyNmoWA3kkVEep9O3WTNfuxbQDh9DgElvAiaoBg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 8b786d76b9.

# Review: failure_is_monotone_in_stress (github:theodoreOnzGit)

```toml
[kovan]
id = "review-failure-is-monotone-in-stress-theodoreonzgit-351b95e0"
kind = "review"
origin = "human"
created = "2026-10-12T16:47:12+08:00"
modified = "2026-10-12T16:47:12+08:00"
target = "fn:351b95e01b6d3f77"

[review]
path = "crates/boon-lay/src/fuel_failure/weibull.rs::failure_is_monotone_in_stress"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-12"
signed_at = "2026-10-12T16:47:13+08:00"
commit = "eb29c0cb6295a15fe5889e4505c4a28a5e3ce91d"
hash = "sha256:56e275f8814a0f56baa77d6d623c89c513c53a828e17c155ff378b18ba7ff1cb"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:4f41a7d1002fb86dae938001b977e9d208946bedf27faf10f859b5a7fa70eafe"

[review.callees]
"fn:dd41ff2953124783" = "sha256:909caa1fbab166b538d958e5778a9e4b2bbb0ac2b44faa8f50ef9c0624a91539"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: This is a test, it is meant to panic"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: This is a test, it is meant to panic"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "other: This is a test, not applicable"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "other: This is a test"

[review.authorship]
kind = "agent"

[review.signature]
key = "k1"
alg = "ed25519"
value = "fSp1Us/WNMGh9QxYQ+ILJLzDQM2Gc/T3VBmRH9jGVPEbeD85U3KugAOn798GSZV36AxP6grY1YtoW1pA+w33DA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

test function

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-12, commit eb29c0cb62.
