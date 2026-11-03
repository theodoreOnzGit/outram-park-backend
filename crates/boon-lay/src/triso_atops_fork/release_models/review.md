# Review record: crates/boon-lay/src/triso_atops_fork/release_models

```toml
[kovan]
id = "review-record-crates-boon-lay-src-triso-atops-fork-release-models"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: clamp_release_fraction (github:theodoreOnzGit)

```toml
[kovan]
id = "review-clamp-release-fraction-theodoreonzgit-f6be716e"
kind = "review"
origin = "human"
created = "2026-10-27T09:35:33+08:00"
modified = "2026-10-27T09:35:33+08:00"
target = "fn:f6be716e9120f882"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::clamp_release_fraction"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T09:35:34+08:00"
commit = "91934eeb016eb0d80d0f23adb46992045220277c"
hash = "sha256:2f5d2accd5ec7c03eba0b3cc142650142421730e7e9ddbb6b09b20232d4a76ff"
doc_hash = "sha256:9047e99599739afc0683d85aab306d9c8b94f6fcda22a9f8b59ddcd068f186f7"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.callees]
"fn:20bea5a8b58e1368" = "sha256:ae6e39a6895555ffce440e4e034714a793db30baf969406c1922ae501add0404"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: guarded between 0 and 1"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: Clamps review fraction to 0.0 to 1.0, this is a guard function"
maintainability = "yes"
numerical_hazards = "handled"
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
value = "F4GtcfC1IKJrYABisatdw9CBT4XZW6tgC8a72qZklJWh6bKRZKB0rIZuT23OEJld1pefPXOvQuifx0/npaIrCQ=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 91934eeb01.

# Review: rb_fail_noble_gases (github:theodoreOnzGit)

```toml
[kovan]
id = "review-rb-fail-noble-gases-theodoreonzgit-353a07bd"
kind = "review"
origin = "human"
created = "2026-10-27T12:24:58+08:00"
modified = "2026-10-27T12:24:58+08:00"
target = "fn:353a07bd52f264c2"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::rb_fail_noble_gases"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T12:24:59+08:00"
commit = "4fde2ff41781eafd127968a6ae472ceccfb98fed"
hash = "sha256:5ec0f06f3819a0691018bc1695ae96980e0752ef79094cfc66d0a499457191e0"
doc_hash = "sha256:73c2cd155a32f7056b02e1550fb4b06c3b773af52d3e7cdaee94f83c512215f1"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:20bea5a8b58e1368" = "sha256:ae6e39a6895555ffce440e4e034714a793db30baf969406c1922ae501add0404"
"fn:7d2c37be6eb3e8f3" = "sha256:e03a479bf5c1837af8e2aef9589eb6759382a67e74340d003406d4abfac2caa0"
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: probably follow upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: probably follow upstream behaviour"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
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
value = "/v+bORLkTAsm8PcByUDCmw4OUKQdAhl6RGDP++viT8+LQilxokoVVI/igXkWtVgGI/2KANWrEmDyQpqlqotGBg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

I see table 4 from the triso atops paper (Stoyer 2026), providing n, b and c, numbers checked by eye
implements equation 6a as a whole.

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 4fde2ff417.

# Review: BREAKTHROUGH_SERIES_TERMS (github:theodoreOnzGit)

```toml
[kovan]
id = "review-breakthrough-series-terms-theodoreonzgit-458e6cb0"
kind = "review"
origin = "human"
created = "2026-10-27T12:36:36+08:00"
modified = "2026-10-27T12:36:36+08:00"
target = "fn:458e6cb04d771957"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::BREAKTHROUGH_SERIES_TERMS"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T12:36:37+08:00"
commit = "f7829f27b797b85bbc291f5aa47070f5ccf5227e"
hash = "sha256:84b0d72ed966e1d0418f1dfb8b271b755b7005929796a62ebed91a2ea2d2e967"
doc_hash = "sha256:ac613ced7d811d964024825c53d6c742dc7ea4d515f27370aeef3e3059928f70"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

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
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "FHtFdIcjeNClfcrGqiyI7t8gPks/C0H1lHxrUyUXH9hJJMC171imXZiJwHKEgT6nG5x4kf5OvcqWWuknHiMlAA=="
```

normally this helps with equation 6c, and the code claims upstream is 1000 terms. I compared directly with the breakthrough_model in upstream trisoatops, therein, the num_terms is 1000 by default, but user can specify

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit f7829f27b7.

# Review: breakthrough_model (github:theodoreOnzGit)

```toml
[kovan]
id = "review-breakthrough-model-theodoreonzgit-18f13088"
kind = "review"
origin = "human"
created = "2026-10-27T12:41:43+08:00"
modified = "2026-10-27T12:41:43+08:00"
target = "fn:18f130883edf5867"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::breakthrough_model"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T12:41:45+08:00"
commit = "cda8e50526000d514165b5be10d7b678e2d2b3ea"
hash = "sha256:16ede897131c661b82088042b31556fe0f4fce43be03f667f09d8dcc336aeefb"
doc_hash = "sha256:1414f7e3e353f22d6dc195aff9f34678360d6b726afb626ab1795b03e9cf354f"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:20bea5a8b58e1368" = "sha256:ae6e39a6895555ffce440e4e034714a793db30baf969406c1922ae501add0404"
"fn:458e6cb04d771957" = "sha256:84b0d72ed966e1d0418f1dfb8b271b755b7005929796a62ebed91a2ea2d2e967"
"fn:4b22abf19474b4fd" = "sha256:f105945f85a7fe9030aebd3d9209c60f17da6afab1486f0cec92f932329a775f"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"
"fn:f6be716e9120f882" = "sha256:2f5d2accd5ec7c03eba0b3cc142650142421730e7e9ddbb6b09b20232d4a76ff"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: port upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: port upstream behaviour"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
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
value = "tZrXm96jj1vqmKknL/qKsUEX4v4jNAr39GcJ8RR7t5BGQU+tisLJodIDKVs5XthpFzQkQ66pFOGGkWsAgiFGCw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

checked d_prime against upstream Dp,

verfied to port upstream behaviour by inspecting via neovim

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit cda8e50526.

# Review: BOOTH_SERIES_TERMS (github:theodoreOnzGit)

```toml
[kovan]
id = "review-booth-series-terms-theodoreonzgit-5b6ec8ac"
kind = "review"
origin = "human"
created = "2026-10-27T12:44:36+08:00"
modified = "2026-10-27T12:44:36+08:00"
target = "fn:5b6ec8acc1bbca98"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::BOOTH_SERIES_TERMS"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T12:44:38+08:00"
commit = "798ad7ebbe6de4775293b3f0743e0c0249bc196c"
hash = "sha256:2b661d8344b964b3aa9ccc0e424f5da346fe9e35269728c9c2e5800e0b9757f1"
doc_hash = "sha256:9d6feba7dc7a8b5e98f51b077eacaeffe76178080dc06c63cb25f45bb3fb8f9c"
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
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "rLsWgJ/igEQ+D3rgbGoygdeaRHcMVKCUWioAoeOFV3afQAj1Q8sze72XD+Wb1dWlaMFkOhSzaa/42JjovdguCA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

Booth series terms, 5000 upstreamm verified under def booth_longlived in TRISO-ATOPS github repo

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 798ad7ebbe.

# Review: booth_longlived (github:theodoreOnzGit)

```toml
[kovan]
id = "review-booth-longlived-theodoreonzgit-2fc35c22"
kind = "review"
origin = "human"
created = "2026-10-27T13:15:11+08:00"
modified = "2026-10-27T13:15:11+08:00"
target = "fn:2fc35c224f3db467"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::booth_longlived"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T13:15:13+08:00"
commit = "a28510a13ae6a0555d8f0b6630e52ece9ad34ec6"
hash = "sha256:f575c1def404f9f91f00739d3fb8df45e0f45b53f3442d34bce903ecdba88881"
doc_hash = "sha256:2c96cfaec1309abe3c0037ead175529a6691daa3e45b221b415d14830ed328cb"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:20bea5a8b58e1368" = "sha256:ae6e39a6895555ffce440e4e034714a793db30baf969406c1922ae501add0404"
"fn:4b22abf19474b4fd" = "sha256:f105945f85a7fe9030aebd3d9209c60f17da6afab1486f0cec92f932329a775f"
"fn:5b6ec8acc1bbca98" = "sha256:2b661d8344b964b3aa9ccc0e424f5da346fe9e35269728c9c2e5800e0b9757f1"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream behaviour"
maintainability = "yes"
numerical_hazards = "other: follow upstream behaviour, no guards"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
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
value = "j2WarC9g9YPUyyF27ztNacKZoavKWftWMytRX36oHoszUM1pmvU2EnHIcHFAuwRJBQiMX/STPqsaFhYaX/+KBA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

slightly different form from booth_longlived in python, pow(-2) vs 1/pow(2)

release fraction is otherwise the same,
up to 5000 terms, matches upstream directly, but returns a ratio.

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit a28510a13a.

# Review: booth_shortlived_fast_diffuse (github:theodoreOnzGit)

```toml
[kovan]
id = "review-booth-shortlived-fast-diffuse-theodoreonzgit-ba1b5c74"
kind = "review"
origin = "human"
created = "2026-10-27T13:20:55+08:00"
modified = "2026-10-27T13:20:55+08:00"
target = "fn:ba1b5c7454e73103"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::booth_shortlived_fast_diffuse"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T13:20:56+08:00"
commit = "510148ba3474e9122a160701f88e5fdde5b91738"
hash = "sha256:a9f6b2311983f00f4945e0cbbc3a56a3737330d0540708d0490c52ae282bf5c2"
doc_hash = "sha256:a5e3cd6bab355f1f9070bae28827c26d0377271456b185d01c769d0ecd0989f7"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:20bea5a8b58e1368" = "sha256:ae6e39a6895555ffce440e4e034714a793db30baf969406c1922ae501add0404"
"fn:633a3996c8a68876" = "sha256:606d6687c54f107831c1155d56e9e021894f6289dc8e7f613e320167925e270c"
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"

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
sessions = [
    "https://claude.ai/code/session_01AxCVfsESPWrowa6mMV5skq",
    "https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "Xy+k/7jcuzLFwNNepqJPdQoG34aWvIZGawnvyI/SbJg5LsaLAEkg4yN5z043Vh4ekN/LCYptLDzSCVThcs+5CA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

checked against upstream 
def booth_shortlived_fastdiffuse in INL TRISO-ATOPS

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 510148ba34.

# Review: ATTENUATION_SERIES_TERMS (github:theodoreOnzGit)

```toml
[kovan]
id = "review-attenuation-series-terms-theodoreonzgit-d81841c8"
kind = "review"
origin = "human"
created = "2026-10-27T13:26:56+08:00"
modified = "2026-10-27T13:26:56+08:00"
target = "fn:d81841c86e868358"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::ATTENUATION_SERIES_TERMS"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T13:26:57+08:00"
commit = "d6a4078bacaac8819d51d47b02d0ee7ef4415a03"
hash = "sha256:89b67ee54c7ccbea118e032ed164b3cd210d3f66004ab934765ffbd2dee12a56"
doc_hash = "sha256:fe4e23180a5249882f19884fe74b1d66c6a730539962ac0c4071d1666b8eef11"
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
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "0ZyAO7j4GDkrZrOn6XkX8er1O1y+Gy440W1Euo3enIW+yqMl8TAPIrGikyHEP97u4nH1rx804koTicoNIC2yBQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

checked against upstream in
def attenuation_factor(Optional[int]=500)

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit d6a4078bac.

# Review: ATTENUATION_FACTOR_CAP (github:theodoreOnzGit)

```toml
[kovan]
id = "review-attenuation-factor-cap-theodoreonzgit-40ae7325"
kind = "review"
origin = "human"
created = "2026-10-27T13:30:07+08:00"
modified = "2026-10-27T13:30:07+08:00"
target = "fn:40ae7325634a6cb2"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::ATTENUATION_FACTOR_CAP"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T13:30:08+08:00"
commit = "277b869b3a66fae044fc445883c6329a07314ae6"
hash = "sha256:592d21c9d20158f91f15be8cb7044e39664a0b092598e6c8f8976a45bc511129"
doc_hash = "sha256:02243111470ebd87edad81143aa5ca197290634fe6b35b93e86a21cd7590bfda"
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
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "sS6RlxdIUlM0UjmZf2tuVZNTsgMtD/Gq9rJImRyiAzwMcpQJc8k3/ZBtLBHOyWBjnVHDXnkGTHUpsH6Z3HwRCQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

the comments said to ask dave lol, in the python

but otherwise okay, it is 1e8, checked against Eq 8 and 9 
verified in python and in TRISO ATOPS paper

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 277b869b3a.

# Review: attenuation_factor (github:theodoreOnzGit)

```toml
[kovan]
id = "review-attenuation-factor-theodoreonzgit-40683b29"
kind = "review"
origin = "human"
created = "2026-10-27T13:33:49+08:00"
modified = "2026-10-27T13:33:49+08:00"
target = "fn:40683b291bd14f34"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::attenuation_factor"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T13:33:50+08:00"
commit = "8fbad382d7dacd664706063742127aa3be235d63"
hash = "sha256:b6a7153f4139fde336fffa0b6a41deb9fcf210521ddf9c60576c08fdc0a36f5b"
doc_hash = "sha256:292ca6588e9badf739e610d59844bfb1637956f9205210b7efb2c7132c130762"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:40ae7325634a6cb2" = "sha256:592d21c9d20158f91f15be8cb7044e39664a0b092598e6c8f8976a45bc511129"
"fn:4b22abf19474b4fd" = "sha256:f105945f85a7fe9030aebd3d9209c60f17da6afab1486f0cec92f932329a775f"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"
"fn:d76bd3ba0e568fb8" = "sha256:4137e7fe123572c33ffc4fb51dc372d0b228b82cfdc2b8cbef8a21fee12fc462"
"fn:d81841c86e868358" = "sha256:89b67ee54c7ccbea118e032ed164b3cd210d3f66004ab934765ffbd2dee12a56"

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
units_documented = "yes"
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
value = "o/vcxDHVC6B8wvRn2VlkJ4ilEx2V4pYuISVc8SPXA0bG/V47/JLGsFY1WZpel5cORL78BmzxNB9SAVQUOY4TAQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

ports upstream behaviour, including the ask dave part that if sum of terms is 1, then return the cap,
ask dave 
inside the TRISO-ATOPS code, totally dunno why

code checked to follow usptream triso atops behaviour

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 8fbad382d7.

# Review: BOOTH_TRANSIENT_ZERO_FLOOR (github:theodoreOnzGit)

```toml
[kovan]
id = "review-booth-transient-zero-floor-theodoreonzgit-6759d46f"
kind = "review"
origin = "human"
created = "2026-10-27T14:16:59+08:00"
modified = "2026-10-27T14:16:59+08:00"
target = "fn:6759d46f6358435b"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::BOOTH_TRANSIENT_ZERO_FLOOR"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:17:00+08:00"
commit = "561d019e0cdb6fdbc33d041eb52fc410c9cc0bdc"
hash = "sha256:355cacaab1be201475f63cc9d8c56c23b0fe518729a9ee61fe1cf75c7e8bd032"
doc_hash = "sha256:52b89567d62a3e0d6ef75453f8d47e5ce6f3b0173d26176b0506e391c4c7623e"
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
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "75F+BJRsRnDnVz/e3KF4n+BX+B4/P6o6IxJmKVfc7wV4uF8JR94WRVeQ/7DFUv2mC0azF5dJrWRO63ed1vGKCg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

checked against upstream def booth_transient

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 561d019e0c.

# Review: RF_GRAPH_SERIES_TERMS (github:theodoreOnzGit)

```toml
[kovan]
id = "review-rf-graph-series-terms-theodoreonzgit-b16fbb3a"
kind = "review"
origin = "human"
created = "2026-10-27T14:19:16+08:00"
modified = "2026-10-27T14:19:16+08:00"
target = "fn:b16fbb3a2a858910"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::RF_GRAPH_SERIES_TERMS"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:19:17+08:00"
commit = "7de448b1b70e7ad2b3aeae22ff38f77c61610479"
hash = "sha256:2b661d8344b964b3aa9ccc0e424f5da346fe9e35269728c9c2e5800e0b9757f1"
doc_hash = "sha256:c03d037a872f7c0f6b4c356c588d6e198c19f89477f97abd8758e8d3051cfe04"
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
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "+/FqSnyrJc2KsK3J1IdV0R1IJH1kQ92PIT94315Ht16cHcR8HoiPP4Pqrzz2WEwY1jMNCL8peSfj2nGk/cxDBQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

checked against upstream that 5000 terms is the default used, users can supply terms themselves otherwise

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 7de448b1b7.

# Review: rf_graph (github:theodoreOnzGit)

```toml
[kovan]
id = "review-rf-graph-theodoreonzgit-fffbbf11"
kind = "review"
origin = "human"
created = "2026-10-27T14:22:26+08:00"
modified = "2026-10-27T14:22:26+08:00"
target = "fn:fffbbf11526a97b3"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::rf_graph"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:22:27+08:00"
commit = "e21630c96dcf9b85c8a025a985d8db06d8bfdd72"
hash = "sha256:dfe628eecc32532c8ef23b1b136bdae0fc2b5f0a2ddb69de9a51b87f2aecd9fc"
doc_hash = "sha256:3ae40407cbe207216f7eca32da2e01a4bda48242450d56f051ac26ba00bd00e6"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:20bea5a8b58e1368" = "sha256:ae6e39a6895555ffce440e4e034714a793db30baf969406c1922ae501add0404"
"fn:4b22abf19474b4fd" = "sha256:f105945f85a7fe9030aebd3d9209c60f17da6afab1486f0cec92f932329a775f"
"fn:b16fbb3a2a858910" = "sha256:2b661d8344b964b3aa9ccc0e424f5da346fe9e35269728c9c2e5800e0b9757f1"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: copies upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: copies upstream behaviour"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
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
value = "epCMJaULamz91IQcj3wH5peERGLgYsVG2fvlz0tcKghk61cSolRsihcOVxuV8UILFQayh8CemGElGPHO7FQeAA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

verified to copy upstream behaviour, visually checked against Python source code

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit e21630c96d.

# Review: booth_transient (github:theodoreOnzGit)

```toml
[kovan]
id = "review-booth-transient-theodoreonzgit-7b3031e1"
kind = "review"
origin = "human"
created = "2026-10-27T14:26:02+08:00"
modified = "2026-10-27T14:26:02+08:00"
target = "fn:7b3031e11dc9b5cf"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::booth_transient"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:26:03+08:00"
commit = "ff2a2e70c429c1da908fabc7da75c02b902b39c2"
hash = "sha256:c627628d5efbd84722c393487a8d27f59f0ef9f21931477acabe25d2ec1d44fd"
doc_hash = "sha256:53a474d9e1ce66bdffa4f7455e0adbf1aaab9a9d577f11ee3810b8a31422b6ac"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:20bea5a8b58e1368" = "sha256:ae6e39a6895555ffce440e4e034714a793db30baf969406c1922ae501add0404"
"fn:4b22abf19474b4fd" = "sha256:f105945f85a7fe9030aebd3d9209c60f17da6afab1486f0cec92f932329a775f"
"fn:5b6ec8acc1bbca98" = "sha256:2b661d8344b964b3aa9ccc0e424f5da346fe9e35269728c9c2e5800e0b9757f1"
"fn:6759d46f6358435b" = "sha256:355cacaab1be201475f63cc9d8c56c23b0fe518729a9ee61fe1cf75c7e8bd032"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follows upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follows upstream behaviour"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
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
value = "UgIe0OJE0NH9Yu6S92pQNlJqj+k9FH4ULILOVqD+Clp66/wkpv7SkKQ77teTPdxR7XrXMwuH2lohXe7Dn7LsCQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

follows upstream behaviour, checked against 
def booth_transient: ...

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit ff2a2e70c4.

# Review: breakthrough_model_transient (github:theodoreOnzGit)

```toml
[kovan]
id = "review-breakthrough-model-transient-theodoreonzgit-f5b5a929"
kind = "review"
origin = "human"
created = "2026-10-27T14:30:16+08:00"
modified = "2026-10-27T14:30:16+08:00"
target = "fn:f5b5a929974c73b5"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::breakthrough_model_transient"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T14:30:17+08:00"
commit = "ba83522c85c9cb6cb1b2b6045f5b3605fe282de0"
hash = "sha256:59ea489b3bf31f24b6959d29acb77d3d93a09bf580c7318115afc0cbc382c578"
doc_hash = "sha256:4091bb258d79d23cb56fb5b1aff3f10da19937ebe837adf1c2030ba0dd97db7a"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"

[review.callees]
"fn:20bea5a8b58e1368" = "sha256:ae6e39a6895555ffce440e4e034714a793db30baf969406c1922ae501add0404"
"fn:458e6cb04d771957" = "sha256:84b0d72ed966e1d0418f1dfb8b271b755b7005929796a62ebed91a2ea2d2e967"
"fn:4b22abf19474b4fd" = "sha256:f105945f85a7fe9030aebd3d9209c60f17da6afab1486f0cec92f932329a775f"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"

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
units_documented = "yes"
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
value = "usG3nQFa0T2YodLxEixdoZjia55CUEp3NYD/r+ElGb/gzpJ6Q7ZttCMJO+kHzhwZyv3LqkH2TEYDwaJ8WODlCQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

checked against upstream breakthrough_model_transient,
code differs slightly in form, like the line, cst and ser are now summed inline (slightly less readable
than upstream, but it is recognisable when comparing translation)

but meaning is well preserved

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit ba83522c85.

# Review: booth_transient_large_integral_saturates (github:theodoreOnzGit)

```toml
[kovan]
id = "review-booth-transient-large-integral-saturates-theodoreonzgit-cde6f34d"
kind = "review"
origin = "human"
created = "2026-11-02T13:31:57+08:00"
modified = "2026-11-02T13:31:57+08:00"
target = "fn:cde6f34d4a993bcb"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::booth_transient_large_integral_saturates"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:31:57+08:00"
commit = "65473293395504352935c8ba2df986ee98c93727"
hash = "sha256:e7ce0375ffb113bc132c30d573e21f02474bae8609b5009e421c9f31000ed376"
doc_hash = "sha256:dda6a9a41770dd146326ecd04a48f882d431c81a830f0ed1964739a3ec661f29"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:1ed22d1f3185dd46" = "sha256:a500297501cfa7d906f6313e1dbef06b95129b0212901730652c8efc9bb74eb1"
"fn:7b3031e11dc9b5cf" = "sha256:c627628d5efbd84722c393487a8d27f59f0ef9f21931477acabe25d2ec1d44fd"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "QxlS9CNWTS2b67mKVTCPdmrq7Deoz4Ug1pJDKV6S38wVZkNSIY/wlGdmzKRItDd2YD8Ya0xH8+geVT6Xj1mmAQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

basically checks that as time goes to infinity, release fraction tends to one, physically correct

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 6547329339.

# Review: booth_transient_zero_integral_is_zero (github:theodoreOnzGit)

```toml
[kovan]
id = "review-booth-transient-zero-integral-is-zero-theodoreonzgit-a9bfc6b1"
kind = "review"
origin = "human"
created = "2026-11-02T13:33:05+08:00"
modified = "2026-11-02T13:33:05+08:00"
target = "fn:a9bfc6b10d7a4d29"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::booth_transient_zero_integral_is_zero"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:33:05+08:00"
commit = "1c73865ad08356fe26cb9c4d4be0cab348641390"
hash = "sha256:eed81399bfbdb7490e43876d8ef6470c045ec7655a9896fdb268bf2fc93c258b"
doc_hash = "sha256:2aad8361cdb0e6f4336b07505fcdaaac9f408c63c018e29d1aab489ece6eabd5"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:1ed22d1f3185dd46" = "sha256:a500297501cfa7d906f6313e1dbef06b95129b0212901730652c8efc9bb74eb1"
"fn:7b3031e11dc9b5cf" = "sha256:c627628d5efbd84722c393487a8d27f59f0ef9f21931477acabe25d2ec1d44fd"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "8oo8MwpPb2IW+MdAu4xD/q4NfTKgUsg4PnetKijA7PP8kBS9ogbWaROj3Grq94lZGChU4vyEWvj+URxZ2PrhCQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

Basically tests that release fraction should tend to 0 for booth sphere when time is zero

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 1c73865ad0.

# Review: rat (github:theodoreOnzGit)

```toml
[kovan]
id = "review-rat-theodoreonzgit-1ed22d1f"
kind = "review"
origin = "human"
created = "2026-11-02T13:34:05+08:00"
modified = "2026-11-02T13:35:16+08:00"
target = "fn:1ed22d1f3185dd46"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::rat"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:35:16+08:00"
commit = "50e53de097dd34a72fe840cbed8b165808961274"
hash = "sha256:a500297501cfa7d906f6313e1dbef06b95129b0212901730652c8efc9bb74eb1"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: conveneience function for test"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.signature]
key = "k1"
alg = "ed25519"
value = "uRQLS4ejmFT0ustMKgZjC0jFUC67RJ3io6hD3LgYRh50FiVGGdWsdoRbqKG+cfDtTtguwxE48DYAFZ0+/pEIDw=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 50e53de097.

# Review: len (github:theodoreOnzGit)

```toml
[kovan]
id = "review-len-theodoreonzgit-a5d90099"
kind = "review"
origin = "human"
created = "2026-11-02T13:36:09+08:00"
modified = "2026-11-02T13:36:09+08:00"
target = "fn:a5d900994dded2a6"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::len"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:36:09+08:00"
commit = "c2d2251d0c293f5598c1e52c99d3ed77f3708596"
hash = "sha256:f83d70a686986f6e17bfa437ef3c5d3b750b27891b9b09c5ec646178fa7540b6"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "cannot_happen: conveneience function for test"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "xZeCHr0IQOThbA5VTIZc4ZdNOD8OEcrJ+Ul7O/m/pMAr2wv548hbLiy3XBozlhPh8E+XXiSRKJADI9CpRuMlDA=="
```

conveneience function for test

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit c2d2251d0c.

# Review: area (github:theodoreOnzGit)

```toml
[kovan]
id = "review-area-theodoreonzgit-df312643"
kind = "review"
origin = "human"
created = "2026-11-02T13:36:47+08:00"
modified = "2026-11-02T13:36:47+08:00"
target = "fn:df3126435ca0e9d0"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::area"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:36:47+08:00"
commit = "4b1c2fe00076337fbea0edd0a42e99355f3c6f27"
hash = "sha256:c6327566d5563e5e1a8e72426e288d6bb3a75499223a623f40a083449767a994"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "cannot_happen: conveneience function for test"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "kBZmdRAi/5s5zbxn6vAep819uoU2ggtGQXxZ7PANEKU+snnCoI/4nq/HSScbe863AJEANi5Cex8SALpSXh7oAg=="
```

conveneience function for test

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 4b1c2fe000.

# Review: rf_graph_saturates_to_one (github:theodoreOnzGit)

```toml
[kovan]
id = "review-rf-graph-saturates-to-one-theodoreonzgit-4b2c8157"
kind = "review"
origin = "human"
created = "2026-11-02T13:38:04+08:00"
modified = "2026-11-02T13:38:04+08:00"
target = "fn:4b2c81577e596956"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::rf_graph_saturates_to_one"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:38:04+08:00"
commit = "21f19c2b1afa56d5d8a37cc9e0dca4f7a04f87f5"
hash = "sha256:4ec42f1957e012c6cbd15e03860833b044d91764158a318f8058bcc8bd18844e"
doc_hash = "sha256:02f5aff51efd2b7605c1e9614c4e2db6a8098b36e6f0e12f881b8c4e053947eb"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:a5d900994dded2a6" = "sha256:f83d70a686986f6e17bfa437ef3c5d3b750b27891b9b09c5ec646178fa7540b6"
"fn:df3126435ca0e9d0" = "sha256:c6327566d5563e5e1a8e72426e288d6bb3a75499223a623f40a083449767a994"
"fn:fffbbf11526a97b3" = "sha256:dfe628eecc32532c8ef23b1b136bdae0fc2b5f0a2ddb69de9a51b87f2aecd9fc"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "8P6jKEnuE9FtSE8Ie1CWxtZEUy4qb+yPZnkJHCqgQiGFuQaTbmf9uQAcwB7bNzT5nzKGjhtW2qasf17DRcgXAw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

basically, release fraction goes to 1 at infinite timescales. It is an order of magnitude test

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 21f19c2b1a.

# Review: breakthrough_transient_zero_guard (github:theodoreOnzGit)

```toml
[kovan]
id = "review-breakthrough-transient-zero-guard-theodoreonzgit-b317b886"
kind = "review"
origin = "human"
created = "2026-11-02T13:39:06+08:00"
modified = "2026-11-02T13:39:06+08:00"
target = "fn:b317b88625dac73c"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs::breakthrough_transient_zero_guard"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:39:06+08:00"
commit = "9a6a29cedf3fcb1da6293e6b9c9e6201c35f4701"
hash = "sha256:50eeaef5885ce4585f7e5ba9125e2597e6d75975c48dc1e97bbee77d9f454b45"
doc_hash = "sha256:796a977db3f821e38c0df2b34d59c7f067672b3174ac2bb5dce1e72cb7402919"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:1ed22d1f3185dd46" = "sha256:a500297501cfa7d906f6313e1dbef06b95129b0212901730652c8efc9bb74eb1"
"fn:a5d900994dded2a6" = "sha256:f83d70a686986f6e17bfa437ef3c5d3b750b27891b9b09c5ec646178fa7540b6"
"fn:df3126435ca0e9d0" = "sha256:c6327566d5563e5e1a8e72426e288d6bb3a75499223a623f40a083449767a994"
"fn:f5b5a929974c73b5" = "sha256:59ea489b3bf31f24b6959d29acb77d3d93a09bf580c7318115afc0cbc382c578"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "convergence_order_study"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "B1EreYevRSwW5kigWLhWO9WmQjgLIhPad6F7WsQvr5V97sQ+9anu5/SeR6HebtF/b2m9vrkalt7IA5BdBrsqAA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

basically, this is a test that release fraction for breakthrough transient should be zero at time = 0

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 9a6a29cedf.

# Review: rb_fail (github:theodoreOnzGit)

```toml
[kovan]
id = "review-rb-fail-theodoreonzgit-9d27b0ef"
kind = "review"
origin = "human"
created = "2026-11-03T16:06:14+08:00"
modified = "2026-11-03T16:06:14+08:00"
target = "fn:9d27b0efc9f673f8"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/mod.rs::rb_fail"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T16:06:14+08:00"
commit = "c33b49494d2f6c73d90c2d31efda614d96bb5c5f"
hash = "sha256:4162cefd3282d0b558bf6c799b981584b838e89b2e8a3af48fc5388ee7691186"
doc_hash = "sha256:647c5bfaec3662f5d197ca0921e1886c75bf6916d11588a2956cddde4f847805"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:18f130883edf5867" = "sha256:16ede897131c661b82088042b31556fe0f4fce43be03f667f09d8dcc336aeefb"
"fn:19d2894fc2bfea29" = "sha256:bda6e7a004cfd587541249e3312f5b8f3b0b1824a41d456ed23073859410760e"
"fn:20bea5a8b58e1368" = "sha256:ae6e39a6895555ffce440e4e034714a793db30baf969406c1922ae501add0404"
"fn:2e610cfd767d4eb5" = "sha256:54c7941e23ca22131751599433e7cd9fd539c9953a45f91a3f68435ad803b55c"
"fn:2fc35c224f3db467" = "sha256:f575c1def404f9f91f00739d3fb8df45e0f45b53f3442d34bce903ecdba88881"
"fn:34ff47a5448224c9" = "sha256:1565117e190704e250537a267fcc591cbd6f4f74e8c74c62d6c58955f5437378"
"fn:353a07bd52f264c2" = "sha256:5ec0f06f3819a0691018bc1695ae96980e0752ef79094cfc66d0a499457191e0"
"fn:7f5cefb7c1df6559" = "sha256:4aa1d67f76c9b0ea57e4840755de225c42bc99fbca0793ad77d574160026eb1a"
"fn:ba1b5c7454e73103" = "sha256:a9f6b2311983f00f4945e0cbbc3a56a3737330d0540708d0490c52ae282bf5c2"
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:d9fc661a522b11ce" = "sha256:6c96657ab13ce5ab01b09a4dc4e3d249d08ce5d01102fa98aa74fdf096fb8faa"
"fn:e1cb570a455c31b8" = "sha256:06a9c12470b244d22d5415441b5b842edce0c43e867d9a1bbc36b9c1b69988ca"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "other: ports upstream"
independence = "someone_else"
limits_and_guards = "other: ported upstream (no guards)"
maintainability = "yes"
numerical_hazards = "other: port upstream"
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
value = "DoVGdm0j3kEXER1tWKTiaocCRBVojvj7cNPsQTFfrh9R3oBsNYvvdJqARzQYqNg7jcpPPnqloFNNM5F3UxdUBQ=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

RB fail ported. I see the dispatch logic

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit c33b49494d.

# Review: ReleaseMaterial (github:theodoreOnzGit)

```toml
[kovan]
id = "review-releasematerial-theodoreonzgit-c292835b"
kind = "review"
origin = "human"
created = "2026-11-03T16:25:47+08:00"
modified = "2026-11-03T16:25:47+08:00"
target = "fn:c292835b20327ba2"

[review]
path = "crates/boon-lay/src/triso_atops_fork/release_models/mod.rs::ReleaseMaterial"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T16:25:47+08:00"
commit = "0cfbe7e4bb90b2760d4a9ddc38a712e80389a95a"
hash = "sha256:19afb73e7e8ff4d5b8fb53c75fad6f738518e1d6ac944b40dc3d21043285e8ca"
doc_hash = "sha256:8d12d5a8946bc1219b3175ebc6e1e181bc2294c47f7e876e3c6a1fd80dc94cae"
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
vv_evidence = "none_yet"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "57w1wFY9tcHRZfjXRZA9Yg4fAbdBJiGPCazEeRM3wOKIMWxNlGhCb4FXGm2cxXD7knxxjKLqFFdkFw2Zhpu0Aw=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 0cfbe7e4bb.
