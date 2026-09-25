# The worth of URR + DBRC on the homogenised LCT-008 — a BOUND, not a measurement

GitHub **#307**. Run 2026-09-24/25 by
`examples/lct008_urr_dbrc_ablation.rs`.

## The question

`examples/lct008_ace_roundtrip.rs` reports the ENDF and ACE data routes
agreeing on `k` at **0.78 sigma**:

```text
ENDF route : k_eff = 0.84980 +/- 0.00232
ACE  route : k_eff = 0.85250 +/- 0.00258
difference : +269.3 pcm  (combined sigma 346.6 pcm, 0.78 sigma)  -> AGREE
```

`Nuclide::from_ace` sets `urr: None` and `dbrc: None`; `from_endf_file`
applies both by default. So the comparison is not a pure format check — the
arms differ in physics too. This ablation removes URR and DBRC **on the ENDF
arm** to see how much of the +269.3 pcm that accounts for.

## Methodology

Same five nuclides, densities, volume fractions, radius and settings as the
round-trip example: homogenised sphere `r = 40 cm`, 30 % fuel / 70 % borated
water, 3000 histories x [30 inactive + 120 active], 293.6 K, ENDF/B-VIII.0.
Twelve seeds, both arms per seed, one thing changed.

**The ablation control ran first**, because an ablation that removes nothing
measures nothing and this workspace has shipped one before:

```text
U235  full: urr=true  dbrc=true    ablated: urr=false dbrc=false
U238  full: urr=true  dbrc=true    ablated: urr=false dbrc=false
O16   full: urr=false dbrc=true    ablated: urr=false dbrc=false
H1    full: urr=false dbrc=true    ablated: urr=false dbrc=false
B10   full: urr=false dbrc=true    ablated: urr=false dbrc=false
ablation is real on: ["U235", "U238", "O16", "H1", "B10"]
```

URR is removed from the two uranium nuclides — correctly absent on the light
ones, which have no unresolved range — and DBRC from all five.

## Prediction, recorded before the run

- DBRC raises U-238 capture, so removing it raises `k` (−200 to −400 pcm worth).
- URR self-shields the unresolved range, so removing it lowers `k` (+50 to
  +200 pcm worth).
- They oppose, DBRC normally larger, so the net ablation was predicted at
  **+100 to +300 pcm**, closing most of the observed gap.

## Results, 2026-09-25

| quantity | value |
|---|---|
| `k` with URR+DBRC | 0.84937 (seed-to-seed sd 0.00181) |
| `k` ablated | 0.85001 (seed-to-seed sd 0.00222) |
| **ablation worth** | **+63.5 pcm, sem 77.2, sd 267.5 — 0.8 sigma** |

Per-seed differences (pcm): `+135.7, +167.5, −509.4, +223.0, +214.5, +142.0,
+260.2, +102.6, −307.0, −201.5, +96.8, +437.2`.

**NOT RESOLVED at 2 sigma.** The honest statement is a bound:

> **|worth of URR + DBRC on this case| < 154 pcm at 2 sigma.**

Do **not** quote `+63.5 pcm` as the worth, and do not quote the program's
"explains 24 %" line — a gap cannot be apportioned with an unresolved
numerator.

## The prediction is NOT supported

`+100 to +300 pcm` predicted; `+63.5 ± 77` measured. The **sign** is positive
as predicted, but the magnitude falls below the range and the result does not
exclude zero. Recorded as a miss rather than reinterpreted after the fact.

## A framing error worth more than the prediction miss

**The +269.3 pcm gap this study set out to explain was itself only 0.78 sigma
— never established to exist.** Hunting a mechanism for it was chasing a
difference statistically indistinguishable from zero.

The bound above and the gap's own non-significance are jointly consistent with
the simplest reading: on this homogenised model there is **no demonstrated
route difference and no demonstrated URR+DBRC worth**. That is a coherent
null, not two failures.

## Two methodological findings

1. **Pairing bought nothing.** sd on the difference is 267.5 pcm against
   per-arm sds of 181 and 222 — i.e. `sqrt(2) x ~200`, exactly the uncorrelated
   result. URR and DBRC change how many random draws each history consumes, so
   the two arms' RNG streams diverge from the first URR draw onward and the
   pairing does not correlate them. **Size this as independent arms.** (The
   `future_seed` substream technique used elsewhere in this crate is what would
   restore pairing, by giving URR its own stream.)
2. **Resolving `+63.5` at 3 sigma needs ~162 seeds** (`sem <= 21 pcm`), about
   3.5 h post-merge at the `Sigma_t` fast path's 3.2x transport speed.

## Recommendation: do NOT spend that on this geometry

This model **homogenises** the lumped fuel, which is exactly what destroys the
resonance self-shielding that makes URR and DBRC matter — a 1.030 cm pellet is
176 mean free paths across at the 6.674 eV resonance, giving resonance escape
≈ 0.75 in the real benchmark. Measuring ~60 pcm precisely here would be
measuring the wrong thing well.

**The number that matters belongs on the lumped case** (`lct008_keff.rs`),
where the self-shielding is present. A small worth here says nothing about the
worth there, and the reverse is the claim anyone would actually want.

## What this does and does not settle for #307

- **Settled:** the ACE route's missing URR+DBRC is not shown to bias `k` on
  this homogenised case at the 154 pcm level.
- **Not settled, and still the substance of #307:** the two routes carry
  different physics, which remains true regardless of whether it is detectable
  here. `tests/correct_physics_is_default.rs` still covers only the ENDF path.
- **Not settled:** what the +269.3 pcm gap is. It may be nothing; nobody has
  shown it is something.
