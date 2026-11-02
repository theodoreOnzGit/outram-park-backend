# Review record: crates/boon-lay/src/triso_atops_fork/diffusion

```toml
[kovan]
id = "review-record-crates-boon-lay-src-triso-atops-fork-diffusion"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: GAS_CONSTANT_J_PER_MOL_K (github:theodoreOnzGit)

```toml
[kovan]
id = "review-gas-constant-j-per-mol-k-theodoreonzgit-1be0fd51"
kind = "review"
origin = "human"
created = "2026-10-23T13:35:53+08:00"
modified = "2026-10-23T13:41:43+08:00"
target = "fn:1be0fd5172d8041a"

[review]
path = "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::GAS_CONSTANT_J_PER_MOL_K"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-23"
signed_at = "2026-10-23T13:41:43+08:00"
commit = "48aeedb4e34ac9a042da2e79790c330259535f25"
hash = "sha256:ed0d0d4925d660ad2532da5cfa6edc19abc8feee253f0058272b9883133e9485"
doc_hash = "sha256:94af24af925a661046223097d03fb692a118724cea883b78d296b2be9ac6d840"
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

[review.signature]
key = "k1"
alg = "ed25519"
value = "jC883eT3ep0BphwcKiNOXsCifeLnp+jVbzZ/7KqeyVAYIJtc9LgDbTIeO2EievD0WFX0EFykvcyQMd359XRkDQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/fuel-system-damage/rod-internal-gas-pressure"
kind = "implements"
```

From memory, 8.314 J/mol K, looks correct, sourced from upstream is ok

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-23, commit 48aeedb4e3.

# Review: arrhenius_term (github:theodoreOnzGit)

```toml
[kovan]
id = "review-arrhenius-term-theodoreonzgit-e950fcab"
kind = "review"
origin = "human"
created = "2026-11-02T10:27:49+08:00"
modified = "2026-11-02T10:27:49+08:00"
target = "fn:e950fcabea4ee9e2"

[review]
path = "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::arrhenius_term"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T10:27:49+08:00"
commit = "7fdb68b85b03580d8bf57ddc310dc6cb8d528edf"
hash = "sha256:7faf9ccfcddcda1d8cd8d9f1de564f724c07b6765bbbd4ea91b6755d2561342b"
doc_hash = "sha256:da7b455b628ce5bc1c7f5dba41823e6e07bb8af9ab0c1836baccc56e0f3729ff"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:1be0fd5172d8041a" = "sha256:ed0d0d4925d660ad2532da5cfa6edc19abc8feee253f0058272b9883133e9485"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"

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
sessions = [
    "https://claude.ai/code/session_01AxCVfsESPWrowa6mMV5skq",
    "https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "t0iY0Agf8w6sahzX9WgTndC4VuP6697sbQroLu7eZ1IrWieNjQ92MlDn6RhZc8JzH9gK8S2dIZ0RM/zZax30Aw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 7fdb68b85b.

# Review: celsius_to_kelvin (github:theodoreOnzGit)

```toml
[kovan]
id = "review-celsius-to-kelvin-theodoreonzgit-6e1150d8"
kind = "review"
origin = "human"
created = "2026-11-02T10:28:43+08:00"
modified = "2026-11-02T10:28:43+08:00"
target = "fn:6e1150d88bd2c0b3"

[review]
path = "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::celsius_to_kelvin"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T10:28:44+08:00"
commit = "d4704dd6b27c10d73b760010d9bc4606e380b625"
hash = "sha256:4bb6b2b40fb69225d9d7e154b2818422d44e19047fbf1e3efb2e74510d2d2e67"
doc_hash = "sha256:e492e10b9f3cbaf8630de2397008823233df3d0793825a68d88854301e1b327a"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

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
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "MjRIf5yE8YbGlzf0GMzWm1vTfxgNV0COpcLHzQ7MFMoL10VojL+kE/Zio5b4NEsAvyzXyPqRxGOXZ324hPCPBw=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/unit-and-dimensional-safety"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit d4704dd6b2.

# Review: diffusion_coefficient (github:theodoreOnzGit)

```toml
[kovan]
id = "review-diffusion-coefficient-theodoreonzgit-8bc4c3bc"
kind = "review"
origin = "human"
created = "2026-11-02T10:30:40+08:00"
modified = "2026-11-02T10:30:40+08:00"
target = "fn:8bc4c3bc717bd99d"

[review]
path = "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::diffusion_coefficient"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T10:30:40+08:00"
commit = "2291ab492ffa31bd59b4190e2df095685f640c2e"
hash = "sha256:01a9d764192caed17727d22b033e15e121412eb228415dd63816e5c10edbb912"
doc_hash = "sha256:a8ce80679db11b00620b31f51b14f8a641d83dd72497db940f48eb6a733ad03f"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:6e1150d88bd2c0b3" = "sha256:4bb6b2b40fb69225d9d7e154b2818422d44e19047fbf1e3efb2e74510d2d2e67"
"fn:bbcd28a15cbbf5eb" = "sha256:eb2ee1bfa72e961eccc4f1bb9dc7c1c90fe081cb6aca389e38ac0948778d6040"
"fn:e950fcabea4ee9e2" = "sha256:7faf9ccfcddcda1d8cd8d9f1de564f724c07b6765bbbd4ea91b6755d2561342b"

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
sessions = [
    "https://claude.ai/code/session_012qgX47fwybgAuf2wEiRvBu",
    "https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "euZEcMtxDLL33ZVO3BRqeoPXyMBf3qvox2hKyybRvfNRp7yKhE43n4NObb+Tnxt7LTerOrKwoJS338DISjtyBA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 2291ab492f.

# Review: KernelGraphiteDiffusion (github:theodoreOnzGit)

```toml
[kovan]
id = "review-kernelgraphitediffusion-theodoreonzgit-bbcd28a1"
kind = "review"
origin = "human"
created = "2026-11-02T10:35:07+08:00"
modified = "2026-11-02T10:35:07+08:00"
target = "fn:bbcd28a15cbbf5eb"

[review]
path = "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::KernelGraphiteDiffusion"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T10:35:07+08:00"
commit = "dec1b0f1f70648c33714fcf1c74c9729c21b87ad"
hash = "sha256:eb2ee1bfa72e961eccc4f1bb9dc7c1c90fe081cb6aca389e38ac0948778d6040"
doc_hash = "sha256:8e7e1c2422fa4211ea94db2b18602e843a7c5912c3193f7001f67081ec114a1d"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

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
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "bv/wSZk6hijzrgvL6zo9+evzC/YZjkDkStyQLPMoiogQmsEC7dDd/HKSyu/cCBkAXe8hHBUBz3UOKHttvjkeAw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit dec1b0f1f7.

# Review: DiffusionMaterial (github:theodoreOnzGit)

```toml
[kovan]
id = "review-diffusionmaterial-theodoreonzgit-bfe17211"
kind = "review"
origin = "human"
created = "2026-11-02T10:42:05+08:00"
modified = "2026-11-02T10:42:05+08:00"
target = "fn:bfe17211ff4ff7f2"

[review]
path = "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::DiffusionMaterial"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T10:42:05+08:00"
commit = "444811df970dee14fc6f596b152f39bbfe970977"
hash = "sha256:24a940e2ec020558fbe49b26c8d59fd415f140bd56a083cfb6247806f6d722c4"
doc_hash = "sha256:4bed444d5027274c83affc172e0d240466080a9d49faaf53bd68704010b521e4"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_cases_complete = "yes"
item_holds_extra = "no"
item_impossible_values = "no_checked_when_built"
item_matches_description = "yes"
item_test_use = "other: this is a plumbing thing"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "other: plumbing, not necessary"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "OO5GepB34pcBCQaWVPDeJnQnIq9ZQBe8YQdGIyKv6XZS42BloG9vga6uTxar0dx+qJS5/4ZUfdEs7UfzB1TSDw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 444811df97.
