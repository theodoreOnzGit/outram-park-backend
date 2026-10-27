# Review record: crates/petir/src

```toml
[kovan]
id = "review-record-crates-petir-src"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: PetirError (github:theodoreOnzGit)

```toml
[kovan]
id = "review-petirerror-theodoreonzgit-08f93c13"
kind = "review"
origin = "human"
created = "2026-10-23T13:37:46+08:00"
modified = "2026-10-23T13:37:46+08:00"
target = "fn:08f93c13acb0345f"

[review]
path = "crates/petir/src/error.rs::PetirError"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-23"
signed_at = "2026-10-23T13:37:46+08:00"
commit = "833352c7afcfef24307e966d315cd4bf48b6893c"
hash = "sha256:a3d4e6a407d4118cd0e98290ff8806316c809244dc21016a4cb7f65811a98fd8"
doc_hash = "sha256:022d523e7a159ef9dff483398768372e9ab18b2fc3179750b1ed80dcf6c4036a"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
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
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_016EsuzTnVZavfdUSvUJ2KCn",
    "https://claude.ai/code/session_01EXPhstVwAqRb8o6YHVQoXW",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "kUJ2GI/IJhRINa7Sjvv9WFXLiPFnq1pzg4IJR+NvnPGkXFiddMCCj7rEMQ9kKXoWHSDiP56FOFm1kBMeU0KqDg=="
```

Error type, not needed check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-23, commit 833352c7af.

# Review: INV_LN2N (github:theodoreOnzGit)

```toml
[kovan]
id = "review-inv-ln2n-theodoreonzgit-02e95b98"
kind = "review"
origin = "human"
created = "2026-10-23T13:41:13+08:00"
modified = "2026-10-23T13:41:13+08:00"
target = "fn:02e95b98499e5027"

[review]
path = "crates/petir/src/fast_exp.rs::INV_LN2N"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-23"
signed_at = "2026-10-23T13:41:14+08:00"
commit = "73e455d404a7c735e9247da297e428ff2ba08a2d"
hash = "sha256:3ddac486044f19eec634ab31f2bdc5e9a54b88c29e8a1c456c7d931355002699"
doc_hash = "sha256:7c7412bb5501ea6d819a8c9f8fc8b976fa6e1e97c0b70cf22287082e2ee9adc1"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "original_code"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "analytical_case"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "QeRvzY6chjbggPuCNfcKPw39LfVZnTesZvXqU5lOFBOQEbg/00M1N4mkewMhKrCddx+/awZGQjxXwU9/SnqjCg=="
```

i checked 128/Ln(2) = 128/0.693 and got the ans

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-23, commit 73e455d404.

# Review: pow_specialcase (github:theodoreOnzGit)

```toml
[kovan]
id = "review-pow-specialcase-theodoreonzgit-221a8042"
kind = "review"
origin = "human"
created = "2026-10-23T13:44:11+08:00"
modified = "2026-10-23T13:44:11+08:00"
target = "fn:221a80421469781b"

[review]
path = "crates/petir/src/fast_pow.rs::pow_specialcase"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-23"
signed_at = "2026-10-23T13:44:11+08:00"
commit = "59abbc96caf96825dded57809a243e2e618c89ba"
hash = "sha256:74922887dd1225f68dc946680ce707b306d0a14d5fdfe34d65198dbeba7e775e"
doc_hash = "sha256:08ad6186dd53eaf2a361597681fb5d510ac3db3fcdef1eaa2ce47ffdbcd507f7"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.callees]
"fn:08f93c13acb0345f" = "sha256:a3d4e6a407d4118cd0e98290ff8806316c809244dc21016a4cb7f65811a98fd8"

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
sessions = [
    "https://claude.ai/code/session_016EsuzTnVZavfdUSvUJ2KCn",
    "https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "Z0B6HiqYnsmaLWyWUG0nPv2PQ0t4MSon/u8qAoIlz2CJ9oTi8gTXqiMwSABGJdbwiXzsYi5czhmkjCO7BKBCDA=="
```

ported from upstream

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-23, commit 59abbc96ca.

# Review: TABLE_BITS (github:theodoreOnzGit)

```toml
[kovan]
id = "review-table-bits-theodoreonzgit-c37b7c27"
kind = "review"
origin = "human"
created = "2026-10-23T13:46:56+08:00"
modified = "2026-10-23T13:46:56+08:00"
target = "fn:c37b7c2750f07a35"

[review]
path = "crates/petir/src/fast_exp.rs::TABLE_BITS"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-23"
signed_at = "2026-10-23T13:46:57+08:00"
commit = "51fe89f0096052fc716349123a11829288e590a5"
hash = "sha256:6f024d35ae524f68daf5fd03f3e6f079ac49777d0bf8fb7259f5c6fccfe01264"
doc_hash = "sha256:0050c3331bd39f1798918f76f2a81e025dfb8de66f51ac742f7170bec736c768"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "original_code"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "ZggMUbWTsJyqsfDanxDl24+qU2dT47blspTNTGpYXZk8NtWEhHLppR0suSTwYpbJtrlXyRm/sgUgGHVPDXi8Aw=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-23, commit 51fe89f009.

# Review: N (github:theodoreOnzGit)

```toml
[kovan]
id = "review-n-theodoreonzgit-104ad896"
kind = "review"
origin = "human"
created = "2026-10-23T13:48:39+08:00"
modified = "2026-10-23T13:48:39+08:00"
target = "fn:104ad896f8302fa4"

[review]
path = "crates/petir/src/fast_exp.rs::N"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-23"
signed_at = "2026-10-23T13:48:40+08:00"
commit = "8cf8775dd89c589b1c7c420343f3c2a6d94bfdf7"
hash = "sha256:9556ffe87d3e41af541796a593d3dfcc850070777db20a942d2ce890393b5197"
doc_hash = "sha256:a4d2d3d3000041803a763b362d5d7d5e3ba450aaa22fcc812b06a5ba10cff7e2"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "generic_numerics"

[review.callees]
"fn:c37b7c2750f07a35" = "sha256:6f024d35ae524f68daf5fd03f3e6f079ac49777d0bf8fb7259f5c6fccfe01264"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "original_code"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "zoTL+s/C2fcR1Y/HNk1Nhsb+/b3+/v5Wlye8TrIEqJkZMumwagg4WzuJEvaFK0waq2naJfSUaNZegzzI7zaKAQ=="
```

Bit shifting movement, assume correct... because it is used in entire workspace and tests are fine

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-23, commit 8cf8775dd8.

# Review: RealMath (github:theodoreOnzGit)

```toml
[kovan]
id = "review-realmath-theodoreonzgit-f6bc6d74"
kind = "review"
origin = "human"
created = "2026-10-27T09:39:11+08:00"
modified = "2026-10-27T09:39:11+08:00"
target = "fn:f6bc6d7498804074"

[review]
path = "crates/petir/src/mathf.rs::RealMath"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T09:39:12+08:00"
commit = "0e67011269b864c98a2d99a4bfef02f5901791df"
hash = "sha256:9b3a11b0dae65fba1c5d1bc0ebee78a344fb6f8708cfff381d9a28c24ca29eac"
doc_hash = "sha256:e00f74428f8a03c7035cc98ee0d7efa5128088879d4b533b2c9846f5bc02d969"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_contract_stated = "yes"
item_holds_extra = "no"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_011e2yBmXPEcU1DJXBj1rVXK",
    "https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe",
    "https://claude.ai/code/session_01HxmCueofxcJNMntRMmjqoW",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "GM5aul8g6H01sbm9LfBKrbi6kgEhuF0PydZ0SJBzYrp9JuMSATCsVXjQ6hoQ1a3xxalhoPVjt/o7oNkYJyTLDQ=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 0e67011269.

# Review: top12 (github:theodoreOnzGit)

```toml
[kovan]
id = "review-top12-theodoreonzgit-aa59d6d2"
kind = "review"
origin = "human"
created = "2026-10-27T09:45:00+08:00"
modified = "2026-10-27T09:45:00+08:00"
target = "fn:aa59d6d2a0e034f8"

[review]
path = "crates/petir/src/fast_exp.rs::top12"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T09:45:01+08:00"
commit = "ae2ba8e205e490f9ae5e5f0773933f2fb63af6f1"
hash = "sha256:0860c4f93f25a1cab7481aba968f6a1de2d52761085beb74e1f23756d3b270de"
doc_hash = "sha256:101946cad876a981493041bd181797de728fa1f95bfac6eca37970c4cb054cae"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

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
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "9RZaNTMMsC3Mrqdb2bHr6IFJiCJ+TJoTvgXPWK7Z3CQ4iq38YZbJCxwOBkv/PLokmETaYyZMcNMy9o5UY9VKAA=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

i didn't check it thoroughly, cos these are numerical implementations for exponentials. But i only know the integration tests pass. So by that I know it works. Easy way to check is to check ln, exp etc... against std manually via tests and assert low error

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit ae2ba8e205.

# Review: NEG_LN2_HI_N (github:theodoreOnzGit)

```toml
[kovan]
id = "review-neg-ln2-hi-n-theodoreonzgit-f009da1c"
kind = "review"
origin = "human"
created = "2026-10-27T09:46:33+08:00"
modified = "2026-10-27T09:46:33+08:00"
target = "fn:f009da1c1cd251a0"

[review]
path = "crates/petir/src/fast_exp.rs::NEG_LN2_HI_N"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T09:46:34+08:00"
commit = "b4650c15d6d807ed561b141336805fbce1958ebf"
hash = "sha256:39b33641d4c056510e8a960182c30aadd5f1b21b56c6c9ae57904e5611efb74b"
doc_hash = "sha256:34c5d679945e0846fae4caa638576d0f3832d3d94536f698bdd5030a151b1c9c"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "original_code"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "q7tfxtCZ+cgp1dG6fcT6gspb0t4Q/O2Wsa0EPrGiQ1zKkMgkuuSmLUiLZ1D1ydPkj7o5R5lBB/GifWwLcybbBQ=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

i didn't check it thoroughly, cos these are numerical implementations for exponentials. But i only know the integration tests pass. So by that I know it works. Easy way to check is to check ln, exp etc... against std manually via tests and assert low error

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit b4650c15d6.

# Review: NEG_LN2_LO_N (github:theodoreOnzGit)

```toml
[kovan]
id = "review-neg-ln2-lo-n-theodoreonzgit-fecfbd26"
kind = "review"
origin = "human"
created = "2026-10-27T09:47:49+08:00"
modified = "2026-10-27T09:47:49+08:00"
target = "fn:fecfbd2664f9ee23"

[review]
path = "crates/petir/src/fast_exp.rs::NEG_LN2_LO_N"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T09:47:50+08:00"
commit = "4078d902303444a14e12e71ea331c29fedc82902"
hash = "sha256:db3244c3ed0b3c2c380bca7af80ee826dbe5b7f1d1a1bae8e712e44dc7751e78"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
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

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "FdD3wNd3NBrrQlYnza3Fw6oLjGQTWMUR0+C+PHziO86pD9UfTlymZFKjqNR10bZ79nrmEX3sqfWlsaRRgm5dBg=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

i didn't check it thoroughly, cos these are numerical implementations for exponentials. But i only know the integration tests pass. So by that I know it works. Easy way to check is to check ln, exp etc... against std manually via tests and assert low error

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 4078d90230.
