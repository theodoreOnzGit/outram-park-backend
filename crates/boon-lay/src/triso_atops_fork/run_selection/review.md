# Review record: crates/boon-lay/src/triso_atops_fork/run_selection

```toml
[kovan]
id = "review-record-crates-boon-lay-src-triso-atops-fork-run-selection"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: normalise_nuclide_name (github:theodoreOnzGit)

```toml
[kovan]
id = "review-normalise-nuclide-name-theodoreonzgit-3292f77e"
kind = "review"
origin = "human"
created = "2026-11-03T14:51:31+08:00"
modified = "2026-11-03T14:51:31+08:00"
target = "fn:3292f77e069a4060"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_selection/mod.rs::normalise_nuclide_name"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T14:51:32+08:00"
commit = "e6f0fcff35aee1393a8342d6b278fdbbce2581de"
hash = "sha256:6f1cce1318935d762a5e95055133dcd60b18d8b2d565afcb4649635a43a0d0db"
doc_hash = "sha256:8ba7cbafe9285b5b5618d182a98ed1776e881434530ffd03e58e7c2924bf6cc6"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "plumbing"

[review.callees]
"fn:4ef295f5b3d28b64" = "sha256:48e95438f69cba28a4bf0a211b732c7dbe4fabe11f86934ecbc07520522f3496"

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
value = "AgqgGEShI38dggt1RZYq6H1f/orz4S2I3eqRj9oqH8ghWfnTDU22WIzggDumCaM0yuJFb9ttDVU/JVhAqx3PAg=="
```

this is data utility, parser

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit e6f0fcff35.

# Review: select_nuclides (github:theodoreOnzGit)

```toml
[kovan]
id = "review-select-nuclides-theodoreonzgit-2db20ac5"
kind = "review"
origin = "human"
created = "2026-11-03T14:52:48+08:00"
modified = "2026-11-03T14:52:48+08:00"
target = "fn:2db20ac5ca82f2df"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_selection/mod.rs::select_nuclides"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T14:52:48+08:00"
commit = "75e2b96459f647f388a5abe097d10710476bb83d"
hash = "sha256:88f57a620fcad0a9ad2c3a1dd178c5e0c248335e82ec9259d7aae14506c5ada5"
doc_hash = "sha256:4aad173b57721dbb50ce64846945b43664e527663b2fcbad7850c80ab48712dd"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "plumbing"

[review.callees]
"fn:153ff318fdebeb23" = "sha256:5131c46d8d82af19d0ce45b7a27fa03342176bf85ad2fec54aa89a12a63119a7"
"fn:2ac3393d97ea6d61" = "sha256:4181732889c916f7ae826996ea74c978665b65a0f13f4937fb366facddd842bb"
"fn:3292f77e069a4060" = "sha256:6f1cce1318935d762a5e95055133dcd60b18d8b2d565afcb4649635a43a0d0db"
"fn:4ef295f5b3d28b64" = "sha256:48e95438f69cba28a4bf0a211b732c7dbe4fabe11f86934ecbc07520522f3496"
"fn:720763535406a854" = "sha256:468363707eb6f8af68dd8e3d9901d7a76231693bf0a5b4e7e8bc5ad5ee6d5cc1"
"fn:e1cb570a455c31b8" = "sha256:06a9c12470b244d22d5415441b5b842edce0c43e867d9a1bbc36b9c1b69988ca"

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
sessions = ["https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "k06nmVsl7JaWben+BD7Tqn5gYPSVURQKHddINqtNOCpAMS3FXBEmTvhk519tblya5jubs7YuWe+UnRs9/Kc2Bg=="
```

parser function

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 75e2b96459.

# Review: select_nuclides_accident (github:theodoreOnzGit)

```toml
[kovan]
id = "review-select-nuclides-accident-theodoreonzgit-ee64ea35"
kind = "review"
origin = "human"
created = "2026-11-03T14:53:37+08:00"
modified = "2026-11-03T14:53:37+08:00"
target = "fn:ee64ea353c7ab103"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_selection/mod.rs::select_nuclides_accident"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T14:53:37+08:00"
commit = "b4298cae39ad13e55ff95d0259d5a6527788d038"
hash = "sha256:b64465272106ee396d691f1630588ea4b5129a3d60820a756aac7ab81dcc4787"
doc_hash = "sha256:a4ee8adaca48d98dd4606b2ff2667309ecdbae390667b121e4cda44079528e93"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "io_format"

[review.callees]
"fn:3292f77e069a4060" = "sha256:6f1cce1318935d762a5e95055133dcd60b18d8b2d565afcb4649635a43a0d0db"
"fn:4ef295f5b3d28b64" = "sha256:48e95438f69cba28a4bf0a211b732c7dbe4fabe11f86934ecbc07520522f3496"
"fn:546333a5363bc5a2" = "sha256:601d879e0b2947c8c69562c0278d67beccee30a0999a8eb86181de5ebcd65549"
"fn:e1cb570a455c31b8" = "sha256:06a9c12470b244d22d5415441b5b842edce0c43e867d9a1bbc36b9c1b69988ca"

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
sessions = ["https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "nQ3AOP3cv+l9t+89I3s/siaqy/UPb5/HwsXcZU1th5koE87x0O4LCfzr1rZnrEOVB/t/HwMCx06iymRh7pp1CA=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit b4298cae39.

# Review: sort_parents_before_daughters (github:theodoreOnzGit)

```toml
[kovan]
id = "review-sort-parents-before-daughters-theodoreonzgit-65fc83dc"
kind = "review"
origin = "human"
created = "2026-11-03T14:56:47+08:00"
modified = "2026-11-03T14:56:47+08:00"
target = "fn:65fc83dc4226d102"

[review]
path = "crates/boon-lay/src/triso_atops_fork/run_selection/mod.rs::sort_parents_before_daughters"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T14:56:47+08:00"
commit = "df5d00d3f962b37ad17a4b4936445a8abb92fe63"
hash = "sha256:6022a10a17bf072b5a59f7c050d40bbdf24ee661420a5b50571c9fe2a2184120"
doc_hash = "sha256:9e15d049d71610b499b5004e1496696337476ed057f60215309a9d3aba3120bc"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "generic_numerics"

[review.callees]
"fn:e1cb570a455c31b8" = "sha256:06a9c12470b244d22d5415441b5b842edce0c43e867d9a1bbc36b9c1b69988ca"

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
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_0129hXi6YzsN9yjw2A3YAEXR"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "7+hoF+D8DRFvtOK9cf+IfoGCefXmjN/4SS4WW3CKC7BVDwv+B0bOvhN/wAyxwQ79HjHvtZGXLq6TTG7fRGLRBw=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit df5d00d3f9.
