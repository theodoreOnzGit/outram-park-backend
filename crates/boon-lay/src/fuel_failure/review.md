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
modified = "2026-10-13T11:35:35+08:00"
target = "fn:351b95e01b6d3f77"

[review]
path = "crates/boon-lay/src/fuel_failure/weibull.rs::failure_is_monotone_in_stress"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T11:35:35+08:00"
commit = "c0187fd2247ea1d32dc5033e874b81c6d4c9f9d9"
hash = "sha256:56e275f8814a0f56baa77d6d623c89c513c53a828e17c155ff378b18ba7ff1cb"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"

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

[review.signature]
key = "k1"
alg = "ed25519"
value = "U49OiOVvGNdqbMCurvzSoEI12xvN88M4pCIXFIX/65xtu4psTYfujqaOOOcEEwp4LD6/hSJTkamvbDXNv9X6Cg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit c0187fd224.

# Review: median_is_the_scale_parameter (github:theodoreOnzGit)

```toml
[kovan]
id = "review-median-is-the-scale-parameter-theodoreonzgit-81bac657"
kind = "review"
origin = "human"
created = "2026-10-13T11:46:02+08:00"
modified = "2026-10-13T11:46:02+08:00"
target = "fn:81bac657a46a4b00"

[review]
path = "crates/boon-lay/src/fuel_failure/weibull.rs::median_is_the_scale_parameter"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T11:46:02+08:00"
commit = "9310d33f562c2cb25a2ed9e561e23596c9cbe908"
hash = "sha256:4e214aa2d7bff7f54dd7ba87724af2f77bcdcdb2f86eceef60680c9fe5d2a6be"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"

[review.callees]
"fn:dd41ff2953124783" = "sha256:909caa1fbab166b538d958e5778a9e4b2bbb0ac2b44faa8f50ef9c0624a91539"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "no_doc"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"

[review.signature]
key = "k1"
alg = "ed25519"
value = "Kob1Ie9PVzqoqdjrvLduUoXXHyxmFcmBjnThQQ5oqd3pDyF0mpK60RThoCXY9o+SwEcYjnPE2kIqgl84sF1oCg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

Just a test which at the median (centre part of the S curve), this no matter what the shape parameter (m),
The median (s) will always determine the 0.5 mark. This shld be standard for weibull distribution.

Hence, when the pressure is supplied at median (s), it will always return 0.5

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 9310d33f56.

# Review: FailureFraction (github:theodoreOnzGit)

```toml
[kovan]
id = "review-failurefraction-theodoreonzgit-91b8101e"
kind = "review"
origin = "human"
created = "2026-10-13T11:47:44+08:00"
modified = "2026-10-13T11:48:50+08:00"
target = "fn:91b8101ebd5b4ec6"

[review]
path = "crates/boon-lay/src/fuel_failure/mod.rs::FailureFraction"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T11:48:50+08:00"
commit = "c4f5d60bf433463f35c0e9d331210933167e2d39"
hash = "sha256:e25ea516086e0a8fbc02c8ea06250f66767715f97c79fe584e00cef7420d6abe"
doc_hash = "sha256:f96f0f82938286ec4ddf55657d44b14840c3233836fd56da0f822582bb467c2a"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"
no_concept = "plumbing"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.signature]
key = "k1"
alg = "ed25519"
value = "98BjGHyXc4iH4FcTNfcGIYUaH7pf8VncXAXlOFp3Bg2mC+bY4n4FODPjIEU3y/zaMzWwy8BFw9HWRw4kJaZ5Cg=="
```

Feels a bit redundant though, Ratio will suffice. But I suppose this makes it more readable?
And traceable compared with upstream code? or TRISO ATOPS/PANAMA (though we didn't translate
panama, just read docs and let Claude do the implementation)

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit c4f5d60bf4.

# Review: a_larger_modulus_sharpens_the_transition (github:theodoreOnzGit)

```toml
[kovan]
id = "review-a-larger-modulus-sharpens-the-transition-theodoreonzgit-6e81554d"
kind = "review"
origin = "human"
created = "2026-10-13T11:51:30+08:00"
modified = "2026-10-13T11:51:30+08:00"
target = "fn:6e81554d3518df88"

[review]
path = "crates/boon-lay/src/fuel_failure/weibull.rs::a_larger_modulus_sharpens_the_transition"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T11:51:30+08:00"
commit = "c61a1a0a7aca669432886fd611fb13f212eb1e3e"
hash = "sha256:6742eb3ad47c54c18cc85b2a0de3997c2571eed14d9e205d2701c5fdc05e022c"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"

[review.callees]
"fn:dd41ff2953124783" = "sha256:909caa1fbab166b538d958e5778a9e4b2bbb0ac2b44faa8f50ef9c0624a91539"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "no_doc"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"

[review.signature]
key = "k1"
alg = "ed25519"
value = "KVa2m7ynwpylarNAUPk6QmuWB6uBEv2HfslpIcEtve5oD5pFY5zQ9XVNV9NkNN4efeGYhHuKFRijOrmkpYGbBw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

Smoke test really, nothing much here. Good to have anyway

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit c61a1a0a7a.
