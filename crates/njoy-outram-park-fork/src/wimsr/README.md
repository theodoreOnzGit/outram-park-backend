# WIMSR — WIMS reactor-physics libraries

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §WIMSR); upstream Fortran: `wimsr.f90` (2 150 lines,
> commit `ac5adf5`).

## Theory

WIMSR builds libraries for **WIMS** ("Winfrith Improved Multigroup Scheme"), a
widely used lattice-physics code (WIMS-D is freely distributed; WIMS-E was the
commercial 1990-era version) developed at AEE/Winfrith. WIMS uses
**collision-probability** methods to compute fluxes in reactor pin cells and more
complex geometries.

WIMSR reformats GROUPR **GENDF** data into the WIMS library layout: multigroup
cross sections, scattering matrices, fission data, and — importantly for thermal
lattices — **resonance integrals** tabulated for self-shielding, including the
intermediate-resonance treatment WIMS relies on in the near-epithermal range.

## How the port implements it

| Upstream (`wimsr.f90`) | Port | Notes |
|---|---|---|
| `wimsr` driver (l.51-244) | `run_gendf(tape, &WimsrInput)` | the card deck is **not** read — `run()` in the module table stays `NotPorted`; callers fill a [`WimsrInput`](input.rs) (cards 2–8 as fields, `nfid()`, `ifprod_eff()`, `burn_table()`) |
| `wminit` (l.246-421) | `gendf::read_material` | one `TempBlock` per temperature (MF=1/451 header, `sigz`, `egn`), the reaction sections on the DTFR reader's `GendfGroupRecord` layout, `egb` in WIMS (descending) order |
| `xsecs` + `xseco` (l.871-1520) | `xsecs::xsecs` | per temperature block: transport-corrected total, absorption, ν σ_f, σ_f, potential (`MT=2` at the reference σ₀ ± `sigp`), slowing-down power, the P0 matrix with its `l1/l2` bounds, the fission spectrum (`isof`) from the matrix or `MT=452`, `p1flx` and the P1-flux normalisation; the listing quantities (`TempIndependent`, `TempDependent`) |
| `resint` + `rsiout` (l.423-869) | `resint::resint` | per resonance group, temperature and σ₀ (stored ascending): `RI = σ_b σ_a /(σ_b + σ_a)`, `σ_b = σ₀ + λ σ_pot`; ν-fission RIs when fission is present; the listing's flux per unit lethargy |
| `p1scat` + `p1sout` (l.1680-1969) | `p1scat::p1scat` | `ip1opt = 0`: the temperature-independent MF=6 P1 matrices once, elastic above the thermal boundary + `mti`/`mtc` per temperature, upscatter suppression into the first thermal group, `(ig-l1+1, nb, values)` records |
| `wimout` (l.1971-2150) | `wimout::wimout` | the WIMS-D (`iverw = 4`) / WIMS-E text with the Fortran edits `(1p,5e15.8)`, `(i15)`, `(2i15)`, `(3(1p,e15.8,i6))`, `(i6,1p,e15.8,5i6)`, `(1p,e15.8,2i6)` and the `      999999999` separators; `e15_8` reproduces `1PE15.8` exactly |

Indexing: WIMS group `jg = ngnd - ig + 1`; a GENDF word `scr(l+lz+off)`
is `data[off]` of the record. Upstream idioms are kept where they change
output (the thermal boundary `nth` found from the first `mti` pass, the
`l2t .ne. l1t` guard on the P1 bounds, a missing `MF=3/102` at a later
temperature inheriting the previous one's absorption).

**Not ported:** the card-deck reader; `jp1 > 0` user current spectra are
carried in `WimsrInput::p1flx_user` but untested; WIMS-E (`iverw = 5`)
output, `ntemp > 1`, `mtc ≠ 0`, `ifprod ≠ 0`, `iburn > 0` burnup tables and
`inorf > 0` are translated from the same lines but have no oracle.

## Testing

`tests/wimsr_u238_njoy_golden.rs` against `reference-data/wimsr/`
(ENDF/B-VIII.0 U-238, NJOY2016 `ac5adf5`; RECONR 0.001 → BROADR 293.6 K →
THERMR free-gas `MT=221` → GROUPR 29 groups, `iwt=3`, `lord=1`, six
sigma-zeros → WIMSR `iverw=4`, `29 15 8 15`, `ires=1 mti=221 ip1opt=0
isof=1`, eight unit Goldstein lambdas), 2026-09-11:

- **Library:** byte-identical to NJOY's `tape25`, 185/185 lines, on the
  first run (the prediction: every quantity is a sum or ratio of GENDF
  words printed with 9 figures).
- **Listing stages (`iprint = 2`):** potential / slowing-down power
  1.8e-5, transport-corrected total 3.6e-5, absorption 2.4e-5, current
  spectrum 3.0e-5, thermal block 2.3e-5, resonance-integral tables 2.8e-6,
  flux per unit lethargy 4.6e-6, P1 rows 1.6e-5, fission spectrum 1.5e-5 —
  all at the printed precision.
- One finding: the listing's "neutron current spectrum" first came out
  normalised to 1 at `igref` (`p1nrm = 1/p1flx(igref)`) while NJOY prints
  0.99561 there — `xseco` declares `p1nrm` as an **integer**
  (`wimsr.f90:1435`), so the division truncates and the raw `p1flx` is
  printed. Mirrored (the library is unaffected).
- Unit test: the `1PE15.8` edit on the oracle's values.

V&V record: `verification_and_validation/wimsr_u238_vs_njoy2016.md`.

## Caveats

- **Lowest priority (Phase 6)** — WIMS is not an OUTRAM PARK target.
- One material, one temperature, WIMS-D only; the other branches are
  translation-level (see "Not ported").
- The resonance-integral/intermediate-resonance data ties WIMSR to RESXSR;
  treat them together if extended.

## References

- NJOY2016 manual §WIMSR (LA-UR-17-20093)
- `wimsr.f90` (NJOY2016 2016.79); WIMS-D/WIMS-E (AEE/Winfrith)
