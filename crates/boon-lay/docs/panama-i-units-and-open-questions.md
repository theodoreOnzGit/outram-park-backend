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

### Notation defects recorded rather than silently fixed

- **Eq (6b) is used twice** (UO₂ and UCO); the second should be (6c).
- **EO 1607 disagrees with itself**: `σ_oo/m_oo` is 850/8.0 in Table 1 but
  834/8.02 in Fig. 5 and Table 2. Use 834/8.02 for the reactor reproductions.
- **Seven load-bearing correlations carry no equation number** and must be
  cited by page.
- A grouping ambiguity in the Booth `f(τ)` series: the literal reading
  diverges, the consistent one matches Fig. 1.

---

## Verification status

| Equation | Implemented | Verified against | Result |
|---|---|---|---|
| (1) Weibull | yes | — | unit tests only |
| (2) stress, thin shell | yes | — | unit tests only |
| (3) gas pressure | yes | independent `nRT/V` | exact |
| (8a)/(8b) strength | yes | **Table 1, -500-** | **8/8 σ_o** |
| (9a)/(9b) modulus | yes | **Table 1, -500-** | **8/8 m_o** |
| φ_total assembly | yes | — | unit tests only |
| `D_S` (Th,U)O₂, p-487 | yes | **Fig. 2** | 4/4 curves, labels recovered |
| `D_S` UO₂/UCO, p-487 | yes | **Fig. 2** | **disagrees, 6.4×→1.3×** (see above) |
| (7) + `v̇`, p-492 | yes | **Fig. 4** | 5/5 curves, **after a decade fix** |
| (5a) `(Th,U)O₂` OPF | yes | **Fig. 3** | temperature term confirmed |
| (5b)/(5c)/(5d)/(5e) `UO₂` OPF | yes | **Fig. 3** | 4/4 curves, mean 0.0087 |
| (4), (6a)–(6c), (10b)/(10c), (11)/(12) | **no** | — | inputs, deliberately |

Nothing here has been calibrated. Per the workspace rule, the reconstruction
runs uncalibrated and the disagreement is reported when there is one.
