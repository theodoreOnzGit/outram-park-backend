# Review: SteamTable::flash (github:founder)

```toml
[kovan]
id = "review-flash-founder"
kind = "review"
origin = "human"
created = "2026-10-10T10:00:00+08:00"
modified = "2026-10-10T10:00:00+08:00"
target = "fn:3bbbea796e38ff70"

[review]
path = "crates/tampines/src/steam.rs::SteamTable::flash"
by = "github:founder"
rung = 3
date = "2026-10-10"
signed_at = "2026-10-10T14:03:09+08:00"
commit = "0123456789abcdef0123456789abcdef01234567"
hash = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
doc_hash = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
cargo_lock = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
no_concept = "a plain helper"
separation_attestation = "sep-2026-10-10"

[review.callees]
"fn:00000000000000aa" = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
"fn:00000000000000bb" = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"

[review.checklist]
doc_matches_behaviour = "yes"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_b",
    "https://claude.ai/code/session_a",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "10hb43qsHo/ik6xXnt2DREeOnHSNyTyfFTuHbjZ/3UYSg3ou/Z05VnO7nweIbtT1OmpvAXsm32u+d3z5DVI1AA=="

[[relation]]
target = "artifact:iapws-if97#eq-7"
kind = "implements"
```

## Comments

The pinned v3 stamp (GitHub #825): every signed field present.

## Sign-off

Reviewed by github:founder at rung 3 on 2026-10-10, commit 0123456789.

# Architecture: steam flash (github:founder)

```toml
[kovan]
id = "arch-steam"
kind = "architecture"
origin = "human"
created = "2026-10-10T10:00:00+08:00"
modified = "2026-10-10T10:00:00+08:00"

[architecture]
by = "github:founder"
date = "2026-10-10"
signed_at = "2026-10-10T14:05:00+08:00"
commit = "0123456789abcdef0123456789abcdef01234567"
members = [
    "fn:3bbbea796e38ff70",
    "fn:00000000000000aa",
]
member_paths = [
    "crates/tampines/src/steam.rs::SteamTable::flash",
    "crates/tampines/src/steam.rs::saturation",
]
pattern = "concept:flash"

[architecture.signature]
key = "k1"
alg = "ed25519"
value = "tcjqJx2MSdHab5+9dbcSqZkhgmL6cYH1kuazJRai+bZnTK+DmjopupHsOXNv5pthXi4gW9BC3qKPJWLBLkITBw=="

[[relation]]
target = "artifact:iapws-if97"
kind = "implements"
```

The pinned v3 architecture node (GitHub #825).
