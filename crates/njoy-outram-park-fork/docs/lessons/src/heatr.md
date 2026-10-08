# Rung 6 — HEATR and GASPR: heating, damage and gas

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response.

> **Review status: first draft, 2026-10-04, AI-assisted, not yet reviewed by a
> human.**

**Modules:** `heatr` (KERMA, MT=301; damage energy, MT=444), `gaspr` (gas
production, MT=203–207).
**Demo rung:** none planned; these are tables beside σ(E), shown on this page.

## The question

A cross section says how often a reaction happens. It does not say **where
the energy goes**. When a neutron is captured in U-238, about 4.8 MeV is
released, mostly as gamma rays that fly centimetres before depositing it; when
it scatters elastically off hydrogen, the recoiling proton deposits its share
within microns. **How does the data supply "heat deposited per reaction", and
the gas a material accumulates?**

## The shortest answer

- **KERMA** (kinetic energy released in matter) is a cross section weighted
  by the energy that stays local: `H(E) = Σ_x σ_x(E) · Ē_local,x(E)`, in
  eV·barn. Multiply by the flux and the atom density and you get W/cm³.
- **Damage energy** is the part of the recoil energy that goes into
  displacing atoms rather than heating electrons (the Lindhard partition).
- **Gas production** (MT=203–207) is the sum, over every reaction, of
  σ_x(E) times how many protons, deuterons, tritons, ³He or α it releases,
  counting the **residual nucleus** too.

## The formulas

The simplest bound, the **kinematic limit**, assumes every escaping neutron
carries its kinetic energy away and everything else stays. For isotropic
centre-of-mass elastic scattering off a nucleus of mass ratio `A`:

```text
⟨E'⟩ = E (1 + A²) / (A + 1)²        so      H_el(E) = σ_el(E) · E · 2A / (A + 1)²
```

For hydrogen (`A = 1`) the neutron loses half its energy on average, the
textbook check. Other channels, as ported:

```text
capture, charged-particle exits   H = σ (E + Q)
one escaping neutron              H = σ [ E · 2A/(A+1)²  +  Q/(A+1) ]
fission                           H = σ_f [ E + Q_f − ν̄(E) ⟨E'⟩ ]
multi-neutron, continuum          H = σ [ E + Q − ȳ ⟨E'⟩ ],   ⟨E'⟩ from MF=5 or MF=6
```

## What this port does differently from NJOY's default, stated first

NJOY's HEATR [(MacFarlane et al., 2017)](#ref-njoy2016) normally uses the **energy-balance method**: it subtracts the
energy carried away by the evaluation's own photons (from MF=12–15) and
neutrons. The kinematic limit is NJOY's *check* on that result. ~~**This port
computes the kinematic limit only**~~ **This port computes the kinematic limit**
(sub-phases H1–H5), **and** (corrected 2026-10-05,
[#535](https://github.com/theodoreOnzGit/outram-park-backend/issues/535))
`Kerma::with_energy_balance` subtracts the evaluation's photon energy
production from it (MT=442 via the `photon` module). ~~no `LO=2` cascades, no
MF=6 photons, no capture recoil~~ **Since H6a (later on 2026-10-05) MT=442 is
complete**: `LO=2` cascades, MF=6 photons, and capture by energy balance with
the photon recoil, each as HEATR does it, and it matches NJOY2016's MT=442 at
its print precision on Fe-58 and Si-28. The kinematic arm deposits each
reaction's Q as NJOY's `nheat` does (`Kerma::from_endf`): for a discrete level
that is the excitation energy too, which `QI` alone left out. ~~the photon
energy-balance method (H6) is deferred~~ What is **not** ported is the rest of
`nheat`: ~~each reaction's *neutron* energies taken from the evaluation (H6b),~~
the continuum and MF=6 *neutron* energies (`conbar`, `sixbar`, H6b part 2;
since H6b part 1, later on 2026-10-05, elastic and the discrete levels take
theirs from MF=4 through `disbar`, as NJOY does), and NJOY's treatment of a capture whose photons are in MF=6 (H6c). The
damage-energy port (H7) covers two-body recoils only (elastic and discrete
levels, isotropic in the centre of mass). See the module doc of
[`heatr/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/heatr/mod.rs).
~~That KERMA feeds the ACE heating column (rung 9).~~ The ACE heating column
(rung 9) gets the energy-balance value. ~~which has **no NJOY comparison** of
its own: the HEATR record below compares the uncorrected kinematic limit.~~
Against NJOY's energy-balance MT=301 it agrees below the first inelastic
threshold where capture photons are in MF=12 (Si-28, ~~6e-8~~ every point
within 4.7e-7 since H6b part 1), and ~~is **1.9–2.9× NJOY between 2 and 5
MeV** on both nuclides: the photons it subtracts are right, the neutron
energies it keeps are the kinematic estimate~~ above that threshold is within
**0.39 % (Fe-58) and 0.74 % (Si-28)** of NJOY in median since H6b part 1.
Most of the old 1.9–2.9× was MT=4 heated beside its own levels. Numbers:
[`heatr_vs_njoy2016.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/verification_and_validation/heatr_vs_njoy2016.md)
§4–§5 and the [re-measurement log](./remeasured.md).

**And then the whole module (2026-10-05, the full port).** Everything above
is `Kerma`, a reduced-order model built from HEATR's pieces. The rest of
`heatr.f90` (`conbar`, `sixbar`, the MF=6 capture path, `kchk`, the plot
file, the listing) has since been translated routine by routine as
[`heatr::heatr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/heatr/driver/mod.rs), and its output tape is
**identical in every byte** to NJOY2016's on all 62 neutron evaluations in
`reference-data/endf/`, at `local = 0` and again at `local = 1` with the
kinematic check on. The ACE heating column now comes from it
(`heatr::heatr_kerma`), so the 0.39 % and 0.74 % above describe `Kerma`, not
what the ACE table carries. The record is §6 of the same V&V file.

## The code walk

<!-- code-walk: from=crates/njoy-outram-park-fork/src/heatr/kerma.rs::Kerma::from_reconr to=crates/njoy-outram-park-fork/src/heatr/spectra.rs::single_neutron_factor
-->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `kerma.rs::Kerma::from_reconr` to `spectra.rs::single_neutron_factor`: 1 hop, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`kerma.rs::Kerma::from_reconr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/heatr/kerma.rs#L46) — Compute the kinematic-limit KERMA from a reconstructed evaluation.

<!-- snippet-check: crates/njoy-outram-park-fork/src/heatr/kerma.rs:46 fn from_reconr -->
<!-- snippet-check: crates/njoy-outram-park-fork/src/heatr/kerma.rs:69 single_neutron_factor -->

```rust,ignore
{{#include ../../../../../crates/njoy-outram-park-fork/src/heatr/kerma.rs:46:70}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`spectra.rs::single_neutron_factor`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/heatr/spectra.rs#L198) · called at [L69](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/heatr/kerma.rs#L69) — The `2A/(A+1)²` factor from the two-body elastic recoil formula (H1), shared by `HeatingModel::Elastic` (`Q=0`) and `HeatingModel::SingleNeutron` (`Q≠0`, see the derivation below).

<!-- snippet-check: crates/njoy-outram-park-fork/src/heatr/spectra.rs:198 fn single_neutron_factor -->

```rust,ignore
{{#include ../../../../../crates/njoy-outram-park-fork/src/heatr/spectra.rs:198:200}}
```

Unresolved calls inside the functions on this chain:

- in [`kerma.rs::Kerma::from_reconr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/heatr/kerma.rs#L46):
  - UNRESOLVED(closure): `contributes` at [L63](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/heatr/kerma.rs#L63) (→ [`crates/njoy-outram-park-fork/src/heatr/kerma.rs:55`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/heatr/kerma.rs#L55)) — call through a closure or fn-typed binding `contributes`
<!-- /code-walk -->

**Reading the walk.** The block above is generated by `kovan-cli code-walk-check --update`
(rust-analyzer's call hierarchy; every hop links to its lines at the commit this site was built
from). The table below is the same path with what each hop does:

| hop | function | what it does | resolved (kopitiam pass, 2026-10-04) |
|---|---|---|---|
| 0 | `Kerma::from_reconr` | RECONR result + ν̄ + χ + emission spectra → MT=301 | entry point ([source](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/heatr/kerma.rs#@@L:crates/njoy-outram-park-fork/src/heatr/kerma.rs:fn=from_reconr@@)) |
| 1 | `spectra::heating_model`, `single_neutron_factor`, `emission_spectrum`, `neutron_multiplicity` | per-MT dispatch over H1–H5 | kopitiam |
| 1 | `NuBar::at`, `FissionSpectrum::mean_energy`, `eval_lin_lin` | fission's escaping-neutron energy; the section values | kopitiam |
| 0 | `heatr::build_emission_spectra` | ⟨E′⟩ for (n,2n), (n,3n), MT=91 from MF=5/6 | entry point ([source](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/heatr/spectra.rs#@@L:crates/njoy-outram-park-fork/src/heatr/spectra.rs:fn=build_emission_spectra@@)) |
| 0 | `DamageEnergy` (`damage.rs`) | MT=444, the Lindhard partition with NJOY's `E_d` table | entry point ([source](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/heatr/damage.rs#@@L:crates/njoy-outram-park-fork/src/heatr/damage.rs:struct=DamageEnergy@@)) |
| 0 | `GasProduction::from_reconr_and_tape` | MT=203–207 from every reaction, residuals and breakups included | entry point ([source](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/gaspr/mod.rs#@@L:crates/njoy-outram-park-fork/src/gaspr/mod.rs:fn=from_reconr_and_tape@@)) |
| 1 | `GasProduction::from_reconr`, `acer::energy::mf6::parse_mf6_product_yields`, `regrid`, `eval_tab1` | the MT-keyed sum; MT=5's energy-dependent MF=6 yields | kopitiam |
| 2 | `gas_channel`, `gas_yield_for` | per-MT particle yields, the `LR` breakup table; ⁸Be counts as two α | filled by hand |


## The checks

**HEATR against NJOY2016** (record
[`heatr_vs_njoy2016.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/verification_and_validation/heatr_vs_njoy2016.md),
2026-09-17, Fe-58 and Si-28 ENDF/B-VIII.0, 0 K). Below the first inelastic
threshold only elastic and capture are open, so the difference between the
kinematic limit and NJOY's energy balance must be `σ_cap(E) × (Q − ΣE_γ)`, a
constant deficit per capture fixed by the evaluation:

| nuclide | deficit per capture implied | ours / NJOY, 1e-5 to 1e4 eV |
|---|---|---|
| Si-28 | **0** (8.473899e6 vs `QI` 8.473900e6) | 1.000000 – 1.002507 |
| Fe-58 | **205–209 keV** of a 6.58 MeV `Q` (3.18 %) | 1.0318 – 1.0328 |

Si-28's photons balance its Q, and the codes agree to 1e-6 at 1e-5 eV. Fe-58's
capture photons fall ~208 keV short of its own Q, and the +3.27 % is constant
to four figures over eight decades: **the evaluation's** energy-balance
deficiency, exposed, not a code defect.

Damage energy where scattering is isotropic: mean +0.16 % (Fe-58) and +0.31 %
(Si-28). **The cost of the missing MF=4 anisotropy**, measured: up to +28.5 %
(Fe-58, 900 keV) and +68.3 % (Si-28, 580 keV). Above ~1 MeV the KERMA of the
two methods differs by −75 % to +73 %, which this comparison cannot separate.
~~**Open:** NJOY's MT=445 is non-zero on Fe-58 at 562.5 eV, below the kinematic
threshold implied by `E_d = 40 eV`; `disbar` has not been read.~~
**Diagnosed 2026-10-05:** NJOY's MT=445 is non-zero on Fe-58 at 562.5 eV,
below the kinematic threshold implied by `E_d = 40 eV` (594.5 eV), because
`disbar` runs its recoil integral only at nodes 10 % apart in energy and
interpolates linearly between them. NJOY's two sub-threshold points lie on the
chord from a zero node at 550 eV (`500 × 1.1`; the chord crosses zero at
549.97 eV) to the next node at 605 eV. So the smeared threshold is NJOY's
interpolation, and this port, which integrates at every energy, has the sharp
kinematic one.

**GASPR against NJOY2016** (record
[`gaspr_light_nuclides_vs_njoy2016.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/verification_and_validation/gaspr_light_nuclides_vs_njoy2016.md),
2026-09-17, Li-6, Be-9, B-10, H-2, C-12): worst over all 17 sections
~~4.9e-3~~, interpolation between two different grids at the time.
**Re-measured 2026-10-04: worst 4.6e-7**, NJOY's printed precision, now that
RECONR's grid is NJOY's (#340); see [the re-measurement log](./remeasured.md).
Three defects were found on the
way, the largest that **the residual nucleus was not credited at all**:
⁶Li(n,t)⁴He produced a triton and no alpha; ²H(n,γ)³H produced no tritium.

Both are **verification** against NJOY2016 on the same evaluation, two and five
nuclides, at 0 K, with no human review yet. No actinide comparison is committed
(U-238's HEATR tape is 144 MB). The deferred work is
[#535](https://github.com/theodoreOnzGit/outram-park-backend/issues/535).

## Predict

Every quantity so far is **pointwise**: a value at each energy. A
deterministic reactor code (diffusion, discrete ordinates, a lattice code) has
no room for a million points; it solves for, say, 33 or 172 energy groups.
**To make one number for a group that contains fifty U-238 resonances, would
you average σ(E) evenly over energy? Weighted by what?**

**Next:** [Rung 7 — GROUPR and GAMINR: multigroup constants](./groupr.md).

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-njoy2016" style="padding-left: 2em; text-indent: -2em;">MacFarlane, R. E., Muir, D. W., Boicourt, R. M., Kahler, A. C., &#38; Conlin, J. L. (2017). <i>The NJOY Nuclear Data Processing System, Version 2016</i> (Technical Report LA-UR-17-20093). Los Alamos National Laboratory. https://www.osti.gov/biblio/1338791</p>

<!-- references:end -->
