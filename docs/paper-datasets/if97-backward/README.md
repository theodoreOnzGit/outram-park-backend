# Backward equations where IAPWS-IF97 is silent

**Source records:** `crates/tampines-steam-tables/verification_and_validation/`
— the five files under `generated/`, plus
`pressure_bounding_vs_openfoam_pressurecontrol.md`. Tracked as GitHub issue #34.

## The contribution, stated precisely

This is **not** "we implemented IF97 backward equations." IAPWS-IF97 already
publishes backward equations for the common inversions, and reimplementing them
is not a paper.

The contribution is **gap-filling**: fitted Chebyshev correlations for
inversions the standard does not cover at all.

| Inversion | IAPWS-IF97 status | Record |
|---|---|---|
| `p(ρ,h)` single-phase | **no published backward equation** | `generated/p_rho_h_single_phase.md` |
| `p(ρ,h)` Region 1 (subcooled liquid) | **none** | `generated/p_rho_h_region_1_conditioning.md` |
| `p(ρ,h)` Region 4 (two-phase dome) | **none** | `generated/p_rho_h_region_4_two_phase_dome.md` |
| `p(h,s)` near-critical Region 4 | **none** | `generated/region_4_near_critical_p_hs.md` |
| `T(p,h)`, `T(p,s)` Region 5 | **none** (IF97 publishes these for regions 1–3 only) | `generated/region_5_backward_t_ph_t_ps.md` |

`(ρ,h)` matters because it is the state pair a **density-based compressible CFD
solver** actually holds — which is why this work exists at all and where it
connects to Part I's `outram-foam` and the HEM choked-flow solver.

## The honest framing, which is also the interesting part

Each record already states it, and the paper must not quietly drop it:

> IAPWS-IF97 publishes no backward equations for some of the cases covered
> here, so where a reference is quoted it is either an IAPWS equation already
> implemented in this crate **or this crate's own forward equations** — never a
> published backward-equation reference value.

So the error metric is **round-trip consistency against the forward equations**,
not agreement with a published table. That is a weaker claim than an IAPWS
verification and it must be stated as such — but it is the *only* claim
available when the standard is silent, and it is exactly the right claim for a
correlation whose job is to invert those forward equations quickly.

The **Region 1 conditioning** record is the most interesting single result and
should probably drive the narrative:

> The error is overwhelmingly a **low-pressure** effect rather than a liquid
> effect, and the difference matters: the blunt reading ("do not use this in
> subcooled liquid") would rule out ordinary power-cycle conditions where the
> correlation is in fact accurate.

That is a conditioning argument about an ill-posed inversion — `∂p/∂ρ|_h` going
flat — and it generalises beyond water. It is a better paper than an error table.

## To extract (not yet done)

The five records carry their error tables below the methodology sections read so
far. Pull into CSV, one file per inversion:

- grid definition (e.g. single-phase: 60 × 60 `(p,T)`, p log-spaced 1e-3–50 MPa,
  T linear 280–2200 K)
- max / RMS / median relative error, and **where in the domain the max sits**
- the conditioning diagnostic for Region 1 as a function of pressure
- fitted-domain bounds, and behaviour outside them

Also extract the Moody HEM chart comparison from Part I if the paper reuses it
for continuity — check first whether reuse is appropriate or self-plagiarism.

## Regenerating

```bash
cargo test --release -p tampines-steam-tables --lib \
  backward_eqn_chebyshev_experimental::tests::p_rho_h
cargo test --release -p tampines-steam-tables --lib \
  backward_eqn_chebyshev_experimental::tests::region_4
cargo test --release -p tampines-steam-tables --lib \
  backward_eqn_chebyshev_experimental::tests::region_5
```

Generated 2026-09-02 21:45 UTC. **The generated `.md` files are
do-not-hand-edit** — if a number must change, change the fit and regenerate.
