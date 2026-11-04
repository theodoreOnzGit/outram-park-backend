# Review record: crates/outram-mc-libs/src/physics

```toml
[kovan]
id = "review-record-crates-outram-mc-libs-src-physics"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: run_keff_csg_hybrid (github:theodoreOnzGit)

```toml
[kovan]
id = "review-run-keff-csg-hybrid-theodoreonzgit-77b3f887"
kind = "review"
origin = "human"
created = "2026-11-04T16:15:39+08:00"
modified = "2026-11-04T16:15:39+08:00"
target = "fn:77b3f887f8a8f5de"

[review]
path = "crates/outram-mc-libs/src/physics/transport_csg.rs::run_keff_csg_hybrid"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T16:15:39+08:00"
commit = "3181d56c4980b1d59f08c1f95b29a54cd9064b55"
hash = "sha256:cae6e55b141f9b3592ec81f08d6778a8ca23f30c94245ecf2e159b62f78e4c52"
doc_hash = "sha256:841bafa5e9e4d7acc7df579c35c1ffc64d21e95a19daffa73292c0dacb5b030c"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"
no_concept = "io_format"

[review.callees]
"fn:519759eb67aafcb1" = "sha256:8369ae5d0a701c1857a82abdd764f18ce10be05820fc2133a9eb7c9ba173f5d7"
"fn:52240e147d4d4b36" = "sha256:1fa25ed5a1f06ac9fb8802f7e67aa33df81e6072c5a8845dad4cda8d9f219010"
"fn:770c5941dead0a7e" = "sha256:e7da85859203f919743199860493872328e541d23b2146b50d16d2ccd7055b85"
"fn:86fec0a94b7f6b00" = "sha256:e640c6ea7104c96133f22abd1da08a45d5c6f2d27695ecbb91fbbea415ab0a80"
"fn:8a6f6c1ba4a90a90" = "sha256:99a628f8277a00528054b1e951fd7978333f9bb9f86a73c39502ff962d2c98c8"
"fn:8dea6eaa57bcbc5a" = "sha256:1ddfcfc324626293688802c4becc1e9616eb0c220c8e5b2eccd00c8e9ede490a"
"fn:92704201c4b016b7" = "sha256:5bad91d0985b3a73894a31971ee4a6a8ab3ae160bec3f1bb995864835c7271aa"
"fn:b714e8912ed36446" = "sha256:b1d82651ec6ef3a5a5724142c3d1caefd135c7a8d6f0737e6d7c796cfe7bbf2b"
"fn:db39829f500f2e09" = "sha256:698cb5702c783200053f0e822f9b010797aa8b9e8789b0b31a575059095b73d5"
"fn:ff4dfa6416cdd4fc" = "sha256:dbc35cfa9f5f6ca51932de3653592be4a995644f71dd8233f344d32be3f21a76"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "returns_result"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: driver file, this is fine"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_01FbFYNpJxPtK8cotkuBZyPU",
    "https://claude.ai/code/session_01LN5DvQqPQdehQ1YLqvGPXd",
    "https://claude.ai/code/session_01MCKcAXFCyB6muPnCcb2NpZ",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "TUrMzOhjPwfvTR7341zXllo8QxkrlvxnwHlIKznO2SUiGo1F2JNOhfnlzE8ZWDYRfxEJk26etuRYhwaX+VuqBw=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 3181d56c49.
