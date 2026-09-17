# HEATR — heating (KERMA) and damage energy

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §HEATR); upstream Fortran: `heatr.f90` (~6.3k lines).

## Theory

HEATR produces two pointwise quantities and appends them to the PENDF file:

- **Heating / KERMA (MT=301)** — the energy deposited locally per reaction,
  `H(E) = Σ_r σ_r(E)·Ē_dep,r(E)`. The deposited energy per reaction is found by
  **energy balance**: `E_dep = E + Q − Ē'_neutrons − Ē_photons`, i.e. the
  available energy minus what escapes as secondary neutrons and photons. HEATR's
  full method reads the photon-production data (MF=12–15, MF=6) and enforces
  momentum conservation for the residual-nucleus recoil (notably for capture).
- **Damage energy (MT=444)** — the fraction of recoil kinetic energy that goes
  into atomic displacements rather than electronic excitation, via the
  **Lindhard–Robinson partition** `df(E_R)`. Integrated over the recoil spectrum
  it gives DPA-relevant damage-energy cross sections.

A **kinematic KERMA** (no photon files) is available and is what the
energy-balance method is checked against (`kchk`).

## How the port implements it

Ported in [`crate::heatr`] (see `docs/porting-plan.md` §3 for the H1–H7 split):

- **H1–H5** ✅ — kinematic-limit KERMA per reaction class: elastic (H1),
  local-deposition capture/charged-particle (H2), single-escaping-neutron with Q
  (H3), fission (H4, `E + Q_f − ν̄·⟨E'⟩`), and multi-neutron/continuum (H5,
  `E + Q − ȳ·⟨E'⟩` from MF=5/MF=6 emission spectra). Wired into the ACE ESZ
  heating column (4e).
- **H7** 🟡 — damage energy for two-body channels (elastic + discrete inelastic
  levels) with the Lindhard `df` and NJOY's default displacement-threshold table.
- **H6** ⬜ — the full photon energy-balance method (MF=12–15/MF=6, capture recoil
  momentum) is **deferred**; H1–H5 currently stand in as the kinematic limit.
- **H7 remaining** ⬜ — MF=4 angular anisotropy of recoil, continuum/(n,xn)/
  capture-recoil (`capdam`) damage channels.

## Testing

**Unit** — 26/26 `heatr` unit tests (`cargo test -p njoy-outram-park-fork --lib
heatr`): H5 reproduces `σ·(E+Q−ȳ⟨E'⟩)` to <1e-9 with a closed-form Watt mean; H7
reproduces NJOY's `E_d` table and the Lindhard `df/E_R` signature (≈1 near
threshold, <0.1 at 10 MeV). See `docs/porting-plan.md` §3.

**Closed form** — `tests/heatr_kerma_vs_kinematics.rs`: on U-238, KERMA matches
the H1+H2 closed form to **1.8e-16** below the first inelastic threshold, and is
linear in σ to <1e-9. Its doc comment records two incomplete-*oracle* findings
this assertion caught (omitted subthreshold fission, omitted (n,α) ground state).

**Cross-code** — `tests/heatr_vs_njoy2016.rs` against NJOY2016 `ac5adf5f` built
from source and run (`reference-data/heatr/`, decks committed):
- KERMA vs NJOY's `local=1` MT=301 in the capture-dominated region: Si-28 agrees
  to **1e-6**; Fe-58 differs by exactly its evaluation's own **208 keV** photon
  energy-balance deficit, constant to 4 s.f. over eight decades.
- Elastic damage vs MT=445, 1.2–10 keV: mean **+0.16 %** (Fe-58) / **+0.31 %**
  (Si-28).
- The missing MF=4 anisotropy is now **priced**: +28.5 % (Fe-58) / +68.3 %
  (Si-28) on elastic damage in 0.1–0.9 MeV, and MT=444 totals up to 1.81× NJOY.

Write-up: `verification_and_validation/heatr_vs_njoy2016.md`.

## Caveats

- Until H6 lands, KERMA is the **kinematic limit**, not the full photon
  energy-balance value — they differ where photon-production data matters
  (especially capture heating). Measured: exactly zero difference on Si-28,
  +3.27 % on Fe-58, whose evaluation's capture photons fall 208 keV short of
  its Q.
- Damage (MT=444) is two-body only; anisotropy and (n,xn)/capture recoil are
  missing, and MT=444 is not yet emitted as its own ACE MTR reaction. The
  anisotropy's cost is measured above; do not read MT=444 above ~50 keV as
  usable.
- **Open, measured, unexplained:** NJOY's MT=445 is non-zero ~5 % *below* the
  two-body kinematic threshold `E_d` implies (562.5 eV vs 594.5 eV on Fe-58).
  NJOY's `df` cut and its `E_d` table were both read and match this port
  exactly, so the difference is in `disbar`'s recoil bounds, which have not
  been. Magnitude 0.86 eV·b against a scale of 1e5.

## References

- NJOY2016 manual §HEATR (LA-UR-17-20093)
- `heatr.f90` (NJOY2016 2016.79)
- Lindhard et al. (1963); Robinson (1970) — displacement damage partition
