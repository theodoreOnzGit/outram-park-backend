# PANAMA-I — units register and open questions

Reference: Verfondern, K. & Nabielek, H., *The Mathematical Basis of the
PANAMA-I Code for Modeling Pressure Vessel Failure of TRISO Coated Particles
under Accident Conditions*, Forschungszentrum Jülich, HTA-IB-03/90,
1 August 1990. Reprinted as Appendix C, printed pages -479- to -511-.

The report is restricted literature. Only its **equations, constants and
units** are recorded here, with citation, as scientific facts. The document
itself is not in this repository. Implementation is an independent Rust
reconstruction from the published equations; the PANAMA Fortran is
closed-source and was never consulted.

This file exists because the report's own symbol list (page -511-) is
**incomplete or wrong** about several units, in ways that change answers
rather than merely presentation. Each entry records what is printed, what is
used, and *how it was settled* — or that it is still open.

---

## Settled

### `T_B`, `T_m` — irradiation and accident temperature

| | |
|---|---|
| **Printed** | `[°C]` (symbol list, -511-) |
| **Used** | **kelvin** |
| **Settled by** | Table 1 (-500-), reproduced 16/16 |

Every Arrhenius and `10⁴/T` term in the report needs an absolute temperature,
and the conversion is **never stated anywhere in the document**.

Table 1 settles it. Its "after irradiation" columns are *calculated*, not
measured — page -498- states they come from Eqs (8a)/(9a) at `T_B = 1000 °C`
and `Γ = 1·10²⁵ m⁻² EDN`. That makes them sixteen closed-form checks:

| reading | `σ` factor | `m` factor | Table 1 rows reproduced |
|---|---|---|---|
| **kelvin** (1273.15 K) | 0.914206 | 0.875418 | **16 / 16** |
| °C taken literally (1000) | 0.8756 | 0.8104 | 0 / 16 |

This is a discrimination, not a tolerance argument: the two readings are far
apart and only one lands. Pinned by
`strength::tests::table_1_is_reproduced_exactly`, with the negative half in
`the_celsius_reading_of_t_b_is_excluded`.

### `Γ` — fast-neutron fluence

Printed and used in the correlation's own units of **10²⁵ m⁻² EDN**.

Deliberately a bare `f64` rather than a `uom` quantity. `log₁₀Γ_s` and
`log₁₀Γ_m` are fits whose intercepts (0.556, 0.394) are only meaningful in
those units; accepting a dimensioned argument would imply a freedom of unit
choice the correlation does not have. The parameter name carries the unit.

### Eq (3) — the fraction bar's extent

| | |
|---|---|
| **Printed** | the bar appears to span `(V_f/V_k)·R·T/V_m` |
| **Used** | `p = (F_d·F_f + OPF)·F_b·R·T / [(V_f/V_k)·V_m]` |
| **Settled by** | dimensions, and the sign of `∂p/∂T` |

The printed grouping gives `p ∝ 1/(R·T)` — dimensionally wrong, and it would
make the pressure **fall** as the particle heats. Only one grouping is
consistent, and it is the textbook `p = nRT/V_f` with
`n = (F_d·F_f + OPF)·F_b·V_k/V_m`. Pinned by
`pressure::tests::pressure_is_the_ideal_gas_law`, which recomputes it by an
independent route and asserts pressure rises with temperature.

### Eq (1) — `σ_o` is the median, not the characteristic strength

`φ₁ = 1 − exp[−ln2·(σ_t/σ_o)^m]`. The `ln2` makes `φ₁ = 0.5` exactly at
`σ_t = σ_o`. A stock Weibull treats its scale parameter as the
*characteristic* strength, where `φ = 1 − e⁻¹ ≈ 0.632`; substituting one for
the other misplaces the strength scale by `(ln2)^(1/m)`, about 4 % at `m = 8`,
in a direction that flatters the answer and raises no error. Pinned by
`weibull::tests::median_is_the_scale_parameter`.

### `t_B` — irradiation time. **SETTLED: seconds.**

| | |
|---|---|
| **Printed** | `[s]` (symbol list, -511-) |
| **Apparently contradicted by** | Fig. 3's curve labels (`1000 d`); Figs. 7/8's captions (`260 FPD`, `500 FPD`); the validity range on -488- ("66 and 550 full power days") |
| **Settled by** | Fig. 3, reproduced to 0.0087 in `OPF` on seconds and 0.277 on days |

This was the last blocking question, and the most strongly discriminated one
in the report: Eqs (5b)/(5c) carry `2·log t_B`, so seconds-vs-days moves
`log OPF` by about **9.9 decades**.

| reading | mean abs error in `OPF` |
|---|---|
| **seconds** | **0.0087** |
| days | 0.277 — `OPF` collapses to ~0 everywhere |

Four digitised `UO₂` curves, `T_B` 900–1100 °C, `t_B` 500–1000 d.

**The symbol list was right and the three "contradictions" were not.** A curve
*titled* `1000 °C, 1000 d` and a validity range quoted in full-power days are
both human-readable descriptions of the experiment; the *formula* takes
seconds. `oxygen_per_fission_uo2` takes a `uom` `Time` and converts
internally, so a caller cannot reintroduce the confusion.

Fig. 3 digitised by the maintainer, 2026-09-24.

### `f(τ)` — the grouping in the Booth series. **SETTLED: the whole `1 − exp(…)` is the numerator.**

| | |
|---|---|
| **Printed** | the fraction bar spans only `exp(−n²π²τ)/(n⁴π⁴)`, with `(1 −` opening outside it |
| **Used** | `Σ (1 − exp(−n²π²τ))/(n⁴π⁴)` |
| **Settled by** | Fig. 1 (-486-), 78 digitised points, plus two analytic limits |

The literal reading has a summand tending to **1**, so the series diverges: a
1000-term partial sum gives `f(0.1) = −6.0·10⁴` instead of a number in
`[0, 1]`, and doubling the term count doubles the damage.

| reading | `f(0.1)` | `f(0.5)` | `f(1.9)` |
|---|---|---|---|
| literal | −5.99997·10⁴ | −1.19990·10⁴ | −3.1569·10³ |
| **numerator reading** | **0.56365** | **0.86755** | **0.96491** |

Against Fig. 1 the numerator reading gives mean `|Δf|` = **0.0066** over all
78 points (median 0.0020, worst 0.047 at `τ = 0.0313` where the curve is
near-vertical); over the 68 points with `τ ≥ 0.15` it is **0.0028** mean,
0.018 worst, on an ordinate running 0 to 1.

Two analytic limits the figure cannot supply were also checked, and they fix
the `6/τ` normalisation that a plausible-looking curve would not:
`f → 1 − 1/(15τ)` agrees to 4·10⁻¹² at `τ = 10`, and `f → 4√(τ/π) − 3τ/2`
to 2·10⁻⁷ at `τ = 10⁻⁴`.

**A digitisation note.** Fig. 1's y-axis calibration in the maintainer's
digitisation labels the upper gridline `500`. A through-origin fit of the
digitised ordinate against this implementation over `τ ≥ 0.15` gives
**501.29**, i.e. that gridline is `f = 1` to within 0.26 %; `f = y/500` is
used. Fig. 1 digitised by the maintainer, 2026-09-24.

### The 1000-summand cap is the binding cut-off, and it limits accuracy below `τ ≈ 10⁻⁵`

Page -485- gives two stopping rules: 1000 summands ("caution!"), or two
consecutive summands differing by ≤ 10⁻²⁰. The second **never fires first** —
the summand tends to `1/(n⁴π⁴)`, whose consecutive differences reach 10⁻²⁰
only near `n ≈ 5.3·10³`. Measured cost of the cap:

| `τ` | 1000-term sum | error |
|---|---|---|
| 10⁻² … 10¹ | 0.2107 … 0.9933 | ≤ 2·10⁻⁹ |
| 10⁻⁴ | 0.02242 | 3·10⁻⁷ |
| 10⁻⁶ | 0.002276 | ≤ 2·10⁻⁵ |
| 10⁻⁸ | 6.4·10⁻⁴ | ~4·10⁻⁴ — **larger than the answer** |

The report's algorithm is implemented as printed rather than replaced by the
rearranged closed form, which is in any case worse for small `τ` (it cancels
`1/(15τ) ≈ 6.7·10⁶` against itself to produce a number of order 10⁻⁴).

### Eq (12)'s `375` must carry units of m/s

Eq (12) prints `k = (375/d_o)·exp(−556000/(R·T_m))` with `d_o` in metres and
declares `k` in `s⁻¹`. The only reading that balances is **375 \[m/s\]** — the
`k_o` of the page -495- Arrhenius as a decomposition front velocity divided by
the layer it has to eat through. The report never says so.
`decomposition_rate_constant` therefore takes a `uom` `Length` rather than a
bare number. Consequence: `k ∝ 1/d_o`, so a 50 µm layer decomposes 30 % more
slowly than a 35 µm one, and `ζ` scales with it.

### Fig. 7's staging is stated in the text, not inferred

Page -498- states the FRJ2-K11/03 calculation of Fig. 7 includes "the
preceeding heating phases of 100 h at 1400 °C and 100 h at 1500 °C", with
1600 °C thereafter to 1000 h. Nothing about the staging was read off the
curve's slope.

### The p-500 "one order of magnitude" sentence is about Fig. 9, not Fig. 6

Page -500- carries, between Table 1 and Fig. 6, the sentence that the
difference in particle failure at 1600 °C "is within the range of one order of
magnitude", with the 2000 °C difference "somewhat greater". Fig. 6's eight
curves in fact span **4.79 decades** at 248 h, which looks like a
contradiction. It is not: the sentence continues the paragraph at the foot of
page -499-, which introduces **Fig. 9** (35 µm against 50 µm SiC at 1600 and
2000 °C) — and Fig. 9's two curves are about one decade apart at 1600 °C and
somewhat more at 2000 °C. Recorded here because the page layout makes the
mis-reading easy, and an earlier draft of this work made it.


### The log y-axis of Figs. 6, 7 and 8 is calibrated over SEVEN decades where SIX are plotted

**Found 2026-09-24, and it resolves one of the two open disagreements.**

All three figures label `10⁰` at their top gridline and `10⁻⁶` at the bottom,
six decades. All three digitisations carry an upper calibration point entered
as **`10`**:

| figure | `y_axis` calibration as recorded |
|---|---|
| Fig. 6 | `px 410.10 = 0.000001 , px 88.03 = 10` |
| Fig. 7 | `px 418.65 = 0.000001 , px 35.40 = 10` |
| Fig. 8 | `px 393.20 = 0.000001 , px 15.04 = 10` |

If the upper point sits on the `10⁰` gridline, every digitised ordinate is
stretched in the log by **7/6**, and the true value is recovered by
`log₁₀φ_true = −6 + (6/7)·(log₁₀φ_reported + 6)`.

**Three independent tests pick out exactly that factor**, two of which use no
model at all:

| test | as digitised | at `k = 6/7` |
|---|---|---|
| Fig. 8: the two PANAMA curves differ only by Eq (10b), so they must invert to **one** `σ_t` | `σ_with/σ_without` = **1.505**, rel s.d. 0.133 | **1.014**, rel s.d. 0.051 |
| Fig. 7: the same identity, over its 1600 °C stage | **1.535** | **1.000** |
| Fig. 6: eight varieties must invert to **one** `σ_t` | rel s.d. **10.0 %**, residual +23 % … −10 % | rel s.d. **1.4 %**, residual ±2 % |

Free-fitting the factor rather than assuming it gives **0.855** (Fig. 8) and
**0.875** (Fig. 7) against `6/7 = 0.857`.

**Two consequences, in opposite directions.**

1. **Eqs (10b)/(10c) are VERIFIED** — the grain-boundary law connects the
   report's own two curves on both Fig. 7 and Fig. 8 once the axis is read
   correctly. This group was recorded above as having no external check at
   all; it now has one. ~~Not verified against any output of the report.~~
   **CORRECTED 2026-09-24.**
2. **Every residual quoted against Figs. 6, 7 and 8 as digitised is an upper
   bound**, roughly 17 % too wide in the log.

**Nothing has been corrected in the data, and nothing should be.** Deflating
the digitised points would erase the evidence that diagnosed this. The fix is
to **re-digitise** with the axis calibrated on the plotted decades (kovan's
parallelogram calibration and reference grid). Until then both readings are
reported side by side, and the tests assert both.

A related slip, same shape, already recorded above: Fig. 1's upper calibration
point is entered as `500` where the gridline is `f = 1` (the fitted scale came
out 501.29). The pattern is an upper calibration point placed on the top
gridline and given the *next* round value.

**What it does NOT explain.** The Fig. 7/8 late-time drift survives:
Fig. 7's factor falls from 1.96 to 1.70, Fig. 8's whole-run relative s.d. from
19.0 % to 14.3 %. See the entry below.


### Table 2 (-504-) is a second closed-form check, at a different fluence and `T_B` — 6/6

Table 2 lists, for the report's own HTR-Module and HTR-500 cases, an average
irradiation temperature and duration **and** the `OPF`, `σ_o` and `m_o` it
computed from them. That makes six closed checks at **reactor** conditions,
independent of Fig. 3's heating curves and of Table 1 (which is all at
`Γ = 1`, 1000 °C).

| case | `T_B` | `t_B` | quantity | Table 2 | this crate |
|---|---|---|---|---|---|
| HTR-Module | 776 °C | 1020 FPD | `OPF(t=0)`, Eq (5b) | 0.00511 | **0.005110** |
| HTR-500 | 792 °C | 700 FPD | `OPF(t=0)`, Eq (5b) | 0.00316 | **0.003185** (+0.8 %) |
| HTR-Module | 776 °C | `Γ = 1.4` | `σ_o`, Eq (8a) | 756 | **756.1** |
| HTR-500 | 792 °C | `Γ = 1.4` | `σ_o`, Eq (8a) | 754 | **754.4** |
| HTR-Module | 776 °C | `Γ = 1.4` | `m_o`, Eq (9a) | 6.93 | **6.932** |
| HTR-500 | 792 °C | `Γ = 1.4` | `m_o`, Eq (9a) | 6.91 | **6.908** |

Two things follow. It is a **third** independent confirmation that `t_B` is in
seconds and `T_B` in kelvin — on days Eq (5b) is ten decades out. And it is
independent evidence that the Eq (8a)/(9a) degradation law is right at a
fluence and temperature Table 1 does not cover, which is consistent with the
Fig. 6 `m`-residual having turned out to be a digitisation artefact rather than
a defect in the law.

(`σ_oo = 834`, `m_oo = 8.02` — EO 1607 as Fig. 5 and Table 2 give it, not
Table 1's 850/8.0.)


---

## Open

### Missing page -510- (§7, References)

Absent from the source PDF — the file has 39 pages and all 39 render; p-34 is
`-509-` and p-35 is `-511-`. Not blocking, but it is how the underlying
measurement papers (Nabielek 1984, Allelein 1983, Benz 1982, Proksch 1982,
Myers 1977, Horsley 1976, Strigl 1984, Montgomery 1981) would be chased for
their own stated units.

### The UO₂ `D_S` equation disagrees with the report's own Fig. 2

Page -487- prints two reduced-diffusion correlations and plots both in Fig. 2
on the same page. The **(Th,U)O₂** one reproduces its curves; the **UO₂** one
does not reproduce its own.

| `10⁴/T` | figure (dashed curve) | Horsley equation | equation / figure |
|---|---|---|---|
| 3.19 | 10⁻⁵·⁶⁹ | 10⁻⁴·⁸⁹ | **6.4×** |
| 5.59 | 10⁻⁷·³⁴ | 10⁻⁶·⁸⁴ | 3.2× |
| 7.21 | 10⁻⁸·⁴⁶ | 10⁻⁸·¹⁶ | 2.0× |
| 8.59 | 10⁻⁹·⁴⁰ | 10⁻⁹·²⁸ | 1.3× |

A blind fit to the plotted curve gives a slope of **−0.6875** against the
equation's **−0.8116**, so this is a slope disagreement and not an offset — it
cannot be reconciled by a decade or a units error. The transcription was
checked directly against the page image: the equation reads
`log DS = −2.30 − 0.8116·10⁴/T`, as implemented.

`diffusion::reduced_diffusion_coefficient` implements **the equation**, on the
grounds that for a code reconstruction the equation is the specification and
the figure is illustrative. The discrepancy is pinned by
`the_uo2_equation_sits_above_the_figures_dashed_curve` so it cannot be
silently tuned away.

By contrast the (Th,U)O₂ correlation is **verified**: each of the four solid
curves recovers its own printed `% FIMA` label from a blind fit (0.0086,
0.0494, 0.0950, 0.1456 against 0.01, 0.05, 0.10, 0.15), with mean residuals of
0.013–0.035 in `log₁₀ D_S`.

Fig. 2 digitised by the maintainer, 2026-09-24.

### The corrosion-rate pre-factor is a decade out — Fig. 4 proves it

Page -492- prints `v̇ = 5.87·10⁻⁷ · exp(−179500/(R·T))` and Fig. 4 on the
facing page plots Eq (7) with it, for `d_o = 35 µm`, at five isothermal
temperatures. The printed value does not reproduce that figure.

| pre-factor | mean abs error in `d_act/d_o` | worst |
|---|---|---|
| `5.87e-7` as printed | 0.12 – 0.51 per curve | 0.51 |
| **`5.87e-8`** | **0.0055** | 0.0142 |

483 digitised points across all five curves, on an axis running 0 to 1. One
factor of ten reconciles five curves over a 1000 °C span, so this is a typo
and not a modelling difference. `corrosion::FIGURE_PREFACTOR` is the default;
`PRINTED_PREFACTOR` is exposed so the discrepancy stays reproducible.

Fitting `v̇` freely from the figure gives an activation energy of
**160.8 kJ/mol** against the printed 179.5 — but with the pre-factor corrected
the *printed* activation energy fits every curve, so the free fit was
absorbing the decade rather than finding a different energy.

**Note the different resolution from the `D_S` case above.** There the
equation was preferred over the figure; here the figure is preferred over the
equation. The difference is that `D_S`'s two disagreed in *slope*, with no
single parameter reconciling them, whereas this is one constant.

### Eq (7) contradicts the `d_act` implied on page -484-, and Eq (7) is right

Page -484- writes the exact stress as `r·p / (2·d_o·(1 − v̇·t))`, implying
`d_act = d_o·(1 − v̇·t)`. That is **dimensionally inconsistent** — `v̇·t` is a
length, so `1 − v̇·t` subtracts metres from a pure number — and it contradicts
Eq (7) on page -492-:

```
d_act = d_o / (1 + v̇·t/d_o)
```

which is dimensionally sound and is what Fig. 4 plots. Eq (7) is implemented.

**A consequence worth stating.** Substituting Eq (7) into `σ_t = r·p/(2·d_act)`
gives `r·p·(1 + v̇t/d_o)/(2·d_o)` — which is **Eq (2) exactly**. So the
report's description of Eq (2) as an approximation that "describes the state
of affairs more realistically" understates it: given Eq (7), Eq (2) is not an
approximation at all. What it approximates is the -484- form, which is the
wrong one. Pinned by `stress::tests::the_two_routes_to_the_stress_agree_exactly`.

`geometry::actual_thickness` was originally written the -484- way and
corrected 2026-09-24 when Fig. 4 was digitised.

### Fig. 6 recovers ONE common `σ_t` from eight curves — with a residual that is systematic in `m`

Fig. 6 (-500-) is PANAMA's own output for eight SiC varieties at 1600 °C, and
page -498- states its basis: Table 1's after-irradiation values at
`T_B = 1000 °C`, `Γ = 1·10²⁵ m⁻² EDN`. `σ_t(t)` is common to all eight, so
inverting Eq (1) on each curve, `σ_t = σ_o·(−ln(1−φ)/ln2)^(1/m)`, must return
the same value eight times. That needs **none** of the four inputs the caption
omits (geometry, `V_k`, `V_f`, `F_b`, `t_B`), because they only set `σ_t`.

**Result, 2026-09-24** (467 digitised points; 121 lie below the figure's
plotted 10⁻⁶ floor and are excluded):

| quantity | measured |
|---|---|
| rank order of the eight curves | **8/8 reproduced** |
| spread top-to-bottom at 248 h | 4.79 decades |
| recovered common `σ_t` | 132 MPa (130 h) → 163 MPa (248 h) |
| relative s.d. across varieties | 9.0 % → 11.2 % |
| per-curve residual in `log₁₀ φ` at one common `σ_t` | **−0.370 … +0.388**, mean abs **0.232** |

Reproducing the order and the 4.8-decade spread from one stress to ±0.4
decades is a real success for Eq (1) with Eqs (8a)/(9a). ~~But the residual is
**monotone in `m_oo`**, not random~~ — **RESOLVED 2026-09-24: the `m`-trend is
a digitisation artefact, not physics.** Under the `6/7` log-axis reading
established in the Settled section above, the eight varieties collapse onto one
`σ_t` at **1.4 % relative s.d.** with residuals of ±2 %, and the trend
disappears. Eq (1) with Eqs (8a)/(9a) at the report's stated `Γ = 1·10²⁵` and
`T_B = 1000 °C` reproduces Fig. 6. The as-digitised trend is recorded below
because it is what the evidence looked like before the calibration was
diagnosed:

| variety | `m_oo` | residual (decades) |
|---|---|---|
| EO 249-251 | 5.0 | −0.370 |
| HT 150-167 | 6.0 | −0.183 |
| EO 1674 | 7.0 | +0.034 |
| ECO 1541 | 6.4 | +0.099 |
| EO 1607 | 8.0 | +0.222 |
| EC 1338/1339 | 7.4 | +0.241 |
| EO 403-405 | 8.4 | +0.319 |
| EUO 1551 | 8.5 | +0.388 |

Three hypotheses were tested. The third is the answer:

- **Fluence.** Relative s.d. falls monotonically with `Γ`: 10.1 % at the
  stated `Γ = 1`, 5.4 % at 0. But page -498- states Fig. 6's basis is
  `Γ = 1·10²⁵`. `Γ` was not changed, and this turned out to be the wrong
  hypothesis — the deflated data reproduces the figure **at** `Γ = 1`.
- **The plotted floor.** Restricting to points inside the figure's own axis
  leaves the trend intact. Not it either.
- **The log-axis calibration.** `k = 6/7` removes the trend entirely
  (10.0 % → 1.4 %). This is it, and two other figures confirm the same factor
  by a model-free identity.

Worth recording as a process point: the first hypothesis would have "fixed"
the residual by setting `Γ = 0`, which the report's own text excludes, and it
was rejected for that reason rather than adopted because it improved the
number. Had it been adopted, the real cause would never have been found.
Pinned by
`history::tests::figure_6_recovers_one_common_stress_history`, which asserts
the trend as well as its size so it cannot pass by being tuned small.

Fig. 6 digitised by the maintainer, 2026-09-24.

### Fig. 7 code-to-code: the staged history reproduces for 300 h, then drifts by 1.9×

Fig. 7 (-501-) has the only complete stated input set in the report —
`σ_oo = 600 MPa`, `m_oo = 6`, `T_B = 1160 °C`, `t_B = 260 FPD`,
`F_B = 0.09 FIMA`, `Γ = 0.05·10²⁵ m⁻² EDN`, `η̇(T) ≡ 0` — plus the staging
from page -498-. The absolute level still needs the unstated geometry, so the
comparison is on `σ_t` recovered from the report's own `Without Grain Boundary
Corrosion` curve against this chain's `σ_t`, with **one** free scale (the
geometry aggregate `r/(2·d_o·(V_f/V_k))`). No physics constant was adjusted.

| window | relative s.d. of `σ_t^PANAMA / σ_t^chain` | max/min |
|---|---|---|
| **0–300 h, all three stages** | **4.9 %** | 1.20 |
| 300–1000 h | 15.4 % | 1.77 |
| whole run | 21.9 % | 2.17 |

Per stage the ratio is 0.1561 (1400 °C), 0.1473 (1500 °C), 0.1608 (1600 °C,
first 100 h) — within ±4.5 %. **The staging is reproduced**: `OPF(T)` through
Eq (5c), `D_S(T)`, `v̇(T)` and Eq (3)'s explicit `T` all land together across
two step changes.

**Then it drifts.** At 977 h PANAMA's curve implies `σ_t = 282 MPa`; with the
scale fixed over 0–300 h the chain gives **148 MPa**, a factor **1.90**.
PANAMA's curve follows `φ ∝ t^3.21` at late times, i.e. `σ_t ∝ t^0.54`,
whereas in the chain `F_d` has saturated (0.980 at 296 h → 0.9999 at 977 h)
and `OPF` is constant at fixed temperature, leaving only `FKOR` — worth
**4 %** over the last 700 h. Something in PANAMA keeps the pressure climbing
as `√t` after the Booth release is over, and the printed equations do not say
what.

Diagnostic runs (reported, not adopted): the drift is removed only by taking
`OPF = 0` **and** `D_S(1600 °C) ≈ 1.6·10⁻¹⁰ s⁻¹`, which is ~1500× below the
printed `UO₂` value of 2.3·10⁻⁷ and ~9× below the lowest value the
`(Th,U)O₂` correlation can produce. Neither is consistent with page -487-.

**The kernel is not stated** in Fig. 7's caption. `UO₂` with Eq (5c) is used;
it is the tightest of four candidates over 0–300 h (5.0 % relative s.d.,
against 5.7 % for `(Th,U)O₂` at `N = 10`, 6.1 % at `N = 5`, 7.7 % for
`OPF = 0`), but that is a weak preference and is recorded as one.

### Fig. 7 code-to-data: the measurement lies between PANAMA's two curves

The nine measured ⁸⁵Kr points against the report's own curves, in `log₁₀`:

| comparator | mean | mean abs | worst |
|---|---|---|---|
| `Without Grain Boundary Corrosion` (`η̇ ≡ 0`, the caption's case) | **+0.51** | 0.52 | +1.14 (339 h) |
| `With Grain Boundary Corrosion` | −1.49 | 1.49 | −1.85 (223 h) |

So at 1400–1600 °C PANAMA **under**-predicts FRJ2-K11/03 by half a decade
with grain-boundary corrosion off, and over-predicts by 1.5 decades with it
on. This reproduces page -499-'s own reading that the with-corrosion model
"covers the measured values in a conservative approximation". It is a
statement about **PANAMA**, not about this implementation — no part of this
reconstruction enters it.

**Inherited assumption, stated because it is load-bearing:** PANAMA computes a
particle *failure* fraction and Fig. 7 plots a ⁸⁵Kr *release* fraction on the
same axis, i.e. the report equates the two. That identification is inherited
here, not derived. It is the first suspect for any constant offset.

**A digitisation label slip:** the measured series is labelled `90% FIMA`
where the caption reads `9.0 % FIMA` and `F_B = 0.09`. 0.09 is used.

Fig. 7 digitised by the maintainer, 2026-09-24.

### Fig. 8 (AVR GO 2): the late-time drift REAPPEARS isothermally — the driver's staging is exonerated

Fig. 8 is the second validation case and the discriminating one for the drift
recorded above: `σ_oo = 600 MPa`, `m_oo = 6`, `T_B = 950 °C`, `t_B = 500 FPD`,
`F_B = 0.082 FIMA`, `Γ = 0.6·10²⁵ m⁻² EDN`, `η̇ ≡ 0`, **isothermal at
1600 °C**. No stages, so a drift here cannot be the driver mishandling stage
changes.

**It reappears, and worse.** Over 74 digitised points the ratio
`σ_t^PANAMA/σ_t^chain` rises **monotonically from the very first point** —
0.189 at 14.6 h to 0.447 at 967 h, relative s.d. **19.0 %**, max/min 2.37.
There is no flat window at all, where Fig. 7 held to 4.9 % for 300 h. Under
the `6/7` axis reading it falls to 14.3 % and max/min 1.91 — reduced, not
removed.

So Fig. 7's flat first 300 h now looks like the rising temperature masking the
same shortfall, rather than agreement. Both curves run as `σ_t ∝ t^0.52`
(Fig. 8) and `t^0.54` (Fig. 7) late, while the chain has only `FKOR` left once
`F_d` saturates — 4.2 % from 244 h to 967 h here.

Two one-parameter diagnostics were run (**reported, not adopted**; neither has
independent support and each contradicts a figure that does):

| free parameter | best value | printed / figure value | residual |
|---|---|---|---|
| Weibull modulus `m` | 12.0 (10.3 under `6/7`) | **5.57** from Eq (9a) | 0.9 % over 74 points |
| corrosion pre-factor | ≈ 1.1·10⁻⁶ | **5.87·10⁻⁸** (Fig. 4, 483 points) | 4.5 % |

Both are just two ways of saying the same thing: `σ_t` must grow about twice
as much as the printed chain allows. `D_S` is **not** among them — see the
bound below.

### Fig. 8 identifies the kernel: the `UO₂` oxygen correlation is excluded

At fixed temperature `σ_t ∝ (F_d·F_f + OPF)·FKOR` and `F_d ∈ [0,1]`, so the
**largest** growth the chain can produce between two times is
`(F_f + OPF)/OPF` times the `FKOR` ratio. That ceiling depends on `OPF` alone
— not on `D_S`, not on `τ_i`, not on the geometry.

| `OPF` source at 1600 °C | `OPF` | ceiling | Fig. 8 requires | |
|---|---|---|---|---|
| Eq (5c), `UO₂`, `T_B = 950 °C`, `t_B = 500 FPD` | 0.157 | **3.14×** | 4.50× (3.63× under `6/7`) | **excluded** |
| Eq (5a), `(Th,U)O₂`, `N = 5` | 0.036 | 10.2× | 4.50× | ok |

`N = 5` is the report's own AVR value (Eq 5a's symbol note), and GO 2 is an
AVR fuel element, so this is a **positive identification from the figure's own
output** rather than a preference. Contrast Fig. 7, where `UO₂` is only a weak
5.0 %-vs-5.7 % preference and the bound is not violated.

### Fig. 8 code-to-data: PANAMA is not conservative at 1600 °C with `η̇ ≡ 0`

| comparator | n | mean | mean abs | worst |
|---|---|---|---|---|
| `without` corrosion, the caption's 70/26 at 8.2 % FIMA | 4 | **+0.24** | 0.43 | +0.47 |
| `without` corrosion, all three burnups | 9 | **+0.45** | 0.53 | +1.15 |
| `with` corrosion, all three burnups | 9 | −1.52 | 1.52 | −2.74 |

(`log₁₀` of measured over curve; under the `6/7` reading the 70/26 mean is
+0.20 and mean abs 0.37.)

This matters more than Fig. 7's equivalent. Page -479- claims good agreement
**1600–2500 °C** and concedes over-conservatism below it, so Fig. 7's 1400 and
1500 °C stages had an excuse. Fig. 8 is isothermal at 1600 °C and has none:
the `η̇ ≡ 0` case sits **below** the data on the caption's own burnup, and the
70/26 residual changes sign by 302 h — the model crosses the data rather than
tracking it. The 70/7 and 70/15 series at 7.1–7.2 % FIMA are reported as
spread, not as comparators; the PANAMA curve is drawn for 0.082.

Fig. 8 digitised by the maintainer, 2026-09-24.


### `φ₂` has no figure to verify it, and Fig. 6 constrains it only negatively

Nothing in the report plots or tabulates the decomposition chain
(Eqs 11–14b) against `d_o`. What Fig. 6 does settle is that it **cannot** have
used Eq (14a): at 1600 °C with `d_o = 35 µm`, `ζ(300 h) = 3.6·10⁻³`, and
Eq (14a) turns that into `φ₂ = 4.9·10⁻³` — a floor above the *top* of a figure
that runs 2·10⁻⁶ to ~5·10⁻³ and spreads over three decades. Eq (14b) gives
`1.7·10⁻¹⁴`, invisible. So Fig. 6 used the sphere calibration, or `φ₂` off.

### `η̇·t` for a varying history is this crate's extension

Eq (10b) prints `exp(−η̇·t)` with one rate and one time, i.e. for an
isothermal hold. `advance_grain_boundary_exposure` accumulates `∫η̇ dt` by
analogy with Eq (11)'s `∫k dt`. **The report does not state this.** It
collapses to the printed form when the temperature is constant. The
alternative — `η̇` at the current temperature times the total elapsed time —
would retroactively apply the latest temperature to the whole history.

### Eq (4) at `τ_i = 0` is undefined in the report

Eq (4) divides by `τ_i`. `released_gas_fraction` returns **zero** there, on
the grounds that no irradiation means no inventory (and Eq (3) carries
`F_b = 0` in the same limit). This crate's convention, not the report's.

### Fig. 9's 50 µm curve sits ABOVE its 35 µm curve

Fig. 9 (-502-) plots a thicker SiC layer as *more* likely to fail, at both
1600 and 2000 °C. By Eq (2) alone a thicker layer carries less stress at the
same pressure, and by Eq (12) it decomposes more slowly, so both mechanisms
point the other way. The likely reconciliation is that the 50 µm variant keeps
the particle's outer radius and eats into the buffer, cutting `V_f` and
raising `p` — but the caption states neither the geometry nor `V_f`, so this
is **not checked**. Fig. 9 is therefore not used as a verification target.


### HTR-10 applied to PANAMA is an EXTRAPOLATION, and the `f_inc` seam is not what it looked like

`crates/boon-lay/src/fuel_failure/htr10.rs` applies the model to HTR-10, to
supply `triso_atops_fork`'s `FailureFractions::incremental` (`f_inc`) —
currently a `3·10⁻⁵` placeholder in `htgr_sim_v1` taken from TRISO-ATOPS'
constants block for a different fuel line, with release scaling linearly in it.

**PANAMA was built and validated for German TRISO over 1600–2500 °C** (page
-479-); HTR-10's fuel is German-lineage, so this is defensible by lineage and
is **not a validated application**. Two inputs are not published for HTR-10 and
are taken by name from the report's HTR-Module column: `σ_oo/m_oo = 834/8.02`
(EO 1607, footnote 1 page -503-) and `Γ = 1.4·10²⁵`. `T_B` is an **input** —
HTR-10 publishes a maximum fuel temperature, not an average. Everything else is
HTR-10's own or derived from it: the 380/415 µm SiC layer and 250 µm kernel
(IAEA-TECDOC-1382 pt 2 Table 4-17), `F_b = 0.0851` from 80 000 MWd/t, and
`t_B = 1080 FPD` from 10 MW over 27 000 × 5 g HM.

**Normal operation: PANAMA must not replace the placeholder.** `φ₁` at the end
of irradiation is `2.4·10⁻¹⁵` at 700 °C, `9.3·10⁻¹³` at 776 °C, `2.7·10⁻⁹` at
900 °C and `5.9·10⁻⁷` at 1000 °C — four to thirteen decades below `3·10⁻⁵`.
The placeholder is the same order as PANAMA's own as-manufactured target
`φ_o = 6·10⁻⁵` (page -480-), which PANAMA takes as an **input**. The two are
different quantities, and substituting would divide every reported activity by
~10⁷.

**Accident: this is where the seam earns its place.** 200 h isothermal,
`T_B = 776 °C`: `φ_total` = 4.5·10⁻⁹ (1200 °C), 6.8·10⁻⁷ (1400 °C),
**3.10·10⁻⁵ (1600 °C)**, 4.46·10⁻⁴ (1800 °C), 5.07·10⁻³ (2000 °C), 0.978
(2200 °C, of which `φ₂` is 0.977). `φ₂` overtaking `φ₁` between 2000 and
2200 °C matches page -508-.

**The 1600 °C value landing on 3.1·10⁻⁵ beside a 3·10⁻⁵ placeholder is a
coincidence** of two unrelated quantities, and is pinned in the tests as one.

**Not verified.** The workspace's local literature gives HTR-10's geometry,
burnup, enrichment and power but **no measured failure fraction, free-uranium
fraction or release fraction**, so the comparison that would make this a
validation **could not be made** and is not claimed. The nearest check is the
report's own statement that HTR-Module depressurised stays below 10⁻⁶ at 200 h
(page -504-); a flat 200 h at 1600 °C gives 3.1·10⁻⁵ here, an upper bound on a
transient that only briefly peaks, so the two are not in conflict — but without
Fig. 10's temperature history it is not a check either. **Digitising Fig. 10
would make it one.**


### Notation defects recorded rather than silently fixed

- **Eq (6b) is used twice** (UO₂ and UCO); the second should be (6c).
- **EO 1607 disagrees with itself**: `σ_oo/m_oo` is 850/8.0 in Table 1 but
  834/8.02 in Fig. 5 and Table 2. Use 834/8.02 for the reactor reproductions.
- **Seven load-bearing correlations carry no equation number** and must be
  cited by page.
- ~~A grouping ambiguity in the Booth `f(τ)` series: the literal reading
  diverges, the consistent one matches Fig. 1.~~ **SETTLED 2026-09-24** — see
  the `f(τ)` entry above; verified against Fig. 1 to 0.0028 mean over
  `τ ≥ 0.15`.
- **Eq (6b) is printed twice**, for `UO₂` on -491- and again for `UCO` on
  -492-; the second must be (6c). Both values are implemented, under
  `KernelCompound::UraniumOxide` (2.43796·10⁻⁵ m³/mol) and
  `KernelCompound::UraniumOxycarbide` (2.50654·10⁻⁵ m³/mol). A reader chasing
  "Eq (6b)" in the report will find two different molar volumes under it.
- The molar masses of Eqs (6a)–(6c) have **no stated stoichiometry**:
  0.2672 kg/mol is neither `UO₂` at natural enrichment (0.2700) nor `²³⁵UO₂`
  (0.2670). Taken as printed; each equation's own printed quotient is
  reproduced to better than 1 part in 10⁵.

---

## Verification status

| Equation | Implemented | Verified against | Result |
|---|---|---|---|
| (1) Weibull | yes | — | unit tests only |
| (2) stress, thin shell | yes | — | unit tests only |
| (3) gas pressure | yes | independent `nRT/V` | exact |
| (8a)/(8b) strength | yes | **Table 1, -500-**, **Table 2, -504-** | **8/8 σ_o**, plus **2/2** at `Γ = 1.4` |
| (9a)/(9b) modulus | yes | **Table 1, -500-**, **Table 2, -504-** | **8/8 m_o**, plus **2/2** at `Γ = 1.4` |
| φ_total assembly | yes | — | unit tests only |
| `D_S` (Th,U)O₂, p-487 | yes | **Fig. 2** | 4/4 curves, labels recovered |
| `D_S` UO₂/UCO, p-487 | yes | **Fig. 2** | **disagrees, 6.4×→1.3×** (see above) |
| (7) + `v̇`, p-492 | yes | **Fig. 4** | 5/5 curves, **after a decade fix** |
| (5a) `(Th,U)O₂` OPF | yes | **Fig. 3** | temperature term confirmed |
| (5b)/(5c)/(5d)/(5e) `UO₂` OPF | yes | **Fig. 3**, **Table 2** | 4/4 curves, mean 0.0087; **2/2 on Table 2** |
| `f(τ)` Booth series, p-485 | yes | **Fig. 1** + two analytic limits | mean 0.0028 (`τ ≥ 0.15`); grouping settled |
| (4) `F_d` | yes | identity `F_d(τ_i, 0) = f(τ_i)` | exact; **no figure plots `F_d`** |
| (6a)/(6b)/(6c) `V_m` | yes | each equation's own printed quotient | 3/3 to < 1·10⁻⁵ |
| (11)/(12) `ζ`, `k` | yes | — | internal only; **no figure or table** |
| (13)/(14a)/(14b) `φ₂` | yes | **Fig. 6, negatively** | (14a) excluded; (14b) unverified |
| (10b)/(10c) grain boundary | yes, **off by default** | **Figs. 7 and 8, two-curve identity** | `σ_with/σ_without` = 1.014 (Fig. 8), 1.000 (Fig. 7) under the `6/7` axis reading |
| driver, §3.1 | yes | report's own step-independence claim (-482-) | 2·10⁻¹² over 1→3000 steps |
| driver vs **Fig. 6** | — | **Fig. 6** | order 8/8; **1.4 % on one `σ_t`** under the `6/7` axis reading (10.0 % as digitised) |
| driver vs **Fig. 7** (code-to-code) | — | **Fig. 7** | 4.9 % over 0–300 h incl. staging; **drifts to 1.90× by 977 h** |
| PANAMA vs **Fig. 7** (code-to-data) | — | **Fig. 7**, 9 measured points | **+0.51 decades** (`η̇ ≡ 0`); −1.49 with corrosion |
| driver vs **Fig. 8** (code-to-code) | — | **Fig. 8**, isothermal | **19.0 % rel s.d., drifts from the first point** — staging exonerated |
| PANAMA vs **Fig. 8** (code-to-data) | — | **Fig. 8**, 9 measured points | **+0.24 decades** on the caption's burnup; −1.52 with corrosion |
| log y-axis of Figs. 6/7/8 | — | three model-free identities | **calibrated over 7 decades where 6 are plotted** |
| HTR-10 application | yes, [`htr10`] | — | **extrapolation**; no HTR-10 failure data exists locally to check it |

Nothing here has been calibrated. Per the workspace rule, the reconstruction
runs uncalibrated and the disagreement is reported when there is one.
