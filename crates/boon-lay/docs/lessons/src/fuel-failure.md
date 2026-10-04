# When the particle breaks: boon-lay fuel failure

**The problem.** During irradiation, stable fission gases (Xe, Kr) and CO
from kernel oxygen collect in the buffer's voids. When the reactor heats up
in an accident, that gas pressure rises, the SiC layer corrodes and weakens,
and at very high temperature the SiC itself decomposes. Some fraction of
particles fails. What fraction, and when?

The answer in this crate is **boon-lay fuel failure**: the equations of the
PANAMA-I report (Verfondern & Nabielek, HTA-IB-03/90, 1990) implemented in
Rust. It is not the PANAMA code, which this project has never had (see
[How to read](./intro.md)).

## Three failure populations, combined by survival

The model has three ways to fail
([`fuel_failure/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/mod.rs#L48-L61)):

| Term | Mechanism | In the code |
|---|---|---|
| `φ_o` | as-manufactured defects | **an input**, not modelled |
| `φ₁` | pressure-vessel overstress | the Weibull chain below |
| `φ₂` | SiC thermal decomposition above about 2000 °C | `decomposition` |

A particle survives only if it survives all three, so the **survival**
probabilities multiply. The failure fractions are not added
([`total_failure_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/mod.rs#@@L:crates/boon-lay/src/fuel_failure/mod.rs:fn=total_failure_fraction@@)):

```rust,ignore
{{#include ../../../src/fuel_failure/mod.rs:249:258}}
```

Adding them would count a particle failed by two mechanisms twice and could
exceed 1. A test pins both points: the product form, and that the result is
below the naive sum.

## The pressure-vessel chain, `φ₁`

The chain goes gas → pressure → stress → failure probability.

**1. Gas pressure, Eq (3)**, is the ideal-gas law. Gas is the released
fission-gas fraction `F_d` times yield `F_f`, plus oxygen per fission `OPF`,
times burnup `F_b`
([`internal_gas_pressure`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/pressure.rs#@@L:crates/boon-lay/src/fuel_failure/pressure.rs:fn=internal_gas_pressure@@)):

```rust,ignore
{{#include ../../../src/fuel_failure/pressure.rs:80:101}}
```

The printed equation is ambiguous about where its fraction bar ends. Read
one way, pressure would *fall* as the particle heats. The code uses the only
dimensionally consistent reading, which is `p = nRT/V`, and a test checks it
against `nRT/V` computed independently
([doc](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/pressure.rs#L67-L79)).

**2. SiC hoop stress, Eq (2)**, thin shell, with corrosion thinning the
layer over time
([`induced_stress`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/stress.rs#@@L:crates/boon-lay/src/fuel_failure/stress.rs:fn=induced_stress@@)):

```rust,ignore
{{#include ../../../src/fuel_failure/stress.rs:46:56}}
```

**3. Failure probability, Eq (1)**, a Weibull law
([`weibull_failure_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/weibull.rs#@@L:crates/boon-lay/src/fuel_failure/weibull.rs:fn=weibull_failure_fraction@@)):

```rust,ignore
{{#include ../../../src/fuel_failure/weibull.rs:49:62}}
```

### The `ln 2` is load-bearing

Eq (1) is `φ₁ = 1 − exp[−ln2·(σ_t/σ_o)^m]`, not the textbook
`1 − exp[−(σ/σ_c)^m]`. The `ln 2` makes `σ_o` the **median** strength: at
`σ_t = σ_o` exactly half the particles fail. A stock Weibull treats its scale
as the *characteristic* strength, where 63.2 % fail. Swapping one for the
other shifts the strength scale by `(ln2)^(1/m)`, about 4 % at `m = 8`. It
raises no error, and it shifts the answer in the flattering direction. The
test `median_is_the_scale_parameter` pins it
([explanation](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/mod.rs#L110-L119)).

> **Predict before you read on.** If the SiC is irradiated, its median
> strength and Weibull modulus both fall (Eqs (8a)–(9b), in `strength.rs`).
> Which way does that move `φ₁` at fixed stress?

## Thermal decomposition, `φ₂`

Above about 2000 °C, SiC decomposes into gaseous Si and graphite. The layer
stops being a pressure vessel without bursting. The model accumulates an
Arrhenius "action integral" `ζ = ∫ k(T) dt` and reads `φ₂ = 1 − exp(−α·ζ^β)`
directly off it
([`decomposition.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/decomposition.rs)).

One structural point carries over to any transient code: **`ζ` carries the
history, and `φ₂` is not accumulated by increments.** For a temperature
history that rises and then cools, which is what a real transient does,
summing increments of `φ₂` would give a different (wrong) answer from reading
it off `ζ`. `φ₁`, by contrast, *is* accumulated from positive increments.
The accident driver
[`AccidentHistory`](../../api/boon_lay/fuel_failure/history/struct.AccidentHistory.html)
steps the two side by side.

**Unverified, and stated as such.** No figure or table in the report checks
Eqs (11)–(14b). The report's Fig. 6 does rule out one of its two calibrations
(the loose-particle one, Eq (14a)), and a test pins that. The checks that
remain are internal and algebraic.

## Settled ambiguities in the report

Several symbols in the report are ambiguous or misprinted. Each one is
settled against the report's own tables or by dimensions, and recorded in
[`docs/panama-i-units-and-open-questions.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/panama-i-units-and-open-questions.md):

| Symbol | Printed | Used | Settled by |
|---|---|---|---|
| irradiation temperature `T_B` | °C | **kelvin** | Table 1 reproduces 16/16 in kelvin, 0/16 in °C |
| irradiation time `t_B` | seconds | **seconds** | Fig. 3 |
| Eq (3) grouping | bar spans denominator | `R·T` in numerator | dimensions |

All public signatures use `uom`, so a caller cannot pass °C where kelvin is
meant.

## Optional: grain-boundary corrosion is off by default

Eqs (10b)/(10c) model grain-boundary corrosion, which lowers the Weibull
modulus over time. The crate's default is
[`GrainBoundaryCorrosion::Disabled`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/grain_boundary.rs#L107-L119),
because that is the report's own default and its stated normal use. The
crate's doc explains why this is not a case of physics being switched off by
default: the source model specifies it off
([reasoning](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/grain_boundary.rs#L47-L53)).
The choice is a visible enum at the call site.

## Applying it to HTR-10: an extrapolation, reported as one

PANAMA-I was built for **German** TRISO fuel and claims good agreement only
over 1600–2500 °C. HTR-10's fuel is German-lineage, which makes applying the
model *defensible*, but it is not a validated application. Two inputs
(the SiC strength `σ_oo`/`m_oo`, and the fast fluence `Γ`) are not published
for HTR-10 and are taken, by name, from the report's HTR-Module case
([`htr10/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/htr10/mod.rs#L20-L55)).

**Normal operation.** At the end of irradiation the chain gives
([table](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/htr10/mod.rs#L76-L84)):

| `T_B` | `σ_t` | `φ₁` |
|---|---|---|
| 700 °C | 6.9 MPa | 2.8·10⁻¹⁵ |
| 776 °C | 15.2 MPa | 1.2·10⁻¹² |
| 900 °C | 46.3 MPa | 5.0·10⁻⁹ |
| 1000 °C | 103 MPa | 1.6·10⁻⁶ |

These numbers are tiny. It is tempting to use them as the in-service failure
fraction `f_inc` that TRISO-ATOPS needs for normal operation. **That would
answer a different question.** `φ₁` is the accident pressure-vessel
mechanism. In-service failure during normal irradiation is a
manufacturing-and-irradiation defect population, which the PANAMA-I
equations take as an *input*. The fuel-failure module doc compared `φ₁` with
a `3·10⁻⁵` placeholder then used by the `htgr_sim_v1` digital twin, about
seven orders of magnitude apart. That placeholder has since been deleted
(gh:#399), because its stated provenance turned out to be false. The twin
now uses a published HTR-10 design irradiation failure of `5·10⁻⁴` (Liu & Cao
2002), adds `φ₁` and a chemical-attack term to it, and says so
([`fission_product_release.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs#L462-L486)).
The lesson stands: normal-operation `f_inc` needs fuel-qualification data,
not a better accident model.

**Accident.** 200 h at a constant temperature, after `T_B = 776 °C`
([table](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/htr10/mod.rs#L105-L112)):

| accident `T` | `σ_t` | `φ₁` | `φ₂` (14b) |
|---|---|---|---|
| 1200 °C | 50 MPa | 4.53·10⁻⁹ | ~0 |
| 1600 °C | 178 MPa | 3.10·10⁻⁵ | 3.4·10⁻¹⁵ |
| 2000 °C | 370 MPa | 4.79·10⁻³ | 2.78·10⁻⁴ |
| 2200 °C | 536 MPa | 6.07·10⁻² | 0.977 |

The 1600 °C value lands on `3.1·10⁻⁵`, next to the old `3·10⁻⁵` placeholder.
**That is a coincidence, not agreement**: they are different quantities for
different fuel. What the table *does* show is the model's own structure:
`φ₂` overtakes `φ₁` between 2000 and 2200 °C, as the report says it should.

Nothing in this section is compared against measured HTR-10 failure data.
The workspace's literature has none.
