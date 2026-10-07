# Upstream: CoolProp IF97

```toml
[kovan]
id = "upstream"
kind = "upstream"
origin = "human"
created = "2026-10-07T10:00:00+08:00"
modified = "2026-10-07T10:00:00+08:00"

[upstream]
is_port = true
repository = "https://github.com/CoolProp/CoolProp"
commit = "0123456789abcdef0123456789abcdef01234567"
confirmed_by = "github:theodoreOnzGit"
date = "2026-10-07"

[upstream.files]
"steam.rs" = "https://github.com/CoolProp/CoolProp/blob/0123456789abcdef0123456789abcdef01234567/src/IF97.h"
```

# Review: SteamTable::flash (github:theodoreOnzGit)

```toml
[kovan]
id = "review-flash-theodoreonzgit"
kind = "review"
origin = "human"
created = "2026-10-07T10:00:00+08:00"
modified = "2026-10-07T10:00:00+08:00"
target = "code:crates/tampines/src/steam.rs::SteamTable::flash"

[review]
function = "crates/tampines/src/steam.rs::SteamTable::flash"
by = "github:theodoreOnzGit"
rung = 4
date = "2026-10-07"
commit = "0123456789abcdef0123456789abcdef01234567"
hash = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
doc_hash = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
cargo_lock = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"

[review.callees]
"crates/tampines/src/steam.rs::saturation" = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"

[review.checklist]
q1 = "yes"
q8 = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_example"]

[[relation]]
target = "artifact:iapws-if97#eq-7"
kind = "implements"
page = 7

[[relation]]
target = "artifact:arch-flash-loop"
kind = "part_of"
```

## Comments

Checked against IAPWS-IF97 region 4.

## Sign-off

Reviewed by github:theodoreOnzGit at rung 4 on 2026-10-07, commit 0123456789.

# Needs fix: saturation guard

```toml
[kovan]
id = "fix-saturation"
kind = "needs_fix"
origin = "human"
created = "2026-10-07T10:00:00+08:00"
modified = "2026-10-07T10:00:00+08:00"
target = "code:crates/tampines/src/steam.rs::saturation"

[needs_fix]
function = "crates/tampines/src/steam.rs::saturation"
by = "github:theodoreOnzGit"
date = "2026-10-07"
commit = "0123456789abcdef0123456789abcdef01234567"
hash = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
note = "no guard below the triple point"
status = "open"
highlights = ["hl-saturation"]
```

# Highlight: saturation clamp

```toml
[kovan]
id = "hl-saturation"
kind = "annotation"
origin = "human"
created = "2026-10-07T10:00:00+08:00"
modified = "2026-10-07T10:00:00+08:00"
target = "code:crates/tampines/src/steam.rs::saturation"

[annotation]
function = "crates/tampines/src/steam.rs::saturation"
by = "github:theodoreOnzGit"
commit = "0123456789abcdef0123456789abcdef01234567"
needs_fix = "fix-saturation"

[[annotation.selector]]
type = "TextQuoteSelector"
exact = "t.max(273.16)"
prefix = "let t = "

[[annotation.selector]]
type = "TextPositionSelector"
start = 40
end = 53
```

# Architecture: flash loop

```toml
[kovan]
id = "arch-flash-loop"
kind = "architecture"
origin = "human"
created = "2026-10-07T10:00:00+08:00"
modified = "2026-10-07T10:00:00+08:00"

[architecture]
by = "github:theodoreOnzGit"
date = "2026-10-07"
commit = "0123456789abcdef0123456789abcdef01234567"
members = ["crates/tampines/src/steam.rs::SteamTable::flash", "crates/tampines/src/steam.rs::saturation"]
pattern = "concept:numerics/newton-iteration"

[architecture.upstream]
style = "key_value"
line = 1
project = "CoolProp"
repository = "https://github.com/CoolProp/CoolProp"
commit = "0123456789abcdef0123456789abcdef01234567"
url = "https://github.com/CoolProp/CoolProp/blob/0123456789abcdef0123456789abcdef01234567/src/IF97.h"
```

# Deleted functions

```toml
[kovan]
id = "deleted-functions"
kind = "deleted_functions"
origin = "human"
created = "2026-10-07T10:00:00+08:00"
modified = "2026-10-07T10:00:00+08:00"

[[deleted]]
function = "crates/tampines/src/steam.rs::old_flash"
path = "crates/tampines/src/steam.rs::old_flash"
deleted_commit = "89abcdef0123456789abcdef0123456789abcdef"
branch = "develop"
last_review_commit = "0123456789abcdef0123456789abcdef01234567"
reviewers = ["github:theodoreOnzGit"]
```
