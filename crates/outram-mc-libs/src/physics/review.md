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
