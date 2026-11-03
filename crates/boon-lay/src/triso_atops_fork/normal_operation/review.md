# Review record: crates/boon-lay/src/triso_atops_fork/normal_operation

```toml
[kovan]
id = "review-record-crates-boon-lay-src-triso-atops-fork-normal-operation"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: PlantConstants (github:theodoreOnzGit)

```toml
[kovan]
id = "review-plantconstants-theodoreonzgit-515e6933"
kind = "review"
origin = "human"
created = "2026-11-03T15:01:46+08:00"
modified = "2026-11-03T15:01:46+08:00"
target = "fn:515e6933ab0603f9"

[review]
path = "crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs::PlantConstants"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T15:01:46+08:00"
commit = "4e9fcc74fcf5b8cd41686b57b088e24411e73cba"
hash = "sha256:e6d597d01f0fbf4906e858eed1b26c1868f061c52188509463e05e83cc78a311"
doc_hash = "sha256:23536b03e246ecf6ca020ef0a66515f62352e0351f1afecf2bda219af8153ad3"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "generic_numerics"

[review.callees]
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"

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
sessions = [
    "https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7",
    "https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "RlMI1FexXpBIGm0orWSdPFghbi/sGiXTcGViLK0eebaLW5uweLm1ijz5jpHDMFIpeAccYuyNHgOr2YuM+Fs7AQ=="
```

checked that the TRISO ATOPS normal_operation code contains these

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 4e9fcc74fc.

# Review: PlantConstants::new (github:theodoreOnzGit)

```toml
[kovan]
id = "review-new-theodoreonzgit-5a33f79b"
kind = "review"
origin = "human"
created = "2026-11-03T15:03:03+08:00"
modified = "2026-11-03T15:03:03+08:00"
target = "fn:5a33f79b407cf341"

[review]
path = "crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs::PlantConstants::new"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T15:03:03+08:00"
commit = "28f7d496061853e5b4865dc2edd5a91f9d8ae1a8"
hash = "sha256:4c2afe9cb508ba2c2933c0c9cd1e4cf01c88e57848b554e759d1854331c9e2a8"
doc_hash = "sha256:b74b1f0ab7a0d8d0f354ccc43f8a980da0ad8a08085ca595e575eb0ef62b4a98"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "io_format"

[review.callees]
"fn:515e6933ab0603f9" = "sha256:e6d597d01f0fbf4906e858eed1b26c1868f061c52188509463e05e83cc78a311"
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream (no result return)"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream (no result return)"
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
value = "d+2AIUOG97qmmqdMP/i5G7yu2LoQb5P979gGOZDnOjgD0ycDEDnZEd5MEBYD75CUayZwIyuQ9ZWXq63/NpjwAA=="
```

constructor

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 28f7d49606.

# Review: normal_operation_node (github:theodoreOnzGit)

```toml
[kovan]
id = "review-normal-operation-node-theodoreonzgit-9a8a6835"
kind = "review"
origin = "human"
created = "2026-11-03T15:07:28+08:00"
modified = "2026-11-03T15:07:28+08:00"
target = "fn:9a8a6835861d321b"

[review]
path = "crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs::normal_operation_node"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T15:07:28+08:00"
commit = "25f1eeb072143df9d5d76004d3aca6d54905f104"
hash = "sha256:35aa9d1c98f83def7d97ea998bcab736badd6b6b855a3d5be8e59ec40ed3de07"
doc_hash = "sha256:c9428ce3e0124dc3038402c247d0f1062a27a54067ea0a76c64b7d4db43fd803"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:1365b3b5318e0a5d" = "sha256:c942284721244108e667ca31e29958749cc2bb1be0458b653a2b2f3da2bf3d67"
"fn:14ee81ec2df53f3a" = "sha256:12dfe2fd134677e41b2ca49e9db0b202c8d4f3e983719db589a536ea2bc1337e"
"fn:1b4860a3c1b4d2ed" = "sha256:1e23358c5e8c68e95548f0ed7b7aadf6e613039c32ab88dcea7d6fb1ee4b3fe7"
"fn:280a309fa4659843" = "sha256:843074bd5d0871f115f2ba109cf1b6a7d4c1146765fd9d837ced2426cf4f3b02"
"fn:2816dd8674f2b148" = "sha256:3890d7f2c2bf5d78ac0a02d952c219265c6ded009ae509e8ceaaf6ce156eced5"
"fn:2ae73ae4ad5edebf" = "sha256:397acae832a3b2ab140849d2299b17eca0626c642a5de66485cbbbc7770ae0a0"
"fn:34ff47a5448224c9" = "sha256:1565117e190704e250537a267fcc591cbd6f4f74e8c74c62d6c58955f5437378"
"fn:49cbfd358a6976d4" = "sha256:21bde6366bd14b2230818107f9f51697dcb40359f8c429329b0bea39dd191993"
"fn:515e6933ab0603f9" = "sha256:e6d597d01f0fbf4906e858eed1b26c1868f061c52188509463e05e83cc78a311"
"fn:546333a5363bc5a2" = "sha256:601d879e0b2947c8c69562c0278d67beccee30a0999a8eb86181de5ebcd65549"
"fn:8f6c972c90fdceff" = "sha256:9db560f8b8d5220a25ff12870e0702d97acedb8e3ad099a687d937d4bd35d990"
"fn:9515687f16091d7e" = "sha256:d991c04dc42b0782b9cde96107150da5659aab605a8a9360aee6e287f87f3542"
"fn:98e15d9c13a78899" = "sha256:0f0a6db70afe1338a4117433d1546cdbe5d5d3d5267c95e2b2a6ca7cf961ca85"
"fn:d9fc661a522b11ce" = "sha256:6c96657ab13ce5ab01b09a4dc4e3d249d08ce5d01102fa98aa74fdf096fb8faa"
"fn:dcb6acb5e841b5b0" = "sha256:24b2f6ca6faddbc1768e5afef5d98c2498f83bd3ad7a163cbe0edc7e13e24187"
"fn:e6c95a55e6568166" = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"
"fn:ffb1855ec854dd27" = "sha256:a7d1d8385fca2bd85a1bb4bdd69aa11e2beefda8a9895eab3fbfda03a96b8ae1"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "other: follow upstream"
independence = "someone_else"
limits_and_guards = "no_limits: follow upstream"
maintainability = "yes"
numerical_hazards = "other: follow upstream"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7",
    "https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "Iug+SelWMSaEVBEHUIr188Xkv97IySfdV5wfrB0nySE2RsCWbUW23oO96GdHv/ioqxaXO8N513u/CVGRYiz6Cw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

driver file
mirrors normal_operation in trisoatops.py upstream

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 25f1eeb072.

# Review: NodalActivities::to_curies (github:theodoreOnzGit)

```toml
[kovan]
id = "review-to-curies-theodoreonzgit-5a2cfb2f"
kind = "review"
origin = "human"
created = "2026-11-03T15:08:52+08:00"
modified = "2026-11-03T15:08:52+08:00"
target = "fn:5a2cfb2f89e6446a"

[review]
path = "crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs::NodalActivities::to_curies"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T15:08:52+08:00"
commit = "1dc38140681c3830126cfdd3d6b2bee5db965a1b"
hash = "sha256:6f018e714c7040808b9b260d600f70aa1c59b7aa1fb8ebc33668a99f025e9654"
doc_hash = "sha256:378f3b46e5f2a96e928e1aecac4bee1a270bbe5a6cd5fd1291ecd933a4055059"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "generic_numerics"

[review.callees]
"fn:4e747b96b31630e2" = "sha256:684872ed9862c8116c6c7c0d3a5e67c1706f30a9a55d31b4d03b2f1ba1c8232d"
"fn:5c7ac5ccc2a03227" = "sha256:d9cf55d57337141e3fe03a9138a64b2647093b159f37498bc895f95f69708646"
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:dcb6acb5e841b5b0" = "sha256:24b2f6ca6faddbc1768e5afef5d98c2498f83bd3ad7a163cbe0edc7e13e24187"
"fn:e6c95a55e6568166" = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "other: follow upstream"
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
value = "T5ZDbDQARcRgjkXpNqk3ywEMXvOd7VpSyGEBGwvu3W8Gxtd2opdsK58ksjauWvWvyCgP6XFGrQUk031UhKlAAg=="
```

unit conversion

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 1dc3814068.
