# Review record: crates/outram-blender/src/csg

```toml
[kovan]
id = "review-record-crates-outram-blender-src-csg"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: Position (github:theodoreOnzGit)

```toml
[kovan]
id = "review-position-theodoreonzgit-006d5597"
kind = "review"
origin = "human"
created = "2026-11-26T13:07:16+08:00"
modified = "2026-11-26T13:07:16+08:00"
target = "fn:006d55971bc9e1e3"

[review]
path = "crates/outram-blender/src/csg/position.rs::Position"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-26"
signed_at = "2026-11-26T13:07:16+08:00"
commit = "995ac7daca916237ef2a9f3bcb932aacfcd915aa"
hash = "sha256:6f2d4f486afa1490d642f0e58cf39c0aa8e3f10ca27a00c80e35d20a1f2bb7c1"
doc_hash = "sha256:08c95b400b349456ec3fb2a8e064c8a433de040426fe968eec0c5c03dd3e4ec9"
cargo_lock = "sha256:f01119ef723c81b98b22a0bd1030fcc4965b939f2c757c626b5986870ad940ee"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_holds_extra = "no"
item_impossible_values = "yes_nothing_stops"
item_matches_description = "yes"
item_test_use = "used_not_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"

[review.signature]
key = "k1"
alg = "ed25519"
value = "gxsINN5f+Twkxr9dxKzNg4XheCkGv2zaV7uxUZjo1utcHSTJPxBvXV3zRfyt2a3FObi/3j4U2w3yvFr8t4/+DA=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics"
kind = "implements"
```

position follows upstream openmc

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-26, commit 995ac7daca.

# Review: Position::new (github:theodoreOnzGit)

```toml
[kovan]
id = "review-new-theodoreonzgit-cb1fc1a6"
kind = "review"
origin = "human"
created = "2026-11-26T13:08:36+08:00"
modified = "2026-11-26T13:08:36+08:00"
target = "fn:cb1fc1a67d38f99a"

[review]
path = "crates/outram-blender/src/csg/position.rs::Position::new"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-26"
signed_at = "2026-11-26T13:08:37+08:00"
commit = "05e5485cb5c1d2eb96304bd877f2cdd1151cd4d0"
hash = "sha256:f790c9da3f10cbda1553ed41644b4801ff19f4f8daa06a9cbcb0447510b628a1"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:f01119ef723c81b98b22a0bd1030fcc4965b939f2c757c626b5986870ad940ee"
no_concept = "generic_numerics"

[review.callees]
"fn:006d55971bc9e1e3" = "sha256:6f2d4f486afa1490d642f0e58cf39c0aa8e3f10ca27a00c80e35d20a1f2bb7c1"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "matches_upstream_no_error_return: constructor"
independence = "someone_else"
limits_and_guards = "matches_upstream_no_guard: just a constructor"
maintainability = "yes"
numerical_hazards = "other: upstream port doesn't guard, but yes"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"

[review.signature]
key = "k1"
alg = "ed25519"
value = "33U5SNjuLvkZvn6hezMseAiuzaX+VjKr/FLO99R+CO8YOQpHDDNLLTOIHu1WGdOnVs24DZCEYMRagTyHxe6ODQ=="
```

constructor

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-26, commit 05e5485cb5.
