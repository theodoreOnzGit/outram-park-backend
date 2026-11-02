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

# Review: AccidentFractions (github:theodoreOnzGit)

```toml
[kovan]
id = "review-accidentfractions-theodoreonzgit-48f1bc73"
kind = "review"
origin = "human"
created = "2026-11-02T13:47:05+08:00"
modified = "2026-11-02T13:47:05+08:00"
target = "fn:48f1bc7388b4aee5"

[review]
path = "crates/boon-lay/src/triso_atops_fork/accident/mod.rs::AccidentFractions"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:47:06+08:00"
commit = "924bcde174407e76c119b77249234fed7e6ecf37"
hash = "sha256:4da0ac43908bdaeb66bf1b8090087e7e71676f71dc2d235cbbee149afeb39b86"
doc_hash = "sha256:7c62060a0abce2181bddb5a9413905af661937d2edbdef4c2f21b30eedd4f4ec"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "plumbing"

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

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "2zwx5v+zKo+SuI9hqX1a5EZEdE601Da2BxN3K7uh84FJYEBez64TB4K/b62MWJMXbUFSAvFX9qY+0vG7JbYsCQ=="
```

data structure to hold accident fractions, it is a np.ndarray fraction under release activity

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 924bcde174.

# Review: AccidentFractions::total (github:theodoreOnzGit)

```toml
[kovan]
id = "review-total-theodoreonzgit-43bcac57"
kind = "review"
origin = "human"
created = "2026-11-02T13:48:27+08:00"
modified = "2026-11-02T13:48:27+08:00"
target = "fn:43bcac572a02bd8c"

[review]
path = "crates/boon-lay/src/triso_atops_fork/accident/mod.rs::AccidentFractions::total"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:48:27+08:00"
commit = "517305576683f842467fad2349159a255836f6ff"
hash = "sha256:5a509edc87ba58d77d2568e6f497c335b2672f45f24e844e22b60ee71d93a30c"
doc_hash = "sha256:fcc99306f5cba5d675bd4b3068251c6ecae1cb524dcb6a2dafe7ca332bb9a25d"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:48f1bc7388b4aee5" = "sha256:4da0ac43908bdaeb66bf1b8090087e7e71676f71dc2d235cbbee149afeb39b86"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream"
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
value = "zmLGMhJsBdT7z1EwG5GDU/77cp+9Isl9fjhCSWSLR1aA0H3MXYi+nvtaJ5YfCR0Xl0m4eUhKqIGJ8ZFUULBcDQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

sums up all the "failure/accident" fractions in the struct, utility function

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 5173055766.

# Review: AccidentFractions::normal_operation_sum (github:theodoreOnzGit)

```toml
[kovan]
id = "review-normal-operation-sum-theodoreonzgit-30f22e6a"
kind = "review"
origin = "human"
created = "2026-11-02T13:49:49+08:00"
modified = "2026-11-02T13:49:49+08:00"
target = "fn:30f22e6aa555e522"

[review]
path = "crates/boon-lay/src/triso_atops_fork/accident/mod.rs::AccidentFractions::normal_operation_sum"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:49:49+08:00"
commit = "372ffa19c7812255c685124b8309f8becd73ebbf"
hash = "sha256:f686099b10d09642a2f1f9fb1996aafa32b3f6d31e75e887100d5ded75363435"
doc_hash = "sha256:d1fcf421c45bd331e8ba863536ce2b2c881dd2d21ea825b8c34e66890195cc1d"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:48f1bc7388b4aee5" = "sha256:4da0ac43908bdaeb66bf1b8090087e7e71676f71dc2d235cbbee149afeb39b86"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream"
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
value = "JEafafJ8JByNFWVCUI14CKjS9jN9vG7aLnakkCsrQ6eaAhc60oZyGruJpf0rpSGEbc043eectjIUmPQpO+PWAA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

utility function which helps calculate release during normal operation
in release_activity in triso atops python (i checked)

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 372ffa19c7.

# Review: AccidentFractions::accident_sum (github:theodoreOnzGit)

```toml
[kovan]
id = "review-accident-sum-theodoreonzgit-f6f5fe90"
kind = "review"
origin = "human"
created = "2026-11-02T13:50:43+08:00"
modified = "2026-11-02T13:50:43+08:00"
target = "fn:f6f5fe90eb866047"

[review]
path = "crates/boon-lay/src/triso_atops_fork/accident/mod.rs::AccidentFractions::accident_sum"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:50:43+08:00"
commit = "e90a0b421f35fc3b655d2fa0ff36a1f09e879d90"
hash = "sha256:4242b94ba43697db40fc2694ba3e577cea664b67c94a6d1e8e0eb861c337b870"
doc_hash = "sha256:1b5b57d99799530261be2bc9d48ef7dd8a47a4cac6ea3332ba833501e1d520c6"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:48f1bc7388b4aee5" = "sha256:4da0ac43908bdaeb66bf1b8090087e7e71676f71dc2d235cbbee149afeb39b86"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream"
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
value = "dkuzS5dw4CyFSDvkFLn9ibv72XXQXePUYqpjuG64b3SRUGihSE1jKiq16Tfqxu7bh5X9mgEC2YTYtsk92yKVCA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

release that follows accidents

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit e90a0b421f.

# Review: R_GAS (github:theodoreOnzGit)

```toml
[kovan]
id = "review-r-gas-theodoreonzgit-b1ee7eed"
kind = "review"
origin = "human"
created = "2026-11-02T13:52:33+08:00"
modified = "2026-11-02T13:52:33+08:00"
target = "fn:b1ee7eedd6db4f0c"

[review]
path = "crates/boon-lay/src/triso_atops_fork/accident/mod.rs::R_GAS"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:52:33+08:00"
commit = "76b52e584c9411180daee6ffb75fdfd279888097"
hash = "sha256:5140abd75c3afe78136b9b104be0a18e2588a93d1e3edf8b845200fe4b53606e"
doc_hash = "sha256:bada9c8f945daf0d97a8ef83716f47ff4c72edf9b38c6c41750da273e5aafe28"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "generic_numerics"

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
sessions = ["https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "42Hglxd4H7jrGskYRUGHEA9A2iwCXaPUaFZOiJP6rn3t0ECc29KsGp8OLrU5HVXwjvs2gJT7LjMPzFoCAmRcAg=="
```

this is just gas constant, dunno why need to define again, but if it works, it works. (not the best IMO, should import)

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 76b52e584c.

# Review: atoms_to_curies (github:theodoreOnzGit)

```toml
[kovan]
id = "review-atoms-to-curies-theodoreonzgit-36a8c82d"
kind = "review"
origin = "human"
created = "2026-11-02T13:53:40+08:00"
modified = "2026-11-02T13:53:40+08:00"
target = "fn:36a8c82d150d877e"

[review]
path = "crates/boon-lay/src/triso_atops_fork/accident/mod.rs::atoms_to_curies"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:53:40+08:00"
commit = "3c2ffe4874829a38056bf3aa63abb3a9d98a3f83"
hash = "sha256:52d6f152c907bf7f51f9def7287b84f20efb017e8ac76d12eebf0a5b65fdcd45"
doc_hash = "sha256:42361dd7c892586981d8b46b0106fff04d21721844a904405d1427bdebe34455"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follows upstream"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follows upstream"
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
value = "jVEQAdnwQSnadQCOSiK7uSahiYuVqjI5/JyS7M781aPv6389rZQPmPr68szRfuiskVlfxvwNKrBt3d/FAHDaBw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

atoms to curies conversion, generic utility, probably mass porting.... it is given elsewhere tho... but nvm lah

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 3c2ffe4874.

# Review: ReleaseMaterial (github:theodoreOnzGit)

```toml
[kovan]
id = "review-releasematerial-theodoreonzgit-0c21cc39"
kind = "review"
origin = "human"
created = "2026-11-02T13:54:41+08:00"
modified = "2026-11-02T13:54:41+08:00"
target = "fn:0c21cc39d4fb8228"

[review]
path = "crates/boon-lay/src/triso_atops_fork/accident/mod.rs::ReleaseMaterial"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:54:41+08:00"
commit = "9a86ab63a530230c620baaea3e6910189c63f85d"
hash = "sha256:79b187a51027aa0d263a5be10e90506b3a7b3de48b961eeac4b984d01f18257e"
doc_hash = "sha256:b036b033773c4a527c928966d9e7f28ad7a7b7b6b4437139388ec02c451202f9"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
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
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "y8iuh4TabL6Lifaa6NcRDBH1+K0SyGT+Nb9GzPDYgv26z3FZktXITPHRWZgSA31tn5iQVSWoslWGniHPGj0zDg=="
```

enum could probably be combined, because the two members Kernel and Graphite also appear elsewhere. But i suppose this is fine for now

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 9a86ab63a5.

# Review: NormalOperationNode (github:theodoreOnzGit)

```toml
[kovan]
id = "review-normaloperationnode-theodoreonzgit-e8bfdd85"
kind = "review"
origin = "human"
created = "2026-11-02T13:55:40+08:00"
modified = "2026-11-02T13:55:40+08:00"
target = "fn:e8bfdd8514f1aacb"

[review]
path = "crates/boon-lay/src/triso_atops_fork/accident/mod.rs::NormalOperationNode"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:55:40+08:00"
commit = "ba91b652747c77cdf9710a5e67a6b71da04527fc"
hash = "sha256:9001999a6600f6623b7fa7a2747b60f9547e00ed8ad2204701b5d862fa7fe685"
doc_hash = "sha256:c2394cfe51244d5eb5887e3bea1c90b14ca54c34aeac7d8be75888c33e0ec6b1"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_holds_extra = "no"
item_impossible_values = "other: mirrors upstream"
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
value = "zRu/9h0j618tyhhCrCP3f8+NGWkU0+46SiPfsddoIKdnE17h1OPtV2mT8vhLbk+gcSTJr7tcp2VcWqvljikjDg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

follow upstream behaviour

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit ba91b65274.

# Review: distribute_inventory_axially (github:theodoreOnzGit)

```toml
[kovan]
id = "review-distribute-inventory-axially-theodoreonzgit-1821cd8b"
kind = "review"
origin = "human"
created = "2026-11-02T13:59:31+08:00"
modified = "2026-11-02T13:59:31+08:00"
target = "fn:1821cd8bfaa22038"

[review]
path = "crates/boon-lay/src/triso_atops_fork/accident/mod.rs::distribute_inventory_axially"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:59:31+08:00"
commit = "658ce839c902d64ec4c97bf3746791ec264fd9f5"
hash = "sha256:a06ab686b38bf13447eacc123d9e2f493f2b7aff664151f75aab6d8de014d14b"
doc_hash = "sha256:f99867ab55a8b7b4dbd48ecb713c2266280a0e69a588885d347208d17aa556d8"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follows upstream"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: guards against 0 axial nodes"
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
value = "Wq5lQjRiAMMHdHkkJjM23q4vv2F9KYjlkTddxgw5rA2HxxHnyJTjyeMlGwLhXaznc19CLcqgI8EcwAWpGTsMBg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

ported invenstory_processing upstream 
looks ok! but deviates as improvement, explained in comments. I suppose an even split also works. 
Which is the intent of the function

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 658ce839c9.
