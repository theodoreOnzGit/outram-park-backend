# Review record: crates/outram-mc-libs/src/rng

```toml
[kovan]
id = "review-record-crates-outram-mc-libs-src-rng"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: maxwell (github:theodoreOnzGit)

```toml
[kovan]
id = "review-maxwell-theodoreonzgit-80d7b4ca"
kind = "review"
origin = "human"
created = "2026-11-25T15:48:50+08:00"
modified = "2026-11-25T15:48:50+08:00"
target = "fn:80d7b4ca412b265b"

[review]
path = "crates/outram-mc-libs/src/rng/distributions.rs::maxwell"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-25"
signed_at = "2026-11-25T15:48:50+08:00"
commit = "ded35ad893f789e03b7b9a0caae63701ce92ac64"
hash = "sha256:16bc8d4c42d852500bddb77271702690b6e6af91bf647f0aef16329c367722a4"
doc_hash = "sha256:56c2428f923be4fb43c300c7d13bad5e1efa121a756b03839096cb70d2c9196c"
cargo_lock = "sha256:4a48251d078c2265757f8e935e6bb409d9e9caff8c543ed902119772ad0d2989"

[review.callees]
"fn:681ddf6544e7b60b" = "sha256:17638ea97982299a8f568290be2092c01c5d16f34183e2d73318e4c404ae4734"
"fn:7d2c37be6eb3e8f3" = "sha256:e03a479bf5c1837af8e2aef9589eb6759382a67e74340d003406d4abfac2caa0"
"fn:810110ab0e15d3a7" = "sha256:36092fde7654bdb4c9ed9083fe9bb588f4bee49ff71da7a559fb22f5abcfc634"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "matches_upstream_no_error_return: faithful port of openmc"
independence = "someone_else"
limits_and_guards = "matches_upstream_no_guard: faithful port of openmc"
maintainability = "yes"
numerical_hazards = "other: faithful port of openmc"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "mixed"
sessions = [
    "https://claude.ai/code/session_012Jf5jeczK6fvKddBWDrLwE",
    "https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "IrI5mgwV7zkBKkliVbWewLbD8Isu+kl1rcxzhMnLfgJGHsds9NPcSYTuHi/DNNXXNNq/Q/67ATcbnFrGNTr9AA=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics"
kind = "implements"
```

matches openmc quite well. Here is OpenMC's code:

double maxwell_spectrum(double T, uint64_t* seed)
{
  // Set the random numbers
  double r1 = prn(seed);
  double r2 = prn(seed);
  double r3 = prn(seed);

  // determine cosine of pi/2*r
  double c = std::cos(PI / 2. * r3);

  // Determine outgoing energy
  return -T * (std::log(r1) + std::log(r2) * c * c);
}

looks about right!

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-25, commit ded35ad893.

# Review: watt (github:theodoreOnzGit)

```toml
[kovan]
id = "review-watt-theodoreonzgit-ddcbfd0a"
kind = "review"
origin = "human"
created = "2026-11-25T15:51:08+08:00"
modified = "2026-11-25T15:51:08+08:00"
target = "fn:ddcbfd0af4d2a0a5"

[review]
path = "crates/outram-mc-libs/src/rng/distributions.rs::watt"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-25"
signed_at = "2026-11-25T15:51:09+08:00"
commit = "5730eb689d13eb0f607a89105a91f07552490dde"
hash = "sha256:e5826c07d17b7e7e4f09779812ef1de7dea6fb143a8914ca9279e12b16222db6"
doc_hash = "sha256:fd8dca0dc228d91581e6a51d87abe9e803eb74abca412a756ece38c4e046b540"
cargo_lock = "sha256:4a48251d078c2265757f8e935e6bb409d9e9caff8c543ed902119772ad0d2989"

[review.callees]
"fn:681ddf6544e7b60b" = "sha256:17638ea97982299a8f568290be2092c01c5d16f34183e2d73318e4c404ae4734"
"fn:80d7b4ca412b265b" = "sha256:16bc8d4c42d852500bddb77271702690b6e6af91bf647f0aef16329c367722a4"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "matches_upstream_no_error_return: follow openmc"
independence = "someone_else"
limits_and_guards = "matches_upstream_no_guard: follow openmc"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "mixed"
sessions = ["https://claude.ai/code/session_012Jf5jeczK6fvKddBWDrLwE"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "03ZxwpvuUgSLKaz+7tlwZ3ic3xBmEsb/xXt5DdEhenDWZvvvt03D2uISs60Gq+u5TAOj7MqhmXqIguFGVSUTCQ=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics"
kind = "implements"
```

Matches openmc well:
double watt_spectrum(double a, double b, uint64_t* seed)
{
  double w = maxwell_spectrum(a, seed);
  return w + 0.25 * a * a * b +
         uniform_distribution(-1., 1., seed) * std::sqrt(a * a * b * w);
}
Where uniform distribution is:


double uniform_distribution(double a, double b, uint64_t* seed)
{
  return a + (b - a) * prn(seed);
}

So essentially it is identical.

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-25, commit 5730eb689d.

# Review: normal_variate (github:theodoreOnzGit)

```toml
[kovan]
id = "review-normal-variate-theodoreonzgit-73d31751"
kind = "review"
origin = "human"
created = "2026-11-25T16:45:37+08:00"
modified = "2026-11-25T16:45:37+08:00"
target = "fn:73d3175114d13b5a"

[review]
path = "crates/outram-mc-libs/src/rng/distributions.rs::normal_variate"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-25"
signed_at = "2026-11-25T16:45:37+08:00"
commit = "c0d377264b49bb9e1c395f41e326bf966064799f"
hash = "sha256:3d856bb6ed888639cd9db5979b48bc8ab8a6691769765b8f6ae85b1641901b53"
doc_hash = "sha256:82d9396182c90299069f52bdd6985dddec7b158238858a3c0d7d31f31e3c7638"
cargo_lock = "sha256:4a48251d078c2265757f8e935e6bb409d9e9caff8c543ed902119772ad0d2989"

[review.callees]
"fn:69b82be4e3b98232" = "sha256:75f540b65fc7cde1e46fa6fd0757e43d4363f85490cebc324acba5198ae12632"
"fn:7d2c37be6eb3e8f3" = "sha256:e03a479bf5c1837af8e2aef9589eb6759382a67e74340d003406d4abfac2caa0"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "cannot_happen: normal distributino sampler, positive or negative ok, and upstream also doesn't guard aganst NaN etc."
independence = "someone_else"
limits_and_guards = "no_limits: normal distributino sampler, positive or negative ok, and upstream also doesn't guard aganst NaN etc."
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"

[review.signature]
key = "k1"
alg = "ed25519"
value = "chkvVlut2e+QFs3xbMHqQmxMfH11WCsgdWD6+f7xlpe8nhDbyrM5yrJckrNHHrGnmg8O3ZiT5K9sY2ApkNydCA=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics"
kind = "implements"
```

normal distributino sampler, positive or negative ok, and upstream also doesn't guard aganst NaN etc.

upstream:
double normal_variate(double mean, double standard_deviation, uint64_t* seed)
{
  // Sample a normal variate using Marsaglia's polar method
  double x, y, r2;
  do {
    x = uniform_distribution(-1., 1., seed);
    y = uniform_distribution(-1., 1., seed);
    r2 = x * x + y * y;
  } while (r2 > 1 || r2 == 0);
  double z = std::sqrt(-2.0 * std::log(r2) / r2);
  return mean + standard_deviation * z * x;
}

looks legit

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-25, commit c0d377264b.

# Review: isotropic_direction (github:theodoreOnzGit)

```toml
[kovan]
id = "review-isotropic-direction-theodoreonzgit-7988a62a"
kind = "review"
origin = "human"
created = "2026-11-25T16:47:10+08:00"
modified = "2026-11-25T16:47:10+08:00"
target = "fn:7988a62af8c6ea5b"

[review]
path = "crates/outram-mc-libs/src/rng/distributions.rs::isotropic_direction"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-25"
signed_at = "2026-11-25T16:47:10+08:00"
commit = "dffd97179bfc31350e13c7e526074081eaafd29b"
hash = "sha256:576fe9036e71d2d607368c650c190582e256534a212cb77704d2acc72253662d"
doc_hash = "sha256:fa82d7214c781596ab2664347b7d1ef8cb7f297757987ba6f67af9284924c371"
cargo_lock = "sha256:4a48251d078c2265757f8e935e6bb409d9e9caff8c543ed902119772ad0d2989"

[review.callees]
"fn:681ddf6544e7b60b" = "sha256:17638ea97982299a8f568290be2092c01c5d16f34183e2d73318e4c404ae4734"
"fn:810110ab0e15d3a7" = "sha256:36092fde7654bdb4c9ed9083fe9bb588f4bee49ff71da7a559fb22f5abcfc634"
"fn:d76bd3ba0e568fb8" = "sha256:4137e7fe123572c33ffc4fb51dc372d0b228b82cfdc2b8cbef8a21fee12fc462"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "cannot_happen: unsigned u64, no need to worry about bad things, i think"
independence = "someone_else"
limits_and_guards = "no_limits: unsigned u64, no need to worry about bad things, i think"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "mixed"
sessions = [
    "https://claude.ai/code/session_012Jf5jeczK6fvKddBWDrLwE",
    "https://claude.ai/code/session_01AhE6XBx3nRd81aHLQrciQe",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "tv5UUOTRhZvc82XCAisypc3yN3EVLNm0cwkTVIKEXYMp8VoP2tBNFrPk7ms0nqHv4aGsQrvZvEtwEVDjWBNqDw=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics"
kind = "implements"
```

looks about right, the spherical coordinate vector is given
number guards also apply

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-25, commit dffd97179b.
