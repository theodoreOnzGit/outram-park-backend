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
