# GASPR — gas-production cross sections

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §GASPR); upstream Fortran: `gaspr.f90` (~1.15k lines).

## Theory

The light products of nuclear reactions — protons, deuterons, tritons, ³He, and
alphas — accumulate as gases in structural and cladding materials and drive
swelling/embrittlement. GASPR forms the **total gas-production** cross sections:

| MT | Species |
|---|---|
| 203 | total proton (H-1) production |
| 204 | total deuteron production |
| 205 | total triton production |
| 206 | total ³He production |
| 207 | total alpha production |

Each is a **yield-weighted sum** over every reaction that emits that particle:
`σ_MT2xx(E) = Σ_r y_{r,species} · σ_r(E)`, where `y` is the multiplicity of the
species in reaction *r*. Existing MT=203–207 sections are removed first, then
recomputed and added to the directory. GASPR is usually run after BROADR.

## How the port implements it

**Ported** in [`crate::gaspr`]: MT=203–207 are computed as a yield-weighted sum
over the reconstructed MF=3 sections that survive NJOY's skip list
(`gaspr.f90:471-491`). The per-event yield has three parts, all of them
needed:

1. **The ejectiles the MT names** — `gas_channel(mt, lr)`, transcribed from
   `gaspr.f90:501-820`, covering MT=5/11/16/17/22–45/51–91/102–200.
2. **`LR` breakup on the inelastic levels** — for MT=51–91 the ejectile set
   depends on the MF=3 `LR` flag, not on the MT (`gaspr.f90:565-608`). This is
   common where gas production matters: in ENDF/B-VIII.0, Li-6 carries `LR=32`
   on 30 levels, Li-7 `LR=33` on 31, B-10 a mix of `LR=22/28/35`, C-12 `LR=23`.
3. **The residual nucleus** — `gaspr.f90:821-826`. With
   `izr = ZA_target + 1 − Σ ZA_ejectile`, a residual of 1001/1002/1003/2003/2004
   adds one of that gas and 4008 (⁸Be, particle-unbound) adds **two** alphas.

Part 3 was missing until 2026-09-17, and it is not a corner case: ⁶Li(n,t)
lost its alpha, ⁹Be(n,2n) produced no helium at all, and ²H(n,γ)³H produced no
tritium. A lookup keyed on MT alone **cannot** express it, because the residual
depends on the target.

MT=5's energy-dependent MF=6 multiplicities (`gaspr.f90:100-240`) need the
evaluation as well as the reconstruction, so they live on the second entry
point `GasProduction::from_reconr_and_tape`.

## Testing

**Ported and verified against NJOY2016 itself.**

- **Cross-code:** `tests/gaspr_vs_njoy2016.rs` — Li-6, Be-9, B-10, H-2 and C-12
  against PENDF tapes produced by NJOY2016 `ac5adf5f` built from source and
  executed, committed with their decks under `reference-data/gaspr/`. All 17
  sections both codes produce agree, worst **4.9e-3** relative, typical 1e-4 to
  8e-4; every threshold pair within 1 %. Write-up:
  `verification_and_validation/gaspr_light_nuclides_vs_njoy2016.md`.
- **Unit:** 14 tests (`--lib gaspr`) — additivity, multi-particle yields,
  two-species channels, non-gas exclusion, the upstream skip list, `LR`
  breakup, each residual rule on a named textbook reaction (⁶Li(n,t)⁴He,
  ³He(n,p)³H, ⁹Be(n,2n)→2α, ²H(n,γ)³H) with ¹⁰B(n,α)⁷Li as the negative
  control, and a nucleon/charge-conservation sweep over the whole 90-entry
  ejectile table.

## Caveats

- The legacy **MT=600–849** detailed-breakup fallback (pre-ENDF/B-VI style) is
  **not ported** — rare in ENDF/B-VII/VIII, which use the lumped channels. Note
  this is GASPR's *input* fallback; RECONR's synthesis of the lumped MT=103–107
  from those levels is separate and **is** now done
  (`reconr::synthesise_lumped_particle_channels`, added the same day after this
  comparison found B-10's MT=105 missing).
- `parse_mf6_product_yields` does not disambiguate two subsections sharing a
  `ZAP` via `LIP`/`JP`; only the first would be read. No evaluation in
  `reference-data/endf/` does that on MT=5.
- The `run()` driver returns `NotPorted`; use `crate::gaspr`.
- **No human V&V.** Cross-code agreement with NJOY on the same evaluation is
  not agreement with measurement.

## References

- NJOY2016 manual §GASPR (LA-UR-17-20093)
- `gaspr.f90` (NJOY2016 2016.79)
