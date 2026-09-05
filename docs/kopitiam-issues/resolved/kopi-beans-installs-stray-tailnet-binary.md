# kopi-beans: `cargo install kopi-beans` also installs an undocumented, unprefixed `tailnet_*` binary

- **Tool / version when observed:** kopi-beans 0.1.1 (from `cargo install --list`)
- **Platform:** Linux x86_64 (Arch)
- **Date observed:** 2026-08-07
- **Upstream:** [kopitiam#15](https://github.com/theodoreOnzGit/kopitiam/issues/15)

## What I ran

    $ cargo install kopi-beans
    $ cargo install --list

## What happened

Installing kopi-beans placed a second, undocumented binary with an unprefixed
`tailnet_*` name onto `PATH` alongside `bn`. An unprefixed generic name in
`~/.cargo/bin` risks colliding with unrelated tooling, and it was not
mentioned in the crate's documentation.

## What I expected

`cargo install kopi-beans` to install exactly one documented binary, `bn` —
or, if a helper binary is genuinely needed, for it to carry a `kopi-`/`bn-`
prefix and be documented.

## Impact here

Low but real: this workspace's `CLAUDE.md` mandates installing kopi-beans from
crates.io, so every clone would have picked up the stray binary.

## Resolved

- **Fixed in:** kopi-beans 0.1.2
- **Verified:** 2026-08-07
- **Re-ran:** `cargo install --list` and `ls ~/.cargo/bin/ | grep -i tail`
- **New output:**

      $ cargo install --list | grep -A5 kopi-beans
      kopi-beans v0.1.2:
          bn

      $ ls ~/.cargo/bin/ | grep -i tail
      (no output — the binary is absent)

  kopi-beans 0.1.2 installs only `bn`, and no `tailnet_*` binary is present.

- **Note:** the upstream GitHub issue was still marked **OPEN** at the time of
  this verification. It needs the maintainer to close it:

      gh issue close 15 --repo theodoreOnzGit/kopitiam \
        --comment "Fixed in kopi-beans 0.1.2 — verified 2026-08-07; cargo install --list shows only bn, no tailnet_* binary."
