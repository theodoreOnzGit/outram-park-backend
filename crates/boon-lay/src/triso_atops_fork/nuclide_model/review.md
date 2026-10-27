# Review record: crates/boon-lay/src/triso_atops_fork/nuclide_model

```toml
[kovan]
id = "review-record-crates-boon-lay-src-triso-atops-fork-nuclide-model"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: TrisoAtopsNuclide (github:theodoreOnzGit)

```toml
[kovan]
id = "review-trisoatopsnuclide-theodoreonzgit-546333a5"
kind = "review"
origin = "human"
created = "2026-10-22T16:54:36+08:00"
modified = "2026-10-22T16:54:36+08:00"
target = "fn:546333a5363bc5a2"

[review]
path = "crates/boon-lay/src/triso_atops_fork/nuclide_model/mod.rs::TrisoAtopsNuclide"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-22"
signed_at = "2026-10-22T16:54:37+08:00"
commit = "85d72a7d150af651b071d2d365040416aae0f0cf"
hash = "sha256:601d879e0b2947c8c69562c0278d67beccee30a0999a8eb86181de5ebcd65549"
doc_hash = "sha256:f45de76db1028eacbe9d5d680e4099148ff98c8a7a748c8d1188f7619f9df840"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_holds_extra = "no"
item_impossible_values = "no_checked_when_built"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "7osxvU6NPI3nyeCbXT1KJSpoN6JpVMl9sfAeLjljbzdiSP88/HRILE+KfEEY3Xh2mMq3Kzob1ElDNeHynScCAA=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-22, commit 85d72a7d15.

# Review: TrisoAtopsNuclide::decay_constant (github:theodoreOnzGit)

```toml
[kovan]
id = "review-decay-constant-theodoreonzgit-34ff47a5"
kind = "review"
origin = "human"
created = "2026-10-22T17:06:25+08:00"
modified = "2026-10-22T17:06:25+08:00"
target = "fn:34ff47a5448224c9"

[review]
path = "crates/boon-lay/src/triso_atops_fork/nuclide_model/mod.rs::TrisoAtopsNuclide::decay_constant"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-22"
signed_at = "2026-10-22T17:06:26+08:00"
commit = "9c13a3e5c9f71ff7c567782ba0c5288e7b8ec383"
hash = "sha256:1565117e190704e250537a267fcc591cbd6f4f74e8c74c62d6c58955f5437378"
doc_hash = "sha256:36eb9f1f43ffcb35a159439586cb3cea2e7d3870ac509d8ce8ca486e043fe6b9"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.callees]
"fn:546333a5363bc5a2" = "sha256:601d879e0b2947c8c69562c0278d67beccee30a0999a8eb86181de5ebcd65549"
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: decay constant lah, no need so complex"
independence = "someone_else"
limits_and_guards = "guard_waived_research: It's a simple decay constant lambda"
maintainability = "yes"
numerical_hazards = "other: again, it's not like we are going to use 0 half life here"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "afSRRxLeDYZnB9LvDjOqAFXlpF1dxu1md8BcSGdzQXQd4tm6/SkJOOM9GAxu1dnVLvJaQbAONg5JUdDP1mUdBA=="
```

half life only lah to decay constant

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-22, commit 9c13a3e5c9.

# Review: TrisoAtopsNuclide::from_seconds (github:theodoreOnzGit)

```toml
[kovan]
id = "review-from-seconds-theodoreonzgit-2604ea37"
kind = "review"
origin = "human"
created = "2026-10-23T13:29:18+08:00"
modified = "2026-10-23T13:29:18+08:00"
target = "fn:2604ea3760303a76"

[review]
path = "crates/boon-lay/src/triso_atops_fork/nuclide_model/mod.rs::TrisoAtopsNuclide::from_seconds"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-23"
signed_at = "2026-10-23T13:29:19+08:00"
commit = "a9cbcbde2790757a03de080636b0c499267cd9b4"
hash = "sha256:1ac14b2abf0c3d3ad35432d1002e4daa625c86de476671a892de028aed1d67a1"
doc_hash = "sha256:46cf54445ccd30af9eeda53e0695f5f8a18f80bc9f4cf02c24eccf056b7c1b2f"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.callees]
"fn:546333a5363bc5a2" = "sha256:601d879e0b2947c8c69562c0278d67beccee30a0999a8eb86181de5ebcd65549"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: research level, it's ok"
independence = "someone_else"
limits_and_guards = "guard_waived_research: it's simple enough to understand that we shouldn't put a negative number"
maintainability = "yes"
numerical_hazards = "none_found"
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
value = "55Xz+GTU62ybCLimti6awHcdvtHHxiVomX8SjF4eQusY8bQ4unuL9tvsZ6Xb+Ea1ZBA6/xZiXQqJdXI1qrkiCQ=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-23, commit a9cbcbde27.

# Review: supported_nuclides (github:theodoreOnzGit)

```toml
[kovan]
id = "review-supported-nuclides-theodoreonzgit-521d4591"
kind = "review"
origin = "human"
created = "2026-10-27T13:50:32+08:00"
modified = "2026-10-27T13:50:32+08:00"
target = "fn:521d4591f6090c64"

[review]
path = "crates/boon-lay/src/triso_atops_fork/nuclide_model/nuclide_database.rs::supported_nuclides"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T13:50:33+08:00"
commit = "2a1f858701e671cc4bfd4e0469e68ccad87656bd"
hash = "sha256:c1b5eab6019b4dca2762a7ff8deab0138105f6c7bbb60f7b2e91c1bb9aeb341e"
doc_hash = "sha256:07a941c96e165f4994f5ca69ea6aa56b7cdc63b64753b21398d8f8c128bcc043"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:2604ea3760303a76" = "sha256:1ac14b2abf0c3d3ad35432d1002e4daa625c86de476671a892de028aed1d67a1"
"fn:546333a5363bc5a2" = "sha256:601d879e0b2947c8c69562c0278d67beccee30a0999a8eb86181de5ebcd65549"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: ports upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: ports upstream behaviour"
maintainability = "yes"
numerical_hazards = "none_found"
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
value = "qbvy10nHhx+MN4nMsAlqlYJqn3mCHDA1VEaaLtiuLtHv4R61MnPt+p7tp4lWKTNWyy8RTn1PxTR7S3mLa/wiBw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

check against upstream, ports bugs for Tc99, ermm looks correct lei ( is there something wrong in comment)

Checked members, not A,Z and halflife (probabbly works, but not checking... tooo many numbers to check...) If numbers match upstream, i think is good enough

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 2a1f858701.

# Review: find_nuclide (github:theodoreOnzGit)

```toml
[kovan]
id = "review-find-nuclide-theodoreonzgit-e1cb570a"
kind = "review"
origin = "human"
created = "2026-10-27T13:52:28+08:00"
modified = "2026-10-27T13:52:28+08:00"
target = "fn:e1cb570a455c31b8"

[review]
path = "crates/boon-lay/src/triso_atops_fork/nuclide_model/nuclide_database.rs::find_nuclide"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T13:52:29+08:00"
commit = "dc8e7dd83f33712ab85cbe4e171cd98706674837"
hash = "sha256:06a9c12470b244d22d5415441b5b842edce0c43e867d9a1bbc36b9c1b69988ca"
doc_hash = "sha256:ce1c4e21313978a2797e4790adfcf6bbd1f932b9787285236fe888a7b955e53d"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.callees]
"fn:521d4591f6090c64" = "sha256:c1b5eab6019b4dca2762a7ff8deab0138105f6c7bbb60f7b2e91c1bb9aeb341e"
"fn:546333a5363bc5a2" = "sha256:601d879e0b2947c8c69562c0278d67beccee30a0999a8eb86181de5ebcd65549"

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
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "dKAlzaBb605MAjFs0yWeWDyAAUnCPFNlBHeA/08q6zGS76XnFY1gNessDdLe0UzTmbUW5fbHYjRQrqof6swnDg=="
```

helper function for nuclides

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit dc8e7dd83f.

# Review: TRISO_ATOPS_NUCLIDE_COUNT (github:theodoreOnzGit)

```toml
[kovan]
id = "review-triso-atops-nuclide-count-theodoreonzgit-26e142a3"
kind = "review"
origin = "human"
created = "2026-10-27T13:54:51+08:00"
modified = "2026-10-27T13:54:51+08:00"
target = "fn:26e142a3e0e9a27f"

[review]
path = "crates/boon-lay/src/triso_atops_fork/nuclide_model/nuclide_database.rs::TRISO_ATOPS_NUCLIDE_COUNT"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T13:54:52+08:00"
commit = "b2f644f8e9461dcf504fc4a2f21afe0a69fcac83"
hash = "sha256:ab2bfea2e83dfbf5804aa9f93f1af3c66fd2c1d6e6e2639fc74def842a6ed3f7"
doc_hash = "sha256:57a2dc7e89a67a535a954352e8b08ab98c057e3a9c2086470af16978dd96eab7"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "other: just a count of nuclides"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "l7Y28fKiaxXxFMMlXa2dCF2H2vEj8WvBmnemDSrYi0S7NWoS5aq0uHxQcxpEIc/RR+hjwAX+VjoO52zMuBpfCw=="
```

counted number of nuclide count, by pasting the entire
 python nuclides in libreoffice calc, counted that we have 84 nuclides

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit b2f644f8e9.
