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
