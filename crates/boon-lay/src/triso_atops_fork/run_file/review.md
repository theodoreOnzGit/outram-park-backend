# Review record: crates/boon-lay/src/triso_atops_fork/run_file

```toml
[kovan]
id = "review-record-crates-boon-lay-src-triso-atops-fork-run-file"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: CONSTANT_UNITS (github:theodoreOnzGit)

```toml
[kovan]
id = "review-constant-units-theodoreonzgit-9bb59490"
kind = "review"
origin = "human"
created = "2026-10-27T14:33:14+08:00"
modified = "2026-10-27T14:33:14+08:00"
target = "fn:9bb59490b81dec81"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_file/upstream.rs::CONSTANT_UNITS"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:33:15+08:00"
commit = "a3b9d1a896c6e4cb3307c99d5b5d22a53e222521"
hash = "sha256:5c2b41727d5e8459d4ba83ddfc4fb5bd418b4481e47d807482adf2dd20279fb9"
doc_hash = "sha256:04dce5e42dc9dca6f028893b23f7ea93996503afbc3305c2cf0d2d22fcf2181f"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "original_code"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "other: This is defined inline in upstream port, under const_units, verfied to match upstream exactly"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_012qgX47fwybgAuf2wEiRvBu"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "h5rCz8za3vT3zG5q3QrtivktHHj7/23GDmViUy/WIKFp/tE7HK8lwp2QAOHPA9kYXQsUSYaCWMEzS1Ao7MNoCg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit a3b9d1a896.

# Review: CONSTANT_NAMES (github:theodoreOnzGit)

```toml
[kovan]
id = "review-constant-names-theodoreonzgit-77eb4548"
kind = "review"
origin = "human"
created = "2026-10-27T14:35:15+08:00"
modified = "2026-10-27T14:35:15+08:00"
target = "fn:77eb4548b2a8440f"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_file/upstream.rs::CONSTANT_NAMES"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:35:17+08:00"
commit = "90031329500c0070eee25b6b688590cbb0466031"
hash = "sha256:eb30dd38df4a20a378104a599f92bd158fb892e9ac79b8aaaf60be81c15abe5f"
doc_hash = "sha256:987b52d7e172661f690e20bd54cf32941112490854b6079fa654f935ceb36db8"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

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
sessions = ["https://claude.ai/code/session_012qgX47fwybgAuf2wEiRvBu"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "yyzvTKSi0VI8NtMh4ArVsOb+61jnnpEhlwE2bgnTSg/3gXS2qpkd9NaA1W2r4Wv4ff3ZWc9D1RKFEjrakbjPBg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

combines const_names and accident_constants, concatenated, verified

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 9003132950.

# Review: TimeUnit (github:theodoreOnzGit)

```toml
[kovan]
id = "review-timeunit-theodoreonzgit-7211ef55"
kind = "review"
origin = "human"
created = "2026-10-27T14:39:26+08:00"
modified = "2026-10-27T14:39:26+08:00"
target = "fn:7211ef55974ed7bc"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_file/mod.rs::TimeUnit"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:39:26+08:00"
commit = "b7b65613294e991744911b058c2c5b708d728e81"
hash = "sha256:3b9fff3ac196256b5f0eb77c152443f93ebc5236d4c9a444e8922e2f4b63f60f"
doc_hash = "sha256:b44b50d8469e189e6d814e4dedbfd4013c4c500df7ab6f72cb312746914245e3"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_cases_complete = "yes"
item_holds_extra = "no"
item_impossible_values = "no_checked_when_built"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "JKle1BUdef3JIx1F7yiQF4uxGYa1mBHXtdVbbReRc62Bl+MVFpAGyQOL1XcHDVC/Db9Ha3enY8NiJXDm0GJ4CQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

Confirmed to be okay, follows upstream 

under def: convert time

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit b7b6561329.

# Review: TimeUnit::seconds (github:theodoreOnzGit)

```toml
[kovan]
id = "review-seconds-theodoreonzgit-f5caa9d4"
kind = "review"
origin = "human"
created = "2026-10-27T14:41:58+08:00"
modified = "2026-10-27T14:41:58+08:00"
target = "fn:f5caa9d482058451"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_file/mod.rs::TimeUnit::seconds"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:41:59+08:00"
commit = "1b030b00e61afaad2fe98d4137e1d93bba926a59"
hash = "sha256:3593bac03a446b265a57636ccde1593a5a95861ffe005d72b0b74a42291220fa"
doc_hash = "sha256:638390792b9e563fbe5b74e29a803315376272909cc035eba0063a36aa227717"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:7211ef55974ed7bc" = "sha256:3b9fff3ac196256b5f0eb77c152443f93ebc5236d4c9a444e8922e2f4b63f60f"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "cannot_happen: enum conversion, if matched correctly, supplies a number straight"
independence = "someone_else"
limits_and_guards = "no_limits: this is a conversion factor, that's all"
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
value = "ENNy1B9dTZ1ofPS+k+GZzcR5CUHKPt+dQFJOlYAPb3wSA9GUxxkIrcL4XrxARrtvShjp/rSlhaDK653ZcwqnDg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

Time unit conversion, matches convert_time numerics via enum, much cleaner

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 1b030b00e6.

# Review: TimeUnit::parse (github:theodoreOnzGit)

```toml
[kovan]
id = "review-parse-theodoreonzgit-bb3bb965"
kind = "review"
origin = "human"
created = "2026-10-27T14:43:34+08:00"
modified = "2026-10-27T14:43:34+08:00"
target = "fn:bb3bb965bdd5e28b"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_file/mod.rs::TimeUnit::parse"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:43:35+08:00"
commit = "089852228616056d5b5a37429c6babdd117133bf"
hash = "sha256:48a765403379e66dd4a8549e3e096079126eab35e691976607ba056dd88822b6"
doc_hash = "sha256:8d8f43ef3615f0c40f54794faa82366734540966e5b65f5b88cecc2df637a0ed"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.callees]
"fn:7211ef55974ed7bc" = "sha256:3b9fff3ac196256b5f0eb77c152443f93ebc5236d4c9a444e8922e2f4b63f60f"

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
value = "lJvx7WwjHQFbDeyCiHXzU9LzPrp4apGSWzMx5SQxcb78P/Thcz3gn62H0YgVwJyDDaxZRXzQJCfXCvqS81v3Aw=="
```

unit conversion, matches upstream

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 0898522286.

# Review: TimeUnit::to_time (github:theodoreOnzGit)

```toml
[kovan]
id = "review-to-time-theodoreonzgit-fca9138d"
kind = "review"
origin = "human"
created = "2026-10-27T14:46:53+08:00"
modified = "2026-10-27T14:46:53+08:00"
target = "fn:fca9138d7d203340"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_file/mod.rs::TimeUnit::to_time"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:46:54+08:00"
commit = "1f76da59b4ee1a4e25144af65430a9b65d4f0c08"
hash = "sha256:530a1d93326283c1aae3eb344ff52025409ee276d36989f0933c6c8cb6d2a96d"
doc_hash = "sha256:b57e0d3bf8b29ddd0f90b76e4406a00ef616ab68032a37a47a0099c838965335"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.callees]
"fn:7211ef55974ed7bc" = "sha256:3b9fff3ac196256b5f0eb77c152443f93ebc5236d4c9a444e8922e2f4b63f60f"
"fn:f5caa9d482058451" = "sha256:3593bac03a446b265a57636ccde1593a5a95861ffe005d72b0b74a42291220fa"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: a conversion factor utility"
independence = "someone_else"
limits_and_guards = "no_limits: just a helper to convert to uom"
maintainability = "yes"
numerical_hazards = "other: people expected not to mess this up"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "dvVfpsd7fhMp3pgLC3gNptlH1LtHRzcpPtsZzABMDNoSsj1X05pkDcs2zn7PYp5jYXjKjHMYZCxT9vSGwxCrAw=="
```

unit conversion, that's all

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 1f76da59b4.

# Review: RunFile (github:theodoreOnzGit)

```toml
[kovan]
id = "review-runfile-theodoreonzgit-72d8fa4b"
kind = "review"
origin = "human"
created = "2026-10-27T14:48:44+08:00"
modified = "2026-10-27T14:48:44+08:00"
target = "fn:72d8fa4bc6abed85"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_file/mod.rs::RunFile"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:48:45+08:00"
commit = "c599144516112b151d0e9229a4b0f466432f8a0a"
hash = "sha256:8207b7d66b9e7c72ce2320dddc75f66c09cb93a737d1cfa5d29d19a16d7e0ca3"
doc_hash = "sha256:b7dcc6360eb96dd2e5efba8ecd91ee88ce8746ecb39137d7bce14ebf81ae32c6"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

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
sessions = [
    "https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR",
    "https://claude.ai/code/session_012qgX47fwybgAuf2wEiRvBu",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "tT5MRwheAVSjRHVse/qEWmM5ezBmXDgMkGgGPIjkYMQheUAf1hEiyuqXBW51YREc6zKoXRaQ176uc0F0JIP0CA=="
```

contains well documented structs that a read run file converts into

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit c599144516.

# Review: RunConfig (github:theodoreOnzGit)

```toml
[kovan]
id = "review-runconfig-theodoreonzgit-eed943d2"
kind = "review"
origin = "human"
created = "2026-10-27T14:53:35+08:00"
modified = "2026-10-27T15:51:42+08:00"
target = "fn:eed943d26b55606f"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_file/mod.rs::RunConfig"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T15:51:44+08:00"
commit = "84ab93aebdae374e9013d0a9b00445cc6514b8b1"
hash = "sha256:3307dbb791136310a0d59eaff533c204489b79140b582fe41747ffad75008b11"
doc_hash = "sha256:f882ef36296dedb25d00a4dd7260ea7eca61ee57b0c24a1ad2fe4b8dbe1a3ea0"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_holds_extra = "no"
item_impossible_values = "no_checked_when_built"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.signature]
key = "k1"
alg = "ed25519"
value = "7pfe6P2ZSz3nlT9zBgEvaB4BAuTjcVfUJTjeMbZvNW1aO/I3LSFMsxSnhhEMVPqdXmngSJvC1NgT2h80CF1tCg=="
```

have to check this inline with process_run_file

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 84ab93aebd.

# Review: RunFileError (github:theodoreOnzGit)

```toml
[kovan]
id = "review-runfileerror-theodoreonzgit-becf4414"
kind = "review"
origin = "human"
created = "2026-10-27T15:54:50+08:00"
modified = "2026-10-27T15:54:50+08:00"
target = "fn:becf4414924ea48c"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_file/mod.rs::RunFileError"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T15:54:51+08:00"
commit = "c79d0f55413077845d7a9ea08dc6155cf91c0fe9"
hash = "sha256:03911392642a949e6715c830c43ffabcc8361ce6a997327a99d69f0fecf923c5"
doc_hash = "sha256:95f5c4f5069e327a78ecc5d3a21111c22f07ef280685046bbe5cfeaf2d4a29a9"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_cases_complete = "yes"
item_holds_extra = "no"
item_impossible_values = "no_checked_when_built"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "mrlMN2TrO7un0nHbT6ZRwMgzixJyKymCIEW1mWyBj6ZiTf6Up8sUEpNvcJkncNclZu0ViPzQtVCpYii/d5bLBw=="
```

enum for error

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit c79d0f5541.

# Review: RunFile::to_config (github:theodoreOnzGit)

```toml
[kovan]
id = "review-to-config-theodoreonzgit-88b7dd99"
kind = "review"
origin = "human"
created = "2026-10-27T16:35:56+08:00"
modified = "2026-10-27T16:35:56+08:00"
target = "fn:88b7dd996a2dac5b"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_file/mod.rs::RunFile::to_config"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T16:35:58+08:00"
commit = "94f5ddb954737e46b6b721580fe774829bd03266"
hash = "sha256:5a0cbc48ac0c59c6d6a09d3a9aef8ea910cbc3f7a176c0ff45aef1e016be1dfd"
doc_hash = "sha256:9a41f505d71d33810b9b22abd462f6d122713623e687be8bf73175f0ace7e820"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.callees]
"fn:7211ef55974ed7bc" = "sha256:3b9fff3ac196256b5f0eb77c152443f93ebc5236d4c9a444e8922e2f4b63f60f"
"fn:72d8fa4bc6abed85" = "sha256:8207b7d66b9e7c72ce2320dddc75f66c09cb93a737d1cfa5d29d19a16d7e0ca3"
"fn:becf4414924ea48c" = "sha256:03911392642a949e6715c830c43ffabcc8361ce6a997327a99d69f0fecf923c5"
"fn:eed943d26b55606f" = "sha256:3307dbb791136310a0d59eaff533c204489b79140b582fe41747ffad75008b11"
"fn:fca9138d7d203340" = "sha256:530a1d93326283c1aae3eb344ff52025409ee276d36989f0933c6c8cb6d2a96d"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "returns_result"
independence = "someone_else"
limits_and_guards = "no_limits: this is a helper function or type conversion function"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "3cv6i/CLC3tCQOFsmJCmE+boISxhJxHxFwl8VEUL8WhE34kP95Lnuy1zyK+YTK4VjRg8cXovYdxb25OFNLpgDQ=="
```

constructor

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 94f5ddb954.
