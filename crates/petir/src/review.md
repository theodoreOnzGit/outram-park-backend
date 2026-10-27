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

# Review: POLY (github:theodoreOnzGit)

```toml
[kovan]
id = "review-poly-theodoreonzgit-b493e81f"
kind = "review"
origin = "human"
created = "2026-10-27T09:49:28+08:00"
modified = "2026-10-27T09:49:28+08:00"
target = "fn:b493e81f2c32888f"

[review]
path = "crates/petir/src/fast_exp.rs::POLY"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T09:49:29+08:00"
commit = "b537f424f037c969843541b6fd67138d1338d620"
hash = "sha256:8250add97cd5ef8e683017359d20da471e248af973c6c66cf554ea8f862678b6"
doc_hash = "sha256:465c84d1cdafe22e0a4b528bdf8c4f8b1d9fae70542263c3c436a67f14cdf44b"
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
value = "2lsd8hbyO0jGLfxYMHShN1/A95vo09KTZDMgB+cp+5Hefcj3F/kpwqHCg/97nKiXwDuvDBygmGS+9iOlXvCTDA=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

i didn't check it thoroughly, cos these are numerical implementations for exponentials. But i only know the integration tests pass. So by that I know it works. Easy way to check is to check ln, exp etc... against std manually via tests and assert low error

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit b537f424f0.

# Review: SHIFT (github:theodoreOnzGit)

```toml
[kovan]
id = "review-shift-theodoreonzgit-8a8edbd2"
kind = "review"
origin = "human"
created = "2026-10-27T09:50:11+08:00"
modified = "2026-10-27T09:50:11+08:00"
target = "fn:8a8edbd25eb705a5"

[review]
path = "crates/petir/src/fast_exp.rs::SHIFT"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T09:50:12+08:00"
commit = "b537f424f037c969843541b6fd67138d1338d620"
hash = "sha256:0cb43d27a403e21c0cbe885498aa83e066aa063ef9ab4d7816f3ba812f4e7fbc"
doc_hash = "sha256:1b0d6a5eb3b3a11dd0066c914bd93bd6d97959a1ddd6b36ff1eabab7ff094220"
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
value = "80o4EFzdgUe/ptlY0cgiWVIXqh6xSIq4HBvs27q/n2NKQz7hMPC0K/dDOVKgSg4USGpBXjZou7dONeUaUTLnBg=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

i didn't check it thoroughly, cos these are numerical implementations for exponentials. But i only know the integration tests pass. So by that I know it works. Easy way to check is to check ln, exp etc... against std manually via tests and assert low error

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit b537f424f0.

# Review: specialcase (github:theodoreOnzGit)

```toml
[kovan]
id = "review-specialcase-theodoreonzgit-35ce8461"
kind = "review"
origin = "human"
created = "2026-10-27T10:09:26+08:00"
modified = "2026-10-27T10:09:26+08:00"
target = "fn:35ce846190fc2d4b"

[review]
path = "crates/petir/src/fast_exp.rs::specialcase"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:09:28+08:00"
commit = "62138e0e2d7729558db82c00b0fa7b281ec4c758"
hash = "sha256:5b0b66e99ef3f6d6ed1118dfda21ed9df6f6e2b19c0e6d0e237c91492bf5e486"
doc_hash = "sha256:6fe0ba7b748f63731ddd2c0556d58312453ddcd4c8f5a05ac677f17aed840fbe"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

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
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_016EsuzTnVZavfdUSvUJ2KCn",
    "https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "47xlUe2hikUZZmy9hkKZlEqVXUXjypzoHv0yv8IGltb0ErH5VGR1M8OZ7H/lj3mLSZVHdwBomauASnaU/7rrDw=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

i didn't check it thoroughly, cos these are numerical implementations for exponentials. But i only know the integration tests pass. So by that I know it works. Easy way to check is to check ln, exp etc... against std manually via tests and assert low error

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 62138e0e2d.

# Review: TAB (github:theodoreOnzGit)

```toml
[kovan]
id = "review-tab-theodoreonzgit-cf5d9f86"
kind = "review"
origin = "human"
created = "2026-10-27T10:11:01+08:00"
modified = "2026-10-27T10:11:34+08:00"
target = "fn:cf5d9f8646da487f"

[review]
path = "crates/petir/src/fast_exp.rs::TAB"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:11:34+08:00"
commit = "ec310448870e5498c8314366d7160ce53ffa4a3e"
hash = "sha256:faeacadcc7eaa20fa65ecdba4cdff63c38953e439f4a72492ed5dd7803f3e7dd"
doc_hash = "sha256:7ce028f3e8656d6adf8644555f5bc5b8fbb57017b4c32d5c10095b2512e3cb44"
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
value = "LV3xdmkKO4em6ExxzlyWkgepRAqBTZ0DY4gP+NILVBuKs7OzVj1gtYt+NpYS6nY+rpYDUSWA3/1a3//mlT6rBg=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

i didn't check it thoroughly, cos these are numerical implementations for exponentials. But i only know the integration tests pass. So by that I know it works. Easy way to check is to check ln, exp etc... against std manually via tests and assert low error

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit ec31044887.

# Review: exp_inner (github:theodoreOnzGit)

```toml
[kovan]
id = "review-exp-inner-theodoreonzgit-a35dd137"
kind = "review"
origin = "human"
created = "2026-10-27T10:12:52+08:00"
modified = "2026-10-27T10:12:52+08:00"
target = "fn:a35dd137eb82556b"

[review]
path = "crates/petir/src/fast_exp.rs::exp_inner"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:12:53+08:00"
commit = "ce7b25865e1ecccb6d7292a82120e9e3bc435ef4"
hash = "sha256:3f1324f589067609d28cf9b149e2c704be0f4333667ea2953e45f76dabe875db"
doc_hash = "sha256:1d78b1bcc841b924753e7073450d28c6923621bfbcbf4bee927e6878a42fe6e6"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:02e95b98499e5027" = "sha256:3ddac486044f19eec634ab31f2bdc5e9a54b88c29e8a1c456c7d931355002699"
"fn:08f93c13acb0345f" = "sha256:a3d4e6a407d4118cd0e98290ff8806316c809244dc21016a4cb7f65811a98fd8"
"fn:104ad896f8302fa4" = "sha256:9556ffe87d3e41af541796a593d3dfcc850070777db20a942d2ce890393b5197"
"fn:35ce846190fc2d4b" = "sha256:5b0b66e99ef3f6d6ed1118dfda21ed9df6f6e2b19c0e6d0e237c91492bf5e486"
"fn:8a8edbd25eb705a5" = "sha256:0cb43d27a403e21c0cbe885498aa83e066aa063ef9ab4d7816f3ba812f4e7fbc"
"fn:aa59d6d2a0e034f8" = "sha256:0860c4f93f25a1cab7481aba968f6a1de2d52761085beb74e1f23756d3b270de"
"fn:b493e81f2c32888f" = "sha256:8250add97cd5ef8e683017359d20da471e248af973c6c66cf554ea8f862678b6"
"fn:c37b7c2750f07a35" = "sha256:6f024d35ae524f68daf5fd03f3e6f079ac49777d0bf8fb7259f5c6fccfe01264"
"fn:cf5d9f8646da487f" = "sha256:faeacadcc7eaa20fa65ecdba4cdff63c38953e439f4a72492ed5dd7803f3e7dd"
"fn:f009da1c1cd251a0" = "sha256:39b33641d4c056510e8a960182c30aadd5f1b21b56c6c9ae57904e5611efb74b"
"fn:fecfbd2664f9ee23" = "sha256:db3244c3ed0b3c2c380bca7af80ee826dbe5b7f1d1a1bae8e712e44dc7751e78"

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
sessions = ["https://claude.ai/code/session_016EsuzTnVZavfdUSvUJ2KCn"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "h3C5+yIMbBKQmkG84ZSY3hnEMUsGdrzpK0jwHNJRHBWTvkYeT8KG3nmLDr9zbf4mKeFRBLasTDK9E3zDjSltAQ=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

i didn't check it thoroughly, cos these are numerical implementations for exponentials. But i only know the integration tests pass. So by that I know it works. Easy way to check is to check ln, exp etc... against std manually via tests and assert low error

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit ce7b25865e.

# Review: Result (github:theodoreOnzGit)

```toml
[kovan]
id = "review-result-theodoreonzgit-d617fbdf"
kind = "review"
origin = "human"
created = "2026-10-27T10:14:25+08:00"
modified = "2026-10-27T10:14:25+08:00"
target = "fn:d617fbdffd394aec"

[review]
path = "crates/petir/src/error.rs::Result"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:14:25+08:00"
commit = "48134bf39f9369f0dde5c9035ccd554d0dcbd347"
hash = "sha256:820f8eea66f7155cce3213bde598d9463a70f3894b16ae9ca952687c719bd9d8"
doc_hash = "sha256:9f0742ee35b94794607c08e218e1a30968a75a237e7601dd4ce5bcc3f59907fc"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.callees]
"fn:08f93c13acb0345f" = "sha256:a3d4e6a407d4118cd0e98290ff8806316c809244dc21016a4cb7f65811a98fd8"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "WbLQa56gVREtOXz3741lotL2o2rgm7q6dIYgY6rhXNjuTV5nEmhPG6DnC23Y6B4Ti8b+16TUgX3lcDSlexnBBg=="
```

Just an alias

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 48134bf39f.

# Review: exp_ieee (github:theodoreOnzGit)

```toml
[kovan]
id = "review-exp-ieee-theodoreonzgit-736bf673"
kind = "review"
origin = "human"
created = "2026-10-27T10:16:17+08:00"
modified = "2026-10-27T10:16:17+08:00"
target = "fn:736bf673e39f3a8c"

[review]
path = "crates/petir/src/fast_exp.rs::exp_ieee"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:16:18+08:00"
commit = "749c168f0a6dc6ec9c3d64eedec2aa7c8b9ecffb"
hash = "sha256:232800c8f5eec8f0bfa11d66de5b1c233dca169736ce729d12e206e76890f1e8"
doc_hash = "sha256:8b7fef71b15fdbb95f073c6bcf6f433b5f3ebee74838112ccfdb7beb28623da4"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.callees]
"fn:a35dd137eb82556b" = "sha256:3f1324f589067609d28cf9b149e2c704be0f4333667ea2953e45f76dabe875db"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream behaviour, even f64 exp does not do checks on boundedness"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream behaviour, even f64 exp does not do checks on boundedness"
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
value = "h/VCDpuWokiGq1/KPyg8p9786tEeg1ZulpH3f/Gw/2LLNe3k0sWn6+MK9I1cYVC2zEMXPDAqPNb9PNtq7ufvAQ=="
```

just to unwrap the result for convenience

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 749c168f0a.

# Review: exp (github:theodoreOnzGit)

```toml
[kovan]
id = "review-exp-theodoreonzgit-821b9ab2"
kind = "review"
origin = "human"
created = "2026-10-27T10:17:08+08:00"
modified = "2026-10-27T10:17:08+08:00"
target = "fn:821b9ab2fcd8a610"

[review]
path = "crates/petir/src/fast_exp.rs::exp"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:17:09+08:00"
commit = "749c168f0a6dc6ec9c3d64eedec2aa7c8b9ecffb"
hash = "sha256:74a51af43189b1073897cb6a0a2ef417cb633f01654ca0ca90cc036cf7996d46"
doc_hash = "sha256:47a8344f1d47d98e0ebdf7ac53c4ae98c3c470fba860609fc962e45705c468b5"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.callees]
"fn:a35dd137eb82556b" = "sha256:3f1324f589067609d28cf9b149e2c704be0f4333667ea2953e45f76dabe875db"
"fn:d617fbdffd394aec" = "sha256:820f8eea66f7155cce3213bde598d9463a70f3894b16ae9ca952687c719bd9d8"

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
sessions = [
    "https://claude.ai/code/session_016EsuzTnVZavfdUSvUJ2KCn",
    "https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "l2KGg5xUZprkq+Ak2KzrHvQEEdcTKPcKOAmmm+OjVzGvlE5xPWfBwuI5753A1tUNRn/BauSYkCLBhUKJg/yiAQ=="
```

just returns result, plumbing

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 749c168f0a.

# Review: exp (github:theodoreOnzGit)

```toml
[kovan]
id = "review-exp-theodoreonzgit-20504b49"
kind = "review"
origin = "human"
created = "2026-10-27T10:18:52+08:00"
modified = "2026-10-27T10:18:52+08:00"
target = "fn:20504b494388607e"

[review]
path = "crates/petir/src/real.rs::exp"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:18:53+08:00"
commit = "7b6156000c47a9fb28d2448de55180dcad640314"
hash = "sha256:7301788b66338921d1071bb4aaf5c29a93fbc5fa48ec86024a794384a20cdd53"
doc_hash = "sha256:6e097fce0133e9dc352b422a35bddad06f56628d0c27682f4dff85fd64f83dd0"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.callees]
"fn:736bf673e39f3a8c" = "sha256:232800c8f5eec8f0bfa11d66de5b1c233dca169736ce729d12e206e76890f1e8"

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
value = "oMqMkL67BlQlG92vGe8CHC9ouB/oAjd3wyNjrgHQ7CVBXZmMFM2clKZehdmY3QZDW0qrluJ1INKwZADHUu8yDw=="
```

just a trait wrapper

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 7b6156000c.

# Review: f64::r_exp (github:theodoreOnzGit)

```toml
[kovan]
id = "review-r-exp-theodoreonzgit-d2f19c5a"
kind = "review"
origin = "human"
created = "2026-10-27T10:20:34+08:00"
modified = "2026-10-27T10:20:34+08:00"
target = "fn:d2f19c5ac1bd0c10"

[review]
path = "crates/petir/src/mathf.rs::f64::r_exp"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:20:34+08:00"
commit = "fc5ce0aaaf2b68b819236482e3b1aea9d7d04b12"
hash = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.callees]
"fn:20504b494388607e" = "sha256:7301788b66338921d1071bb4aaf5c29a93fbc5fa48ec86024a794384a20cdd53"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream behaviour similar to std library"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream behaviour similar to std library"
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
value = "hJ3W894TdtC/VXXWHCLnJCNYMI/9/3pdnCgpJnIHnE8SJ3/jZ0SyYCBSURYcau3jE1BX3+x8JqSvaW0nm7ONBQ=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit fc5ce0aaaf.

# Review: RealMath::r_ln (github:theodoreOnzGit)

```toml
[kovan]
id = "review-r-ln-theodoreonzgit-f65fbba1"
kind = "review"
origin = "human"
created = "2026-10-27T10:21:39+08:00"
modified = "2026-10-27T10:21:39+08:00"
target = "fn:f65fbba1c5e66aca"

[review]
path = "crates/petir/src/mathf.rs::RealMath::r_ln"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:21:40+08:00"
commit = "32ba57a9cfdacdbc81f6be69dfee5aad411411bc"
hash = "sha256:0b939b6d08ab806ef4167f8e713e246f0932e482b6f636961bd3cd0cd571bedb"
doc_hash = "sha256:badcd6631f8f876045674c57af48b8a91828d0de29c24d73ba8a2241618b297a"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.callees]
"fn:f6bc6d7498804074" = "sha256:9b3a11b0dae65fba1c5d1bc0ebee78a344fb6f8708cfff381d9a28c24ca29eac"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "other: trait"
independence = "someone_else"
limits_and_guards = "other: trait"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_011e2yBmXPEcU1DJXBj1rVXK",
    "https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "WaC78nTxiKB86M+z2XynUCnkYB94gLT10juz6uJ2qdrd+ZbP7WFViZsS05uOMWeEyQwqCXuVjfLMUh8Ou9iAAg=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 32ba57a9cf.

# Review: top16 (github:theodoreOnzGit)

```toml
[kovan]
id = "review-top16-theodoreonzgit-2f2a1115"
kind = "review"
origin = "human"
created = "2026-10-27T10:24:04+08:00"
modified = "2026-10-27T10:24:04+08:00"
target = "fn:2f2a1115e8ad0d6b"

[review]
path = "crates/petir/src/fast_log.rs::top16"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:24:05+08:00"
commit = "208c51c70608805f96f94cf41745dbbfc24780cf"
hash = "sha256:c465535d58459eb43465c178ef42189077d7d38cc05567278e44005829fbb2f9"
doc_hash = "sha256:4de149d7e354b6bf70c3821f5cb57c964388f15d0b7513c5161ee27363554172"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream behaviour"
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
value = "6kjIK+xKVVEfUYDuFRdhayYPT1VVPhPwMa3Wdp8G719ClKf9QjPIuM0+qRFRjNwTVXhIX8EujGZQszlK4nwYBg=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 208c51c706.

# Review: TABLE_BITS (github:theodoreOnzGit)

```toml
[kovan]
id = "review-table-bits-theodoreonzgit-70b3047a"
kind = "review"
origin = "human"
created = "2026-10-27T10:25:37+08:00"
modified = "2026-10-27T10:26:10+08:00"
target = "fn:70b3047a2e010694"

[review]
path = "crates/petir/src/fast_log.rs::TABLE_BITS"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:26:10+08:00"
commit = "fedd77e8d1b5c5f60a3ebe9b275c4557a50d8347"
hash = "sha256:0ff679b66db05902f416dbd4f3aed3348e0606394b6db8e60b0864a63dc967c9"
doc_hash = "sha256:acb365202338dbcd9a8c82b22a5413c67630a248b50b1ebde592794964ce779c"
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
value = "vy6U/vAUlndkOsMqSn99+TsaENX6Xq6veCJCovsNcaNEwnrKbKzdu5LdHBhxdOi9OKf4PhkCQkqxFEiwFt0NAQ=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit fedd77e8d1.

# Review: OFF (github:theodoreOnzGit)

```toml
[kovan]
id = "review-off-theodoreonzgit-c273ee3b"
kind = "review"
origin = "human"
created = "2026-10-27T10:27:35+08:00"
modified = "2026-10-27T10:27:35+08:00"
target = "fn:c273ee3b6bd4de00"

[review]
path = "crates/petir/src/fast_log.rs::OFF"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:27:35+08:00"
commit = "352ea14410c66b273c662cb9fb49d933f9e628de"
hash = "sha256:a82d55590b31c265a749a5721db003fd70d22024f2e46399262069e8e3730211"
doc_hash = "sha256:0b9f27d561ce0144ba7e00d08e0274c83230fd6962a10d46d3de4c5754a2f136"
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
value = "C7LgK+n99zx0sWLsna+Pl4h9DttvmYwgKzqX9QOz36NsNSrTwfiMPU2gI+3V15DnA4dTihlfI3aWATjDhS4YDw=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 352ea14410.

# Review: LO (github:theodoreOnzGit)

```toml
[kovan]
id = "review-lo-theodoreonzgit-25fe3d3d"
kind = "review"
origin = "human"
created = "2026-10-27T10:28:21+08:00"
modified = "2026-10-27T10:28:21+08:00"
target = "fn:25fe3d3decf7ce49"

[review]
path = "crates/petir/src/fast_log.rs::LO"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:28:22+08:00"
commit = "352ea14410c66b273c662cb9fb49d933f9e628de"
hash = "sha256:201dddfd92068ea32bea7667272258723820625d19c6c5b1931d3f6dafd29cda"
doc_hash = "sha256:7fc48369fb6f9cd369b7e932ca9d9589fd788905cda701e3131e325be7b1014b"
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
value = "4ceppxVIL7CmJRuArGtaXZ221SJ3c+XrjqH34qCLmGSKWvO5fat4p1aZw8wA/BsayGZyP/iCYVBW6KdUJyJ7AA=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 352ea14410.

# Review: HI (github:theodoreOnzGit)

```toml
[kovan]
id = "review-hi-theodoreonzgit-22cb4bc6"
kind = "review"
origin = "human"
created = "2026-10-27T10:29:33+08:00"
modified = "2026-10-27T10:29:33+08:00"
target = "fn:22cb4bc6a2a77661"

[review]
path = "crates/petir/src/fast_log.rs::HI"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:29:34+08:00"
commit = "33254d3a59f178eba9fa8d8c85ef9c63796ebdd9"
hash = "sha256:8ff28653083fc3a6803bd690320f169af860616c1dd7930ccd01d5c24845c4ca"
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
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "B1UwizKyfnG9sp8C5C7IGTUyk3oadiIhk3/lUsT3JQXxQnPcmAYK3OTgyf3CH0v+sztsPOXesX9jUWAxfxaZDw=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 33254d3a59.

# Review: LN2_HI (github:theodoreOnzGit)

```toml
[kovan]
id = "review-ln2-hi-theodoreonzgit-55f5867c"
kind = "review"
origin = "human"
created = "2026-10-27T10:30:45+08:00"
modified = "2026-10-27T10:30:45+08:00"
target = "fn:55f5867c7efea93a"

[review]
path = "crates/petir/src/fast_log.rs::LN2_HI"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:30:46+08:00"
commit = "7111eb627558fb4ce044742f70403b06f7cb75b8"
hash = "sha256:944a61c34d62f4032d260fb1b08e3ff8b0a07b896fc503d1f1244ec1fb1ead72"
doc_hash = "sha256:e70f75a903be71f7945ff61e226139e70f598bc20f43c03b566e388488f0f831"
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
value = "gxW4clY1PZFowLOjY5KrueY/nmxYO8z8SDLPy/qXR/1/qXV+rtaY9eE6gj2VLq8Krlgg6/robOwpWNkvAzMACQ=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 7111eb6275.

# Review: LN2_LO (github:theodoreOnzGit)

```toml
[kovan]
id = "review-ln2-lo-theodoreonzgit-73252081"
kind = "review"
origin = "human"
created = "2026-10-27T10:34:21+08:00"
modified = "2026-10-27T10:34:21+08:00"
target = "fn:73252081d22789bd"

[review]
path = "crates/petir/src/fast_log.rs::LN2_LO"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:34:22+08:00"
commit = "843f0965152e08ff1c53a4ba25d40e8dc1c110e7"
hash = "sha256:74c491b1f00f48a95a42c7359a920aa2ca7289166f69d0d6a3dd347995bc9012"
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
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "M3SvMqGPl7HqSda4DFtzMJOQ8Cg7FzocQp80k8K0Kj5nnvMd47FBML7Hx/McbQn4AudrDjp6oGCq3n3/mat7Aw=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 843f096515.

# Review: POLY (github:theodoreOnzGit)

```toml
[kovan]
id = "review-poly-theodoreonzgit-29e331bf"
kind = "review"
origin = "human"
created = "2026-10-27T10:38:05+08:00"
modified = "2026-10-27T10:38:05+08:00"
target = "fn:29e331bfde2a9f99"

[review]
path = "crates/petir/src/fast_log.rs::POLY"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:38:07+08:00"
commit = "0424fcb15e09802006ae69e3afa7992bfd48e576"
hash = "sha256:6f63ce511aa945c00619b63706054e156343b83b1fa95170719eccb0699a825d"
doc_hash = "sha256:11384761902db08634c579b8e99601fa3749ecb800a2315495168a79a8cc43dd"
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
value = "4l0IFuRdcmiTX9oW562oBRm4PR3dkAZ14kkQEgFUcTOnlW7Qv+uh7zt/Y3H2iIQjYuTX3GhthOjvsmaqKnJYCw=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 0424fcb15e.

# Review: POLY1 (github:theodoreOnzGit)

```toml
[kovan]
id = "review-poly1-theodoreonzgit-e3223da1"
kind = "review"
origin = "human"
created = "2026-10-27T10:42:23+08:00"
modified = "2026-10-27T10:42:23+08:00"
target = "fn:e3223da1526eb451"

[review]
path = "crates/petir/src/fast_log.rs::POLY1"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:42:24+08:00"
commit = "489ad53cfc1417cde16ee20f5ecfb4ebd9f2d549"
hash = "sha256:6d4b1ce530888897af78e644e7b5b2b3654f2d4974698f3562e4c34cd60bbc06"
doc_hash = "sha256:1bed854ab9bba3e6f917f1bc5584fc74ecafd2e15b8f5160776088786c5b0f8c"
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
value = "Of5NCmH9lSfG/OXri6llaZydmZzw4eV1BB30caF90u748lmaCh/6O9c3OvvovazVlgyt7sz40JWd6gzefiDhDQ=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 489ad53cfc.

# Review: TAB (github:theodoreOnzGit)

```toml
[kovan]
id = "review-tab-theodoreonzgit-aa6160a4"
kind = "review"
origin = "human"
created = "2026-10-27T10:47:42+08:00"
modified = "2026-10-27T10:47:42+08:00"
target = "fn:aa6160a4a299a109"

[review]
path = "crates/petir/src/fast_log.rs::TAB"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:47:42+08:00"
commit = "9a058f381bd1dc437493ec3a550ba75df7074365"
hash = "sha256:e70799c146f2a442f8fcf13ad2a2f2f18c8a8d282bf80cc2ca82355dd18d9bc4"
doc_hash = "sha256:3410733d7071e8ca8b3e5ba801aec93b137b39958b8fee3df83462264752b32c"
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
value = "UdZSjY8xxavro6GuhlWcrptx1Q8+sL1FNUN+6C38jOU1DvibmyfziTmE7LBs2lwq+SYtryyd6VlY+2yyA8wUAw=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 9a058f381b.

# Review: TAB2 (github:theodoreOnzGit)

```toml
[kovan]
id = "review-tab2-theodoreonzgit-8f9dcf7f"
kind = "review"
origin = "human"
created = "2026-10-27T10:49:14+08:00"
modified = "2026-10-27T10:49:14+08:00"
target = "fn:8f9dcf7fdeb05d87"

[review]
path = "crates/petir/src/fast_log.rs::TAB2"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:49:15+08:00"
commit = "39c36dd1543fc409d74dbbd7bd5cdca5ffe992ac"
hash = "sha256:34adebb4cb4886461e81049bfe15739e9d7ee3dca133f41077c3921b1537fb19"
doc_hash = "sha256:fd63745334c147a9403105f05651e60c848bfb20f5dbf6248773b33fdd53af74"
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
value = "O0xQ7kPgG3364GtrjeBYQskJl9PWAaBUaJYgFLvdVRcT009SDMwF/z/klWcAoJQF0VpxInfEAIjdEkA+kloVAA=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 39c36dd154.

# Review: N (github:theodoreOnzGit)

```toml
[kovan]
id = "review-n-theodoreonzgit-1d314ac3"
kind = "review"
origin = "human"
created = "2026-10-27T10:51:10+08:00"
modified = "2026-10-27T10:52:01+08:00"
target = "fn:1d314ac39142cca5"

[review]
path = "crates/petir/src/fast_log.rs::N"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:52:02+08:00"
commit = "3168572d4064756f6112e6a5bac2c88ead4296f4"
hash = "sha256:eb823a8426d05ee336eddd6289d681d14780401beec0088474b57a8b4e720d75"
doc_hash = "sha256:5a3c80fbe7dc0e9fe5a6fe49c9f1964cd61bde4c7df2b0acc3900e1a84b8db1c"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:70b3047a2e010694" = "sha256:0ff679b66db05902f416dbd4f3aed3348e0606394b6db8e60b0864a63dc967c9"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "original_code"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.signature]
key = "k1"
alg = "ed25519"
value = "KSloGM5EtRCYdybxeWmG3/4NhhoXIjaDb0qfWARQLOWzQeLCmTrT3L3xvpWfVG+VOvfmES29b1aN1M+7PRGACA=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 3168572d40.

# Review: ln_inner (github:theodoreOnzGit)

```toml
[kovan]
id = "review-ln-inner-theodoreonzgit-a582eca0"
kind = "review"
origin = "human"
created = "2026-10-27T10:53:47+08:00"
modified = "2026-10-27T10:53:47+08:00"
target = "fn:a582eca0da7ab3eb"

[review]
path = "crates/petir/src/fast_log.rs::ln_inner"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T10:53:48+08:00"
commit = "e8a3ed28bbebe120c9c44488610eaa9ff5a8bb5e"
hash = "sha256:4eba12e0784d5d24c29e6a8876cee433dcb90c78abd35902000d5154b304a021"
doc_hash = "sha256:504a3234f9b9e290630377248667f7801df933ed634b5b42561989efc9721d32"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:08f93c13acb0345f" = "sha256:a3d4e6a407d4118cd0e98290ff8806316c809244dc21016a4cb7f65811a98fd8"
"fn:1d314ac39142cca5" = "sha256:eb823a8426d05ee336eddd6289d681d14780401beec0088474b57a8b4e720d75"
"fn:22cb4bc6a2a77661" = "sha256:8ff28653083fc3a6803bd690320f169af860616c1dd7930ccd01d5c24845c4ca"
"fn:25fe3d3decf7ce49" = "sha256:201dddfd92068ea32bea7667272258723820625d19c6c5b1931d3f6dafd29cda"
"fn:29e331bfde2a9f99" = "sha256:6f63ce511aa945c00619b63706054e156343b83b1fa95170719eccb0699a825d"
"fn:2f2a1115e8ad0d6b" = "sha256:c465535d58459eb43465c178ef42189077d7d38cc05567278e44005829fbb2f9"
"fn:55f5867c7efea93a" = "sha256:944a61c34d62f4032d260fb1b08e3ff8b0a07b896fc503d1f1244ec1fb1ead72"
"fn:70b3047a2e010694" = "sha256:0ff679b66db05902f416dbd4f3aed3348e0606394b6db8e60b0864a63dc967c9"
"fn:73252081d22789bd" = "sha256:74c491b1f00f48a95a42c7359a920aa2ca7289166f69d0d6a3dd347995bc9012"
"fn:8f9dcf7fdeb05d87" = "sha256:34adebb4cb4886461e81049bfe15739e9d7ee3dca133f41077c3921b1537fb19"
"fn:aa6160a4a299a109" = "sha256:e70799c146f2a442f8fcf13ad2a2f2f18c8a8d282bf80cc2ca82355dd18d9bc4"
"fn:c273ee3b6bd4de00" = "sha256:a82d55590b31c265a749a5721db003fd70d22024f2e46399262069e8e3730211"
"fn:e3223da1526eb451" = "sha256:6d4b1ce530888897af78e644e7b5b2b3654f2d4974698f3562e4c34cd60bbc06"

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
sessions = ["https://claude.ai/code/session_016EsuzTnVZavfdUSvUJ2KCn"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "sQIY6aoLyzdCkCF+FapcyA0Qm1H8j36PtDrrVypzEpWHlNINvc8QaMOxHWtm34cmMPmhVppcov4y17W+iXgwAA=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit e8a3ed28bb.

# Review: ln (github:theodoreOnzGit)

```toml
[kovan]
id = "review-ln-theodoreonzgit-fcdb6720"
kind = "review"
origin = "human"
created = "2026-10-27T12:00:46+08:00"
modified = "2026-10-27T12:00:46+08:00"
target = "fn:fcdb672040f501d8"

[review]
path = "crates/petir/src/fast_log.rs::ln"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T12:00:47+08:00"
commit = "d448f29da64bf901f1d7f46c516844bd0147f428"
hash = "sha256:3cfa21da8f0bd9f0a82b91879d385df46108849fb4aaa7f0e3adcc8232ca98f0"
doc_hash = "sha256:d977e09ece2a88de58902a2242f2aa2716f616b862b1a89b82dfa08b725e95d0"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:a582eca0da7ab3eb" = "sha256:4eba12e0784d5d24c29e6a8876cee433dcb90c78abd35902000d5154b304a021"
"fn:d617fbdffd394aec" = "sha256:820f8eea66f7155cce3213bde598d9463a70f3894b16ae9ca952687c719bd9d8"

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
value = "90G/+/HbhmvMBab/lgw2liPi12wWlx9qNyLTXBbxj9OwBy/wK2xxeQnHdzHUNzxVqsNy5hvQGn62Xar4+O+OBw=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

I'm not reviewing this in depth, this was ported, but one can see if behaviour is similar to ln in an integration test. Easy to check

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit d448f29da6.

# Review: ln_ieee (github:theodoreOnzGit)

```toml
[kovan]
id = "review-ln-ieee-theodoreonzgit-ef0af16c"
kind = "review"
origin = "human"
created = "2026-10-27T12:05:39+08:00"
modified = "2026-10-27T12:05:39+08:00"
target = "fn:ef0af16c3f734b81"

[review]
path = "crates/petir/src/fast_log.rs::ln_ieee"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T12:05:42+08:00"
commit = "d448f29da64bf901f1d7f46c516844bd0147f428"
hash = "sha256:7f04b7bd8596eaee5f6d5a6e7b694a8aafd95cdbf92f00b1ab2f50a7634b0a9b"
doc_hash = "sha256:61aa0eeb30b62fd390a0f13e7e81ad4f61e8ba23ac0cf3f92c95f8a5a586da4b"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:a582eca0da7ab3eb" = "sha256:4eba12e0784d5d24c29e6a8876cee433dcb90c78abd35902000d5154b304a021"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream behaviour"
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
value = "PRf+uqdPh69yhSgLcdM92Jbw2AxoQm8t076w4HlHF/EToEUQP7ePabQ7HV2GGke7hmeVE07JN5CtzUs/LGcdDw=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

ln not to be tested now, in future can manually compare with std::f64::ln, but testing has been automated already. doesn't break workspace behaviour.

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit d448f29da6.

# Review: ln (github:theodoreOnzGit)

```toml
[kovan]
id = "review-ln-theodoreonzgit-f420940c"
kind = "review"
origin = "human"
created = "2026-10-27T12:10:27+08:00"
modified = "2026-10-27T12:10:27+08:00"
target = "fn:f420940c421868d5"

[review]
path = "crates/petir/src/real.rs::ln"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T12:10:28+08:00"
commit = "af02f81620b60bea5e5ea05fa77f3e7b26d95ada"
hash = "sha256:c77682df9fcfb54754c3495c5f4273e55c29db18b1e3932c70896aadd3831928"
doc_hash = "sha256:c1c2b7e19b0eec592045469ad8310cd28ff851613cff2b3a6da0f60246b37ef2"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:ef0af16c3f734b81" = "sha256:7f04b7bd8596eaee5f6d5a6e7b694a8aafd95cdbf92f00b1ab2f50a7634b0a9b"

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
value = "lAncSGNR7L4fCgRaHTDILCM5s0mNeAKMyBSDAPRlkm7iDxgxXuEicX/NmFO643jLMMpWyEyCqXGx1NJPupssCQ=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

ln can be tested against other libraries, i think it was

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit af02f81620.

# Review: f64::r_ln (github:theodoreOnzGit)

```toml
[kovan]
id = "review-r-ln-theodoreonzgit-7d2c37be"
kind = "review"
origin = "human"
created = "2026-10-27T12:21:30+08:00"
modified = "2026-10-27T12:21:30+08:00"
target = "fn:7d2c37be6eb3e8f3"

[review]
path = "crates/petir/src/mathf.rs::f64::r_ln"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T12:21:32+08:00"
commit = "4f22149e71cde284d031a2668a76591df4444a38"
hash = "sha256:e03a479bf5c1837af8e2aef9589eb6759382a67e74340d003406d4abfac2caa0"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:f420940c421868d5" = "sha256:c77682df9fcfb54754c3495c5f4273e55c29db18b1e3932c70896aadd3831928"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream behaviour"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "FWvHeH8FB30LOayucj+lpwnC864jpaS6wy9W7rccHjcAo/0l2H34AtsnLHRLDfYSTdqsjStQsKIf0mcJNL8AAQ=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

ln is not going to be tested manually now, can be tested against std::f64 in some other crates perhaps

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 4f22149e71.

# Review: powi (github:theodoreOnzGit)

```toml
[kovan]
id = "review-powi-theodoreonzgit-1f2f25c2"
kind = "review"
origin = "human"
created = "2026-10-27T12:27:42+08:00"
modified = "2026-10-27T12:27:42+08:00"
target = "fn:1f2f25c2acab772b"

[review]
path = "crates/petir/src/real.rs::powi"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T12:27:43+08:00"
commit = "d0d8c2f70865c36b8adf60df03ef128ae0e50fca"
hash = "sha256:c7d13752b9737868073c7a5ed58ec60fd5ccadee97da86fcd6e29723031574f4"
doc_hash = "sha256:5b2758505aee0f2d69e9077e1fcc749d6cdba6d43952a9aa5cd18ab92b3896f1"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream"
maintainability = "yes"
numerical_hazards = "other: follow upstream (powi functions also ok lah, no guards present)"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01NDXAowF1MThMfLpQdgWxvh"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "kItbb2wkc1KtB7QW6tojxpMoqwL+X4HXQ9fY9Wara4KWwBZenXIOt1GTeUHFG95vd0b+mai0833ryPr8OE3NCw=="

[[relation]]
target = "concept:07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties/numerical-robustness"
kind = "implements"
```

code is pretty straightforward, multiply by number of times m is given

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit d0d8c2f708.
