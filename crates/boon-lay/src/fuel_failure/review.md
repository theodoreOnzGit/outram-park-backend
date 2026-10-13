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
modified = "2026-10-13T15:00:07+08:00"
target = "fn:dd41ff2953124783"

[review]
path = "crates/boon-lay/src/fuel_failure/weibull.rs::weibull_failure_fraction"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:00:07+08:00"
commit = "55d541bb7dd6743bcd70f1eeb1771edbd396c6ea"
hash = "sha256:909caa1fbab166b538d958e5778a9e4b2bbb0ac2b44faa8f50ef9c0624a91539"
doc_hash = "sha256:52c13390871b8991d976b786d6612153fdd5adfbc946875a3e173157e8b39949"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.callees]
"fn:91b8101ebd5b4ec6" = "sha256:e25ea516086e0a8fbc02c8ea06250f66767715f97c79fe584e00cef7420d6abe"

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
value = "luZ+wSc3fNOkEVaimXDeKdl7ccuoNVhhNY8Jt/6iguHGMPZQYQiMwhwUcGEjVTQhloluaNrHR2YbxTchi2DACQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 55d541bb7d.

# Review: failure_is_monotone_in_stress (github:theodoreOnzGit)

```toml
[kovan]
id = "review-failure-is-monotone-in-stress-theodoreonzgit-351b95e0"
kind = "review"
origin = "human"
created = "2026-10-12T16:47:12+08:00"
modified = "2026-10-13T15:01:24+08:00"
target = "fn:351b95e01b6d3f77"

[review]
path = "crates/boon-lay/src/fuel_failure/weibull.rs::failure_is_monotone_in_stress"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:01:25+08:00"
commit = "e13e29d7f302272fec458a742e0c58964bb76067"
hash = "sha256:56e275f8814a0f56baa77d6d623c89c513c53a828e17c155ff378b18ba7ff1cb"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

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
value = "G2EviF0CODEcbMkmSNxz+MarQGuMTj2o4CZ1T/3SXiUaO1aUy5MxRT4uphsMb5iRH8uaQmn+wImK2brqLr6EDg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit e13e29d7f3.

# Review: median_is_the_scale_parameter (github:theodoreOnzGit)

```toml
[kovan]
id = "review-median-is-the-scale-parameter-theodoreonzgit-81bac657"
kind = "review"
origin = "human"
created = "2026-10-13T11:46:02+08:00"
modified = "2026-10-13T15:01:42+08:00"
target = "fn:81bac657a46a4b00"

[review]
path = "crates/boon-lay/src/fuel_failure/weibull.rs::median_is_the_scale_parameter"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:01:42+08:00"
commit = "b88c329a28beeae6ee2c68797b9ccd2b411ab569"
hash = "sha256:4e214aa2d7bff7f54dd7ba87724af2f77bcdcdb2f86eceef60680c9fe5d2a6be"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

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

[review.signature]
key = "k1"
alg = "ed25519"
value = "rgpT+OJAlgTe2LjXtPN4OFGZvuYBpvd8KXiCNZKq6i/RBWscUJZM9rrMzkT5oP2xpBAApfe+w92cpnuRrIQ/Dw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit b88c329a28.

# Review: FailureFraction (github:theodoreOnzGit)

```toml
[kovan]
id = "review-failurefraction-theodoreonzgit-91b8101e"
kind = "review"
origin = "human"
created = "2026-10-13T11:47:44+08:00"
modified = "2026-10-13T14:27:58+08:00"
target = "fn:91b8101ebd5b4ec6"

[review]
path = "crates/boon-lay/src/fuel_failure/mod.rs::FailureFraction"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T14:27:59+08:00"
commit = "67b8e9e0aecd01a456dd36442c1dbf2c346e64a0"
hash = "sha256:e25ea516086e0a8fbc02c8ea06250f66767715f97c79fe584e00cef7420d6abe"
doc_hash = "sha256:f96f0f82938286ec4ddf55657d44b14840c3233836fd56da0f822582bb467c2a"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"
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
value = "D1xh5F5zhEzZxTT8DsW8UEofCAxBozisIBzFQ1wSJxWi+/DEZ/mXjT/+qM7GbZytzLiRn8DDte1AXsRhmK3XBA=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 67b8e9e0ae.

# Review: a_larger_modulus_sharpens_the_transition (github:theodoreOnzGit)

```toml
[kovan]
id = "review-a-larger-modulus-sharpens-the-transition-theodoreonzgit-6e81554d"
kind = "review"
origin = "human"
created = "2026-10-13T11:51:30+08:00"
modified = "2026-10-13T15:01:07+08:00"
target = "fn:6e81554d3518df88"

[review]
path = "crates/boon-lay/src/fuel_failure/weibull.rs::a_larger_modulus_sharpens_the_transition"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:01:07+08:00"
commit = "c8ddf4a75e30b77fb08e36d08c1c4dc7ebf44efb"
hash = "sha256:6742eb3ad47c54c18cc85b2a0de3997c2571eed14d9e205d2701c5fdc05e022c"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

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

[review.signature]
key = "k1"
alg = "ed25519"
value = "RY1dAxlG0v9tenioXbsQNoMoIPXkCor50nYHNAnmIIkitlT2EVNqWl5LEnIqL40BrPzT9ydqkCJXxpoxLksRCQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit c8ddf4a75e.

# Review: SicLayer (github:theodoreOnzGit)

```toml
[kovan]
id = "review-siclayer-theodoreonzgit-5b2b33cb"
kind = "review"
origin = "human"
created = "2026-10-13T13:14:51+08:00"
modified = "2026-10-13T14:15:24+08:00"
target = "fn:5b2b33cb94ca9a56"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T14:15:24+08:00"
commit = "4db197128453b1fd984507ecf283473f5beb7201"
hash = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"
doc_hash = "sha256:906929f0b67869b164723095932403455a17afd7412cfaa864313375a0637108"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

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

[review.signature]
key = "k1"
alg = "ed25519"
value = "UL7/2x0fFczLOtgbiwPeCqGmXRQIpDoc5EAyuYNz0Csl/fpgQEspJA+3dXwRabRP29TXCc3JWxSeD/L9LyfVBQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 4db1971284.

# Review: SicLayerError (github:theodoreOnzGit)

```toml
[kovan]
id = "review-siclayererror-theodoreonzgit-f4afa171"
kind = "review"
origin = "human"
created = "2026-10-13T13:32:49+08:00"
modified = "2026-10-13T14:29:03+08:00"
target = "fn:f4afa171b919e323"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayerError"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T14:29:03+08:00"
commit = "912b2915a5d1dbda986c85add3ad58fe38e3ffdf"
hash = "sha256:cac43f9f475f58b14059cc80467030488465fb3e65af4bb412a8bd7492080689"
doc_hash = "sha256:c8e79ddd48a039ad8f13f9692ee0b6c42d3d71225563929b57abe5e3988f96aa"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"
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

[review.signature]
key = "k1"
alg = "ed25519"
value = "szCu/BwO9EDZKURjaI4zbXRXb7PHqsB3ABlpy8WxxMT4iOHAeHxXjnyJVzWQtivcfayoRT+W8cAotM2bpTpYCg=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 912b2915a5.

# Review: SicLayer::new (github:theodoreOnzGit)

```toml
[kovan]
id = "review-new-theodoreonzgit-c57b58c2"
kind = "review"
origin = "human"
created = "2026-10-13T13:34:45+08:00"
modified = "2026-10-13T14:31:59+08:00"
target = "fn:c57b58c2e76f63ae"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::new"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T14:31:59+08:00"
commit = "fcbd81ccbe016d7068d517dfce37b8b9481378a5"
hash = "sha256:6a4876a9539dd7eabd2cb91002663c2858c4ad50c382d19a5fbe05bcbf988bcf"
doc_hash = "sha256:657efa4d19b327c50f93f98cb7ff77f9bece00c062e190d80e00eb86b30575e3"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"
no_concept = "upstream_structure"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"
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

[review.signature]
key = "k1"
alg = "ed25519"
value = "ZNr3PeGqS0K8oZt5JDxhKcN3mJJi77UD/SMTH7kstTYcbVt6Gn1sirHiYl48HdkcNV6IYYHkddmsaIEDQhDaBg=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit fcbd81ccbe.

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
modified = "2026-10-13T15:02:15+08:00"
target = "fn:2b07126502aff09a"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::cbrt_mean_radius"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:02:15+08:00"
commit = "3ff8af1ddb0c9cc767d191809dd555caee852c99"
hash = "sha256:6e0cc2c8f38953ed77c6c5ae6e96a20c995869623b64610b219fc6b3a9536e09"
doc_hash = "sha256:06721c64aa730efe26eecb58063b6785470b0de8e279edb2af5142d7a15f7890"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"

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

[review.signature]
key = "k1"
alg = "ed25519"
value = "VaiyMZ1kZANwH+55Nlnw79TjPJnDXwIomPla2NA8X+ukHe6XHhqo1vkCJbrAx5vcqMrQyfW8CaelX2sS+CplCA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 3ff8af1ddb.

# Review: SicLayer::inner_radius (github:theodoreOnzGit)

```toml
[kovan]
id = "review-inner-radius-theodoreonzgit-4b866c63"
kind = "review"
origin = "human"
created = "2026-10-13T13:43:59+08:00"
modified = "2026-10-13T14:29:43+08:00"
target = "fn:4b866c63ca3b0c52"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::inner_radius"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T14:29:43+08:00"
commit = "912b2915a5d1dbda986c85add3ad58fe38e3ffdf"
hash = "sha256:ffd77532a13d71570ed4abfd96d862326985edb735a43ebb38d8602172fe4ae4"
doc_hash = "sha256:9621f3edd59f767ddb4c4ccfcc52a8b0281dfea7c877aecde8292e15d0942db9"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"
no_concept = "generic_numerics"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"

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

[review.signature]
key = "k1"
alg = "ed25519"
value = "07PtCqdbCkIgTsNnyVpfqC1VCLiCMxsHbAvjSPqoCrXfZFFuJwfFXzFaowk7WcCj/7Xq1L3yJ5RwkqbDGJL3BA=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 912b2915a5.

# Review: SicLayer::outer_radius (github:theodoreOnzGit)

```toml
[kovan]
id = "review-outer-radius-theodoreonzgit-687b102a"
kind = "review"
origin = "human"
created = "2026-10-13T13:45:09+08:00"
modified = "2026-10-13T14:30:48+08:00"
target = "fn:687b102a57f570d9"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::outer_radius"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T14:30:48+08:00"
commit = "fcbd81ccbe016d7068d517dfce37b8b9481378a5"
hash = "sha256:935e0917fd06aff0afb7130e62d19601da998c566f5d6517b6215dc4560eb136"
doc_hash = "sha256:01bb0209c5cc7e043bfbc2c601d65ed01ed01f468b290c307cf6035835574d04"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"
no_concept = "generic_numerics"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"

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

[review.signature]
key = "k1"
alg = "ed25519"
value = "dOGMcyLSr1fSNEOUwaBAwXhq++eqJcMfdWFW4KPB7MHo4q7o4lmc0HpH+Sdom+OJ5LEX+H6J+niEsRViZg3JAA=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit fcbd81ccbe.

# Review: SicLayerError::fmt (github:theodoreOnzGit)

```toml
[kovan]
id = "review-fmt-theodoreonzgit-8b48ef30"
kind = "review"
origin = "human"
created = "2026-10-13T13:46:45+08:00"
modified = "2026-10-13T14:30:08+08:00"
target = "fn:8b48ef30aabd6ed1"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayerError::fmt"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T14:30:08+08:00"
commit = "fcbd81ccbe016d7068d517dfce37b8b9481378a5"
hash = "sha256:8ffe22a3277f083e611ebed49974ed8a61ffc1e7b5b01c584718b2a5672a2067"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"
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

[review.signature]
key = "k1"
alg = "ed25519"
value = "uOXjHqeaqX09pxHTBBF6S+bcDMkCtTKKlVqvtuq4H7mE63KntOZ1GXKdPNRz8smMc7Z2I2i8EKqDLMOLMBRfAA=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit fcbd81ccbe.

# Review: SicLayer::initial_thickness (github:theodoreOnzGit)

```toml
[kovan]
id = "review-initial-thickness-theodoreonzgit-fcce2a00"
kind = "review"
origin = "human"
created = "2026-10-13T13:48:21+08:00"
modified = "2026-10-13T13:48:21+08:00"
target = "fn:fcce2a003b237da0"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::initial_thickness"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T13:48:22+08:00"
commit = "bf4cfdc0aa85888153ebcaa785e4fd3a9ed3e806"
hash = "sha256:e665814f5470eed89b70cd9e974d70ec2d2ed400481ab8673925c70d414e44b4"
doc_hash = "sha256:0a1635e772bae07a3d0e21bdf5e1820a0bc4be1022588ce829d675d67a17db7d"
cargo_lock = "sha256:429b87f858825d5074269a2f185f57b1f97af45be1278c87e0a8320a81f2f550"
no_concept = "generic_numerics"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:ad3ca5f101a93f4b16f5b38275849a22be42f0adb309c3e2db7483022a40cc92"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "cannot_happen: constructor alr guards this"
independence = "someone_else"
limits_and_guards = "no_limits: Constructor already guards this"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"

[review.signature]
key = "k1"
alg = "ed25519"
value = "Vuz3BGl5xkwwlBShPZVYRi86N7yuAWimOTifMjTlDGMlWl77g89IMQPNq9gswSTfz3GGB0slAf54b6etTuJaCg=="
```

just returns thickness of outer-inner radius, before corrosion or anything

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit bf4cfdc0aa.
