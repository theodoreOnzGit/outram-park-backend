# Review record: crates/outram-mc-libs/src/material

```toml
[kovan]
id = "review-record-crates-outram-mc-libs-src-material"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: sample_chi (github:theodoreOnzGit)

```toml
[kovan]
id = "review-sample-chi-theodoreonzgit-825a79e4"
kind = "review"
origin = "human"
created = "2026-11-25T15:43:46+08:00"
modified = "2026-11-25T15:43:46+08:00"
target = "fn:825a79e443ac2864"

[review]
path = "crates/outram-mc-libs/src/material/nuclide.rs::sample_chi"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-25"
signed_at = "2026-11-25T15:43:47+08:00"
commit = "e5ae2118df25a2b1bcd3c58749c5628581f46652"
hash = "sha256:e9acd9e0a6e7581f4ff503c6a57abce3fdda6b63357ed4a2bb2eead784fbd87a"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:4a48251d078c2265757f8e935e6bb409d9e9caff8c543ed902119772ad0d2989"

[review.callees]
"fn:1908577c54eb6c34" = "sha256:1520d2bbbc61c1fe91173e5f65748d02b583adc93a1d6d8ffedf4c48561efeb7"
"fn:1d08615bd6c7649b" = "sha256:553a54ef7f23daecc33a254cf02973bf86b081036da1fb5e6c6123740ee6149c"
"fn:2993b3e95ff59d54" = "sha256:a3a50b7d11b2453552b3d60be74994514f61c0c481d3f21729423312bb08f6a5"
"fn:681ddf6544e7b60b" = "sha256:17638ea97982299a8f568290be2092c01c5d16f34183e2d73318e4c404ae4734"
"fn:6d306a540b3af9b7" = "sha256:3cdc12a6d6e63e424ad0969bf444f1a2791056c91e5aa28e98b0dbbf38497fe8"
"fn:9c11bc57171820e7" = "sha256:6a37471f95707478ecf918e5acd67e4d276c8b982d5b658fe44a5867f83e7623"
"fn:c255c9d36f1c9e80" = "sha256:d81212fbd8c93d1a29b659251f0e036c27a75e8145a9dcf1f1a1dcc9a5b56470"
"fn:ddcbfd0af4d2a0a5" = "sha256:e5826c07d17b7e7e4f09779812ef1de7dea6fb143a8914ca9279e12b16222db6"
"fn:fd0b8b0efd90aff2" = "sha256:cd3395c2b11020fd0583b18bf15870dcbda6612b240251a430a944c54bfadb80"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow openmc port"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follows openmc port"
maintainability = "yes"
numerical_hazards = "other: follow openmc port, it is faithful"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "mixed"
sessions = [
    "https://claude.ai/code/session_012Jf5jeczK6fvKddBWDrLwE",
    "https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "UVoD4SxTsF4uZLE/r/+i1I8tmf0EikZz+JjLtB/ZxGwOztjv4A0v2Hq107ZdCBDWwCwV5Lm5/F9rn+jdycR5Bw=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics"
kind = "implements"
```

Consulted with ChatGPT to search openmc source, sample_chi seems to be a faithful port mirroring the intent of OpenMC here: https://github.com/openmc-dev/openmc/blob/develop/src/distribution_energy.cpp

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-25, commit e5ae2118df.
