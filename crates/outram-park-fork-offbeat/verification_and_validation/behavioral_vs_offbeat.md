# Densification, relocation and swelling vs OFFBEAT 80e84450 (gh:#1393, round 2, case 1)

Measured 2026-11-13.  Predictions: `offbeat_differential_gh1393/PREDICTIONS_round2.md` section B
(committed before the driver was compiled).  Route and caveats as in `conductivity_vs_offbeat.md`:
standalone C++ transcription of the `correct()` bodies (`dump_behavioral.cpp`), fixture
`reference_behavioral.txt` (3 147 rows), test `tests/offbeat_differential_behavioral.rs`.
**Not a run of the compiled OFFBEAT library.**  Upstream archive sha256
`7b5cde47ceba60903850f02e66b4eb6591cebfc7d272539b8189d625934b1e93`.

`check_verbatim.sh` lists 18 lines not found; all are renames of `T[cellI]`, `Bu_->internalField()[cellI]`
and similar field look-ups by locals (`Ti`, `Bu`), loop/step bookkeeping lines, or one multi-line
expression split across two lines of the driver; the arithmetic is unchanged.

## What is compared, and what cannot be

Upstream densification and swelling are explicit incremental updates against `oldTime()` fields
(plus `min`/`max` against the old value); the port's are stateless.  Compared where the two
coincide (old fields zero, `F = 1`, `delta = 0`), or as a derived-bound convergence test:

| family | form of the comparison |
|---|---|
| densification Empirical, FRAPCON; relocation FRAPCON (modified and legacy); swelling Constant, FrCrAl, FRAPCON, AIM1 / Generalized 15-15Ti, Wright-Sham, correlation PyC; BISON-Zy growth with one step | single step, gates G1/G2 |
| swelling MATPRO UO2 | N explicit steps (N = 1..4096) at fixed T, density vs the port's analytic integral; derived bound |
| swelling FBR-MOX | N explicit steps vs the port's closed form; derived straddle bound |
| BISON-Zy growth, N = 2..64 steps | path dependence vs a second-order prediction |

Not compared (reasons in PREDICTIONS_round2.md B): swelling PARFUME buffer / PyC (not ported; need flux,
timestep and interpolation tables); `swellingGrowthMatproZy` (checked 2026-11-13: absent from
upstream `Make/files`, so not compiled; the port doc's claim is correct); relocation recovery
(`recoveryFraction` default 0); the `min(.., old)` / `max(old, ..)` histories;
`intra/intergranularGasSwelling` additions.

## Results

G1: `|port - up| <= 8 eps * scale`; G2: relative < 1e-14 where `|up| >= 0.2 scale` (derivations in
PREDICTIONS_round2.md).

| model | compared | skipped (outside port window) | bit-identical | worst relative (all rows) | worst G1 ratio (1.0 = limit) | G1 / G2 failures |
|---|---:|---:|---:|---:|---:|---|
| DensEmpirical | 96 | 0 | 56 | 2.1e-16 | 0.096 | 0 / 0 |
| DensFrapcon | 1 440 | 0 | 778 | 3.3e-1 (near-zero result, see below) | 0.156 | 0 / 0 |
| Reloc (modified + legacy) | 768 | 0 | 492 | 2.3e-16 | 0.129 | 0 / 0 |
| SwConstant | 10 | 0 | 10 | 0 | 0 | 0 / 0 |
| SwFrCrAl | 8 | 2 | 8 | 0 | 0 | 0 / 0 |
| SwFrapcon | 42 | 0 | 39 | 7.2e-15 | 0.061 | 0 / 0 |
| SwAim1 | 30 | 5 | 8 | 4.1e-15 | 0.121 | 0 / 0 |
| SwGen1515 | 30 | 5 | 6 | 9.9e-16 | 0.107 | 0 / 0 |
| SwWrightSham (after the fix below) | 18 | 17 | 7 | 3.9e-16 | 0.061 | 0 / 0 |
| SwPyC radial / tangential | 56 | 0 | 52 | 5.5e-16 | | 0 / 0 |
| SwZy N = 1 (6 clad types) | 48 | 12 | | | | 0 / 0 |

`DensFrapcon` worst relative 3.3e-1: at near-zero burnup the converged result is the difference of terms of
size 1e-3 that cancel to ~1e-12; the absolute difference is 0.156 of the G1 limit.  Reported, not gated by G2.

Multi-step cases:

* **MATPRO UO2 stepping**: 40 (T, porosity, burnup) cases x 7 step counts; error of the explicit sum
  against the port integral stays within the derived bound `|d| F / N + (N+8) eps` (worst error / bound
  0.487) and decays as N^-0.998 .. N^-0.98 (worst `|slope + 1|` 0.0215 over N = 64..1024).  The
  predicted first-order convergence is observed.
* **FBR-MOX stepping**: 40 cases, 32 agree to rounding only, bound violations 0, worst d / bound 0.60.
* **BISON-Zy path dependence**: 96 transverse rows with N = 2..64 and total growth S <= 0.02: observed
  deviation from the closed form `0.75 S (1 - 1/N)` within 2.8 % of the prediction (gate 20 %); axial
  growth telescopes to 1.8e-16.  This is an upstream property (the increment transform is non-linear),
  not a port defect: the port's cumulative form equals upstream with one step.

FRAPCON densification fixed-point: upstream converged in exactly 2 iterations on all 1 440 grid points
(prediction: <= 10).  The `abs(B-B0)` overload assumption is still unverified (see PREDICTIONS B3).

## PORT DEFECT found and fixed: Wright-Sham Hastelloy N swelling

`swellingWrightShamHastelloyN::correct` adds `0.01/3 * (tfact*fluxfac(new) - tfact*fluxfac(old))`
with `fluxfac = par1 dpa^par2 + par3`; `par3 = -0.981` cancels in the difference, so the swelling
accumulated from zero fluence is `0.01 tfact par1 dpa^par2`, zero at zero dose and never negative.  The port kept
the offset in its cumulative form (`strain` returned `0.01 tfact (par1 dpa^par2 + par3)`) and documented
the resulting negative values below ~1 dpa as "upstream's behaviour, reproduced faithfully".  That doc
claim was false: no OFFBEAT history produces it.  All 18 compared rows failed G1 (relative differences up
to 1.35) before the fix.  Fixed in `src/materials/behavioral/swelling.rs` by dropping the offset; docs
corrected in place with strike-through; the unit test that pinned the old negative branch was replaced by
`wright_sham_is_zero_at_zero_dose_and_never_negative`.  Moved numbers (volumetric strain at the 490 C
peak, old -> new): 1e25 n/m^2 -2.545e-3 -> +7.265e-3; 1.98e25 -8.29e-6 -> +9.802e-3; 1.99e25 +1.338e-5 ->
+9.823e-3; 1e26 +1.013e-2 -> +1.994e-2; 4e26 +2.681e-2 -> +3.662e-2.  No caller outside the crate uses
this variant (searched the workspace).  Prediction B8 ("no port defect") was wrong.

## Localisation (`localise_beh.cpp`, `localise_beh_output.txt`)

* Reloc: `deltaGap*(Gap/D)/100` (upstream) vs `(deltaGap/100)*(Gap/D)` (port): differs on exactly 276 of
  768 rows = the 276 non-bit-identical rows in the Rust test.  Fully localised.
* SwFrapcon: `Bu/0.881` vs `(Bu/1000/0.881)*1000`: 3 rows differ, = the 3 non-bit-identical rows.  Fully localised.
* DensEmpirical: `(asym/3)*(1-exp)` vs `(asym*(1-exp))/3`: 28 of 96 rows; the Rust test has 40
  non-bit-identical rows, so 12 are not explained by operation order; attributed (not isolated) to
  PETIR `r_exp` against glibc `exp`.
* SwAim1 / SwGen1515 / DensFrapcon: operation-order causes found (`f*1e-22` vs `(f*1e4)/1e26` base
  rounding on 7 of 28 AIM1 grid points; `exp(par2*pow(x,2))` vs `exp(b*x*x)`; the resintering constant
  `10960/100` vs `109.6`) but the counts were not reproduced against the Rust totals (the localiser grid
  differs), so these are *consistent with* rather than *proved to be* the cause.

## Prediction scorecard

1 (G1/G2) held after the fix.  2 (bit-identity < 100 % for Empirical): held (56/96).  3 (<= 10 iterations): held (2).
4 (resintering route 1-2 ulp): consistent, not separately isolated.  5 (first-order convergence, bound): held.
6 (FBR straddle bound): held.  7 (BISON-Zy path dependence within 20 %): held (2.8 %).  8 (no port defect): **wrong**
(Wright-Sham).

Reproduce:

```sh
cd crates/outram-park-fork-offbeat/verification_and_validation/offbeat_differential_gh1393
g++ -O2 -ffp-contract=off -o dump_behavioral dump_behavioral.cpp && ./dump_behavioral > reference_behavioral.txt
cargo test --release -p outram-park-fork-offbeat --test offbeat_differential_behavioral -- --nocapture
```

Data provenance: OFFBEAT sources (GPL-3.0, gitlab.com/foam-for-nuclear/offbeat, commit
80e84450a115b0c411e1bfa5d166379f6bf6c084), accessed 2026-11-13; no other data.
