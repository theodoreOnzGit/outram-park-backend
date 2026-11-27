# teh-o-prke

Point Reactor Kinetics Equations Module for the Teh-O package

Teh-O is the Transport, Eigenvalue and Hybrid Open Source Solver. It is meant to 
sound like Teh-O from Southeast Asia (Singapore Specifically).

- **Teh-O:** **T**ransport, **E**igenvalue and **H**ybrid **O**pen-source
  solver.
- **PRKE:** **P**oint **R**eactor **K**inetics **E**quations.

**Scope, and the one question it answers:** how does a reactor's total
power change in time after a reactivity change, treating the neutron
population as a single point with delayed-neutron precursors, feedback and
decay heat?

This crate makes **point kinetics studyable and challengeable**: a short
derivation of the equations from a neutron balance (`docs/prke_theory.typ`,
after Lamarsh and Bell & Glasstone), the N-group equations solved by backward
Euler and an OpenFOAM-derived RKF45, and every limiting case a student can
work by hand (the in-hour equation, a prompt-only Nordheim-Fuchs excursion)
checked against the code. The feedback sum is also differenced against
GeN-Foam's own point-kinetics class row by row.

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Lamarsh (2001) and Bell & Glasstone (1970), cited in `docs/prke_theory.typ` (BibTeX in `docs/prke_theory.bib`); RELAP-7 Theory Manual (INL/EXT-14-31366) §7.1–7.5, private, cited by equation number only; ENDF/B-VIII.0 U-235 MAT 9228 MT=455 delayed-neutron data (gh:#1319); ANS-5.1 1978 draft decay-heat set (`src/ans_decay_heat.rs`; later ANS tables are copyrighted and user supplied); ORNL-TM-1626 (1966) for MSRE circulating-fuel reactivity loss (`tests/msre_n4_circulation_loss_tm1626.rs`). Upstream code: OpenFOAM time-stepping and `SquareMatrix` LU (GPL-3.0, `src/time_stepping/`, `src/matrix.rs`); GeN-Foam `652b3da` (GPL-3.0, EPFL) point-kinetics feedback (`src/feedback_mechanisms/fast_reactor.rs`). |
| Mathematical equations | `docs/prke_theory.typ` (PDF beside it): neutron balance to the point kinetics equations with delayed precursors. Module docs: N-group PRKE and in-hour equation (`src/zero_power_prke/n_group_prke.rs`), circulating-fuel PRKE (`circulating.rs`), Nordheim-Fuchs (`src/nordheim_fuchs.rs`), RELAP5 separable feedback (`src/feedback_mechanisms/separable.rs`), fast-reactor feedback, Xe/Sm poisons, decay heat. |
| Source code | `src/`: `zero_power_prke` (`PointKinetics`, `SixGroupPRKE`, `CirculatingPointKinetics`), `feedback_mechanisms`, `fuel_temperature_feedback`, `control_rod_feedback`, `decay_heat`, `ans_decay_heat`, `point_reactor`, `nordheim_fuchs`, `delayed_neutron_layer`, `time_stepping`, `matrix`. |
| Interactive simulations | `examples/fhr_sim_v1` (egui FHR educational simulator); CLI examples `examples/reactivity_insertion_transient.rs` and `examples/decay_heat_after_shutdown.rs`. |
| Visualisations | The FHR simulator's live graph pages (`examples/fhr_sim_v1/app/graph_pages`). No committed images. |
| Verification and validation evidence | `verification_and_validation/prke_analytic_checks.md` (2026-11-01): six-group results bit-identical after the N-group refactor; in-hour `omega` 3.8e-6 to 7.0e-5 relative; prompt-only Nordheim-Fuchs peak 1.7e-4 and energy 8.3e-5 relative. `verification_and_validation/genfoam_differential/` (2026-11-13): against GeN-Foam `652b3da`, bit-identical on 830 feedback rows except one Doppler row per history (1 ulp in `ln`), and the GeN-Foam-form step 300/300 bit for bit. |
| Human reviews and corrections | 0 functions carry kovan human-review stamps. The maintainer chose ENDF/B-VIII.0 as the delayed-neutron source (gh:#1319, 2026-11-10) and the rod-worth out-of-range default (2026-11-10), both recorded in `AGENTS.md`. No entry in `docs/human-corrections-to-ai-work.md` names this crate. |

**Status:** declared mature on 2026-09-05 (`AGENTS.md`, the thinnest
declaration in the workspace) on unit tests and literature constants; the
analytic and GeN-Foam checks above are AI drafts not yet reviewed by a human.
U-233 and Pu-239 still use a textbook decay-constant set of unidentified
source, and actinide decay heat (gh:#1081) is not implemented.

## `no_std` + `alloc` (gh:#1600)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p teh-o-prke --lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). Every model builds without it: the
zero-power and n-group PRKEs, the six-group solvers (`ndarray` without its
`std` feature), the feedback mechanisms, decay heat, the point reactor, the
Nordheim-Fuchs stepper (including its first-order actuator lag, through
`chem-eng-real-time-process-control-simulator`, itself `no_std` since
gh:#1612) and the delayed-neutron layer. `std` adds only what the crate's
dependencies carry under theirs (chem-eng's CSV helpers); the library itself
prints and reads clocks only in tests. With `std` on (the default) nothing
changes.

## FHR Educational Simulator

To showcase teh-o-prke, an FHR educational simulator was constructed.
This includes PRKE with feedback for:

1. delayed neutron precursor
2. two control rod banks
3. fuel temperature feedback

This also includes accounting for:
1. decay heat

More features to be added in future...


```sh
cargo run --release -p teh-o-prke --example fhr_sim_v1
```

(Fixed 2026-11-27: `-p teh-o-prke` added so the command works from the
workspace root.)

~~Please remember to run the client AFTER the server.~~ **CORRECTED
2026-11-27** — `examples/fhr_sim_v1` is a single egui app with no server or
client (checked: no server/client code under `examples/fhr_sim_v1/`).

# prerequisites

~~You'll need openblas to run this on linux.~~ **CORRECTED 2026-11-27** —
no BLAS is needed: `Cargo.toml` has no `ndarray-linalg`, and the solve is the
pure-Rust `src/matrix.rs` (as `AGENTS.md` records, corrected 2026-11-01).

# licensing 

The point reactor kinetics code here copies some of the time stepping 
algorithm source files available in OpenFOAM. These are licensed files  
are available under GPLv3. The source files in Rust directly translte 
these source files. To respect OpenFOAM copyright, the PRKE files here 
are also released under GPLv3 (~~GPLv3~~ the crate is **AGPL-3.0-only** as a combined work, ported files keep GPL-3.0; relicensed 2026-10-20, gh:#973).

**The dense LU matrix solver (`src/matrix.rs`) is also from OpenFOAM** —
specifically, it is an inlined copy of `outram-foam-basic-lib`'s
`matrix::SquareMatrix` (itself an OpenFOAM translation), copied in directly
rather than kept as a path dependency so that the inter-crate dependency
graph stays acyclic (a future `tampines`/`nee_soon` composition can pull in
`teh-o-prke` and `tuas_boussinesq_solver` together without a dependency
loop). See the header of `src/matrix.rs` for the full attribution.


## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

(Added 2026-11-27: this README had no Bookkeeping table; both axes are left
unchecked because no sign-off is recorded for this crate.)

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
