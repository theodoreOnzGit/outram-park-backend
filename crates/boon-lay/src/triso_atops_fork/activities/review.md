# Review record: crates/boon-lay/src/triso_atops_fork/activities

```toml
[kovan]
id = "review-record-crates-boon-lay-src-triso-atops-fork-activities"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: Activity (github:theodoreOnzGit)

```toml
[kovan]
id = "review-activity-theodoreonzgit-e6c95a55"
kind = "review"
origin = "human"
created = "2026-10-23T13:31:59+08:00"
modified = "2026-10-29T15:00:54+08:00"
target = "fn:e6c95a55e6568166"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/mod.rs::Activity"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-29"
signed_at = "2026-10-29T15:00:54+08:00"
commit = "58fc040a0b0b71200b9427aee1f74d65bf986b00"
hash = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"
doc_hash = "sha256:f6248642edc573217e7d25f7f668e2940a034f99f8e7b6305ef26a68d7da0440"
cargo_lock = "sha256:f98236dd033e5a874219475b1066e85b1068ae8fb68882c2c9732d8f6bc0f51e"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
rung4_test_catches_wrong_physics = "compared_with_tolerance"
rung4_tests_pass = "all_pass"
rung4_tolerance_justified = "from_outside"
rung4_usable_without_source = "from_name_and_description"
rung4_wrong_quantity_by_accident = "no_named_and_unit_carrying"
vv_case_author = "human_wrote_and_verified"
vv_evidence = "unit_tests_only"

[review.signature]
key = "k1"
alg = "ed25519"
value = "c977fMH5hq8DjfFpBc3CT9Xj+2KS6ysIfltTGjgPbP5IeDDCznnxXWt4MzW4JDROVlC1ra1wiwLHYsVfn4KKBg=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-29, commit 58fc040a0b.

# Review: OTHER_ATTENUATION_FACTOR (github:theodoreOnzGit)

```toml
[kovan]
id = "review-other-attenuation-factor-theodoreonzgit-a3654ef9"
kind = "review"
origin = "human"
created = "2026-10-27T16:41:14+08:00"
modified = "2026-10-27T16:41:14+08:00"
target = "fn:a3654ef9ead0b08e"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::OTHER_ATTENUATION_FACTOR"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T16:41:15+08:00"
commit = "6d57915a171dd6bbe3a9578daa7f343b945048cc"
hash = "sha256:592d21c9d20158f91f15be8cb7044e39664a0b092598e6c8f8976a45bc511129"
doc_hash = "sha256:34bb960b406d9eaa334e3b1389de2fad1ae2d5dad2e5df14fe7ee9c39096a2a6"
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
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "+wjoTn79ieWzKQX6chC7pi/xr/JFGG2gbrRV44El9ONMQZfZ3E1upuUM/T7rBQN4eXdS3syg4cy+RKd4MQuIAg=="
```

attenuation factor is defined inline for base_activities upstream, checked to be 1e8

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit 6d57915a17.

# Review: SourceAndGraphite (github:theodoreOnzGit)

```toml
[kovan]
id = "review-sourceandgraphite-theodoreonzgit-ee87ae97"
kind = "review"
origin = "human"
created = "2026-10-27T16:43:35+08:00"
modified = "2026-10-27T16:43:35+08:00"
target = "fn:ee87ae9710c2fa05"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::SourceAndGraphite"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-27"
signed_at = "2026-10-27T16:43:36+08:00"
commit = "c1bf1b3cbe7b7a743f4b5c8e0c48ed521648d9db"
hash = "sha256:9d7ee874d9552f077202594f2437fd8a72a3d2c0b37446ecad11026af97edd94"
doc_hash = "sha256:c9fdcb596cfd2b0a8a12bca63109756297b74ed9cda0b60da56ceedc304671dd"
cargo_lock = "sha256:e64692c3b7ae29e78a10dec21f6ba0c5502e037c0755dfdeb220d8c2f3bc37a9"
no_concept = "plumbing"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_holds_extra = "no"
item_impossible_values = "other: just datastructure, not guarding cos it copies upstream"
item_matches_description = "yes"
item_test_use = "used_and_checked"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "other: data structure utility"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "5zrHmidghv4ppuYuaAahhE43xv9gt4tWHhkvbXgu3E07BuKpi9ol7PXUGru9GwL52pAOSKyUp9rpNS40MQYVDQ=="
```

python returns (S,G) for source rate and graphite activity. 
We are returning a struct rather than a tuple in Python. Acceptable

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-27, commit c1bf1b3cbe.

# Review: activity_from_atom_count (github:theodoreOnzGit)

```toml
[kovan]
id = "review-activity-from-atom-count-theodoreonzgit-19e5783b"
kind = "review"
origin = "human"
created = "2026-10-29T15:14:07+08:00"
modified = "2026-10-29T15:14:07+08:00"
target = "fn:19e5783b37ef814f"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/mod.rs::activity_from_atom_count"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-29"
signed_at = "2026-10-29T15:14:07+08:00"
commit = "ead303e96d468032a5d9d350add9da2e95667838"
hash = "sha256:ad68db54587104e69bd653cfadd3f834f3d65ded6a28ca0be769331eadbc74e2"
doc_hash = "sha256:b5bf22b5fd7181c754fecaca6efd9b7c08fa418127ded5124eed314f36d2f596"
cargo_lock = "sha256:f98236dd033e5a874219475b1066e85b1068ae8fb68882c2c9732d8f6bc0f51e"

[review.callees]
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:e6c95a55e6568166" = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: wrapper, follows upstream behaviour"
independence = "someone_else"
limits_and_guards = "no_limits: wrapper, follows upstream behaviour"
maintainability = "yes"
numerical_hazards = "none_found"
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
value = "s0eIHqyuU2DwjfsTVVYcWIvDwATTTUijF6ZLe1twKQK2+x5dz9O1m9npZU5UQedLSTSiLdKhowAhoD/9LYeeBw=="

[[relation]]
target = "concept:16-nuclear-fuel-cycle/fuel-depletion/decay-data"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-29, commit ead303e96d.

# Review: atom_count_from_activity (github:theodoreOnzGit)

```toml
[kovan]
id = "review-atom-count-from-activity-theodoreonzgit-cef7daa7"
kind = "review"
origin = "human"
created = "2026-10-29T15:15:08+08:00"
modified = "2026-10-29T15:15:08+08:00"
target = "fn:cef7daa77e029e71"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/mod.rs::atom_count_from_activity"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-29"
signed_at = "2026-10-29T15:15:08+08:00"
commit = "a1d3a93e1aa5f7098a210aaef1383ed5999126bd"
hash = "sha256:ac2db408ddbe78a00dbef118ac3cf96734e5998abd38409cd602473c3a347010"
doc_hash = "sha256:60e2c9e7ecab0d92480008c5ef4a399aff576371f0eda497f14a3b9f8ca0f0bc"
cargo_lock = "sha256:f98236dd033e5a874219475b1066e85b1068ae8fb68882c2c9732d8f6bc0f51e"

[review.callees]
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:e6c95a55e6568166" = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: wrapper, follows upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: wrapper, follows upstream behaviour"
maintainability = "yes"
numerical_hazards = "none_found"
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
value = "E9Z1QPT/A0734nxlYV759ymh5xrivOwSk+4wHopnoPhN+C+AAMaAs3Sk1HShM0HLhn3r08cLV9ouBU6sFDm6BA=="

[[relation]]
target = "concept:16-nuclear-fuel-cycle/fuel-depletion/decay-data"
kind = "implements"
```

basic equations, make sense. 
Numbers are unitless, f64 is ok

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-29, commit a1d3a93e1a.

# Review: BQ_PER_CI (github:theodoreOnzGit)

```toml
[kovan]
id = "review-bq-per-ci-theodoreonzgit-672b140f"
kind = "review"
origin = "human"
created = "2026-10-29T15:15:58+08:00"
modified = "2026-10-29T15:15:58+08:00"
target = "fn:672b140fc503762f"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/mod.rs::BQ_PER_CI"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-29"
signed_at = "2026-10-29T15:15:58+08:00"
commit = "8e840bfb8d3036e5587075c298ce569c4a299f8f"
hash = "sha256:7cdec66678905becb105d95cc0922974e31363d078e8e18357228304ad8bce5f"
doc_hash = "sha256:3a052dbf0d18b2543d7c5ab9c84831a0dc9c733134382d392f79450c31a25b3e"
cargo_lock = "sha256:f98236dd033e5a874219475b1066e85b1068ae8fb68882c2c9732d8f6bc0f51e"

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
value = "amgIJcPeOafXDg52bBMPmY9ryCoEdWv1BhJET22kGaEsXzLEF0T+IsUHxUmbdycaL/XbaadhVLdUHsSVoDdYCQ=="

[[relation]]
target = "concept:16-nuclear-fuel-cycle/fuel-depletion/decay-data"
kind = "implements"
```

ported from TRISO-ATOPS, also from memory, this is correct

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-29, commit 8e840bfb8d.

# Review: curies_from_becquerels (github:theodoreOnzGit)

```toml
[kovan]
id = "review-curies-from-becquerels-theodoreonzgit-4e747b96"
kind = "review"
origin = "human"
created = "2026-10-29T15:16:54+08:00"
modified = "2026-10-29T15:16:54+08:00"
target = "fn:4e747b96b31630e2"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/mod.rs::curies_from_becquerels"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-29"
signed_at = "2026-10-29T15:16:54+08:00"
commit = "7a372a7c7f48dc44aaef70239c5e141ee7799846"
hash = "sha256:684872ed9862c8116c6c7c0d3a5e67c1706f30a9a55d31b4d03b2f1ba1c8232d"
doc_hash = "sha256:3dcf2bcbc290ad731dbf8faeaf3a37a29d3dfe86ed30f81cf713fd517b41892b"
cargo_lock = "sha256:f98236dd033e5a874219475b1066e85b1068ae8fb68882c2c9732d8f6bc0f51e"

[review.callees]
"fn:672b140fc503762f" = "sha256:7cdec66678905becb105d95cc0922974e31363d078e8e18357228304ad8bce5f"
"fn:e6c95a55e6568166" = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: wrapper, follows upstream behaviour"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: wrapper, follows upstream behaviour"
maintainability = "yes"
numerical_hazards = "none_found"
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
value = "CynMhxKHMOMZa+Ex6mSKiA0AliirVSSdZLkvrEQ1yx3hJRqlpz3qXW8K6lKz9rFdGJcw/DZMpBRI9ySksRRRDw=="

[[relation]]
target = "concept:16-nuclear-fuel-cycle/fuel-depletion/decay-data"
kind = "implements"
```

wrapper, follows upstream behaviour

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-29, commit 7a372a7c7f.

# Review: becquerels_from_curies (github:theodoreOnzGit)

```toml
[kovan]
id = "review-becquerels-from-curies-theodoreonzgit-44fd31aa"
kind = "review"
origin = "human"
created = "2026-10-29T15:17:45+08:00"
modified = "2026-10-29T15:17:45+08:00"
target = "fn:44fd31aa536c0e7c"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/mod.rs::becquerels_from_curies"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-10-29"
signed_at = "2026-10-29T15:17:45+08:00"
commit = "fb50ebbb8ccc6eafce4d692132c8c07c352471a9"
hash = "sha256:7104d2332e8fdac8384e558146e0fd17ddc6de5ff99c23c6ad619fc88331a963"
doc_hash = "sha256:c7acc82a7b4053df3285cd66c7b4e794961b5d2fe94123b6238faad24b398804"
cargo_lock = "sha256:f98236dd033e5a874219475b1066e85b1068ae8fb68882c2c9732d8f6bc0f51e"

[review.callees]
"fn:672b140fc503762f" = "sha256:7cdec66678905becb105d95cc0922974e31363d078e8e18357228304ad8bce5f"
"fn:e6c95a55e6568166" = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"

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
sessions = ["https://claude.ai/code/session_01CyNsZF5YyFwvCf1QyrSya7"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "F9XCLmNAsqpy33hoQLpG3Ac2gLibOVKG65xZ07q0BX2h9tbbzeRraAqTh+0paY/0KvQRkhXyFyJi5F6elDLOBw=="

[[relation]]
target = "concept:16-nuclear-fuel-cycle/fuel-depletion/decay-data"
kind = "implements"
```

follows upstream behaviour,
physically correct, basic decay

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-10-29, commit fb50ebbb8c.

# Review: plate_out_steadystate (github:theodoreOnzGit)

```toml
[kovan]
id = "review-plate-out-steadystate-theodoreonzgit-8ea2eef1"
kind = "review"
origin = "human"
created = "2026-11-02T13:20:47+08:00"
modified = "2026-11-04T15:14:49+08:00"
target = "fn:8ea2eef18751cdc9"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs::plate_out_steadystate"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T15:14:49+08:00"
commit = "807aa57d91b634061207bd8bca49d96dc3f7f4a5"
hash = "sha256:a4582845d242eaac109123741e2f00ee38dbd45ddf6133b85561fbea369e9750"
doc_hash = "sha256:e5252425075dd535fa65ba2501b62fd701771dd5bfc754259743c0620e72ff26"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"

[review.callees]
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

[review.signature]
key = "k1"
alg = "ed25519"
value = "Hy2E+s1JRprw/bc8cTCHV45Mc/mGz87yvJwXIqYnAXghloGUkovKV76S3y7Bwr7UlY4H3+G74VsExXWiR8h+Bw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

plating out function matches upstream plate_out_steadystate in TRISOATOPS python
This is the analytical integral

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 807aa57d91.

# Review: plate_out (github:theodoreOnzGit)

```toml
[kovan]
id = "review-plate-out-theodoreonzgit-9515687f"
kind = "review"
origin = "human"
created = "2026-11-02T13:22:38+08:00"
modified = "2026-11-02T13:22:38+08:00"
target = "fn:9515687f16091d7e"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs::plate_out"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:22:38+08:00"
commit = "7fcaafd0ee1ce469b4deb2fe24295efc20f1d3c9"
hash = "sha256:d991c04dc42b0782b9cde96107150da5659aab605a8a9360aee6e287f87f3542"
doc_hash = "sha256:a97ff3f2cd557d22803ff93bd69ca094f1ff441f633474c6c77b6835f76df231"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"

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
value = "z6LAZLBiUpuqjskVWxSKOT0CkH8Ed9i/TEOve8Xa0HwpcS4/5PJhxxnSJOCec/uro/5CNLXltg6IhQSifOwXAA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

follow upstream

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 7fcaafd0ee.

# Review: clean_up_steadystate (github:theodoreOnzGit)

```toml
[kovan]
id = "review-clean-up-steadystate-theodoreonzgit-71fb69d2"
kind = "review"
origin = "human"
created = "2026-11-02T13:24:40+08:00"
modified = "2026-11-02T13:24:40+08:00"
target = "fn:71fb69d2e7699d01"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs::clean_up_steadystate"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T13:24:40+08:00"
commit = "a51d4a95165b59bada5d2d36547167334be11f88"
hash = "sha256:9f4767f618e0131eca8d5260adee1618b636347412ca1c787e3046340f897bf8"
doc_hash = "sha256:c636e5be24989291a6385693fd08f716a816bbfc4cf00dacade2dc55d64b6c75"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream"
maintainability = "yes"
numerical_hazards = "other: follow upstream"
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
value = "GQrRtC3+Bt6GqebW8g9DnRxqC15ykFmoejBUTLT5Onczq18c73rqCca9I247pAE/sLgi5V/LZl7GNvyf1NWpCw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

follow upstream, HPS (helium purification system)

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit a51d4a9516.

# Review: clean_up (github:theodoreOnzGit)

```toml
[kovan]
id = "review-clean-up-theodoreonzgit-1365b3b5"
kind = "review"
origin = "human"
created = "2026-11-02T13:26:30+08:00"
modified = "2026-11-04T15:15:53+08:00"
target = "fn:1365b3b5318e0a5d"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs::clean_up"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T15:15:53+08:00"
commit = "57c15041aee75278b58ac7425c0097e1291a6d91"
hash = "sha256:c942284721244108e667ca31e29958749cc2bb1be0458b653a2b2f3da2bf3d67"
doc_hash = "sha256:a09bb66dc21dd22bdf65b64e83cce12a6038e8e36da5d8f0f85b6de95caacdd5"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"

[review.callees]
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"

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

[review.signature]
key = "k1"
alg = "ed25519"
value = "lFHNF2K7gc4pvHG6kaCJs566Vt2qj+YfT+DFW3bMHqE69zedosTcL+aMsDIrG7a4XRGzgaOCVY7rMwmuYgPWDw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

follow upstream, checked against clean_up python code
This is the analytical integral (see doc comments)

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 57c15041ae.

# Review: circulating (github:theodoreOnzGit)

```toml
[kovan]
id = "review-circulating-theodoreonzgit-98e15d9c"
kind = "review"
origin = "human"
created = "2026-11-02T13:28:00+08:00"
modified = "2026-11-04T15:14:19+08:00"
target = "fn:98e15d9c13a78899"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs::circulating"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T15:14:19+08:00"
commit = "7be5389ecccdb826b3a40143867fe1d1c72c3461"
hash = "sha256:0f0a6db70afe1338a4117433d1546cdbe5d5d3d5267c95e2b2a6ca7cf961ca85"
doc_hash = "sha256:5bf8ec6362689f806a0311b17fc870a8d0d5d41496795b82d1348620e2c0c72d"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"

[review.callees]
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream"
maintainability = "yes"
numerical_hazards = "other: follow upstream"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.signature]
key = "k1"
alg = "ed25519"
value = "QRvJ1MSg+OB4ZQNx2b7V5OQFppJcLOR4SdCG26yQ/nBJY/25dqrdD4EGpAegL5w+2EuSMChXhykra8++Zr2XCw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

follow upstream, circulating function in TRISO ATOPS
This is the analytical integral

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 7be5389ecc.

# Review: circulating_steadystate (github:theodoreOnzGit)

```toml
[kovan]
id = "review-circulating-steadystate-theodoreonzgit-269096ca"
kind = "review"
origin = "human"
created = "2026-11-02T13:29:22+08:00"
modified = "2026-11-04T15:14:35+08:00"
target = "fn:269096cad00d2563"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs::circulating_steadystate"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T15:14:35+08:00"
commit = "800187369bc78f62383254bbe8031bcc1313b927"
hash = "sha256:8020635ac2d7627b8c13c4e3f24285459c10eda8c1ce2c8971c0e8e6e80dcd03"
doc_hash = "sha256:88fe9e9b0c5e86b51b35d2f61fc43be2844e6c8b7f412b6f7d8a310b3d97ef55"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"

[review.callees]
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream"
maintainability = "yes"
numerical_hazards = "other: follow upstream"
test_reach = "reached_and_checked"
unintended_function = "no"
units_documented = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.signature]
key = "k1"
alg = "ed25519"
value = "pqWrHv/JpYLilb9anySVxFmJG4sEs0flww+Wr1Z1RQs69iSQf7D6O8G/6KFnrkRLlJ1+CduJypzf9wqRIOEnDw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

follow upstream, circulating_steadystate in python TRISO ATOPS
This is the analytical integral

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 800187369b.

# Review: base_activities (github:theodoreOnzGit)

```toml
[kovan]
id = "review-base-activities-theodoreonzgit-2095c07b"
kind = "review"
origin = "human"
created = "2026-11-02T16:57:03+08:00"
modified = "2026-11-02T16:57:03+08:00"
target = "fn:2095c07b5f4097a8"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::base_activities"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T16:57:03+08:00"
commit = "3c09462778ea3cffa663e0efcff2394c80a603cb"
hash = "sha256:fb4c3bd2aed5f83efe1505efbff2698445a4da2c6aae7a960ff839b607a18b64"
doc_hash = "sha256:c3abd162b02b0563c26381a9c1eee9b7b5429221f6b646d26448bd764effe349"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:40683b291bd14f34" = "sha256:b6a7153f4139fde336fffa0b6a41deb9fcf210521ddf9c60576c08fdc0a36f5b"
"fn:a3654ef9ead0b08e" = "sha256:592d21c9d20158f91f15be8cb7044e39664a0b092598e6c8f8976a45bc511129"
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"
"fn:d9fc661a522b11ce" = "sha256:6c96657ab13ce5ab01b09a4dc4e3d249d08ce5d01102fa98aa74fdf096fb8faa"
"fn:ee87ae9710c2fa05" = "sha256:9d7ee874d9552f077202594f2437fd8a72a3d2c0b37446ecad11026af97edd94"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: follow upstream behaviour"
independence = "someone_else"
limits_and_guards = "no_limits: follow upstream behaviour"
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
value = "txV39hGPE5sq/4s0VET+xZxYk59tfLR006Q5gNsK2gZzkzyHV3x29Wlw5X4bMb2imDbh14I4Oaa2RAQctYnHAw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

checked against base activities in TRSIO ATOPS python

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit 3c09462778.

# Review: release_rate (github:theodoreOnzGit)

```toml
[kovan]
id = "review-release-rate-theodoreonzgit-2a3b6cbe"
kind = "review"
origin = "human"
created = "2026-11-02T17:00:31+08:00"
modified = "2026-11-02T17:00:31+08:00"
target = "fn:2a3b6cbe3931c7f4"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::release_rate"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-02"
signed_at = "2026-11-02T17:00:31+08:00"
commit = "a34ac667586fb91961a544146a9d2b5146ef095a"
hash = "sha256:d49baaabedeb0b5c2e4ad39013ae33f1a918fcd1f0d9aab32ec975c4553dd78a"
doc_hash = "sha256:3926128d29c988ee8eb9d14268bc0f2486989f3b941b38e90b95e7d240ecf36e"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:20bea5a8b58e1368" = "sha256:ae6e39a6895555ffce440e4e034714a793db30baf969406c1922ae501add0404"
"fn:2816dd8674f2b148" = "sha256:3890d7f2c2bf5d78ac0a02d952c219265c6ded009ae509e8ceaaf6ce156eced5"
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"
"fn:d9fc661a522b11ce" = "sha256:6c96657ab13ce5ab01b09a4dc4e3d249d08ce5d01102fa98aa74fdf096fb8faa"
"fn:e6c95a55e6568166" = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"
"fn:fb54a5dca16f9f45" = "sha256:f686099b10d09642a2f1f9fb1996aafa32b3f6d31e75e887100d5ded75363435"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: port upstream"
independence = "someone_else"
limits_and_guards = "no_limits: port upstream"
maintainability = "yes"
numerical_hazards = "other: port upstream"
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
value = "MTec8n0F9Ipo4f2bDjuIwq+fzgvJf4PD3deLQwbMJB8QcJuFfWobLMvKsDYCAdGBKf8iyzBY8GqP9HM6HzbKCw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

checked against release_rate in python TRISO atops codebase

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-02, commit a34ac66758.

# Review: FailureFractions (github:theodoreOnzGit)

```toml
[kovan]
id = "review-failurefractions-theodoreonzgit-2816dd86"
kind = "review"
origin = "human"
created = "2026-11-03T11:12:21+08:00"
modified = "2026-11-03T11:12:21+08:00"
target = "fn:2816dd8674f2b148"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::FailureFractions"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T11:12:21+08:00"
commit = "0c1c0113524a4dff98bb44a4ef3c0bb696b2a697"
hash = "sha256:3890d7f2c2bf5d78ac0a02d952c219265c6ded009ae509e8ceaaf6ce156eced5"
doc_hash = "sha256:2ff6c99b71931d4d3c4d8d497c4ba028b11bbd7cd1baa0869dc7040339590ff7"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

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
value = "hv0w79Y1p1HmMo5HIbG9kA5rqJEg5elm7yEyzID281/k3QY5+5znInEMEZjZngzDHb8k4/koKHPkagAlsq0VAw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

seen the terms under eq 23, data storage utility

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 0c1c011352.

# Review: FailureFractions::with_fuel_failure_incremental (github:theodoreOnzGit)

```toml
[kovan]
id = "review-with-fuel-failure-incremental-theodoreonzgit-c8df3cd6"
kind = "review"
origin = "human"
created = "2026-11-03T11:16:43+08:00"
modified = "2026-11-03T11:16:43+08:00"
target = "fn:c8df3cd65b455789"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::FailureFractions::with_fuel_failure_incremental"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T11:16:43+08:00"
commit = "2206c5e8efed13412c878c9738a5adb3a6fb25b8"
hash = "sha256:060c27486b5b8923250691f9f8eeef94a396128c31578e7bbf90c0fcbad13832"
doc_hash = "sha256:6a1848785a7c94128f7ee49ab545b77b8ac9a38ed3bd807e3ee932549c90f853"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "plumbing"

[review.callees]
"fn:2816dd8674f2b148" = "sha256:3890d7f2c2bf5d78ac0a02d952c219265c6ded009ae509e8ceaaf6ce156eced5"
"fn:51521ea418d380ec" = "sha256:f229314ca26d4c1bd969276a00e71384d0956238d1e5adf56f639e8737e8c8ec"
"fn:f05e8930a2188530" = "sha256:749fc2c38715c20489e5f3554271e09f6ee87f1b55f551dfabf103467942b3cc"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "cannot_happen: it takes in an entire struct, not its job to guard"
independence = "someone_else"
limits_and_guards = "no_limits: takes in an entire struct"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"

[review.signature]
key = "k1"
alg = "ed25519"
value = "b7/qhpcCfVBOoxeSPGjKEz0Xc4Bp2Kgz7ce5wPUEckG5emjmHpu/VfC9PeSEo7RpYmAq6VQqm4gTXSStIrQlAg=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 2206c5e8ef.

# Review: base_activities_for (github:theodoreOnzGit)

```toml
[kovan]
id = "review-base-activities-for-theodoreonzgit-ffb1855e"
kind = "review"
origin = "human"
created = "2026-11-03T11:18:47+08:00"
modified = "2026-11-03T11:18:47+08:00"
target = "fn:ffb1855ec854dd27"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::base_activities_for"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T11:18:47+08:00"
commit = "9275cce1524b0cbd6805e5f10fa7757e33ab534e"
hash = "sha256:a7d1d8385fca2bd85a1bb4bdd69aa11e2beefda8a9895eab3fbfda03a96b8ae1"
doc_hash = "sha256:85e8e5758d9f137e2d08f9176396932952da2d925b7e7b32102fffd195e029b8"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:2095c07b5f4097a8" = "sha256:fb4c3bd2aed5f83efe1505efbff2698445a4da2c6aae7a960ff839b607a18b64"
"fn:40ae7325634a6cb2" = "sha256:592d21c9d20158f91f15be8cb7044e39664a0b092598e6c8f8976a45bc511129"
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"
"fn:90c61e88ff3aaad8" = "sha256:702bb4da43da4fd696f48054a2e447cdd8835219c40a2d817bed2df4d06a569c"
"fn:b4300d995422f7cf" = "sha256:1ec2c617f1f0e953148c45600858c2e267e666ce1b816c307cb2cd67d91a5f3c"
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"
"fn:d9fc661a522b11ce" = "sha256:6c96657ab13ce5ab01b09a4dc4e3d249d08ce5d01102fa98aa74fdf096fb8faa"
"fn:ee87ae9710c2fa05" = "sha256:9d7ee874d9552f077202594f2437fd8a72a3d2c0b37446ecad11026af97edd94"
"fn:fc42897456ddcda8" = "sha256:99ab9e07c9d01a858b47d5649f98c035be52983fc3847b7614406757d66dbd8d"

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
sessions = ["https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "Lj+c0bHKudsWT2iE7i14KMLkc1pbQ8JbulIR8BsViE7yXmSnZwCXMqu+fUqesUtycd9QXIUnM9DwZWRjTDpVBA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

ablation testing/sensitivity testing utility depending on how TrisoAtops is read (TrisoAtopsVariant struct)

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 9275cce152.

# Review: release_rate_for (github:theodoreOnzGit)

```toml
[kovan]
id = "review-release-rate-for-theodoreonzgit-14ee81ec"
kind = "review"
origin = "human"
created = "2026-11-03T11:20:14+08:00"
modified = "2026-11-03T11:20:14+08:00"
target = "fn:14ee81ec2df53f3a"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::release_rate_for"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T11:20:14+08:00"
commit = "4a788b5f72dfdae52cf52bfd83405af5f777f610"
hash = "sha256:12dfe2fd134677e41b2ca49e9db0b202c8d4f3e983719db589a536ea2bc1337e"
doc_hash = "sha256:ddfad359862967957f74ae2456e15b7aa3dd94c846cc5a1be7d9cf9f0959394f"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:04cbe7e951a3ef50" = "sha256:345ee09cfb571ad3eb7be4d63de0422ed36a5c74f207444c5530a4bd9e9ffb03"
"fn:20bea5a8b58e1368" = "sha256:ae6e39a6895555ffce440e4e034714a793db30baf969406c1922ae501add0404"
"fn:2816dd8674f2b148" = "sha256:3890d7f2c2bf5d78ac0a02d952c219265c6ded009ae509e8ceaaf6ce156eced5"
"fn:2a3b6cbe3931c7f4" = "sha256:d49baaabedeb0b5c2e4ad39013ae33f1a918fcd1f0d9aab32ec975c4553dd78a"
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"
"fn:ba64fcae0172d25c" = "sha256:0196f4e99e8ef64c6a3357865c16449940fa6b65552c3b83369cff84cf21db0b"
"fn:d9fc661a522b11ce" = "sha256:6c96657ab13ce5ab01b09a4dc4e3d249d08ce5d01102fa98aa74fdf096fb8faa"
"fn:e6c95a55e6568166" = "sha256:ea14821948029f3e6fad9e6158fe9078d576f804d7c111f2f14856b0c0fc6605"
"fn:fc42897456ddcda8" = "sha256:99ab9e07c9d01a858b47d5649f98c035be52983fc3847b7614406757d66dbd8d"

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
sessions = ["https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "aeay0qlFPhOCgdXEGlabUIs4Krq2nVg0FVmmwVJ1hN4e61OU8CXh1F3ceaRfxyUJv4ktKzBBoFePTo45ExgaDg=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

ablation/sensitivity testing utility for readings of TRISO ATOPS (uses TrisoAtopsVariant struct/class)

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 4a788b5f72.

# Review: step (github:theodoreOnzGit)

```toml
[kovan]
id = "review-step-theodoreonzgit-f37cec30"
kind = "review"
origin = "human"
created = "2026-11-04T15:10:06+08:00"
modified = "2026-11-04T15:12:09+08:00"
target = "fn:f37cec301c5918a4"

[review]
path = "crates/boon-lay/src/triso_atops_fork/activities/live_pools.rs::step"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T15:12:10+08:00"
commit = "1e369368ecaf4bc2cf6eaaf52c222dc7a2033d19"
hash = "sha256:679dafd954ddb4359227d970029a9de1bfa6d1f77031b39d113ada4194c1d936"
doc_hash = "sha256:a6a6b9300a8a9f32c4d619e6859afd4ff4ef70d2f30aadfb9d8cee48c61d7c26"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"

[review.callees]
"fn:0e9ccca890f6126d" = "sha256:3900326329f1200d7bab64933aa5a5edc6b76f2742a3938411ba4b975ed236ad"
"fn:46b0d3d8a500434a" = "sha256:96c58430e7ccefcba6f2340b7d2b6620ec271f91404971e24e4a3ce5182276f6"
"fn:507456e528cbc960" = "sha256:526e9979f678df8ed90d3915e9570e8be7e1fe96735795252fd39df33a200cb8"
"fn:60da73c1a5c2618c" = "sha256:ecb178c561fbb8da759ba2a0a1716230a693a1fc2423b7fdc03d4a4e9ac28945"
"fn:8d5a30749926de65" = "sha256:8840fbdd71c0be0085c763cb6b8a52b63a179a437c858a4904f8e7745bb05ca2"
"fn:d2f19c5ac1bd0c10" = "sha256:af2237946440613f528ad2ab3ad403cdd3a337ec2bdcfa588745954b7ee7a98f"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: upstream doesn't even give results, we can always wrap it in results guard wrapper later"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: follow upstream behaviour, ATOPS doesn't even check, limited guards present for negative or infinite timestep"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.signature]
key = "k1"
alg = "ed25519"
value = "5ArXiWBQH/QffcwHHsP0j0IwyAzs2ODtqPIerEBfDf/zHAIPJ594hYXwOHjnHFZsxNK9UHdPgG8t9m8VzbqDBA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

SUPER IMPORTANT. This is where the numerical integration happens for each of the pools

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 1e369368ec.
