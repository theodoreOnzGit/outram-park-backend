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

# Review: diffusion_coefficient_sic_ag (github:theodoreOnzGit)

```toml
[kovan]
id = "review-diffusion-coefficient-sic-ag-theodoreonzgit-7f5cefb7"
kind = "review"
origin = "human"
created = "2026-11-02T10:43:55+08:00"
modified = "2026-11-02T10:43:55+08:00"
target = "fn:7f5cefb7c1df6559"

[review]
path = "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::diffusion_coefficient_sic_ag"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T10:43:55+08:00"
commit = "613819d01a38d606b89bd6f9975369c32beca035"
hash = "sha256:4aa1d67f76c9b0ea57e4840755de225c42bc99fbca0793ad77d574160026eb1a"
doc_hash = "sha256:bf84a4913a88b849d1fdc98f0a638a3977b3c711c56d7bf1fde73032946baac4"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:e950fcabea4ee9e2" = "sha256:7faf9ccfcddcda1d8cd8d9f1de564f724c07b6765bbbd4ea91b6755d2561342b"

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
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "hWMy2GNmTWo4SMgyGTYKKBPZO2evx5DyuFuSrNhJXVl/R3A41OOl84jG/Oy0DLpfDB3d15yoqlsYZHtfy5VJDw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

checked against table 5 of triso atops paper
and against backend code

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 613819d01a.

# Review: diffusion_coefficient_sic_ag_for (github:theodoreOnzGit)

```toml
[kovan]
id = "review-diffusion-coefficient-sic-ag-for-theodoreonzgit-5651ddb2"
kind = "review"
origin = "human"
created = "2026-11-02T10:45:37+08:00"
modified = "2026-11-02T10:45:37+08:00"
target = "fn:5651ddb28625398d"

[review]
path = "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::diffusion_coefficient_sic_ag_for"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T10:45:38+08:00"
commit = "d9bc932bfc65d2246b50150ad20b5bb9485e94b8"
hash = "sha256:06d6d142ddb82fd001470fd61c7ed2e4f65f1d3ac98cdf1cb8d94424a1899d08"
doc_hash = "sha256:23ce288466c8a39e1dfb0c23f76a699768247fd85f61edcfb126529fa7c9e9de"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "plumbing"

[review.callees]
"fn:3e752b3c15c4e4fe" = "sha256:d1f72246cf1efe75bc53507767c1fc7af7359636aa5b009d6fba94e241e5bf7a"
"fn:7f5cefb7c1df6559" = "sha256:4aa1d67f76c9b0ea57e4840755de225c42bc99fbca0793ad77d574160026eb1a"
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"
"fn:fc42897456ddcda8" = "sha256:99ab9e07c9d01a858b47d5649f98c035be52983fc3847b7614406757d66dbd8d"

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
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "5EsDpLr+ddKvK3d+bMnTYARxnlVNZQ2rLTFqlJiE83EqAzEAgng2q9aVfudKNJOoyXKVKPQaFRKqpvsPoOTSAA=="
```

based on the TrisoAtopsVariant settings, decides whether a particular reading (by AI) of the paper should be used, to clamp the diffusion using a hard clamp or a partial clamp

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit d9bc932bfc.

# Review: integrate_diffusion_over_time (github:theodoreOnzGit)

```toml
[kovan]
id = "review-integrate-diffusion-over-time-theodoreonzgit-759c40b1"
kind = "review"
origin = "human"
created = "2026-11-02T11:31:34+08:00"
modified = "2026-11-02T11:31:34+08:00"
target = "fn:759c40b16c1f728e"

[review]
path = "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::integrate_diffusion_over_time"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T11:31:34+08:00"
commit = "fe30911754b2ae2385a67465f03d1d313310a2ca"
hash = "sha256:970d9ba0b671193747184b0d2a77945d439f052d9152e0e24604a1864fda2c0d"
doc_hash = "sha256:7e8f8175fd040a2830865bade80c59e8599153503b4f4acf8f3775fe34737814"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "generic_numerics"

[review.callees]
"fn:7f5cefb7c1df6559" = "sha256:4aa1d67f76c9b0ea57e4840755de225c42bc99fbca0793ad77d574160026eb1a"
"fn:8bc4c3bc717bd99d" = "sha256:01a9d764192caed17727d22b033e15e121412eb228415dd63816e5c10edbb912"
"fn:bfe17211ff4ff7f2" = "sha256:24a940e2ec020558fbe49b26c8d59fd415f140bd56a083cfb6247806f6d722c4"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream"
independence = "someone_else"
limits_and_guards = "no_limits: follow upstream"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "/BPyY83TfPTs8yli02+zkakRTW5iGD/yWXFAn1iDrqGW7dk83B13ssVPuL2tMrEY7jGF5Rujip1SuQqLDqV0AA=="
```

integration utility following upstream

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit fe30911754.

# Review: diffusion_coefficient_for (github:theodoreOnzGit)

```toml
[kovan]
id = "review-diffusion-coefficient-for-theodoreonzgit-1b4860a3"
kind = "review"
origin = "human"
created = "2026-11-02T11:33:12+08:00"
modified = "2026-11-02T11:33:12+08:00"
target = "fn:1b4860a3c1b4d2ed"

[review]
path = "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::diffusion_coefficient_for"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T11:33:13+08:00"
commit = "69b171c34063faec334e38be341923762ab7a8f3"
hash = "sha256:1e23358c5e8c68e95548f0ed7b7aadf6e613039c32ab88dcea7d6fb1ee4b3fe7"
doc_hash = "sha256:b29af133b09f4575522a2bfd96b9b0a0e826bfbf6edf0123cd0af3487e9a389e"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:3e752b3c15c4e4fe" = "sha256:d1f72246cf1efe75bc53507767c1fc7af7359636aa5b009d6fba94e241e5bf7a"
"fn:459395474c5875ce" = "sha256:97f8cd3ee0d1655194b4537bb7ef9d16c1f40e279be6fe6bf1b38d1b7b492abd"
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"
"fn:8bc4c3bc717bd99d" = "sha256:01a9d764192caed17727d22b033e15e121412eb228415dd63816e5c10edbb912"
"fn:bbcd28a15cbbf5eb" = "sha256:eb2ee1bfa72e961eccc4f1bb9dc7c1c90fe081cb6aca389e38ac0948778d6040"
"fn:fc42897456ddcda8" = "sha256:99ab9e07c9d01a858b47d5649f98c035be52983fc3847b7614406757d66dbd8d"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: wrapper"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: wrapper"
maintainability = "yes"
numerical_hazards = "none_found"
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
value = "VxZP9r9J51TH/ySfhLTmfYJjUEpRqkiwTUwnt8ugHVMT3/ZViXg1f5TiIqJnCkUEX9ULCXcYxzsvET/4Z03hAg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

wrapper to do ablation as to which settings reproduce triso atops paper result

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 69b171c340.

# Review: integrate_diffusion_over_time_for (github:theodoreOnzGit)

```toml
[kovan]
id = "review-integrate-diffusion-over-time-for-theodoreonzgit-a8b82f97"
kind = "review"
origin = "human"
created = "2026-11-02T11:34:42+08:00"
modified = "2026-11-02T11:34:42+08:00"
target = "fn:a8b82f97dda5e783"

[review]
path = "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::integrate_diffusion_over_time_for"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T11:34:42+08:00"
commit = "eff8f7ead5145800b2ea6ea3cc4cb65e58b07ba3"
hash = "sha256:22cdab21d2bcd7b2a04977b5f4138f9274b8988e899b876f67e2b62a9f3171a4"
doc_hash = "sha256:5d5b887a3fa05dd7a4db22e919f92eed1cfa0432a5271b47c8c7e28a894fd04e"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:1b4860a3c1b4d2ed" = "sha256:1e23358c5e8c68e95548f0ed7b7aadf6e613039c32ab88dcea7d6fb1ee4b3fe7"
"fn:5651ddb28625398d" = "sha256:06d6d142ddb82fd001470fd61c7ed2e4f65f1d3ac98cdf1cb8d94424a1899d08"
"fn:67358d5c0d56fc08" = "sha256:c32aac384075091714d67e7162126819d5d2c4140fa65b594b7e650b6be8a41b"
"fn:759c40b16c1f728e" = "sha256:970d9ba0b671193747184b0d2a77945d439f052d9152e0e24604a1864fda2c0d"
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"
"fn:9a3e33816dabd64b" = "sha256:8c0917b5ed353738bf0972a9f3c18d11059686189a878e614fc7682784bc3f37"
"fn:bfe17211ff4ff7f2" = "sha256:24a940e2ec020558fbe49b26c8d59fd415f140bd56a083cfb6247806f6d722c4"
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
value = "6pTxTtfk2QM3RB/e6UAjXaU3XCY0LAdSzDYfsrPzg3RXh4Ny5fx89Q6Of7WShpleKYXPMWf0Aq2u59gzT/rRBw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

ablation for how integration is done, and which clamping regime is performed

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit eff8f7ead5.
