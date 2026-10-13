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
modified = "2026-10-13T15:22:44+08:00"
target = "fn:29fd23a48ca756df"

[review]
path = "crates/boon-lay/src/fuel_failure/stress.rs::layer"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:22:44+08:00"
commit = "e37775c96ea355d3c27d7b243de43128499886ae"
hash = "sha256:a5bc0eaa4dfd6ab6d8cb39a94d1ab0d35058f689bd5c7fd1d3cffe785cab947a"
doc_hash = "sha256:ee70dc49f575765ba59e8dc8aa1f3e934d4096b672da37dd4cfd18a815b974e0"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"
no_concept = "plumbing"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"
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
value = "kuoM2E9EHS3gC+F2aoI6sJMbA3w8Ma71jAj05QA/GtEvG3h2BfYGAhMzgD5KnEzHfWve/zifT2CER26l0QBDCg=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit e37775c96e.

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
modified = "2026-10-13T15:02:52+08:00"
target = "fn:fcce2a003b237da0"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::initial_thickness"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:02:52+08:00"
commit = "27c157e24883de29f9caa8f65916a15e7e17a21c"
hash = "sha256:e665814f5470eed89b70cd9e974d70ec2d2ed400481ab8673925c70d414e44b4"
doc_hash = "sha256:0a1635e772bae07a3d0e21bdf5e1820a0bc4be1022588ce829d675d67a17db7d"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"
no_concept = "generic_numerics"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"

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

[review.signature]
key = "k1"
alg = "ed25519"
value = "tvDk0adoQAPe4ISqEASUaArr7Ld9UrdQQ2/jUIzggftMm1ZLKUk4prvSFEmRr6SMdmjIqBVDjAgKj2i40FIHAw=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 27c157e248.

# Review: CorrosionInputError (github:theodoreOnzGit)

```toml
[kovan]
id = "review-corrosioninputerror-theodoreonzgit-62323e76"
kind = "review"
origin = "human"
created = "2026-10-13T15:04:28+08:00"
modified = "2026-10-13T15:04:28+08:00"
target = "fn:62323e762b03b9bf"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::CorrosionInputError"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:04:28+08:00"
commit = "100e8df6b175ac18ae8828e2102e6171eb18f1d9"
hash = "sha256:92583fc305b6c05cfec32e7a87ff1144049b4241fa00cd26f8a19ead15c00e69"
doc_hash = "sha256:a11a990604096ceabc7b47f6e3423499347c6c160220e82c0545ccae83904284"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"
no_concept = "plumbing"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_cases_complete = "yes"
item_holds_extra = "no"
item_impossible_values = "no_checked_when_built"
item_matches_description = "yes"
item_test_use = "other: This is just an error struct"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "THCYF156esughCKN5UVmteIm5H2jLR/2kgQbtKk37kIIEkDnaFKrV1XDs9NXrrDgJP5Oi1n2hfxJFJEwVTYYBA=="
```

error type

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 100e8df6b1.

# Review: CorrosionInputError::fmt (github:theodoreOnzGit)

```toml
[kovan]
id = "review-fmt-theodoreonzgit-e9002514"
kind = "review"
origin = "human"
created = "2026-10-13T15:05:42+08:00"
modified = "2026-10-13T15:05:42+08:00"
target = "fn:e9002514133b6a4c"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::CorrosionInputError::fmt"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:05:42+08:00"
commit = "835c0846577a87108a81d1e80efdbd6bf5b2f7f4"
hash = "sha256:9c52f5b79847b414393dd715f3650bb94037d52e330f6e7bcd260eca579573f7"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"
no_concept = "io_format"

[review.callees]
"fn:62323e762b03b9bf" = "sha256:92583fc305b6c05cfec32e7a87ff1144049b4241fa00cd26f8a19ead15c00e69"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "no_doc"
error_handling = "other: formatting"
independence = "someone_else"
limits_and_guards = "no_limits: formatting"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "other: formatting"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "sCtCD/WCyNR/9scQtgnrFZyj7PupUuGLlJb5LJZzw/epbebpfo+08IIIdoLlsVZuqyIKsVNdAFmRCFVdpRjBDg=="
```

formatting function for error

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 835c084657.

# Review: SicLayer::isothermal_thinning_factor (github:theodoreOnzGit)

```toml
[kovan]
id = "review-isothermal-thinning-factor-theodoreonzgit-85780296"
kind = "review"
origin = "human"
created = "2026-10-13T15:07:25+08:00"
modified = "2026-10-13T15:07:25+08:00"
target = "fn:85780296f48ec680"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::isothermal_thinning_factor"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:07:25+08:00"
commit = "cc8ba965b2a59e7b8f5f7187e725cc692bea8091"
hash = "sha256:c8e5ee6a302c3857ee1377d1b0ab0fdd0d3caf0315d2aea4d482b5cab08bacf5"
doc_hash = "sha256:d84a53ea55504c85e5f3dccf58d3a62ddac9ac308d42b26f6569c104d3a69382"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"
"fn:62323e762b03b9bf" = "sha256:92583fc305b6c05cfec32e7a87ff1144049b4241fa00cd26f8a19ead15c00e69"
"fn:fcce2a003b237da0" = "sha256:e665814f5470eed89b70cd9e974d70ec2d2ed400481ab8673925c70d414e44b4"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "returns_result"
independence = "someone_else"
limits_and_guards = "guarded_returns_result"
maintainability = "yes"
numerical_hazards = "handled"
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
value = "a+V2w1OSNa3PnSSOju6iCYuOW4/cN9c7nhD7KsoCR8tA1ENsMzGeaPHHBDkHl9U2vg8oqsmxafsjZjT//CfnCA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

checked against equation on page 492,  works well

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit cc8ba965b2.

# Review: SicLayer::actual_thickness (github:theodoreOnzGit)

```toml
[kovan]
id = "review-actual-thickness-theodoreonzgit-c110fac8"
kind = "review"
origin = "human"
created = "2026-10-13T15:09:26+08:00"
modified = "2026-10-13T15:09:26+08:00"
target = "fn:c110fac8ce3e9964"

[review]
path = "crates/boon-lay/src/fuel_failure/geometry.rs::SicLayer::actual_thickness"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:09:26+08:00"
commit = "f3bcd907a7a9efa1a55c1c674c51af91138625de"
hash = "sha256:055f7daeb9d4f808015285dd822cd2196733ba27bc5da8ccdab88f9f8638129f"
doc_hash = "sha256:fcc46d4a2475f0a9dc8ec378c1ad4fd945de789808c34d2dce69178f528fb108"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.callees]
"fn:5b2b33cb94ca9a56" = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"
"fn:62323e762b03b9bf" = "sha256:92583fc305b6c05cfec32e7a87ff1144049b4241fa00cd26f8a19ead15c00e69"
"fn:85780296f48ec680" = "sha256:c8e5ee6a302c3857ee1377d1b0ab0fdd0d3caf0315d2aea4d482b5cab08bacf5"
"fn:fcce2a003b237da0" = "sha256:e665814f5470eed89b70cd9e974d70ec2d2ed400481ab8673925c70d414e44b4"

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
value = "wlDtRDhDzNmBl1hbeFOzNGdEE8ml8j81KkgWYQ7aI7VJZY0pmHEZSePouGBo84E+qxwIstipc6F7F/JWEYF0Aw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

checked against eq 7 (isothermal thinning factor), in PANAMA document

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit f3bcd907a7.

# Review: induced_stress_exact (github:theodoreOnzGit)

```toml
[kovan]
id = "review-induced-stress-exact-theodoreonzgit-cc30818a"
kind = "review"
origin = "human"
created = "2026-10-13T15:12:48+08:00"
modified = "2026-10-13T15:12:48+08:00"
target = "fn:cc30818aa7756c33"

[review]
path = "crates/boon-lay/src/fuel_failure/stress.rs::induced_stress_exact"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:12:48+08:00"
commit = "04e00535a9f4d450353751526769826e800abe74"
hash = "sha256:19bcf0ea7122ec816b6337fe91e61431c34361bcd05fc24fcecdf5d6b289e34d"
doc_hash = "sha256:b60114464cd95855e4abd1ee01ea1a827bb30aa96ce3e24cdcb61d7901e9f5b7"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.callees]
"fn:2b07126502aff09a" = "sha256:6e0cc2c8f38953ed77c6c5ae6e96a20c995869623b64610b219fc6b3a9536e09"
"fn:5b2b33cb94ca9a56" = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"
"fn:62323e762b03b9bf" = "sha256:92583fc305b6c05cfec32e7a87ff1144049b4241fa00cd26f8a19ead15c00e69"
"fn:c110fac8ce3e9964" = "sha256:055f7daeb9d4f808015285dd822cd2196733ba27bc5da8ccdab88f9f8638129f"

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
value = "a5olP9MnCYfHzB/+vvJaOEzCzWMjVswUKNc2YlA7t36ovx/JueWJxVMH5ogBMuag1EtCnxpoutGc05JIuYQlAA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

checked against page 484 of PANAMA, works well

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 04e00535a9.

# Review: induced_stress (github:theodoreOnzGit)

```toml
[kovan]
id = "review-induced-stress-theodoreonzgit-c04cf2b3"
kind = "review"
origin = "human"
created = "2026-10-13T15:17:17+08:00"
modified = "2026-10-13T15:17:17+08:00"
target = "fn:c04cf2b3db90cf92"

[review]
path = "crates/boon-lay/src/fuel_failure/stress.rs::induced_stress"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:17:17+08:00"
commit = "af92cf3e651924101b00837124370b51db00c01b"
hash = "sha256:dd8d0e1854a1c7fe6a126c017cd5e6d289aaa4fbe9b8edbd82d62b7108fce2e1"
doc_hash = "sha256:46d013e338a45a1f0160beb8480856e8db7007cd177da0d0ae351ec7b177c7a3"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.callees]
"fn:2b07126502aff09a" = "sha256:6e0cc2c8f38953ed77c6c5ae6e96a20c995869623b64610b219fc6b3a9536e09"
"fn:5b2b33cb94ca9a56" = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"
"fn:62323e762b03b9bf" = "sha256:92583fc305b6c05cfec32e7a87ff1144049b4241fa00cd26f8a19ead15c00e69"
"fn:85780296f48ec680" = "sha256:c8e5ee6a302c3857ee1377d1b0ab0fdd0d3caf0315d2aea4d482b5cab08bacf5"
"fn:fcce2a003b237da0" = "sha256:e665814f5470eed89b70cd9e974d70ec2d2ed400481ab8673925c70d414e44b4"

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
value = "lmuq5mcDTF4PGmOZR9DWagUk7rmUZCOidl0NVTtNAVrhgi+0k9kyESDQ/IB5bpD56cO7ICW5MYpz3ouTHNICBA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

looks like the eq 2 on page 484, implemented properly

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit af92cf3e65.

# Review: induced_stress_with_thinning_factor (github:theodoreOnzGit)

```toml
[kovan]
id = "review-induced-stress-with-thinning-factor-theodoreonzgit-bb12bcfe"
kind = "review"
origin = "human"
created = "2026-10-13T15:21:21+08:00"
modified = "2026-10-13T15:21:21+08:00"
target = "fn:bb12bcfe4e38d67a"

[review]
path = "crates/boon-lay/src/fuel_failure/stress.rs::induced_stress_with_thinning_factor"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:21:21+08:00"
commit = "a84bcd2e36b18e3b65a6b19934a4303989058791"
hash = "sha256:df990056b7c16e15de2eabb7091840c8d26efb6792d907e9ac6021a69bfb7562"
doc_hash = "sha256:b5a62bbfd2d05ee79a50dbde20c2d8a5004ed4ad7ef7f122b717fa4ffdfab467"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.callees]
"fn:2b07126502aff09a" = "sha256:6e0cc2c8f38953ed77c6c5ae6e96a20c995869623b64610b219fc6b3a9536e09"
"fn:5b2b33cb94ca9a56" = "sha256:584f858091dbbbf399ba065790793304f75d430ba242966bd1e4deda1518564c"
"fn:fcce2a003b237da0" = "sha256:e665814f5470eed89b70cd9e974d70ec2d2ed400481ab8673925c70d414e44b4"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "other: If someone uses a negative thinning factor, will mess up equation. marking for improvement. But not worth un-stamping"
independence = "someone_else"
limits_and_guards = "guard_present_range_undocumented"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "not_documented"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "gNBJfNtzikoDXG1HYk0enbSLk0ml6OGQWfQpiwofN9dWKSgootbvqSZu6qRigmzNULXKFx5WZ7FmqiEjBe/+DA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

Guards could be improved for fkor, but the equation implemented is essentially correct. Result can also be used to wrap this. good numerical utility anyway. But not up to QA standards

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit a84bcd2e36.

# Review: a_negative_corrosion_rate_is_refused_by_both_routes (github:theodoreOnzGit)

```toml
[kovan]
id = "review-a-negative-corrosion-rate-is-refused-by-both-routes-theodoreonzgit-1b50c85b"
kind = "review"
origin = "human"
created = "2026-10-13T15:23:57+08:00"
modified = "2026-10-13T15:23:57+08:00"
target = "fn:1b50c85b20be031b"

[review]
path = "crates/boon-lay/src/fuel_failure/stress.rs::a_negative_corrosion_rate_is_refused_by_both_routes"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:23:57+08:00"
commit = "a4a253183e85c863e83d9c336d657a9861f89cf9"
hash = "sha256:bb785b7051ca14f3db8d34b7f85695b83ae87107280ece3669b4a2ceec60fa3c"
doc_hash = "sha256:6f71882f491f8ffc92ed8911f3ecc1bad96a88e86faaab7335bdc857c8e4abed"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.callees]
"fn:29fd23a48ca756df" = "sha256:a5bc0eaa4dfd6ab6d8cb39a94d1ab0d35058f689bd5c7fd1d3cffe785cab947a"
"fn:62323e762b03b9bf" = "sha256:92583fc305b6c05cfec32e7a87ff1144049b4241fa00cd26f8a19ead15c00e69"
"fn:c04cf2b3db90cf92" = "sha256:dd8d0e1854a1c7fe6a126c017cd5e6d289aaa4fbe9b8edbd82d62b7108fce2e1"
"fn:cc30818aa7756c33" = "sha256:19bcf0ea7122ec816b6337fe91e61431c34361bcd05fc24fcecdf5d6b289e34d"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
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
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "qi6YARwiQly/Usrvhptp9mNCP5oyCBfaEEJBdL0kJsb4D8JZEZjGcC6+n/HfPef32gHGTFaWopEXvyidqysKCg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

tests if guards are working properly for nonsense input values

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit a4a253183e.

# Review: the_two_routes_to_the_stress_agree_exactly (github:theodoreOnzGit)

```toml
[kovan]
id = "review-the-two-routes-to-the-stress-agree-exactly-theodoreonzgit-edf75b06"
kind = "review"
origin = "human"
created = "2026-10-13T15:25:41+08:00"
modified = "2026-10-13T15:25:41+08:00"
target = "fn:edf75b062b8f2cd5"

[review]
path = "crates/boon-lay/src/fuel_failure/stress.rs::the_two_routes_to_the_stress_agree_exactly"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:25:41+08:00"
commit = "260c08aa0932809aa8d4d24e38dc249bcd290010"
hash = "sha256:e97229bf787a9f190f74599a33369ba5d76157bbfba083b98f3d9789387ff7c5"
doc_hash = "sha256:2241d0c8ebd9b6be73f4658e666b972f3434534ce6098b14ea128badf924be31"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.callees]
"fn:29fd23a48ca756df" = "sha256:a5bc0eaa4dfd6ab6d8cb39a94d1ab0d35058f689bd5c7fd1d3cffe785cab947a"
"fn:c04cf2b3db90cf92" = "sha256:dd8d0e1854a1c7fe6a126c017cd5e6d289aaa4fbe9b8edbd82d62b7108fce2e1"
"fn:cc30818aa7756c33" = "sha256:19bcf0ea7122ec816b6337fe91e61431c34361bcd05fc24fcecdf5d6b289e34d"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
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
sessions = ["https://claude.ai/code/session_01FwJfxkCwJ6bukAQ2gWpSmw"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "mmEaY9WMKxjcrwsKKRVtjJtlNyWll6RuCZZIjUrsQ5O0eMs+Kpx5yEZsA2PBh0SoBPX2UvK0tYWoWillbtpsBA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

This is a regression test. When passes it ensures that induced stress and induced stress exact are about the same. within some relative tolerance

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 260c08aa09.

# Review: MIN_TENSILE_STRENGTH_MPA (github:theodoreOnzGit)

```toml
[kovan]
id = "review-min-tensile-strength-mpa-theodoreonzgit-28160a5b"
kind = "review"
origin = "human"
created = "2026-10-13T15:28:26+08:00"
modified = "2026-10-13T15:28:26+08:00"
target = "fn:28160a5b4e94007b"

[review]
path = "crates/boon-lay/src/fuel_failure/strength.rs::MIN_TENSILE_STRENGTH_MPA"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:28:26+08:00"
commit = "d63530d7e89d3cde29c4d2b03f724adbd51e0a71"
hash = "sha256:e79a1918ddea29db328b79d1f61ac7a89bbddf2396e70cc1896ed20e40cd870b"
doc_hash = "sha256:52623f996a46bce1811961eec8a0533fb16ea7830d3fecd780f426eba6b5aa1c"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "cited"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"

[review.signature]
key = "k1"
alg = "ed25519"
value = "CaFXlLe4Z8D1KpIUr7vEM/RPd1jYUOls9oz3eh1qz/wNXmOYlgn5gbyJmpZk2YRS266WcDozkV9NO5xpkqwtCQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

Page 494 minimum triso tensile strength, checked

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit d63530d7e8.

# Review: MIN_WEIBULL_MODULUS (github:theodoreOnzGit)

```toml
[kovan]
id = "review-min-weibull-modulus-theodoreonzgit-72601fd8"
kind = "review"
origin = "human"
created = "2026-10-13T15:33:55+08:00"
modified = "2026-10-13T15:33:55+08:00"
target = "fn:72601fd84f0cb0d8"

[review]
path = "crates/boon-lay/src/fuel_failure/strength.rs::MIN_WEIBULL_MODULUS"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:33:55+08:00"
commit = "55ef18de56617cec5f5d6c5152314c3e42a38bbb"
hash = "sha256:1f9d6b2ed6a27c7c42760f1007661908cca5967207c725fca41570ae1129e6d6"
doc_hash = "sha256:7624115720911050e9ac3bb545f86a25864dd6b0d9bf12548994c32a3e29c0fb"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "cited"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"

[review.signature]
key = "k1"
alg = "ed25519"
value = "1+oZ4uelM6m4VB2S2fVqzvzOZi2qhe6U/H0uLFRSXQ+GFmE0ThuYVrQOkMC+xa2GQDLwEs9xQ1xVngdFr2SgAw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

checked against page 494 of PANAMA manual

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 55ef18de56.

# Review: irradiated_weibull_modulus (github:theodoreOnzGit)

```toml
[kovan]
id = "review-irradiated-weibull-modulus-theodoreonzgit-ff16d9bf"
kind = "review"
origin = "human"
created = "2026-10-13T15:39:45+08:00"
modified = "2026-10-13T15:41:09+08:00"
target = "fn:ff16d9bf9f457f13"

[review]
path = "crates/boon-lay/src/fuel_failure/strength.rs::irradiated_weibull_modulus"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-13"
signed_at = "2026-10-13T15:41:09+08:00"
commit = "46c92f0a88e66f3ce25ec4932726d5a1cc15f906"
hash = "sha256:57b7d596ab143da4a49bd6837cf26bb2e0736b7a99b61b3a931b3afeb2c9b180"
doc_hash = "sha256:e9557486f5536531c87d37fd5d7f97a47ce47e4204b8fe4a95073ffe5dee0161"
cargo_lock = "sha256:56a96b33380a80b7d9aacfc685db7ea064a6a38f1820dfe55ac9cdb337227dcf"

[review.callees]
"fn:72601fd84f0cb0d8" = "sha256:1f9d6b2ed6a27c7c42760f1007661908cca5967207c725fca41570ae1129e6d6"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "other: eqn implemented correct, but don't want to void a stamp"
independence = "someone_else"
limits_and_guards = "other: Doesn't guard, it is plain f64 function"
maintainability = "yes"
numerical_hazards = "other: doesn't block stamp, but may want to revisit the non-numerical guards"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.signature]
key = "k1"
alg = "ed25519"
value = "QpSiiFBdZM9wsRLIVQrYwIr8F/fGM2qUCiK54xlfjiNV+WzW2KthVfTnCDH38pLzxojZNX8NfingqCzU1jpYAA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

checked against 9a, not 8a

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-13, commit 46c92f0a88.
