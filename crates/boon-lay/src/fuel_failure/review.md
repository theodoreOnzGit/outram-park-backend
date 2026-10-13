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

# Review: SicLayer (github:theodoreOnzGit)

```toml
[kovan]
id = "review-siclayer-theodoreonzgit-5b2b33cb"
kind = "review"
origin = "human"
created = "2026-10-13T13:14:51+08:00"
modified = "2026-10-13T13:30:46+08:00"
target = "fn:5b2b33cb94ca9a56"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T13:30:46+08:00"
commit = "6d236fd247775aaa46f7182c577ec0c853327726"
hash = "sha256:ad3ca5f101a93f4b16f5b38275849a22be42f0adb309c3e2db7483022a40cc92"
doc_hash = "sha256:906929f0b67869b164723095932403455a17afd7412cfaa864313375a0637108"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_holds_extra = "no"
item_impossible_values = "no_checked_when_built"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "xLqcdWEBZi1mgBvz5vei2104j8UNVdwIcgXjpNgWYdFuaHxWkj83mN7jE1EXuz1Wc+KboMeOMsY8NrSgdnpeCg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel"
kind = "implements"
```

Agentically fixed and checked with respect to guards

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 6d236fd247.

# Review: SicLayerError (github:theodoreOnzGit)

```toml
[kovan]
id = "review-siclayererror-theodoreonzgit-f4afa171"
kind = "review"
origin = "human"
created = "2026-10-13T13:32:49+08:00"
modified = "2026-10-13T13:32:49+08:00"
target = "fn:f4afa171b919e323"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayerError"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T13:32:49+08:00"
commit = "00c35bca8d4bd80a8a7a0cbaa570d2dbcaa70f7b"
hash = "sha256:cac43f9f475f58b14059cc80467030488465fb3e65af4bb412a8bd7492080689"
doc_hash = "sha256:c8e79ddd48a039ad8f13f9692ee0b6c42d3d71225563929b57abe5e3988f96aa"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"
no_concept = "plumbing"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_cases_complete = "other: Guard looks good tentatively. But haven't checked for exhaustive cases, i think it's overkill"
item_holds_extra = "no"
item_impossible_values = "no_checked_when_built"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "S2PLpNm6s0JbaQvlYXSYYyomSH/94o8pzcP6LM0Vk2OwiO+CuOAFIJy9ff67RImuMQxoqVhEekZ7Qk55NXlSAw=="
```

just an error type, now with guards

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 00c35bca8d.

# Review: SicLayer::new (github:theodoreOnzGit)

```toml
[kovan]
id = "review-new-theodoreonzgit-c57b58c2"
kind = "review"
origin = "human"
created = "2026-10-13T13:34:45+08:00"
modified = "2026-10-13T13:34:45+08:00"
target = "fn:c57b58c2e76f63ae"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::new"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T13:34:45+08:00"
commit = "ace4d205a04647fc8639d1a8e4c3602c908c4194"
hash = "sha256:6a4876a9539dd7eabd2cb91002663c2858c4ad50c382d19a5fbe05bcbf988bcf"
doc_hash = "sha256:657efa4d19b327c50f93f98cb7ff77f9bece00c062e190d80e00eb86b30575e3"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"
no_concept = "upstream_structure"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:ad3ca5f101a93f4b16f5b38275849a22be42f0adb309c3e2db7483022a40cc92"
"fn:f4afa171b919e323" = "sha256:cac43f9f475f58b14059cc80467030488465fb3e65af4bb412a8bd7492080689"

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
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "vBvvLpfAo276tslnq+22RIJEkk8xujFluqeVgzTA8k+soJMDo/KrFcpO/93jMQh/WdbfLlw799hDs4DTmFYGBg=="
```

This is a constructor with error handling and result handling, prevents inner radius from being more than outer radius

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit ace4d205a0.

# Review: layer (github:theodoreOnzGit)

```toml
[kovan]
id = "review-layer-theodoreonzgit-29fd23a4"
kind = "review"
origin = "human"
created = "2026-10-13T13:38:53+08:00"
modified = "2026-10-13T13:40:59+08:00"
target = "fn:29fd23a48ca756df"

[review]
path = "crates/boon-lay/src/fuel_failure/stress.rs::layer"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T13:40:59+08:00"
commit = "30c0ddb708c40ef1f38d14332b35856ee9cab089"
hash = "sha256:a5bc0eaa4dfd6ab6d8cb39a94d1ab0d35058f689bd5c7fd1d3cffe785cab947a"
doc_hash = "sha256:ee70dc49f575765ba59e8dc8aa1f3e934d4096b672da37dd4cfd18a815b974e0"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"
no_concept = "plumbing"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:ad3ca5f101a93f4b16f5b38275849a22be42f0adb309c3e2db7483022a40cc92"
"fn:c57b58c2e76f63ae" = "sha256:6a4876a9539dd7eabd2cb91002663c2858c4ad50c382d19a5fbe05bcbf988bcf"

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

[review.signature]
key = "k1"
alg = "ed25519"
value = "4kiioejFqg3WGZT7QFfZDYFpCD1b9IeeZTW/iKjvImLhxtHbXHaEBDUblWTS4cuGcgKyN1PybE/WpE1ukPj1AQ=="
```

representative layer for smoke tests, not based on triso particle dimensions:
NUREG/CR-7107, Table 2.10 — Fuel particle data for HTR-10 fuel.
inner layer is 390 microns, outer layer is 425 microns

This is okay because layer is a private function, never gets out beyond the module. Just used for smoke tests

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 30c0ddb708.

# Review: SicLayer::cbrt_mean_radius (github:theodoreOnzGit)

```toml
[kovan]
id = "review-cbrt-mean-radius-theodoreonzgit-2b071265"
kind = "review"
origin = "human"
created = "2026-10-13T13:42:38+08:00"
modified = "2026-10-13T13:42:38+08:00"
target = "fn:2b07126502aff09a"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::cbrt_mean_radius"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T13:42:38+08:00"
commit = "f5aae16042e09e13b794fcc44ebe54a17bd45ba4"
hash = "sha256:6e0cc2c8f38953ed77c6c5ae6e96a20c995869623b64610b219fc6b3a9536e09"
doc_hash = "sha256:06721c64aa730efe26eecb58063b6785470b0de8e279edb2af5142d7a15f7890"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:ad3ca5f101a93f4b16f5b38275849a22be42f0adb309c3e2db7483022a40cc92"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "cannot_happen: Construction of SiCLayer is already guarded, won't have anything wrong"
independence = "someone_else"
limits_and_guards = "guarded_returns_result"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "lwS88BoSf7TxVMwUBVDco8s0j9otpJ8I0QaWHCKOCEOLRAI5Z2xyZTT6NnaIiHe6qEnMCDNuBXm17q5iVBAYBQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

I have verified the cube root mean equation on page 484

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit f5aae16042.

# Review: SicLayer::inner_radius (github:theodoreOnzGit)

```toml
[kovan]
id = "review-inner-radius-theodoreonzgit-4b866c63"
kind = "review"
origin = "human"
created = "2026-10-13T13:43:59+08:00"
modified = "2026-10-13T13:43:59+08:00"
target = "fn:4b866c63ca3b0c52"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::inner_radius"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T13:43:59+08:00"
commit = "f006756abc128c92d0c04db6f8b20673e2ee85a5"
hash = "sha256:ffd77532a13d71570ed4abfd96d862326985edb735a43ebb38d8602172fe4ae4"
doc_hash = "sha256:9621f3edd59f767ddb4c4ccfcc52a8b0281dfea7c877aecde8292e15d0942db9"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"
no_concept = "generic_numerics"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:ad3ca5f101a93f4b16f5b38275849a22be42f0adb309c3e2db7483022a40cc92"

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
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "vWIPqtnNnjkC4k0kMnQh5B1lFA/L7RNmrlbGV37l/K+c8ViHtP7+mXJ3wWCf5NYilucPQTLZJsf/aMoFPRZJAg=="
```

Just a function getter for inner radius

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit f006756abc.

# Review: SicLayer::outer_radius (github:theodoreOnzGit)

```toml
[kovan]
id = "review-outer-radius-theodoreonzgit-687b102a"
kind = "review"
origin = "human"
created = "2026-10-13T13:45:09+08:00"
modified = "2026-10-13T13:45:09+08:00"
target = "fn:687b102a57f570d9"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::outer_radius"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T13:45:09+08:00"
commit = "0b509581991a54eb79e6d05cc2582976fb50ab3c"
hash = "sha256:935e0917fd06aff0afb7130e62d19601da998c566f5d6517b6215dc4560eb136"
doc_hash = "sha256:01bb0209c5cc7e043bfbc2c601d65ed01ed01f468b290c307cf6035835574d04"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"
no_concept = "generic_numerics"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:ad3ca5f101a93f4b16f5b38275849a22be42f0adb309c3e2db7483022a40cc92"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "cannot_happen: guarded through constructor"
independence = "someone_else"
limits_and_guards = "no_limits: Already guarded thru constructor"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "T5322spFdvIfwzcg42DNgGd+SUkC4mqXFrXesE20mlSr37kkxxdVCctfD3pvC1o0Of8fl2MXjJ1ZZboPEQzFAg=="
```

getter for outer radius

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 0b50958199.

# Review: SicLayerError::fmt (github:theodoreOnzGit)

```toml
[kovan]
id = "review-fmt-theodoreonzgit-8b48ef30"
kind = "review"
origin = "human"
created = "2026-10-13T13:46:45+08:00"
modified = "2026-10-13T13:46:45+08:00"
target = "fn:8b48ef30aabd6ed1"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayerError::fmt"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T13:46:46+08:00"
commit = "a23162e19ef91a2519bf111f2beb9fd1fca953f1"
hash = "sha256:8ffe22a3277f083e611ebed49974ed8a61ffc1e7b5b01c584718b2a5672a2067"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"
no_concept = "io_format"

[review.callees]
"fn:f4afa171b919e323" = "sha256:cac43f9f475f58b14059cc80467030488465fb3e65af4bb412a8bd7492080689"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "no_doc"
error_handling = "returns_result"
independence = "someone_else"
limits_and_guards = "guarded_returns_result"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "other: formatter"
vv_evidence = "other: just formatting"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "PzQlpNNJXIP4k3IjpRDgXLcrH6fkmHP7vwVLe8f8ZcpWNhibM/qgmBBM32kCjL5AJlHW57zyQP/PIEP5YSBgBg=="
```

helps return useful error outpur

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit a23162e19e.
