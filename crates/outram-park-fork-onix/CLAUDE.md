# CLAUDE.md — outram-park-fork-onix

Guidance for Claude Code (and other AI assistants) working in this crate. The
workspace root `CLAUDE.md` binds here as everywhere; this file adds what is
specific to this crate. Added 2026-10-20: the member-crate rule requires one,
and this crate had none.

## What it is

An independent pure-Rust translation of **ONIX**
(<https://github.com/jlanversin/ONIX>, MIT, reference commit `7328dc6`). It
builds the Bateman burnup matrix from caller-supplied decay data, one-group
reaction rates and fission yields, and steps the inventory with **order-16
CRAM**, using the same algorithm and coefficients as ONIX
`onix/salameche/cram.py`. It is data-free; the data is meant to come from
`njoy-outram-park-fork`.

**Status: untrusted AI-assisted draft, no human V&V.** Checked 2026-10-20:
- **Verification, not validation:** `tests/vv_bateman.rs` checks CRAM16
  against closed-form Bateman solutions. The README records the measured
  errors (2026-08-04), from 3.8e-15 for a three-member chain to 1.8e-14 for
  atom conservation.
- **Not yet ported:** OpenMC coupling, ONIX's nuclide-data libraries,
  predictor-corrector schemes, order-48 CRAM, and the input and reporting
  machinery.
- **No consumer:** no other crate depends on it (none in any
  `crates/*/Cargo.toml`).

It belongs to the MSRE digital-twin group, with `outram-park-fork-moltres` and
`outram-park-fork-thermochimica` (`docs/claude-md/members-and-dependencies.md`;
scoping in `docs/reactor-scoping/msre.md`).

## Overlap: read before adding depletion code

The workspace has **another CRAM**. `petir::depletion` (`crates/petir/src/depletion/`)
holds order-16 and order-48 CRAM, ported from OpenMC `openmc/deplete/cram.py`
(first in `outram-mc-libs/src/depletion/`, then moved to `petir`). It also
holds OpenMC's depletion integrators, and `outram-mc-libs` depends on it.

- This crate's CRAM16 is a separate translation, from ONIX. Two
  implementations of the same algorithm can drift apart.
- **Before changing either one, compare them.** A useful check is a test
  asserting that this crate's `cram16` and `petir`'s order-16 path agree on
  the `vv_bateman` cases. **Not done yet; no such test exists.**
- **New depletion capability (order-48, integrators) should reuse `petir`**
  rather than being ported a second time from ONIX.

## Provenance (keep it)

- `src/cram.rs` (and the other ported modules) carry the provenance header:
  ONIX, commit `7328dc6`, MIT, relicensed as GPL-3.0-only (the file headers; ~~GPL-3.0-only~~ the crate is **AGPL-3.0-only** since 2026-10-20, gh:#973). Keep it.
- `LICENSE.onix` and `upstream_source/README.md` keep the MIT notice. The
  reference clone is gitignored; only the README is tracked.
- ~~#973 (workspace relicence to AGPL-3.0) will change the crate licence~~ **DONE 2026-10-20**: the crate is AGPL-3.0-only (gh:#973). The
  MIT notice stays.

## Working rules for this crate

- **Upstream is the specification** (root `CLAUDE.md`, "Debugging a port").
- **Data stays in `njoy-outram-park-fork`.** Do not bundle decay, cross-section
  or yield libraries here.
- Release builds only: `cargo test -p outram-park-fork-onix --release`.
- Regenerate `docs/outram-park-fork-onix-api.md` with
  `kovan-cli api-docs outram-park-fork-onix` when a public doc comment changes.
