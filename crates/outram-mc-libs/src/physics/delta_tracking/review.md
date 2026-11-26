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
modified = "2026-11-05T13:30:33+08:00"
target = "fn:ff4dfa6416cdd4fc"

[review]
path = "crates/outram-mc-libs/src/physics/delta_tracking/majorant.rs::Majorant"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-05"
signed_at = "2026-11-05T13:30:33+08:00"
commit = "1bc36e4bd6e5afafeef9a4c246d6f511f42a0070"
hash = "sha256:dbc35cfa9f5f6ca51932de3653592be4a995644f71dd8233f344d32be3f21a76"
doc_hash = "sha256:4a23f03016faced710ed0d59b2ece3db71cf8260272111f21fa3fa9eb0c80197"
cargo_lock = "sha256:8d1e3de013cc9c858ee701ee28e0c9b16af71ff4dee19df1ceefe164838594af"

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

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_017j6t92YEuE649Nx5e7k8nD",
    "https://claude.ai/code/session_01FbFYNpJxPtK8cotkuBZyPU",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "DbJQvsvAA8O9cRb/dQAQsU5P2q1wdyXBNXRRbtCl+xnd0+d7SHQhff5MAN8bkeiFo9MaxVGu1kaykYhjN4ZmAQ=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-05, commit 1bc36e4bd6.

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
