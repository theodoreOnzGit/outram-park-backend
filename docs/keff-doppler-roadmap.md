# Roadmap — Keff (U-235/U-233) + U-238 Doppler broadening

Cross-crate plan for two near-term OUTRAM PARK goals, spanning the
`openmc-libs` Monte Carlo kernel and the `njoy-outram-park-fork` data toolkit.
Opened 2026-07.

## Goals

1. **Keff of a bare critical sphere** for **U-235 (Godiva)** and **U-233
   (Jezebel-23 / Flattop-23)**, computed by the Rust `openmc-libs` MC kernel.
2. **Doppler broadening of the U-238 (n,γ) absorption spectrum**, computed in
   Rust and compared against a result OpenMC already produced.

## Hard constraint — data ships *inside* the crate

Some **low-fidelity** nuclear data must live in-crate so the end user downloads
nothing. **Chosen format: windowed multipole (WMP).** It is compact (KB–MB per
nuclide) and — crucially for goal 2 — broadens analytically. The bulky OpenMC
pregenerated `.h5` (ENDF/B-VIII) is **reference only**, downloaded manually by
the maintainer; it is never shipped.

Provenance: WMP is **MIT CRPG** work (`github.com/mit-crpg/WMP_Library`, MIT),
not NJOY/LANL. Add `LICENSE-WMP` (MIT) + a NOTICE credit before embedding data.

---

## Data strategy

**All nuclear data lives in `njoy-outram-park-fork`** (see `docs/architecture.md`);
`openmc-libs` is data-free and pulls cross sections through njoy's `XsProvider`.

| Need | Source (all in `njoy-outram-park-fork`) | Ships in-crate? |
|---|---|---|
| σ_t/σ_a/σ_f in the resonance range, any T | **WMP** (`wmp::WindowedMultipole`) + `wmp::faddeeva` | ✅ embedded zstd blob |
| ν̄(E) | ENDF MF=1/452 (ACER 4b) or hardcoded | ✅ tiny table (`nuclear_data::secondary::NuBar`) |
| χ(E) fission spectrum | Watt (2 params) or MF=5 (ACER 4d) | ✅ tiny (`nuclear_data::secondary::FissionSpectrum`) |
| Provider surface consumed by transport | `nuclear_data::XsProvider → MicroXs` | n/a (API) |
| Reference σ(E,T), reference Keff | OpenMC pregenerated `.h5` + ICSBEP | ❌ download / cite only |

**WMP coverage — decided.** WMP must reach up into the fast fission spectrum so
the bare-sphere Keff is covered, **provided the blob stays small**. Where a
nuclide's `e_max` falls short, fill the high-energy gap with a thin lean-ACE
pointwise tail (`nuclear_data::LeanAce`) rather than bloating the multipole set.
Watch the per-nuclide footprint as the deciding constraint.

**Reference extraction is HDF5-free in-crate.** `openmc-libs` has no HDF5 dep.
Extract the reference U-238 (n,γ) curve (and Keff) from the `.h5` *offline*
(python + openmc) into a small committed CSV/JSON fixture used as the test
oracle.

---

## Priority 2 — U-238 Doppler (do this first: smaller, self-contained)

A cross-section-level comparison, no transport required. Good first proof that
the WMP path works.

**njoy-outram-park-fork — WMP evaluator** (all nuclear data lives here)
1. Port `wmp::faddeeva` — pure-Rust `w(z)` (TOMS 916 or OpenMC's rational
   approximation). *This is the one physics kernel gating everything WMP.*
2. Port `wmp::WindowedMultipole::evaluate` (window walk + curve-fit + Doppler
   pole sum) and `wmp::WindowedMultipole::from_blob` (blob decode).
3. Bake a U-238 WMP blob (offline: MIT `WMP_Library` h5 → zstd) and load it.

**njoy-outram-park-fork — independent oracle** (uses already-ported modules)
4. `RECONR` (✅) → 0 K pointwise U-238 σ(n,γ); `BROADR` (✅) → broaden to T.

**Gate.** For T ∈ {0, 300, 1000, 2500} K over the resonance range, the three
curves agree within tolerance:
`WMP-analytic (Rust) ≈ BROADR-kernel (Rust njoy) ≈ OpenMC .h5 reference`.
Commit the `.h5`-extracted curve as the fixture; the njoy BROADR path is a
cross-check that isolates whether any disagreement is in WMP or in the data.

---

## Priority 1 — bare critical-sphere Keff

Needs real transport. Depends on Priority 2's WMP evaluator plus geometry and
physics still stubbed in `openmc-libs`.

**openmc-libs — transport gaps (from `docs/port-reference.md`)**
1. Geometry: `Sphere::distance`, `Cell::contains`, `Universe::find_cell`,
   `geometry::distance_to_boundary` — a bare sphere is one surface / one cell /
   vacuum outside, the minimal CSG case.
2. `Nuclide::xs_at_energy` — delegate to njoy's provider surface
   (`njoy_outram_park_fork::nuclear_data::XsProvider::Multipole { wmp, nu, chi }`
   → `MicroXs`). openmc-libs stays data-free; it only calls `micro(e, temp_k)`.
3. `physics::scatter` (elastic CM kinematics; isotropic-CM first cut),
   `physics::fission` (ν sampling → fission bank), `physics::transport`
   (collision + history loop).
4. Source: isotropic point/uniform-sphere spatial + Watt energy (`source::*`).
5. k-eigenvalue power iteration over the existing `particle::bank` fission bank.

**njoy-outram-park-fork — secondary data**
6. ACER **4b** (ν̄ / NU block, MF=1/452) and **4d** (χ, MF=5) for U-233/235 →
   the `NuBar` / `FissionSpectrum` tables. Hardcode from ENDF as a stopgap if
   4b/4d slip, flagged as such.

**Benchmarks (ICSBEP).**
- **Godiva** — HEU-MET-FAST-001, bare U(93.7) sphere, r ≈ 8.741 cm, keff = 1.0000 ± 0.0010.
- **U-233** — Jezebel-23 / Flattop-23 (U-233 bare/​reflected sphere).

**Gate.** Rust MC Keff matches the ICSBEP benchmark (and an OpenMC run on the
same `.h5`) within statistics (target a few hundred pcm at first).

---

## Cross-crate data flow

```
                 MIT WMP_Library (.h5)          ENDF/B-VIII (raw)
                        │ offline bake                 │
                        ▼                               ▼  njoy: RECONR→BROADR (oracle)
   njoy: wmp::WindowedMultipole ◄── zstd blob    reference σ(n,γ)(T)
        │  evaluate(E,T)  [wmp::faddeeva]               │
        │                                               ▼
        ├───────────────► Priority 2 test ◄──── OpenMC .h5 reference curve
        │                 (U-238 Doppler)
        ▼
   njoy: nuclear_data::XsProvider ── + ν̄,χ (nuclear_data::secondary; ACER 4b/4d)
        │  micro(E,T) → MicroXs
        ▼
   openmc-libs: Nuclide::xs_at_energy (delegates to njoy XsProvider)
        │
        ▼
   physics (scatter/fission/transport) + geometry (bare sphere)
        │
        ▼
   k-eigenvalue power iteration ──► Priority 1: Keff  ◄── ICSBEP + OpenMC
```

## Suggested order

1. `faddeeva` + `WindowedMultipole::evaluate` (unlocks both priorities).
2. **Priority 2** end-to-end (U-238 Doppler) — validates WMP cheaply.
3. Geometry (sphere) + `Nuclide::xs_at_energy` WMP backing.
4. Physics (scatter → fission → transport) + source.
5. **Priority 1** Keff on Godiva, then U-233.
6. njoy ACER 4b/4d to replace any hardcoded ν̄/χ with ENDF-derived tables.

## Open questions

- Delayed neutrons: prompt-only Keff first, or include the delayed fraction for
  the benchmark comparison?
- Elastic angular fidelity for fast U: isotropic-CM first cut vs a1 from MF=4 —
  how much does it move Godiva Keff?

## Resolved

- **WMP `e_max` coverage** — extend the WMP window up into the fast fission
  spectrum as long as the embedded blob stays small; fill any high-energy gaps
  with the `nuclear_data::LeanAce` pointwise tail. (2026-07)
- **Where WMP data lives** — in `njoy-outram-park-fork` (all nuclear data lives
  there; `openmc-libs` stays data-free and pulls via `XsProvider`). The
  `openmc-data-wmp` sibling-crate idea is dropped. (2026-07)
```
