# outram-park-fork-onix

Pure-Rust depletion / burnup — an independent translation of the MIT-licensed [ONIX](https://github.com/jlanversin/ONIX) depletion code.

Solves the depletion (Bateman) equations via a CRAM (Chebyshev Rational Approximation Method) solver, producing the actinide + fission-product isotopic inventory that feeds the MSRE fission-product-migration chemistry. Data-free: consumes cross sections/decay data from `njoy-outram-park-fork`.

> **⚠️ Untrusted AI-assisted draft — pending human V&V.** First-pass port under
> the MSRE digital-twin epic (`op-6w0`, bead `op-6w0.2`). Independent OUTRAM
> PARK fork; not affiliated with the upstream project. Not for nuclear facility
> operation, reactor control, safety-critical, or licensing decisions.

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
(`cram16`, `clamp_nonnegative`), `driver` (`DepletionSystem`).

**Verification (not validation).** `tests/vv_bateman.rs` checks CRAM16 against
the closed-form Bateman solution: a three-member decay chain (max abs error
3.8e-15), total-atom conservation (1.8e-14), secular equilibrium (activity
ratio 1.000001), a transmutation burnup step (9.1e-15), fission-yield split
(exact ratio), and multi-step composition (8.4e-15) — all measured 2026-08-04.
These verify implementation-correctness only; physical validation against
measured inventories is out of scope for this draft.

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
