# THERMR — thermal scattering cross sections from S(α,β)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §THERMR); upstream Fortran: `thermr.f90` (~3.4k lines).

## Theory

At thermal energies (below ~4 eV) the target atom is bound in a molecule or
crystal, so free-gas broadening (BROADR) is wrong. Scattering is governed by the
**thermal scattering law** S(α, β) — the dynamic structure factor in the
dimensionless momentum transfer α and energy transfer β. THERMR turns MF=7 S(α,β)
into pointwise cross sections and secondary distributions:

- **Coherent elastic** — Bragg diffraction from crystalline planes; a sawtooth
  σ(E) with edges at the Bragg energies, `σ(E) = (1/E)·Σ_{E_i<E} f_i`.
- **Incoherent elastic** — bound-atom elastic with a Debye–Waller form,
  `σ(E,T) = (σ_b/2)·(1 − e^{−4EW'})/(2EW')`, plus its angular law.
- **Incoherent inelastic** — the bound double-differential kernel
  `d²σ/dΩdE' ∝ (σ_b/4π)·√(E'/E)·S(α,β)`, integrated for σ(E→E') and σ_inel(E).

## How the port implements it

**Ported** in [`crate::thermr`]: `mf7` (MT=2 coherent/incoherent elastic, MT=4
incoherent inelastic S(α,β) parsing, with per-temperature table selection and
the effective-temperature table), `coherent` (σ_coh + Bragg reflection
cosines/weights), `incoherent_elastic` (closed-form σ + equiprobable cosines via
analytic CDF inversion), `inelastic` (double-differential kernel **incl. the
short-collision-time (SCT) tail beyond the tabulated (α,β) grid**, σ(E→E'), and
the `nieb×nang` equiprobable emission table for the ACE ITXE block). The
consumer surface for Monte Carlo is `scattering`
(`IncoherentInelasticScattering`); the ACE `…t` table writer is
[`crate::acer::thermal`] (`AceTable::thermal_from_mf7`).

`temperature_thinning` is a **study tool, not a production path**: it measures
what dropping tabulated temperatures from an evaluation would cost, by
withholding a tabulated temperature, interpolating to it from the ones kept
(through the same kernel and `LI` law the production reader uses), and
comparing against the evaluation's own values. It also does leave-one-out
characterisation of the *existing* interpolation and an ENDF-byte model for the
MT=4 section. Nothing in it changes how a requested temperature is resolved.

## Testing

**Ported and verified** — Al-27 (σ_b≈1.45 b; σ_inel rises to σ_free≈1.35 b near
1–2 eV) and H-in-ZrH; ACE round-trip in `tests/thermal_ace.rs` and
`tests/thermal_ace_zrh.rs`. See `docs/porting-plan.md` §4f.

**H-in-H₂O (ENDF/B-VIII.0, 293.6 K)** — `tests/thermal_h2o_sab.rs` validates the
incoherent-inelastic path with four analytic/limiting checks (2026-07-15):
free-atom high-E limit σ_inel(8 eV)=20.707 b/H vs σ_free=20.436 b (+1.33 %);
thermal σ 104.2 b/molecule vs literature ~103 b; detailed balance of d²σ
machine-exact (rel 1.7e-16); T_eff(293.6 K)=1194 K. The 17.4 MB `tsl-HinH2O`
file is not checked in, so the test reads `$HINH2O_TSL` (or a default path) and
skips when absent. See `docs/ai-fleet-review/op-cjw-thermr-h2o/REVIEW_MANIFEST.md`.

**Temperature-grid thinning cost (graphite, ENDF/B-VIII.0, 2026-08-13)** —
`tests/thermal_temperature_thinning.rs` (8 tests, 0.49 s) with the full report
in `examples/temperature_thinning_study.rs` and the durable record in
`verification_and_validation/thermal_temperature_grid_thinning.md`. Headline:
MF=7/MT=4 is 99.59 % of the 8.7 MB tape, so thinning its ten temperatures is
where the bytes are. Keeping 296/600/1200/2000 K costs **1.6–3.1 %** in
`σ_total(0.0253 eV)` across 293–1000 K and saves 54.3 % of the tape;
296/400/500/600/1000/2000 K costs **nothing** below 600 K (it keeps every point
HTR-10 operates between) and still saves 36.2 %. Every candidate grid meets a
5 % criterion over 293–1000 K; **none meets 1 %**. The error is concentrated at
high incident energy and high-`Q` Bragg edges, outside the thermal window.

## Caveats

- **`σ_inel` at an interpolated temperature leaves its tabulated bracket above
  ~0.5 eV** — 4.4175 b at 393.15 K / 3.9 eV against a bracket of
  [4.6097, 4.6367] (measured 2026-08-13, MAT 30; ~4–5 % low at 3.9 eV, ≲ 1 %
  low at 0.5 eV, correct at and below 0.1 eV). `σ_inel(E,T)` is monotone in `T`
  across the tabulated grid, so this is a defect in the `LI = 4` `S(α,β)`
  temperature interpolation, not an approximation error. It affects every
  non-tabulated temperature request. Found by the thinning study; **not fixed**
  — see that study's V&V record.
- Only the **IFENG=0** (equiprobable) inelastic form is emitted — the
  skewed/continuous **IFENG=1/2** forms are not ported.
- Multi-scatterer mixing is taken as `nmix = 1`.
- Generating S(α,β) when an evaluation lacks it is **LEAPR**'s job (see
  `../leapr/README.md`), not THERMR's — unported, but ENDF/B ships MF=7 directly.
- The `run()` driver returns `NotPorted`; use `crate::thermr`.

## References

- NJOY2016 manual §THERMR (LA-UR-17-20093)
- `thermr.f90` (NJOY2016 2016.79)
- ENDF-102, File 7 thermal scattering format
