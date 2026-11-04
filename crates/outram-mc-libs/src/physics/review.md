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

# Review: run_keff_csg_inner (github:theodoreOnzGit)

```toml
[kovan]
id = "review-run-keff-csg-inner-theodoreonzgit-8dea6eaa"
kind = "review"
origin = "human"
created = "2026-11-04T16:17:44+08:00"
modified = "2026-11-04T16:17:44+08:00"
target = "fn:8dea6eaa57bcbc5a"

[review]
path = "crates/outram-mc-libs/src/physics/transport_csg.rs::run_keff_csg_inner"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T16:17:45+08:00"
commit = "f6ec0edeb94798331a9412a9a0b7430ab3014e17"
hash = "sha256:1ddfcfc324626293688802c4becc1e9616eb0c220c8e5b2eccd00c8e9ede490a"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"

[review.callees]
"fn:040aba8efb432ef5" = "sha256:7c639956abcc9f80d87c115c539ff05c1cb365f813ef51ab84be1eb66627e398"
"fn:519759eb67aafcb1" = "sha256:8369ae5d0a701c1857a82abdd764f18ce10be05820fc2133a9eb7c9ba173f5d7"
"fn:52240e147d4d4b36" = "sha256:1fa25ed5a1f06ac9fb8802f7e67aa33df81e6072c5a8845dad4cda8d9f219010"
"fn:770c5941dead0a7e" = "sha256:e7da85859203f919743199860493872328e541d23b2146b50d16d2ccd7055b85"
"fn:86fec0a94b7f6b00" = "sha256:e640c6ea7104c96133f22abd1da08a45d5c6f2d27695ecbb91fbbea415ab0a80"
"fn:8a6f6c1ba4a90a90" = "sha256:99a628f8277a00528054b1e951fd7978333f9bb9f86a73c39502ff962d2c98c8"
"fn:92704201c4b016b7" = "sha256:5bad91d0985b3a73894a31971ee4a6a8ab3ae160bec3f1bb995864835c7271aa"
"fn:abf40e74bfa8f0f0" = "sha256:f5de0296047daa4fe4fa7ed9b5f1626d4f16d2f7afa8fcd45f0a1f63676c1f6f"
"fn:afd63c8186b7d3f0" = "sha256:8fde0942a8070435b9881325a9fa0496b88ae6a532c8369a786e6bd696068bac"
"fn:b714e8912ed36446" = "sha256:b1d82651ec6ef3a5a5724142c3d1caefd135c7a8d6f0737e6d7c796cfe7bbf2b"
"fn:d1aa2573e4826e29" = "sha256:747bb603044b1a4c929f5f8561443286c1b83445f1ac19c2de5cc30fc2265b1c"
"fn:db39829f500f2e09" = "sha256:698cb5702c783200053f0e822f9b010797aa8b9e8789b0b31a575059095b73d5"
"fn:ff4dfa6416cdd4fc" = "sha256:dbc35cfa9f5f6ca51932de3653592be4a995644f71dd8233f344d32be3f21a76"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "matches_upstream_no_error_return: well, upstream doesn't really guard, or do results, so this is propagated"
independence = "someone_else"
limits_and_guards = "no_limits: wrapper file, helps to run keff for csg based on the Gpu/Cpu/multithread"
maintainability = "yes"
numerical_hazards = "other: follow upstream"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_01LN5DvQqPQdehQ1YLqvGPXd",
    "https://claude.ai/code/session_01WwDSeg94xH5ovvjPM1Gh5Y",
    "https://claude.ai/code/session_01YSS7P3yZtTHxQ6cko369iZ",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "Hlwc2HBcA8JDGkUDdLF2UZmwozAIyZuDZLqeJdFyVxxSRKprUbuEBdPC79zYNphjXKpQRRKhrDBU0jbilGXYDg=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit f6ec0edeb9.

# Review: run_keff_csg_par_progress (github:theodoreOnzGit)

```toml
[kovan]
id = "review-run-keff-csg-par-progress-theodoreonzgit-fd0f6ad7"
kind = "review"
origin = "human"
created = "2026-11-04T16:21:16+08:00"
modified = "2026-11-04T16:21:16+08:00"
target = "fn:fd0f6ad7cc3d429c"

[review]
path = "crates/outram-mc-libs/src/physics/transport_csg.rs::run_keff_csg_par_progress"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T16:21:16+08:00"
commit = "0e7aa027da8cf14211e7831fb5454bfb4f022096"
hash = "sha256:d46dee64b38f716db37361f9873fa3a1cb20efb76aa4cc3a175a2544cda5fb64"
doc_hash = "sha256:b63e2e2b603a9b818abd34f4501c721ef18a7f48375f4392ce914ca986683bec"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"

[review.callees]
"fn:015742ee4e8bd724" = "sha256:6bac6d53345a1502561556dc720a51cd2235d3994913dfb84619fbdb55bb2c45"
"fn:040aba8efb432ef5" = "sha256:7c639956abcc9f80d87c115c539ff05c1cb365f813ef51ab84be1eb66627e398"
"fn:21a3b7c5b5c4a907" = "sha256:e8fcb6ad4ea70da331d6166edcd942529a3ae2b2b147cf46909bfccb3e1eb724"
"fn:2cdc0dbae63c02c1" = "sha256:89cef8038fdc9569259fd2a0f2ab842b7d7bde6d8f2694c845bd7242f606f8a7"
"fn:2cded3d4387501f7" = "sha256:4d004af88658c881d90d18f4344e1a139d4d86437d5aecaba2981ad15b4dd8ae"
"fn:3f3460664f557a39" = "sha256:65b7a0a638ee16eba25cb743ba41d70dee34df624d1a6104607e9b54ea8e0e23"
"fn:4116882d2cadda1e" = "sha256:c59a67c03c2481a4c123d35e54b7525825952e08bb922896fd6b28116df64dd0"
"fn:519759eb67aafcb1" = "sha256:8369ae5d0a701c1857a82abdd764f18ce10be05820fc2133a9eb7c9ba173f5d7"
"fn:52240e147d4d4b36" = "sha256:1fa25ed5a1f06ac9fb8802f7e67aa33df81e6072c5a8845dad4cda8d9f219010"
"fn:5abb39fac7b58261" = "sha256:e6fb24c21707ee73e67c44e8974abbc92f5be2ee192dfc83e60158c9c786dd06"
"fn:6d38de9e18f0a94a" = "sha256:3879035a9ecf292750ef7a1d64851594962f4f9cb0da7e532d35867e6d8bd2e1"
"fn:757cff99c20b882d" = "sha256:32eb6dd7c6c3e49fec60c0a0525c65ab630d5703bf259f228b66aef435c86333"
"fn:770c5941dead0a7e" = "sha256:e7da85859203f919743199860493872328e541d23b2146b50d16d2ccd7055b85"
"fn:7ee1d0622f3a3c03" = "sha256:117dbffad5dcec75647e2a0ead5bdd022f28ebd344ad0b539870dd3259b070be"
"fn:81233a9a4c4e8bb4" = "sha256:837d89dc1e99c224557884a95d14d1e6aea6966eac74983993a9d376f859e3f7"
"fn:857bb1e623c23d22" = "sha256:10a0eedf9c551a3976f07e7820108368dc9e140e933be5ed59dca29f742a5e64"
"fn:86fec0a94b7f6b00" = "sha256:e640c6ea7104c96133f22abd1da08a45d5c6f2d27695ecbb91fbbea415ab0a80"
"fn:8a6f6c1ba4a90a90" = "sha256:99a628f8277a00528054b1e951fd7978333f9bb9f86a73c39502ff962d2c98c8"
"fn:8fe2543edeb1c058" = "sha256:62ace7c8a4bc1c8987476cbc18f9773738f76654ab40813c06b995bc6908d6f6"
"fn:92704201c4b016b7" = "sha256:5bad91d0985b3a73894a31971ee4a6a8ab3ae160bec3f1bb995864835c7271aa"
"fn:b714e8912ed36446" = "sha256:b1d82651ec6ef3a5a5724142c3d1caefd135c7a8d6f0737e6d7c796cfe7bbf2b"
"fn:bb6e9f0eeab1198d" = "sha256:38e868b4132c3529f5424e24f083abedcfc2df378bff8a19be45939b0fde5c58"
"fn:c5c249ea5441011c" = "sha256:ac257f1d5609a800b65cf7863dde481789eaa5c71c674f5104df8eaa69efcaed"
"fn:cd8b7c2ff08fac8a" = "sha256:6a31cd3e8b0f1679f36cbfb30a6a2519c99493c9308c42bfdb72089acd311dbe"
"fn:d323f192bd894aa1" = "sha256:0a7c5a1cb9cc1a1ccd8cdc69d8cbe2da2aa55fe684e66a8ee468118a557a4101"
"fn:db39829f500f2e09" = "sha256:698cb5702c783200053f0e822f9b010797aa8b9e8789b0b31a575059095b73d5"
"fn:f95bb63623a66403" = "sha256:d5595a40390b93e5e4a7bbd874845532cbee3d0ef6650286d2efa13d38dbc2c2"
"fn:ff4dfa6416cdd4fc" = "sha256:dbc35cfa9f5f6ca51932de3653592be4a995644f71dd8233f344d32be3f21a76"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "matches_upstream_no_error_return: upstream is python, which throws error, now extended to other code"
independence = "someone_else"
limits_and_guards = "matches_upstream_no_guard: upstream is python, which throws error, now extended to other code"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_017U4MVUkoVswAemZhP6iGHg",
    "https://claude.ai/code/session_017j6t92YEuE649Nx5e7k8nD",
    "https://claude.ai/code/session_01EXPhstVwAqRb8o6YHVQoXW",
    "https://claude.ai/code/session_01FbFYNpJxPtK8cotkuBZyPU",
    "https://claude.ai/code/session_01LN5DvQqPQdehQ1YLqvGPXd",
    "https://claude.ai/code/session_01MCKcAXFCyB6muPnCcb2NpZ",
    "https://claude.ai/code/session_01NDXAowF1MThMfLpQdgWxvh",
    "https://claude.ai/code/session_01WwDSeg94xH5ovvjPM1Gh5Y",
    "https://claude.ai/code/session_01YSS7P3yZtTHxQ6cko369iZ",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "sl7y5QIurecFlU3lPQq6DGC1u6dbx8XZJjnOeq69TxAmD5NXJSECkuJxYFhHuo5ChiMOZEALvqcoIZFKe2HdBw=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport"
kind = "implements"
```

driver file
uses rayon for parallelism,
box source
then runs the files, in parallel
does tallies
and then assembles them back tgt

Impt function as it manages parallelism, and dries code

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 0e7aa027da.

# Review: transport_generation_par (github:theodoreOnzGit)

```toml
[kovan]
id = "review-transport-generation-par-theodoreonzgit-857bb1e6"
kind = "review"
origin = "human"
created = "2026-11-04T16:23:20+08:00"
modified = "2026-11-04T16:23:20+08:00"
target = "fn:857bb1e623c23d22"

[review]
path = "crates/outram-mc-libs/src/physics/transport_csg.rs::transport_generation_par"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T16:23:20+08:00"
commit = "318e1c060ab70a0f264100090332a5314f05c8ea"
hash = "sha256:10a0eedf9c551a3976f07e7820108368dc9e140e933be5ed59dca29f742a5e64"
doc_hash = "sha256:6dac1883d1ad42b8ac920ba8d49f690cc6c3642455bb1cc731cf1f41dd52cfc6"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"

[review.callees]
"fn:015742ee4e8bd724" = "sha256:6bac6d53345a1502561556dc720a51cd2235d3994913dfb84619fbdb55bb2c45"
"fn:03f2c5a46bf665de" = "sha256:d28100d4bc72cdcc346a6e5353cd64183a321c17b7b437c6c45377ffdf44b5c1"
"fn:21a3b7c5b5c4a907" = "sha256:e8fcb6ad4ea70da331d6166edcd942529a3ae2b2b147cf46909bfccb3e1eb724"
"fn:22c30e2054607d5a" = "sha256:fde0089b897ca31e42dce8b5cdd328b720b2adcec4d46b9c967b0494ea793dfb"
"fn:2cdc0dbae63c02c1" = "sha256:89cef8038fdc9569259fd2a0f2ab842b7d7bde6d8f2694c845bd7242f606f8a7"
"fn:3af0fd8a516bcb0d" = "sha256:e85f5d08ffef84733b73557178704497dada475b248b5fded3a5435a3588d21b"
"fn:52240e147d4d4b36" = "sha256:1fa25ed5a1f06ac9fb8802f7e67aa33df81e6072c5a8845dad4cda8d9f219010"
"fn:62a9bff6bf32728c" = "sha256:1a57aeb5fcc9142a4d157438a3bc14a3a85c2a96652ae079dc91a84ddac947e9"
"fn:72eac18850a2356b" = "sha256:ef74819872880be210268d59443be2cbbb2ddb3db8ee0891e03534c4e53f72d5"
"fn:770c5941dead0a7e" = "sha256:e7da85859203f919743199860493872328e541d23b2146b50d16d2ccd7055b85"
"fn:78922ce6d9a08c5f" = "sha256:4ef24abc0d6d00567d0023377c3b84ad87fef6e43f6e8ffd872a67365b88e56b"
"fn:86fec0a94b7f6b00" = "sha256:e640c6ea7104c96133f22abd1da08a45d5c6f2d27695ecbb91fbbea415ab0a80"
"fn:8a6f6c1ba4a90a90" = "sha256:99a628f8277a00528054b1e951fd7978333f9bb9f86a73c39502ff962d2c98c8"
"fn:c18707f1370d3347" = "sha256:962b128136c7bc85d8cba4f5cbc9ab48a0c2c900c0ce872793308fe46a7888df"
"fn:c5c249ea5441011c" = "sha256:ac257f1d5609a800b65cf7863dde481789eaa5c71c674f5104df8eaa69efcaed"
"fn:cef42e1a8f2fd45a" = "sha256:65de7eb0adb8398f4098ea482e1d84c064feee199ee81e633208a9438a32dd60"
"fn:db39829f500f2e09" = "sha256:698cb5702c783200053f0e822f9b010797aa8b9e8789b0b31a575059095b73d5"
"fn:ff4dfa6416cdd4fc" = "sha256:dbc35cfa9f5f6ca51932de3653592be4a995644f71dd8233f344d32be3f21a76"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: not quite ported from upstream openmc, but ports it style in that it throws errors"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: not quite ported from upstream openmc, but ports it style"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_017j6t92YEuE649Nx5e7k8nD"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "qOZKDTQ4vJJdCzb8WmbpnvTN1BLZWrdRw9/DE0tmeeWOwU0ZlfL36X+AwCqCDpNGCPJ5IIQUxUHWclS4rRjoBg=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport"
kind = "implements"
```

manages seeds in parallel, reassmebles them so that they are byte identical to series
therefore parallel simulations can be deterministic

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 318e1c060a.

# Review: transport_history_vr (github:theodoreOnzGit)

```toml
[kovan]
id = "review-transport-history-vr-theodoreonzgit-cef42e1a"
kind = "review"
origin = "human"
created = "2026-11-04T16:25:21+08:00"
modified = "2026-11-04T16:25:21+08:00"
target = "fn:cef42e1a8f2fd45a"

[review]
path = "crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T16:25:21+08:00"
commit = "42d0916defef3530d659c0f66b54ff56a8cbedcb"
hash = "sha256:65de7eb0adb8398f4098ea482e1d84c064feee199ee81e633208a9438a32dd60"
doc_hash = "sha256:06d78e8ff1388694f265e361a26491ea0eefa3eacd9407b62f2938e4225f80cb"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"

[review.callees]
"fn:00a77a52e8929c4c" = "sha256:8da69d95fe0febfcd6054aaa45c7790a0bf3e7bea4fb2d88d860e32b94f10a0d"
"fn:0822560e7bc2e8a6" = "sha256:87f0aaeb1f9599335b6a4fff01cf63654fb52accda874e1383dbd65e9c93134e"
"fn:09b7e10308acf43b" = "sha256:5f46b7c3e2940b9e0daf1ee1301f964210e959d70109a247733a09d18dde4160"
"fn:0a490fad8ac72ee0" = "sha256:d3d4ef5ba7746249df590464697197b63299ded42c1fbb4dac17f51ed88d3257"
"fn:0f6588f48f32b497" = "sha256:e1567c54ca06af36e11426ba0365e47a3259a350408a0d5ee534f76ed2f42592"
"fn:0f99421b635d41b3" = "sha256:19703f6cc1dcf2fd498bb57700dc65a7f09def10de79becbe3ae695db6894ff9"
"fn:1195285e937a6177" = "sha256:e4fcff1a56100cd710bb512bd8e6e1a3c82bc0d0b14360b92f9efa119758115f"
"fn:12777b41a9c0caf1" = "sha256:3fc23e7ebde3efe638cdd86584c30736ceacd4ba0d206c529882a7bbe32e0fd5"
"fn:140b06573daae0a0" = "sha256:bcc9ef9a96be9ae92341072abd98dab7e0b806df4eb97b5d329d616c8e0c7c94"
"fn:18480bf3a03dd12d" = "sha256:e71e3a7e0de933c65a9afc05be056d122b80036ca986f68be5a327093d2dfa33"
"fn:1a557b281337746a" = "sha256:a3f92c1b64adede0ba1fca6b0a2c7fe32bb706e089a6dbca6d61e9338a046aed"
"fn:1ac1ee406f438ff8" = "sha256:e22bea0437d9673d4924550a31199fba7e1f2f999fdef67fa3903af100602ab6"
"fn:1e030aad38736e8f" = "sha256:68857563c6145f7d010091c5dbcb88d6e9b5104c66c1594fb092db3680ea07b1"
"fn:1f3f471fa1dca083" = "sha256:910ad5158d88128b6eb20e65588564736b7b69218ab7747cb61fb24fa8a1c7e2"
"fn:21a3b7c5b5c4a907" = "sha256:e8fcb6ad4ea70da331d6166edcd942529a3ae2b2b147cf46909bfccb3e1eb724"
"fn:23794db85e1f47f0" = "sha256:36759ccd60e1222a586d4c863889ed3fb66e1e9d5fb4fa7a0a2e206e98b27343"
"fn:243e3b4d8f0a431d" = "sha256:4878131359ba5f6635a525ca71f8a11f37e84f5a4574519129516cd1a42dfe3b"
"fn:2c635c25343a2ff5" = "sha256:ae53a22359913ef32d13e8d056ee705f8f6a8a9d1cf61b91febdd365599204eb"
"fn:2ea2a35501c9a10a" = "sha256:c99e497501ef88d111e2c87ae485feaa37afd449925798e93065dcd258e57b9a"
"fn:332158ca55606b16" = "sha256:8eb7ec8ff55eaa53a98006538b95d59948db1508a1c5fc28b88528b0a4cd6c0c"
"fn:3705e270d44880a5" = "sha256:43d3623e15e78ba7c9fa734e27d1f94ea5d8c80556ca03cb08eeb0f3f7584281"
"fn:37836d247be84011" = "sha256:efcfb3541700d87ef106284d139075e8fadaba508cfbf851d2a8b9254717517a"
"fn:3eb1744367556890" = "sha256:7c7645e37d2e0a90109b7cfef14996235b8897d5bc598f3d508263603fcb9d1c"
"fn:3feaf37e993173ea" = "sha256:7caeadf892724d84eb5aa04044e7e7d2c6dd24f7b5976552c7fab6e886fa9cb7"
"fn:4be0c44e69368917" = "sha256:07091845ce7975081662aed91d66fc9eb52673e77373bb7fabc7c183346b83d1"
"fn:52240e147d4d4b36" = "sha256:1fa25ed5a1f06ac9fb8802f7e67aa33df81e6072c5a8845dad4cda8d9f219010"
"fn:5863995cd13c0aee" = "sha256:4ea78d6d365f86882a6a3fab75aac2ac97feabd77cf3dfa32b16eab4a634d87c"
"fn:59581b344a457fe4" = "sha256:6729ca363b972204518b8bef734cc936dedcb24b6ca2904fbb2e902e7f08f647"
"fn:5a06a1d67a83440c" = "sha256:c250210a795336734928d50f3d0faeeef23f2c43506e715914f7f6787fd1e5cc"
"fn:5e292087c2ddba7c" = "sha256:928a60e8f1c65b754b427bcb06599d67a7154391f33caa761f5d9aa79bf65966"
"fn:5f95312bc39d2df9" = "sha256:c2785b6693043c0ab44f7cea33d6f2e05fa8268f21d6642b4c91210715f151ab"
"fn:6123757201240381" = "sha256:757968458e0e42472085e41c4cc346b30544d8f150ddf4fd7c5cb97be2567370"
"fn:62a9bff6bf32728c" = "sha256:1a57aeb5fcc9142a4d157438a3bc14a3a85c2a96652ae079dc91a84ddac947e9"
"fn:66b92b1e796d5c1d" = "sha256:f228ab4cace62a6ee5da47084b7fa654a164a600b8fed5c00c3ce598fc7791c4"
"fn:675edefa7ec82788" = "sha256:a3f83ba6b25ab5e805efb5428ad1965e9fb5e1853850960a89f9f3c03180fc81"
"fn:67f8f1fbe42352db" = "sha256:d4bbe5cc855da8706183bd62d23794cb05d3eca8d0ffab469bac9f7ac29a0034"
"fn:681ddf6544e7b60b" = "sha256:17638ea97982299a8f568290be2092c01c5d16f34183e2d73318e4c404ae4734"
"fn:685562dc49cd31f6" = "sha256:5ac8d97524ae031df2a2d444a7677a6daf2a768a2da32b8002af3b36c9ef272f"
"fn:6e3ed21ff65fc77d" = "sha256:c815937608340ea575acbcab787d6f2cee55a471d0a8971216ca7ce5c31f3519"
"fn:713f8d16a6b2c476" = "sha256:8987b8afe0c1ea3446167a0d34f8f9d56f982e42d1e38da48dc78da836259aa6"
"fn:7988a62af8c6ea5b" = "sha256:576fe9036e71d2d607368c650c190582e256534a212cb77704d2acc72253662d"
"fn:7a5de66356f689d7" = "sha256:b5b2646ff219e1012c52c1feb17d451649fbfcd2a24391e79990bc4f19e8f81c"
"fn:7ab881065e0cfeda" = "sha256:6da0243fd095d442b1ae5acff469c9606dccad7506289d95244ef8fba99424a8"
"fn:7d2c37be6eb3e8f3" = "sha256:e03a479bf5c1837af8e2aef9589eb6759382a67e74340d003406d4abfac2caa0"
"fn:8175f499a098875c" = "sha256:8f4df2300fa134a066a6afacfec58e865508d167a816ab04cda0f3650264a98f"
"fn:83ec5419de1db3fb" = "sha256:20dc86b2fe12d8869ab0547f0f14f26245133381544d2a8cb74517f9bcce7576"
"fn:840704c589d69206" = "sha256:c68ad1d8b8ee4ac2ac4a9f2e554283a4b22028af03947e0b91d95d0614337a0b"
"fn:864fa04d54a24f75" = "sha256:dd2305b855465a736659ab5dde1c18f9f66151b8eaff43bd777bc3bb283ba649"
"fn:86fec0a94b7f6b00" = "sha256:e640c6ea7104c96133f22abd1da08a45d5c6f2d27695ecbb91fbbea415ab0a80"
"fn:87daab4e924439b8" = "sha256:cd6222ffdcedbc9f9c4e34acddccbf6dee75940742dab46542ef606125dee567"
"fn:8a6f6c1ba4a90a90" = "sha256:99a628f8277a00528054b1e951fd7978333f9bb9f86a73c39502ff962d2c98c8"
"fn:9619d1b46cba8e39" = "sha256:771ae7d92c4b3fab4a22f499a9b0a628b8501ac0f1f25def721019423b0cb10c"
"fn:97bcbfbc0e4f4eaa" = "sha256:ceb8ca327c5c21b8339fc2c15600b7e8d213e111dfe51b928abe01ab86ba75ec"
"fn:98351e2eb9e8546f" = "sha256:63f5f6f1c4c26cf32911f9a1caa9bcf472ebc25b5f014226f7e8d418566e61d0"
"fn:9bab4a87c66a242d" = "sha256:366021d75a229e073475bff6dceee825aea9cd8935a6e8aada5f6aaf1f9b0436"
"fn:9f7525274cd574bf" = "sha256:d771760040fa3cee1829a3b04fb70651d721904e5c797f362b5be3dbcb947ac0"
"fn:a24c53d59babe3aa" = "sha256:242a7b771ef924d0226de89d33a34c145c2d5750fabbfb16d83603aa1c319179"
"fn:a78cfe4566782884" = "sha256:f3d26b9e0b3f4506dfcdf88f2c2dc69cd56483922a3f3e91f5306f58c9192fe1"
"fn:a8b7745b6e2e3712" = "sha256:a7fe10ae7d7d4c2405acbc927dc0e645342cd1792299ceb89e4698f51e4bcc8e"
"fn:adbe3923203a54c0" = "sha256:33b0f1a901d07b3ed10e6fcc95922b38f5ce5fcadd294ae5a694913c7cfd2d24"
"fn:b0aefaba11ce8557" = "sha256:9701cf1fa01ebc4065694ed079b8aa8e267e1785da4e665f16990593d6e941c6"
"fn:b5494bc83ca9b269" = "sha256:03cde797c537c20ad2b485cdffa8b2a13eca50cb29dadfc0b53d71b01f7d9c49"
"fn:b71e7b1ffdea3c51" = "sha256:05756fd772a8cb777c4355aa853de22ccd8c1401b31365adf30a77638e2d7fa3"
"fn:b85fc451c5091b32" = "sha256:46984c079e478962aa352b191cf6d9ce74ee07871df201a8390bbc42cbab7cde"
"fn:c18707f1370d3347" = "sha256:962b128136c7bc85d8cba4f5cbc9ab48a0c2c900c0ce872793308fe46a7888df"
"fn:c2e30c296c179aee" = "sha256:517d77d0a3c7645eebf38b7fde662964cc22a7a97aca9f852a11e5ef743f1642"
"fn:c82351ffd343b81d" = "sha256:6bb1c0bac063a9ce12292527459db66279732482b818b647370047427e0de591"
"fn:c9317aaeb4555a01" = "sha256:2e5b9488092678c0f3ca088fc3ecd13ce02fb39b9e316689a77149e3903195ee"
"fn:cbb241b81b8f27f6" = "sha256:800fd6470a5dd8b47c43c9327abd89cc9e3ebb298facecaf18e582103723809a"
"fn:cc6fcee9f862a1a5" = "sha256:a206f29c34c86253b208e994cf1460dd62cf6f9b73174314367211bd6a915ed7"
"fn:ccaef4c37195ff0f" = "sha256:5ea0df5c2b0f4c29315f1bf7b5995416d1c7e852d8f9d1dcede9184fa47f1080"
"fn:d02fb722410de7c4" = "sha256:32efbe5029b76929a75fc166f41ade79cb0bc7d1a6a34768c97621624908023d"
"fn:d04e21632f1ca72a" = "sha256:749e289d503cef3d5461d0ddc1c54109f016cdcdec0ac2a4169e6059794e7112"
"fn:d4d9d7fd0cc7cc9f" = "sha256:c540b6d8cb3d2c5179adab14b0fe8fe883170acbca2027c21412c19b1d08cfb4"
"fn:d7fa62d4719efb04" = "sha256:d69dd2d35b91120efe79dd70eab9947c165a41a669fa2f94f0acb0891207307f"
"fn:daaa61a67bcdf62b" = "sha256:30096dc445acfb6fd5b3325e24b6ce85300ad34f76027dbbfb48f2d44eb65296"
"fn:dacc520a32256e17" = "sha256:90b216cec84da7f76dd0ab6fb87affcc54624dd54fa12abb834385ec2262fa78"
"fn:db39829f500f2e09" = "sha256:698cb5702c783200053f0e822f9b010797aa8b9e8789b0b31a575059095b73d5"
"fn:e1787dfe7b266275" = "sha256:de6b851fdd3fedfffafdf8218392298d6e606c4ee8582dc3d0dfbcb8d102095c"
"fn:e1e248a14f38c41f" = "sha256:7694a0f828268a5ddb3898070a83284ce64550fa9a29d69f5a2b4f0447cab597"
"fn:e4c99200efa66b74" = "sha256:6e75e89ccb8a423db4d7546af236c86b9fb20458a869bca0458552bc63dfec5b"
"fn:ea7286e0f393b8e3" = "sha256:0a97e2b89221e975e88eb3f06e09ff0a63839c4c207d5acfd6a73c1aa41c1665"
"fn:ef7e3b7801b0e07e" = "sha256:dfd96fba48b7234160d5f7d5208d64b48203b3c1e2e51b400ae628a169df8b02"
"fn:f47e3b4136e7376c" = "sha256:ddb4b384b1f65b46da752ee3859bd8693e301f17857666987aa74ccc1fae5206"
"fn:f867b08a2a68f27e" = "sha256:83c6a2c92625ebc73e6ad23ecf68393f0d131a1919cb0de680492e7a0de94e1e"
"fn:febe9059e18378e4" = "sha256:91277c7181ea88e28e9d4119ce34ce231ec2e32910b79e8308fa33678569fa67"
"fn:ff4dfa6416cdd4fc" = "sha256:dbc35cfa9f5f6ca51932de3653592be4a995644f71dd8233f344d32be3f21a76"
"fn:ff59051b88d7fa16" = "sha256:3e4e00f1d11e36099dbba1a583670b08f3e69cd5ec9e7c84c01f29e5ca4e0032"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: transport_history_vr copies the upstream style of no result"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: transport_history_vr copies the upstream style of no result"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_011SL9Ux39GmSSCVLYuP9ZRT",
    "https://claude.ai/code/session_017j6t92YEuE649Nx5e7k8nD",
    "https://claude.ai/code/session_019rKVr4tmtzxqs6WvW1NZS3",
    "https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe",
    "https://claude.ai/code/session_01CjnDKfBV9HHERA3rmCyeKc",
    "https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7",
    "https://claude.ai/code/session_01EXPhstVwAqRb8o6YHVQoXW",
    "https://claude.ai/code/session_01FbFYNpJxPtK8cotkuBZyPU",
    "https://claude.ai/code/session_01LN5DvQqPQdehQ1YLqvGPXd",
    "https://claude.ai/code/session_01MCKcAXFCyB6muPnCcb2NpZ",
    "https://claude.ai/code/session_01NDXAowF1MThMfLpQdgWxvh",
    "https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc",
    "https://claude.ai/code/session_01UKkBkWTc7hmuyRCZxyQUqZ",
    "https://claude.ai/code/session_01YSS7P3yZtTHxQ6cko369iZ",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "uQnpVuNvhCJx/azWtc+pXiuv3QQwAXt3/H9nSFh4i5YLyb4BKQ/dA5Q7Kjy0Ke2oS9/h4K7mEais7IIiQFUvDg=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport"
kind = "implements"
```

transport_history_vr copies the upstream style of no result

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 42d0916def.

# Review: HistoryOutcome (github:theodoreOnzGit)

```toml
[kovan]
id = "review-historyoutcome-theodoreonzgit-23794db8"
kind = "review"
origin = "human"
created = "2026-11-04T16:29:32+08:00"
modified = "2026-11-04T16:29:32+08:00"
target = "fn:23794db85e1f47f0"

[review]
path = "crates/outram-mc-libs/src/physics/transport_csg.rs::HistoryOutcome"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T16:29:32+08:00"
commit = "16c69a39bb807308898f70098b52d2dd6556a8b9"
hash = "sha256:36759ccd60e1222a586d4c863889ed3fb66e1e9d5fb4fa7a0a2e206e98b27343"
doc_hash = "sha256:2ef2cf530dc3ad6e98e0a932fd386bb9e98dd7e7d64dd84ade7fac4eed7c4cc7"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"

[review.callees]
"fn:ef7e3b7801b0e07e" = "sha256:dfd96fba48b7234160d5f7d5208d64b48203b3c1e2e51b400ae628a169df8b02"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_holds_extra = "no"
item_impossible_values = "no_checked_when_built"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "none_yet"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_017j6t92YEuE649Nx5e7k8nD",
    "https://claude.ai/code/session_01FbFYNpJxPtK8cotkuBZyPU",
    "https://claude.ai/code/session_01LN5DvQqPQdehQ1YLqvGPXd",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "+GWrTC3uw/GaRveQMUIQIcPIyqCcrRpIzUYgembErSCyzohvA41fcTLRlXiSAxpDyrhJ0ZOlXREIh5zKj0vrAQ=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport"
kind = "implements"
```

uses survival biasing (i'm not too familiar with it), 
but otherwise, quite straightforward for a neutron transport person

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 16c69a39bb.
