# outram-park-fork-onix

Pure-Rust depletion / burnup — an independent translation of the MIT-licensed [ONIX](https://github.com/jlanversin/ONIX) depletion code.

Solves the depletion (Bateman) equations via a CRAM (Chebyshev Rational Approximation Method) solver, producing the actinide + fission-product isotopic inventory ~~that feeds the MSRE fission-product-migration chemistry. Data-free: consumes cross sections/decay data from `njoy-outram-park-fork`.~~ **CORRECTED 2026-11-27** — meant to feed the MSRE fission-product-migration chemistry, but no other crate depends on it yet (`AGENTS.md`; no `crates/*/Cargo.toml` names it). Data-free: the caller supplies decay data, reaction rates and yields; they are meant to come from `njoy-outram-park-fork`, but this crate does not depend on it (its dependencies are `petir`, `uom` and `thiserror`, verified in `Cargo.toml`).

**Name.** `outram-park-fork-onix` is an OUTRAM PARK fork, named for its
upstream, ONIX; the backronym is OUTRAM PARK's own, letter by letter:
**O**pen-source **U**nified **TRA**nsient **M**ulti-Physics **A**dvanced
**R**eactor simulation **K**it.

**The question it answers:** given decay data, one-group reaction rates,
fission yields and an initial inventory, what is the nuclide inventory after a
depletion step?

**What this crate makes studyable and challengeable.** The Bateman burnup
matrix and the order-16 CRAM, small enough to read in one sitting beside ONIX's
Python (`onix/salameche/cram.py`, `mat_builder`), with every ported module
carrying its ONIX provenance header. The differential against ONIX run from
source is a worked example of a comparison that challenged the port: it found
missing ground-to-isomer channels and wrong daughter states for excited
parents, and records the matrix before and after the fix. The workspace also
holds OpenMC's CRAM in `petir`, so a reader can compare two faithful
translations of the same algorithm and see why they differ by round-off.

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`).

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream: ONIX (github.com/jlanversin/ONIX, MIT), commit `7328dc6`, provenance in `upstream_source/README.md`, `LICENSE.onix` and each module's header. CRAM: Pusa (2011), "Rational Approximations to the Matrix Exponential in Burnup Calculations", *Nucl. Sci. Eng.* 169, 155-167, and Pusa and Leppanen, cited in `src/cram.rs`. In `crates/kovan-literature/CATALOGUE.md`: `robertson1965msre` (ORNL-TM-728, MSRE design), listed for the MSRE group this crate belongs to. |
| Mathematical equations | The Bateman equations `dn/dt = A n` and their solution `n(dt) = exp(A dt) n0` (this README and `src/matrix.rs`, `src/driver.rs`); the order-16 CRAM partial-fraction form (`src/cram.rs`, `petir::depletion::onix_cram`). |
| Source code | `src/nuclide.rs`, `src/reactions.rs`, `src/chain.rs`, `src/matrix.rs` (`BurnupMatrix`), `src/cram.rs` (wrapper over `petir::depletion::onix_cram::cram16_pfd` since gh:#999), `src/driver.rs` (`DepletionSystem`). |
| Interactive simulations | None yet. |
| Visualisations | None yet. |
| Verification and validation evidence | `tests/vv_bateman.rs` (2026-08-04): CRAM16 against closed-form Bateman, three-member chain max abs error 3.8e-15, atom conservation 1.8e-14. `verification_and_validation/onix_differential/RESULTS.md` (2026-11-13, gh:#1404): burnup matrix from ONIX's own libraries 36 143 of 36 143 entries matched after the fix (19 184 bit-identical, worst relative 5.1e-16); 10 steps of 30 d normwise 4.5e-14 after the fix, 2.40e-6 before. `tests/cram16_move_fingerprints.rs` pins the move to `petir`. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md`. Struck-through corrections in `AGENTS.md` (the CRAM move to `petir`, the cross-check against OpenMC's CRAM, done 2026-10-20). |

**Status:** the Bateman matrix and CRAM16 have machine-run evidence against
closed-form solutions and against ONIX itself; none of it is human-reviewed,
and no inventory has been compared with a measured one.

> **⚠️ Untrusted AI-assisted draft — pending human V&V.** First-pass port under
> the MSRE digital-twin epic (`op-6w0`, bead `op-6w0.2`). Independent OUTRAM
> PARK fork; not affiliated with the upstream project. Not for nuclear facility
> operation, reactor control, safety-critical, or licensing decisions.

## `no_std` + `alloc` (gh:#1589)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p outram-park-fork-onix --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). `std` only turns on `thiserror`'s `std`; everything else is `no_std` + `alloc` already. With `std` on (the default)
nothing changes.

## Depletion core (implemented)

Stand-alone (precomputed-input) depletion: the caller supplies decay data,
one-group reaction rates, fission yields, and an initial inventory; the crate
assembles the Bateman burnup matrix `A` (units `1/s`) and computes
`n(dt) = exp(A*dt) * n0` with the **order-16 CRAM** solver — the same algorithm
and coefficients as ONIX `onix/salameche/cram.py`.

```rust
use outram_park_fork_onix::{
    DepletionSystem, DecayData, ReactionRates, FissionYields, Nuclide, DecayMode,
};

let a = Nuclide::new(50, 100, 0);
let b = Nuclide::new(51, 100, 0);
let c = Nuclide::new(52, 100, 0);

let mut sys = DepletionSystem::new();
sys.add_nuclide(a, DecayData::single_mode(1e-2, DecayMode::BetaMinus),
                ReactionRates::none(), FissionYields::empty()).unwrap();
sys.add_nuclide(b, DecayData::single_mode(1e-3, DecayMode::BetaMinus),
                ReactionRates::none(), FissionYields::empty()).unwrap();
sys.add_nuclide(c, DecayData::stable(),
                ReactionRates::none(), FissionYields::empty()).unwrap();

let n0 = sys.inventory_vector(&[(a, 1.0)]).unwrap();
let n = sys.deplete(&n0, 100.0).unwrap(); // deplete 100 s
```

Public modules: `nuclide` (`Nuclide`, packed `zamid`), `reactions`
(`DecayMode`, `ReactionChannel` + daughter lookup), `chain` (`DecayData`,
`ReactionRates`, `FissionYields`), `matrix` (`BurnupMatrix`), `cram`
(`cram16`, `clamp_nonnegative`), `driver` (`DepletionSystem`). Since
2026-10-20 (gh:#999) the CRAM16 arithmetic itself lives in `petir` as
`petir::depletion::onix_cram::cram16_pfd`; `cram16` is a wrapper with the same
signature.

**Verification (not validation).** `tests/vv_bateman.rs` checks CRAM16 against
the closed-form Bateman solution: a three-member decay chain (max abs error
3.8e-15), total-atom conservation (1.8e-14), secular equilibrium (activity
ratio 1.000001), a transmutation burnup step (9.1e-15), fission-yield split
(exact ratio), and multi-step composition (8.4e-15) — all measured 2026-08-04.
These verify implementation-correctness only; physical validation against
measured inventories is out of scope for this draft.

**Differential against ONIX (gh:#1404, 2026-11-13).** The upstream Python was run and
compared: `verification_and_validation/onix_differential/` (`RESULTS.md`). `CRAM16`
differs from numpy's LAPACK-based `CRAM16` by at most 3e-14 normwise (6 of 96 entries
bit-identical; a Python replica of this crate's pole solve inside the upstream source is
96 of 96 bit-identical, so the difference is LAPACK against the textbook elimination). The
burnup matrix matched on 99 of 99 synthetic and 36143 of 36143 entries built from ONIX's
own libraries (88 and 19184 bit-identical, the rest within 4 ulp) **after** fixing two
defects the comparison found: no ground-to-isomer channels, and daughters of excited
parents keeping `m`. Before the fix 403 of 36143 entries were missing and 31 in the wrong
row.

**Not yet ported:** OpenMC coupling, ONIX's bundled nuclide-data libraries
(caller supplies data), predictor-corrector flux schemes, order-48 CRAM (ONIX
ships only order-16), and the input/sequence/reporting machinery.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
