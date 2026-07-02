# Roadmap — Keff (U-235/U-233) + U-238 Doppler broadening

Cross-crate plan for two near-term OUTRAM PARK goals, spanning the
`openmc-libs` Monte Carlo kernel and the `njoy-outram-park-fork` data toolkit.
Opened 2026-07.

## Progress (2026-07)

- ✅ **Faddeeva kernel** `w(z)` — pure-Rust Weideman rational approximation
  (`wmp::faddeeva`), validated against scipy `wofz` to 1e-6. No FFI.
- ✅ **WMP evaluator** `wmp::WindowedMultipole::evaluate` — window walk +
  curve-fit background + analytic Doppler pole sum; faithful to OpenMC `wmp.cpp`.
- ✅ **WMP HDF5 reader** `wmp::WindowedMultipole::load_h5` — behind the
  `wmp-hdf5` feature (pure-Rust `hdf5-pure`, no system libhdf5).
- ✅ **Real U-238 Doppler demonstrated** (MIT `WMP_Library`, ENDF/B-VII.1): the
  6.673 eV capture resonance peaks at **22 262 b (0 K) → 7 110 b (294 K) →
  4 283 b (1000 K)** — see `tests/wmp_u238.rs`. This satisfies goal 2's kernel;
  remaining is a quantitative gate vs the OpenMC pointwise `.h5`.
- ⏭ **Next:** read the OpenMC pointwise `.h5` (`endfb-viii.0-hdf5/neutron/`) to
  extract the reference σ(n,γ)(E,T) curve and gate WMP against it; then U-235/233
  + the transport side (geometry → Keff). NB: WMP U is VII.1 while the pointwise
  reference is VIII.0 — for a clean gate, cross-check with njoy BROADR on the
  matching ENDF tape (`ENDF-B-VIII.0/neutrons/n-092_U_238.endf`).

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
| σ_t/σ_a/σ_f in the resonance range, any T | **WMP** (`wmp::WindowedMultipole`) + `wmp::faddeeva` | ✅ WMPB v1 blob (deflate; `to_blob`/`from_blob` done, CORE set not baked yet) |
| ν̄(E) | ENDF MF=1/452 (ACER 4b) or hardcoded | ✅ tiny table (`nuclear_data::secondary::NuBar`) |
| χ(E) fission spectrum | Watt (2 params) or MF=5 (ACER 4d) | ✅ tiny (`nuclear_data::secondary::FissionSpectrum`) |
| Provider surface consumed by transport | `nuclear_data::XsProvider → MicroXs` | n/a (API) |
| Reference σ(E,T), reference Keff | OpenMC pregenerated `.h5` + ICSBEP | ❌ download / cite only |

**Fast-range coverage — REVISED (2026-07, supersedes the lean-ACE-tail plan).**
The WMP ceilings sit *far* below the fission spectrum (measured `e_max`: U-233
**600 eV**, U-235 **2.25 keV**, U-238 **20 keV**), while a bare-sphere fission
spectrum lives at **0.1–10 MeV**. So WMP covers thermal + resonance (Doppler,
self-shielding — its strength) but *none* of the fast range where the Keff
actually lives. The fast tail is therefore handled by **multigroup**, not a
pointwise lean-ACE tail:

- **Below `e_max`:** WMP continuous-energy (keeps analytic Doppler).
- **Above `e_max`:** a coarse fast group set (≈30 groups, fission/hard-spectrum
  weighted). The fast range is smooth, so group-averaging is accurate — this is
  multigroup's home turf; Godiva/Jezebel Keff was historically nailed to a few
  hundred pcm with 16–26 group sets. Data is ~KB/nuclide (group σ + a
  downscatter-dominated transfer matrix) → **<1 MB for the whole CORE set**, vs
  several MB of pointwise. This is why full ACE never ships.

**Transport seam.** Keep CE particles throughout; above `e_max` look up
piecewise-constant group σ for the particle's energy, and on scatter pick an exit
group from the transfer matrix, then sample a continuous within-group energy from
the weighting spectrum ("CE particle, MG data above the ceiling"). The CE loop
stays intact; approximations (stair-stepped σ, within-group exit sampling) are
fine for a first Keff.

**Two must-dos for correctness.** (1) The URR just above `e_max` (U-238 ~20–150
keV) self-shields — use **Bondarenko f-factors** (MG) or probability tables, not
infinite-dilution group averages, or U-238 capture/breeding comes out wrong.
(2) Bake the fast group constants with a **fission/hard spectrum**; they are only
valid for the spectrum they were collapsed with.

The old `nuclear_data::LeanAce` pointwise-tail path is demoted to a fallback for
any nuclide lacking both a WMP entry and a group set; ACE stays an *offline
source* the ACER port emits, from which the group constants are collapsed.

**Reference extraction is HDF5-free in-crate.** `openmc-libs` has no HDF5 dep.
Extract the reference U-238 (n,γ) curve (and Keff) from the `.h5` *offline*
(python + openmc) into a small committed CSV/JSON fixture used as the test
oracle.

---

## Priority 2 — U-238 Doppler (do this first: smaller, self-contained)

A cross-section-level comparison, no transport required. Good first proof that
the WMP path works.

**njoy-outram-park-fork — WMP evaluator** (all nuclear data lives here)
1. ✅ `wmp::faddeeva` — pure-Rust `w(z)` (Weideman); validated vs scipy `wofz`.
2. ✅ `wmp::WindowedMultipole::evaluate` (window walk + curve-fit + Doppler pole
   sum). `from_blob` (embedded-blob decode) still TODO — the shipping path.
3. ✅ Load U-238 via `load_h5` (`wmp-hdf5` feature). Real Doppler broadening of
   the 6.67 eV capture resonance confirmed (`tests/wmp_u238.rs`). Blob-baking for
   the zero-dependency shipped build is the remaining step.

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
   njoy: wmp::WindowedMultipole ◄── WMPB blob    reference σ(n,γ)(T)
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
- **Fast range = multigroup, not pointwise lean-ACE** — WMP ceilings (600 eV –
  20 keV) sit far below the fission spectrum, so the fast tail is a coarse fast
  group set (<1 MB CORE) with a CE-particle/MG-data seam at `e_max`; URR needs
  Bondarenko self-shielding, constants baked with a hard spectrum. Full ACE never
  ships. See the "Fast-range coverage" block above. (2026-07)
```
