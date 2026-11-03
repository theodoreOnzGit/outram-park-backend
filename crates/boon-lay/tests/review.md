# Review record: crates/boon-lay/tests

```toml
[kovan]
id = "review-record-crates-boon-lay-tests"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: atops2026paper_against_the_papers_printed_releases (github:theodoreOnzGit)

```toml
[kovan]
id = "review-atops2026paper-against-the-papers-printed-releases-theodoreonzgit-9511bb6f"
kind = "review"
origin = "human"
created = "2026-11-03T16:49:28+08:00"
modified = "2026-11-03T16:49:28+08:00"
target = "fn:9511bb6f3a667d6e"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::atops2026paper_against_the_papers_printed_releases"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T16:49:29+08:00"
commit = "8f5f24a863b8ac7f74c53307aa48fa351484a45b"
hash = "sha256:0d9dafd6dd06d36a7b6612a14099543bd73b3d7b22d2e4177ffdc5144915f20a"
doc_hash = "sha256:02c4afe6bbdca37b16088976ccb16a16c7ab17acc4990f7a706e2fe9aa4ca24b"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:0a906b3f44704108" = "sha256:0477dcbfb31598d520ea9a9d75e03cdea025f049280a530db210ff1ea2e067e0"
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"
"fn:9ebac061e78bab77" = "sha256:6c3ed178717c77dab42ad2ed0faf5260d63634da983d3a19fc2353ad13b495e7"
"fn:b754ff5f79ad8f15" = "sha256:00edbc37903be951dbce5ac1e770eee50e2db7c005ea359b3a612e7b3cc237f4"
"fn:b872687bd30a967d" = "sha256:a0ebdf17d87a0897d282f8e148ec15e3648cb75a06fbbb453256f89835cd2337"
"fn:cdad051795ac6be8" = "sha256:a9dec292637eb605ac407d9868d6cad55812ee390eb9f93ac35c12775f64271e"
"fn:e95381509718b938" = "sha256:bd2d0163823daca920081b40fa42e0e840f5ff475e31dca41e6b9e9e87f11010"
"fn:fb5529d08b5edfef" = "sha256:b8d4303f659750e972945ee910c028255321b71167786ea163a0198687360e69"
"fn:fe6fb33576d54327" = "sha256:46f138d56ce352b44596af49695c291d91336b112f064f5295c2d69556342815"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "4RBuqInqRcEiBOh2jwKz9WD7UJd8xXVfjiGEkclfuswBjYy7puQP4ZurxZPUzsuuNUNfG7h8bI/kqvOtL2MVCA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

This reproduces the test (and prints outputs) for the TRISO atops, reproducing 
paper results, it was found that it uses upstream TRISO ATOPS settings for decay 
rather than paper settings

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 8f5f24a863.
