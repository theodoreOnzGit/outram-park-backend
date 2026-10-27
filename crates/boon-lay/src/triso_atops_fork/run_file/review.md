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
