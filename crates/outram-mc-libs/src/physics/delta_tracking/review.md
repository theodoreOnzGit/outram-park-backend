# Review record: crates/outram-mc-libs/src/physics/delta_tracking

```toml
[kovan]
id = "review-record-crates-outram-mc-libs-src-physics-delta-tracking"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: Majorant (github:theodoreOnzGit)

```toml
[kovan]
id = "review-majorant-theodoreonzgit-ff4dfa64"
kind = "review"
origin = "human"
created = "2026-11-05T13:30:33+08:00"
modified = "2026-11-26T13:16:58+08:00"
target = "fn:ff4dfa6416cdd4fc"

[review]
path = "crates/outram-mc-libs/src/physics/delta_tracking/majorant.rs::Majorant"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-26"
signed_at = "2026-11-26T13:16:58+08:00"
commit = "7fb739fa172a2bc15691039269d02536d71c55de"
hash = "sha256:384c76e3606efc4bb63b1bb0317e83f1886a0c15e52f14f9643819f62e122f83"
doc_hash = "sha256:4a23f03016faced710ed0d59b2ece3db71cf8260272111f21fa3fa9eb0c80197"
cargo_lock = "sha256:f01119ef723c81b98b22a0bd1030fcc4965b939f2c757c626b5986870ad940ee"

[review.callees]
"fn:a1dd3ecc8a6657c5" = "sha256:fc709a134c3a5ffd0cd7dcdfd37921e8362ebc6c8b6999a6cc0592b69836e5ee"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_holds_extra = "no"
item_impossible_values = "yes_nothing_stops"
item_matches_description = "yes"
item_test_use = "no_test_uses"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "none_yet"

[review.signature]
key = "k1"
alg = "ed25519"
value = "4bzyiYtXKzhSJzz7pW5LT7ayD3Yf9U0bGDKgNebJ6PY2bHX/YKqx5HtYfuxImOQllPdTB/l2FeZER2HqLvI1CQ=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-26, commit 7fb739fa17.

# Review: sample_delta_distance (github:theodoreOnzGit)

```toml
[kovan]
id = "review-sample-delta-distance-theodoreonzgit-cf0e7187"
kind = "review"
origin = "human"
created = "2026-11-26T09:13:13+08:00"
modified = "2026-11-26T09:13:13+08:00"
target = "fn:cf0e718778dac441"

[review]
path = "crates/outram-mc-libs/src/physics/delta_tracking/flight.rs::sample_delta_distance"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-26"
signed_at = "2026-11-26T09:13:13+08:00"
commit = "1e60e87f36ff0151869b61daabce8a6d134e2085"
hash = "sha256:e82973a5a7502d89f7151b4e86021f4613a1c70fb5d38782069532be9d4f36b8"
doc_hash = "sha256:df31b2fcc46420b09a0704c35cc364429611cc82ccddb86a38a048f58b7900e1"
cargo_lock = "sha256:4a48251d078c2265757f8e935e6bb409d9e9caff8c543ed902119772ad0d2989"

[review.callees]
"fn:681ddf6544e7b60b" = "sha256:17638ea97982299a8f568290be2092c01c5d16f34183e2d73318e4c404ae4734"
"fn:7d2c37be6eb3e8f3" = "sha256:e03a479bf5c1837af8e2aef9589eb6759382a67e74340d003406d4abfac2caa0"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: well, you are only given majorant and seed, seed is unsigned u64, only positive. majorant has number guards for negative number (fails loudly), infinite flight"
independence = "someone_else"
limits_and_guards = "guarded_panics_justified: well, you are only given majorant and seed, seed is unsigned u64, only positive. majorant has number guards for negative number (fails loudly), infinite flight"
maintainability = "yes"
numerical_hazards = "handled"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FbFYNpJxPtK8cotkuBZyPU"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "edXLSTLAKzhWbGDJlI6vb41a56lHVW4JwGeZISFi/yqt9x/Nw9nZHQvT43dOwTnr3jI7SJHRq8onMN91qcFlCg=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking"
kind = "implements"
```

the delta distance sampler, quite straightforward. Looks physically correct with some number guards.

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-26, commit 1e60e87f36.

# Review: next_variate (github:theodoreOnzGit)

```toml
[kovan]
id = "review-next-variate-theodoreonzgit-819ac0ea"
kind = "review"
origin = "human"
created = "2026-11-26T12:59:05+08:00"
modified = "2026-11-26T12:59:05+08:00"
target = "fn:819ac0ea2f3f4f14"

[review]
path = "crates/outram-mc-libs/src/physics/delta_tracking/flight.rs::next_variate"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-26"
signed_at = "2026-11-26T12:59:05+08:00"
commit = "5736b6aefca2bbff79686ff72db2e52427eb1eff"
hash = "sha256:d3fcbe68cfe290a3feb7960d144ac0faaf1b0371f0ae411ef65c07b8cdc8ae83"
doc_hash = "sha256:15d931682aeb085f427749c2200c0d40fff17a5353c6ee425383b498397488eb"
cargo_lock = "sha256:f01119ef723c81b98b22a0bd1030fcc4965b939f2c757c626b5986870ad940ee"

[review.callees]
"fn:681ddf6544e7b60b" = "sha256:17638ea97982299a8f568290be2092c01c5d16f34183e2d73318e4c404ae4734"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "matches_upstream_no_error_return: just wraps prn without advancing seed"
independence = "someone_else"
limits_and_guards = "matches_upstream_no_guard: just wraps prn without advancing seed"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FbFYNpJxPtK8cotkuBZyPU"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "Z3frTdr6glTx+P4bzDbCXjAXfgfBBV6H0WW1VyrEB/2r8d81u2ZdMcS6NWhhCjFcG61GMzLg6He0vR6eZIulDg=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics"
kind = "implements"
```

just wraps prn without advancing seed

upstream is:

double future_prn(int64_t n, uint64_t seed)
{
  uint64_t fseed = future_seed(static_cast<uint64_t>(n), seed);
  return prn(&fseed);
}

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-26, commit 5736b6aefc.

# Review: travelled (github:theodoreOnzGit)

```toml
[kovan]
id = "review-travelled-theodoreonzgit-cd6cb82e"
kind = "review"
origin = "human"
created = "2026-11-26T13:05:46+08:00"
modified = "2026-11-26T13:05:46+08:00"
target = "fn:cd6cb82e60b1da60"

[review]
path = "crates/outram-mc-libs/src/physics/delta_tracking/handoff.rs::travelled"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-26"
signed_at = "2026-11-26T13:05:47+08:00"
commit = "fc56519933cce2ccdfaa919ce1e0db6d40fe73be"
hash = "sha256:bdfabef6758bddeccced596c88037f3c2b445e5660172be88d87203286f3bb4a"
doc_hash = "sha256:bde5030bebf59edc46e7125ad06d6d92e5a142502cdb8fc1993aeaba283d4aa3"
cargo_lock = "sha256:f01119ef723c81b98b22a0bd1030fcc4965b939f2c757c626b5986870ad940ee"

[review.callees]
"fn:006d55971bc9e1e3" = "sha256:6f2d4f486afa1490d642f0e58cf39c0aa8e3f10ca27a00c80e35d20a1f2bb7c1"
"fn:675edefa7ec82788" = "sha256:a3f83ba6b25ab5e805efb5428ad1965e9fb5e1853850960a89f9f3c03180fc81"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "matches_upstream_no_error_return: openmc dot product, no guards"
independence = "someone_else"
limits_and_guards = "matches_upstream_no_guard: this is openmc dot product"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01FbFYNpJxPtK8cotkuBZyPU"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "arbEq05Jjwb6UOEFPrln8Ng8iOC9Jb76erBLWGInGZ+mFvettsdUdTxFfN4GMyNVQB4VLDmh6zCdNTB44FT/Bg=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics"
kind = "implements"
```

inline double dot(Position other) const
  {
    return x * other.x + y * other.y + z * other.z;
  }

This is the dot product upstream. OpenMC's C++ code verbatim
here we also have dot product, but the function takes two positions to calculate distance
for convenience

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-26, commit fc56519933.

# Review: Majorant::uniform (github:theodoreOnzGit)

```toml
[kovan]
id = "review-uniform-theodoreonzgit-ab058d19"
kind = "review"
origin = "human"
created = "2026-11-26T13:19:28+08:00"
modified = "2026-11-26T13:19:28+08:00"
target = "fn:ab058d1967bb2b6b"

[review]
path = "crates/outram-mc-libs/src/physics/delta_tracking/majorant.rs::Majorant::uniform"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-26"
signed_at = "2026-11-26T13:19:29+08:00"
commit = "8bf046220823844e1f31ed7f53a8a1c6799f4b0d"
hash = "sha256:5bfacb853e6df6e62af3b004a119e0ecb6bd1e524427e71046417edd6fd5803d"
doc_hash = "sha256:bf30e1985c5d15445d4077e2fa20a2298ceef6b8f814abec4bb5b9c4f0a381b3"
cargo_lock = "sha256:f01119ef723c81b98b22a0bd1030fcc4965b939f2c757c626b5986870ad940ee"

[review.callees]
"fn:ff4dfa6416cdd4fc" = "sha256:384c76e3606efc4bb63b1bb0317e83f1886a0c15e52f14f9643819f62e122f83"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "other: don't think much can go wrong, it's a simple constructor"
independence = "someone_else"
limits_and_guards = "guard_waived_research: OpenMC doesn't have guards for such constructors. may want to shim in future"
maintainability = "yes"
numerical_hazards = "other: don't think much can go wrong, it's a simple constructor. use for research"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "unit_tests_only"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_017j6t92YEuE649Nx5e7k8nD",
    "https://claude.ai/code/session_01FbFYNpJxPtK8cotkuBZyPU",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "JaLWfaY9mtJGr25/SNJfpYJMynxtUIzHnHKvBkK/0R7V7N/2EcGKhsuzadfpYP6uNSEefyQunbF+wQmNdbWHAw=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking"
kind = "implements"
```

uniform majorant, straightforward constructor

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-26, commit 8bf0462208.

# Review: Majorant::at (github:theodoreOnzGit)

```toml
[kovan]
id = "review-at-theodoreonzgit-553674e2"
kind = "review"
origin = "human"
created = "2026-11-26T16:08:51+08:00"
modified = "2026-11-26T16:08:51+08:00"
target = "fn:553674e261568aad"

[review]
path = "crates/outram-mc-libs/src/physics/delta_tracking/majorant.rs::Majorant::at"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-26"
signed_at = "2026-11-26T16:08:51+08:00"
commit = "5c55600784ee46c5873b766b9729da4a26051ded"
hash = "sha256:a80da642b1173b9a7fa9c0f13c5f7c17660635791409e408ff463594a4312a8d"
doc_hash = "sha256:e70ecafdcb6a4624e08412d3eefe977675f8c302abced2ff0f7867140f3e036c"
cargo_lock = "sha256:f01119ef723c81b98b22a0bd1030fcc4965b939f2c757c626b5986870ad940ee"

[review.callees]
"fn:2e2d54b40872e265" = "sha256:5134e803bd674f3ace50a897a04dea4a6cb009c073d7fca0b9d774d7dc278b00"
"fn:89dd813310599ab0" = "sha256:7bdf8ffb9a6314a160402eea825a229d7152f5e9f4e4f0ca0fd65d9ce0fbcedd"
"fn:ff4dfa6416cdd4fc" = "sha256:6050a5483d7a80c62698ff825cb454f251e2726e6cc145c8af51f8d29f282ad9"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "other: openmc has no guards or error return, following that as we are"
independence = "someone_else"
limits_and_guards = "guard_waived_research: openmc has no guards, following that standard"
maintainability = "yes"
numerical_hazards = "other: openmc has no guards or error return, following that as we are"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_017j6t92YEuE649Nx5e7k8nD",
    "https://claude.ai/code/session_01FbFYNpJxPtK8cotkuBZyPU",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "OUvkd5ISmd8Zn+UUuvEf0yP2o0Hps2GH1q3P2FW7Jp2bnPYu8Nh/wLfbikrDvT3FmSboys+sXlHmVB62aLnkCw=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics"
kind = "implements"
```

majorants are captured on finite energy grid. 
if the energy happens to be in between two grid points,
we take the larger of the majorant cross sections

of course, there is a chance that cross sections in between the grid are larger than both

not quite sure how that is handled. 
but for this case, i think it works

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-26, commit 5c55600784.
