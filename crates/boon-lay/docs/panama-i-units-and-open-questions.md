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

---

## Open

### `t_B` — irradiation time. **UNRESOLVED, and it blocks the pressure path.**

| | |
|---|---|
| **Printed** | `[s]` (symbol list, -511-) |
| **Contradicted by** | Fig. 3's curve labels (`1000 d`, `500 d`); Fig. 7's caption (`260 FPD`); Fig. 8's caption (`500 FPD`) |
| **Status** | three independent places say full-power days, one says seconds |

`t_B` enters Eqs (5b)/(5c), the UO₂ oxygen-per-fission correlations. `OPF`
feeds Eq (3) directly, so this sets the **absolute internal pressure** and
therefore everything downstream of it. The extraction also records that the
`−10.08` intercept gives implausible `OPF` on *either* reading, so the
discrepancy may not be units alone.

**Not guessed.** Eqs (5a)–(5f) are deliberately not implemented; `OPF` is an
**input** to `pressure::internal_gas_pressure`. A wrong exponent here would
propagate into a plausible-looking absolute pressure and a failure fraction
that still reads as reasonable.

**How to settle it.** Digitise Fig. 3 (`-488-`), evaluate Eq (5b) with `t_B`
in days and in seconds for one labelled curve, and see which reproduces it.
Table 2's "O atoms/fission at t=0" (0.00511 / 0.00316) is a second,
independent numeric check.

### Missing page -510- (§7, References)

Absent from the source PDF — the file has 39 pages and all 39 render; p-34 is
`-509-` and p-35 is `-511-`. Not blocking, but it is how the underlying
measurement papers (Nabielek 1984, Allelein 1983, Benz 1982, Proksch 1982,
Myers 1977, Horsley 1976, Strigl 1984, Montgomery 1981) would be chased for
their own stated units.

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
| (4), (5a)–(5f), (6a)–(6c), (7), (10b)/(10c), (11)/(12) | **no** | — | inputs, deliberately |

Nothing here has been calibrated. Per the workspace rule, the reconstruction
runs uncalibrated and the disagreement is reported when there is one.
