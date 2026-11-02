# Review record: crates/boon-lay/src/triso_atops_fork/accident

```toml
[kovan]
id = "review-record-crates-boon-lay-src-triso-atops-fork-accident"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: release_activity (github:theodoreOnzGit)

```toml
[kovan]
id = "review-release-activity-theodoreonzgit-429d6161"
kind = "review"
origin = "human"
created = "2026-11-02T13:42:05+08:00"
modified = "2026-11-02T13:42:05+08:00"
target = "fn:429d6161b7f8bf68"

[review]
path = "crates/boon-lay/src/triso_atops_fork/accident/mod.rs::release_activity"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:42:06+08:00"
commit = "f3258c99af1f48d41305c7612d64aa318ed7ad32"
hash = "sha256:1dcdaf590cb023c4c1fc8cda6b94984d03aa5a239205d650f88b89dc967fe28f"
doc_hash = "sha256:079894cad8b55a9d32d668387cf32c7367441141c2834132a8a82c1be33ceb5f"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:0c21cc39d4fb8228" = "sha256:79b187a51027aa0d263a5be10e90506b3a7b3de48b961eeac4b984d01f18257e"
"fn:30f22e6aa555e522" = "sha256:f686099b10d09642a2f1f9fb1996aafa32b3f6d31e75e887100d5ded75363435"
"fn:43bcac572a02bd8c" = "sha256:5a509edc87ba58d77d2568e6f497c335b2672f45f24e844e22b60ee71d93a30c"
"fn:48f1bc7388b4aee5" = "sha256:4da0ac43908bdaeb66bf1b8090087e7e71676f71dc2d235cbbee149afeb39b86"
"fn:ce1b83f0efaae6a9" = "sha256:6b315ec2f9a80c7418e82860a0d33777132f84f95799f7f610554a064e5ea862"
"fn:d9fc661a522b11ce" = "sha256:6c96657ab13ce5ab01b09a4dc4e3d249d08ce5d01102fa98aa74fdf096fb8faa"
"fn:e8bfdd8514f1aacb" = "sha256:9001999a6600f6623b7fa7a2747b60f9547e00ed8ad2204701b5d862fa7fe685"
"fn:f6f5fe90eb866047" = "sha256:4242b94ba43697db40fc2694ba3e577cea664b67c94a6d1e8e0eb861c337b870"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream behaviour"
maintainability = "yes"
numerical_hazards = "other: follow upstream behaviour"
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
value = "SN6kYtqtRoYkRemSZqf0lzF9OfmRPrWQ2T0m8KkZFYzZ0iQ6bNF/nGcpOZa396zZcwHc1VrqwbZ/kpk2haftBQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

matches upstream release_activity function

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit f3258c99af.

# Review: release_activity_for (github:theodoreOnzGit)

```toml
[kovan]
id = "review-release-activity-for-theodoreonzgit-26222fa6"
kind = "review"
origin = "human"
created = "2026-11-02T13:45:48+08:00"
modified = "2026-11-02T13:45:48+08:00"
target = "fn:26222fa676fec174"

[review]
path = "crates/boon-lay/src/triso_atops_fork/accident/mod.rs::release_activity_for"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:45:48+08:00"
commit = "ee873063433a15ee6fa261a89a5e6b8214661f30"
hash = "sha256:14c940e773e66ed1f7041c831ede0bf11c00f88a02d5a57569a10dac81a6f293"
doc_hash = "sha256:4df77b6fd5c2c844713cae52d145e79be85540bc23b6fabb185b010901515bd6"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:0c21cc39d4fb8228" = "sha256:79b187a51027aa0d263a5be10e90506b3a7b3de48b961eeac4b984d01f18257e"
"fn:24e4f2d2ea7f17f4" = "sha256:050f20680cf2ebc0fb4f0a7e8b3d49a74fc1fe359546321ecb13713346a7c00f"
"fn:30f22e6aa555e522" = "sha256:f686099b10d09642a2f1f9fb1996aafa32b3f6d31e75e887100d5ded75363435"
"fn:429d6161b7f8bf68" = "sha256:1dcdaf590cb023c4c1fc8cda6b94984d03aa5a239205d650f88b89dc967fe28f"
"fn:48f1bc7388b4aee5" = "sha256:4da0ac43908bdaeb66bf1b8090087e7e71676f71dc2d235cbbee149afeb39b86"
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"
"fn:b31dc309195e5890" = "sha256:cb0506150f5656d4007ad3a0423685c53bc604ebe4185be3d7d919078226cab2"
"fn:d9fc661a522b11ce" = "sha256:6c96657ab13ce5ab01b09a4dc4e3d249d08ce5d01102fa98aa74fdf096fb8faa"
"fn:e8bfdd8514f1aacb" = "sha256:9001999a6600f6623b7fa7a2747b60f9547e00ed8ad2204701b5d862fa7fe685"
"fn:f6f5fe90eb866047" = "sha256:4242b94ba43697db40fc2694ba3e577cea664b67c94a6d1e8e0eb861c337b870"
"fn:fc42897456ddcda8" = "sha256:99ab9e07c9d01a858b47d5649f98c035be52983fc3847b7614406757d66dbd8d"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream behaviour"
maintainability = "yes"
numerical_hazards = "other: follow upstream behaviour"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "diW74fuR2zNOZi20vJjj1OV9ulTVTnvlOydbgV7CRzbRBVSvX627jCCZ/JQs39KBGW+jtcRyPpcTAx0IXD21AQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

ablation test function to wrap release_activity when checking against how the triso atops paper is read.

That is when trying to reproduce triso atops paper results, we can use defaults and settings from the code repo, or from paper. We weren't sure (dicussing w/ claude code) which one it was. So i simply told it to do ablation, sensitivity studies.

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit ee87306343.
