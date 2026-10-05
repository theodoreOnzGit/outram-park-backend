# Rung 5 — When the shell breaks: boon-lay fuel failure

> **Research, education and V&V only.** Nothing here is for reactor
> operation, licensing, safety-critical decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status:** AI-assisted draft, 2026-10-04, not yet human-reviewed.
> [Something doesn't tally?](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=boon-lay%20lesson%20rung%205%20doesn%27t%20tally)

**Core lesson, rung 5 of 8.** Rungs 3 and 4 assumed the coatings were
intact. This rung asks when they stop being intact. The extended page
[When the particle breaks](../../deep-dives/triso-atops/fuel-failure.html) goes equation by equation; this
page is the path through it.

## A naming rule first

- **PANAMA-I** is a 1990 Jülich report (Verfondern & Nabielek,
  HTA-IB-03/90) and the results printed in it.
- **boon-lay fuel failure** is this crate's own Rust code, written from the
  report's equations, and every number that code computes. The PANAMA source
  code was never available to this project, so nothing here is "PANAMA's
  result" for a case the report does not print
  ([the rule in the code](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/mod.rs#L24-L46)).

The report is restricted literature: the code carries its equations and
constants with page numbers, and no prose or figure is copied.

## The hook

> **The problem.** During irradiation, fission gas (Xe, Kr) and CO collect in
> the buffer's pores. In an accident the core heats up, the gas pressure
> rises, the SiC is slowly eaten away, and above about 2000 °C the SiC itself
> decomposes. *What fraction of particles fails, and when?*

**Demo:** [open this rung](../../demos/triso-atops/?rung=failure). Irradiation temperature, hold
temperature and hold time as sliders; φ₁, φ₂ and the in-service total, and
the gas pressure and SiC stress, from `AccidentHistory` step by step; and a
drawn population of 400 particles that fail as the fraction passes each
one's random draw (a picture of the fraction, not 400 simulated particles).

---

## Step 1. How much gas is pushing on the SiC?

**The question.** The SiC is a pressure vessel. What is the pressure inside?

**The shortest answer.** The ideal-gas law, $p = nRT/V$, with $n$ the moles
of gas released into the buffer's pores and $V$ the pore volume.

**The formula** (report Eq (3)):

$$p = \frac{(F_d F_f + \mathrm{OPF})\ F_b\ R\ T}{(V_f/V_k)\ V_m},$$

| Symbol | Meaning | Where it comes from |
|---|---|---|
| $F_d$ | fraction of the stable fission gas that has left the kernel | Booth release, Eq (4), from $D_S$ (report p. -487-) |
| $F_f = 0.31$ | stable fission-gas atoms per fission | printed with Eq (3) |
| $\mathrm{OPF}$ | oxygen atoms per fission that form CO | Eqs (5a)–(5f) |
| $F_b$ | burnup, FIMA | input |
| $V_f/V_k$ | pore volume over kernel volume | geometry ([rung 1](./triso.md)) |
| $V_m$ | molar volume of the kernel compound | Eqs (6a)–(6c) |

**Settled ambiguity.** As printed, the fraction bar seems to put $RT$ in the
denominator, which would make the pressure *fall* as the particle heats.
Only the reading above is dimensionally consistent (it is just $nRT/V$), and
a test pins it against $nRT/V$ computed independently
([doc](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/pressure.rs#@@L:crates/boon-lay/src/fuel_failure/pressure.rs:const=STABLE_FISSION_GAS_YIELD@@)).

**The code walk.**

*In words, read from the source by hand:*

<!-- walk-in-words: from=crates/boon-lay/src/fuel_failure/history.rs::pressure_at to=crates/boon-lay/src/fuel_failure/pressure.rs::internal_gas_pressure -->
1. [`AccidentHistory::pressure_at`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#@@L:crates/boon-lay/src/fuel_failure/history.rs:fn=pressure_at@@)
2. → [`reduced_diffusion_coefficient`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/diffusion.rs#@@L:crates/boon-lay/src/fuel_failure/diffusion.rs:fn=reduced_diffusion_coefficient@@): $D_S = D_{\text{eff}}/r_o^2$ for the kernel type and burnup;
3. → [`dimensionless_time`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/booth.rs#@@L:crates/boon-lay/src/fuel_failure/booth.rs:fn=dimensionless_time@@): $\tau_a = D_S t$;
4. → [`released_gas_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/booth.rs#@@L:crates/boon-lay/src/fuel_failure/booth.rs:fn=released_gas_fraction@@) → [`booth_release_function`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/booth.rs#@@L:crates/boon-lay/src/fuel_failure/booth.rs:fn=booth_release_function@@) twice: $F_d = [(\tau_i+\tau_a) f(\tau_i+\tau_a) - \tau_a f(\tau_a)]/\tau_i$;
5. → [`OxygenSource::oxygen_per_fission`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#@@L:crates/boon-lay/src/fuel_failure/history.rs:fn=oxygen_per_fission@@) → for UO₂ [`oxygen_per_fission_uo2`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/oxygen.rs#@@L:crates/boon-lay/src/fuel_failure/oxygen.rs:fn=oxygen_per_fission_uo2@@) (before or during heating), for (Th,U)O₂ `oxygen_per_fission_thoria`, for UCO zero;
6. → [`molar_volume`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/molar_volume.rs#@@L:crates/boon-lay/src/fuel_failure/molar_volume.rs:fn=molar_volume@@);
7. → [`internal_gas_pressure`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/pressure.rs#@@L:crates/boon-lay/src/fuel_failure/pressure.rs:fn=internal_gas_pressure@@).
<!-- /walk-in-words -->

*Generated by rust-analyzer (`kovan-cli code-walk`); a hop it cannot follow is marked, and a hop filled by hand is labelled:*

<!-- code-walk: from=crates/boon-lay/src/fuel_failure/history.rs::pressure_at to=crates/boon-lay/src/fuel_failure/pressure.rs::internal_gas_pressure -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `history.rs::AccidentHistory::pressure_at` to `pressure.rs::internal_gas_pressure`: 1 hop, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`history.rs::AccidentHistory::pressure_at`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L529) — **Eq (3)** — the internal gas pressure at an instant, with `F_d` from Eq (4) at this elapsed accident time and `OPF` at this temperature.

<!-- snippet-check: crates/boon-lay/src/fuel_failure/history.rs:529 fn pressure_at -->
<!-- snippet-check: crates/boon-lay/src/fuel_failure/history.rs:538 internal_gas_pressure -->

```rust,ignore
{{#include ../../../../../crates/boon-lay/src/fuel_failure/history.rs:529:539}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`pressure.rs::internal_gas_pressure`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/pressure.rs#L80) · called at [L538](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L538) — **Eq (3)** — internal gas pressure from the ideal gas law (page -484-).

<!-- snippet-check: crates/boon-lay/src/fuel_failure/pressure.rs:80 fn internal_gas_pressure -->

```rust,ignore
{{#include ../../../../../crates/boon-lay/src/fuel_failure/pressure.rs:80:101}}
```
<!-- /code-walk -->

**The checks** (each against the report's own printed output, recorded
2026-09-24 in the module docs and
[`docs/panama-i-units-and-open-questions.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/panama-i-units-and-open-questions.md)):

- **The Booth function, Fig. 1.** The printed series is ambiguous too: read
  literally it diverges ($f(0.1) \approx -6\times10^{4}$). The standard
  reading reproduces the 78 digitised points of Fig. 1 to a mean
  $|\Delta f|$ of **0.0066** (0.0028 for $\tau \ge 0.15$)
  ([`booth.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/booth.rs#L59-L85)).
- **$t_B$ is in seconds, Fig. 3.** The symbol list says seconds; the curve
  labels say days. In seconds, Eq (5b) reproduces Fig. 3 to a mean
  $|\Delta\mathrm{OPF}|$ of **0.0087**; in days it collapses to about zero
  everywhere (error 0.277). Getting this wrong moves $\log\mathrm{OPF}$ by
  about 9.9 decades
  ([`oxygen.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/oxygen.rs#L37-L64)).
- **$V_m$ checks itself:** each equation prints its own quotient, reproduced
  to 1 part in 10⁵. The third is printed as a second "(6b)"; the code calls
  it (6c) and says so.
- **A miss in the source, recorded and not tuned away.** For UO₂ the printed
  $D_S$ correlation sits above the report's own Fig. 2 by a factor that falls
  from **6.4×** to 1.3× with temperature: a slope disagreement, not an offset.
  The code implements the equation and tells anyone comparing with Fig. 2 to
  expect it
  ([`diffusion.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/diffusion.rs#L52-L74)).

These are **verification** of a transcription against the report's printed
output. None of them is a comparison with the PANAMA code.

**Predict.** Burnup doubles. What happens to $p$? And if the particle is
heated from 1000 °C to 1600 °C, which terms in the formula change?

---

## Step 2. How much stress does that put in the SiC?

**The question.** A pressure inside a thin spherical shell pulls it apart.
How hard?

**The shortest answer.** The thin-shell (membrane) stress: pressure times
radius over twice the thickness. And the thickness shrinks during the
accident, because the SiC corrodes.

**The formula** (Eq (2), with Eq (7)'s corrosion):

$$\sigma_t = \frac{r\ p}{2\ d_o}\left(1 + \frac{\dot v\ t}{d_o}\right),\qquad \dot v = A\ e^{-179\ 500/RT}\ \text{m/s},$$

with $r = \left(\frac{1}{2}(r_a^3 + r_i^3)\right)^{1/3}$ the report's cube-root mean
radius. The bracket is $\mathrm{FKOR}$, the thinning factor, carried forward
step by step so a varying temperature accumulates correctly.

**Settled ambiguity: the corrosion prefactor is a decade out as printed.**
$A = 5.87\cdot10^{-7}$ as printed does not reproduce Fig. 4, which plots this
very equation; $5.87\cdot10^{-8}$ does, to a mean error of **0.0055** across
483 digitised points on five curves. The figure's value is the default and
both are exposed by name
([`corrosion.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/corrosion.rs#L36-L61)).
One factor of ten reconciling five curves over 1000 °C is a typo, not a
modelling choice.

**The code walk.**

*In words, read from the source by hand:*

<!-- walk-in-words: from=crates/boon-lay/src/fuel_failure/history.rs::pressure_vessel_failure_at to=crates/boon-lay/src/fuel_failure/stress.rs::induced_stress_with_thinning_factor -->
1. [`AccidentHistory::pressure_vessel_failure_at`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#@@L:crates/boon-lay/src/fuel_failure/history.rs:fn=pressure_vessel_failure_at@@) → `pressure_at` (step 1)
2. → [`induced_stress_with_thinning_factor`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/stress.rs#@@L:crates/boon-lay/src/fuel_failure/stress.rs:fn=induced_stress_with_thinning_factor@@) → [`SicLayer::mean_radius`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/geometry.rs#@@L:crates/boon-lay/src/fuel_failure/geometry.rs:fn=mean_radius@@), [`SicLayer::initial_thickness`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/geometry.rs#@@L:crates/boon-lay/src/fuel_failure/geometry.rs:fn=initial_thickness@@).
3. The thinning factor comes from `AccidentHistory::step` → [`advance_thinning_factor`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/corrosion.rs#@@L:crates/boon-lay/src/fuel_failure/corrosion.rs:fn=advance_thinning_factor@@) → [`corrosion_rate`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/corrosion.rs#@@L:crates/boon-lay/src/fuel_failure/corrosion.rs:fn=corrosion_rate@@).
<!-- /walk-in-words -->

*Generated by rust-analyzer (`kovan-cli code-walk`); a hop it cannot follow is marked, and a hop filled by hand is labelled:*

<!-- code-walk: from=crates/boon-lay/src/fuel_failure/history.rs::pressure_vessel_failure_at to=crates/boon-lay/src/fuel_failure/stress.rs::induced_stress_with_thinning_factor -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `history.rs::AccidentHistory::pressure_vessel_failure_at` to `stress.rs::induced_stress_with_thinning_factor`: 1 hop, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`history.rs::AccidentHistory::pressure_vessel_failure_at`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L509) — `φ₁(t, T_m)` for a given elapsed accident time and thinning factor — the full pressure-vessel chain evaluated at one instant.

<!-- snippet-check: crates/boon-lay/src/fuel_failure/history.rs:509 fn pressure_vessel_failure_at -->
<!-- snippet-check: crates/boon-lay/src/fuel_failure/history.rs:516 induced_stress_with_thinning_factor -->

```rust,ignore
{{#include ../../../../../crates/boon-lay/src/fuel_failure/history.rs:509:517}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`stress.rs::induced_stress_with_thinning_factor`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/stress.rs#L107) · called at [L516](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L516) — Eq (2) written against a **carried** `FKOR` rather than an elapsed time (pages -484-, -492-):

<!-- snippet-check: crates/boon-lay/src/fuel_failure/stress.rs:107 fn induced_stress_with_thinning_factor -->

```rust,ignore
{{#include ../../../../../crates/boon-lay/src/fuel_failure/stress.rs:107:115}}
```
<!-- /code-walk -->


```rust,ignore
{{#include ../../../src/fuel_failure/stress.rs:46:56}}
```

**The check.** `the_two_routes_to_the_stress_agree_exactly` pins the
time-based and the thinning-factor routes against each other, and
`stress_rises_monotonically_as_the_layer_thins` the direction.

**Predict.** A layer half as thick at the same pressure: twice the stress?

---

## Step 3. Will the SiC hold? The Weibull law

**The question.** SiC is a ceramic. Two shells made the same way break at
different stresses. How is that spread described?

**The shortest answer.** A Weibull distribution of strength: weakest-link
statistics. At a given stress, a known fraction of the population is weaker
than that, and those particles fail.

**The formula** (Eq (1)):

$$\phi_1 = 1 - \exp\left[-\ln 2\left(\frac{\sigma_t}{\sigma_o}\right)^{m}\right].$$

**The $\ln 2$ is load-bearing.** It makes $\sigma_o$ the **median** strength:
at $\sigma_t = \sigma_o$ exactly half the particles fail. A textbook Weibull
uses the *characteristic* strength (63.2 % fail there). Swapping one for the
other shifts the strength scale by $(\ln2)^{1/m}$, about 4 % at $m = 8$,
raises no error, and moves the answer in the flattering direction. The test
`median_is_the_scale_parameter` pins it.

Irradiation weakens the SiC and widens the spread (Eqs (8a)–(9b)):
$\sigma_o = \sigma_{oo}(1 - \Gamma/\Gamma_s)$ and
$m_o = m_{oo}(1 - \Gamma/\Gamma_m)$, with floors of 196 MPa and 2, and
$\log_{10}\Gamma_s = 0.556 + 650/T_B$, $\log_{10}\Gamma_m = 0.394 + 650/T_B$
([`strength.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/strength.rs#L42-L104)).

**The illustration** (JavaScript; move the stress and the modulus, and watch
the failed fraction and a population of 400 particles):

<div class="bl-anim" data-anim="weibull"></div>

**The code walk.**

*In words, read from the source by hand:*

<!-- walk-in-words: from=crates/boon-lay/src/fuel_failure/history.rs::pressure_vessel_failure_at to=crates/boon-lay/src/fuel_failure/weibull.rs::weibull_failure_fraction -->
1. [`AccidentHistory::pressure_vessel_failure_at`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#@@L:crates/boon-lay/src/fuel_failure/history.rs:fn=pressure_vessel_failure_at@@): picks the modulus — **branch** `GrainBoundaryCorrosion::Disabled` (the default): the irradiated modulus; **branch** `Enabled`: [`corroded_weibull_modulus`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/grain_boundary.rs#@@L:crates/boon-lay/src/fuel_failure/grain_boundary.rs:fn=corroded_weibull_modulus@@), Eqs (10b)/(10c);
2. → [`weibull_failure_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/weibull.rs#@@L:crates/boon-lay/src/fuel_failure/weibull.rs:fn=weibull_failure_fraction@@).
3. The irradiated $\sigma_o$ and $m$ were set when the particle was built, e.g. [`htr10::particle_with`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/htr10/mod.rs#@@L:crates/boon-lay/src/fuel_failure/htr10/mod.rs:fn=particle_with@@) → [`irradiated_strength`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/strength.rs#@@L:crates/boon-lay/src/fuel_failure/strength.rs:fn=irradiated_strength@@), [`irradiated_weibull_modulus`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/strength.rs#@@L:crates/boon-lay/src/fuel_failure/strength.rs:fn=irradiated_weibull_modulus@@).
<!-- /walk-in-words -->

*Generated by rust-analyzer (`kovan-cli code-walk`); a hop it cannot follow is marked, and a hop filled by hand is labelled:*

<!-- code-walk: from=crates/boon-lay/src/fuel_failure/history.rs::pressure_vessel_failure_at to=crates/boon-lay/src/fuel_failure/weibull.rs::weibull_failure_fraction -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `history.rs::AccidentHistory::pressure_vessel_failure_at` to `weibull.rs::weibull_failure_fraction`: 1 hop, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`history.rs::AccidentHistory::pressure_vessel_failure_at`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L509) — `φ₁(t, T_m)` for a given elapsed accident time and thinning factor — the full pressure-vessel chain evaluated at one instant.

<!-- snippet-check: crates/boon-lay/src/fuel_failure/history.rs:509 fn pressure_vessel_failure_at -->
<!-- snippet-check: crates/boon-lay/src/fuel_failure/history.rs:524 weibull_failure_fraction -->

```rust,ignore
{{#include ../../../../../crates/boon-lay/src/fuel_failure/history.rs:509:525}}
```

**2.** → [`weibull.rs::weibull_failure_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/weibull.rs#L49) · called at [L524](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L524) — **Eq (1)** — the fraction of particles failed by pressure-vessel overstress (page -483-, attributed to Nabielek 1984).

<!-- snippet-check: crates/boon-lay/src/fuel_failure/weibull.rs:49 fn weibull_failure_fraction -->

```rust,ignore
{{#include ../../../../../crates/boon-lay/src/fuel_failure/weibull.rs:49:62}}
```
<!-- /code-walk -->

**Grain-boundary corrosion is off by default, and the crate says why.** The
report itself specifies it off for normal use, so this is the source model's
default, not physics switched off for convenience. It is a visible enum at
every call site
([reasoning](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/grain_boundary.rs#L47-L53)).

**The check: Table 1, 16 of 16.** Eqs (8a)/(9a) reproduce all sixteen of the
report's calculated irradiated strengths and moduli, **only** if $T_B$ is in
kelvin (0 of 16 in °C), although the symbol list prints °C. That is how the
unit was settled. Table 2 is reproduced as a second closed-form check
(`table_1_is_reproduced_exactly`, recorded 2026-09-24).

**Predict.** If the SiC is irradiated, both $\sigma_o$ and $m$ fall. Which
way does each move $\phi_1$ at a fixed stress below the median?

---

## Step 4. Above 2000 °C: the SiC decomposes

**The question.** At very high temperature SiC turns into gaseous silicon
and graphite. Then the layer is not a pressure vessel at all.

**The shortest answer.** A second failure population, $\phi_2$, driven by
an Arrhenius "action integral" that remembers the whole temperature history.

**The formulas** (Eqs (11)–(14b)):

$$\zeta = \int k(T)\ \mathrm dt,\quad k = \frac{375}{d_o}e^{-556\ 000/RT},\quad \phi_2 = 1 - e^{-\alpha\zeta^\beta},$$

with $\alpha = 10^{-4}$, $\beta = 4$ for particles in a sphere (Eq (14b),
used here) or $\alpha = \ln2$, $\beta = 0.88$ for loose particles (14a).
For the units to balance, the 375 must be a velocity in m/s; the report
never says so.

**$\zeta$ carries the history; $\phi_2$ is not accumulated.** $\phi_1$ *is*
accumulated from positive increments (a burst particle does not un-burst
when the core cools). $\phi_2$ is read straight off $\zeta$. On a monotone
heat-up the two procedures agree. On a history that rises and then cools,
which every real transient does, summing increments of $\phi_2$ would be
wrong.

**The illustration** (JavaScript; a heat-up and cool-down; compare reading
$\phi_2$ off $\zeta$ with summing increments of a $\phi_2$ re-evaluated at
each step's temperature):

<div class="bl-anim" data-anim="decomp"></div>

**The check: none in the report.** No figure or table in the report checks
Eqs (11)–(14b), and the data that fixed $Q$ (Benz 1982) is not available.
Fig. 6 does rule out the loose-particle calibration (14a): it would put a
floor at $\phi_2 = 4.9\cdot10^{-3}$ under all eight of Fig. 6's curves, which
the figure does not show. **This part of the model is unverified, and the
lesson says so.**

---

## Step 5. Putting it together: the accident history

**The question.** A real accident is a temperature that rises and falls
over days. How are the pieces stepped?

**The shortest answer.** Interval by interval, at each interval's mean
temperature: thin the SiC, grow $\zeta$, add any positive increase of
$\phi_1$, read $\phi_2$, and combine.

**The formula.** A particle survives only if it survives every mechanism, so
the **survival** probabilities multiply:

$$\phi_{\text{total}} = 1 - (1 - \phi_o)(1 - \phi_1)(1 - \phi_2).$$

Adding the fractions would count a particle failed twice and could exceed 1.
$\phi_o$, the as-manufactured defects, is **an input**: the report offers
$6\cdot10^{-5}$ as a target, which the crate exposes as a named constant and
never as a default.

**The code walk.**

*In words, read from the source by hand:*

<!-- walk-in-words: from=crates/boon-lay/src/fuel_failure/history.rs::run to=crates/boon-lay/src/fuel_failure/mod.rs::total_failure_fraction -->
1. [`AccidentHistory::run`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#@@L:crates/boon-lay/src/fuel_failure/history.rs:fn=run@@) / [`run_isothermal`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#@@L:crates/boon-lay/src/fuel_failure/history.rs:fn=run_isothermal@@) → for each interval:
2. → [`AccidentHistory::step`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#@@L:crates/boon-lay/src/fuel_failure/history.rs:fn=step@@):
   - `advance_thinning_factor` (step 2);
   - `pressure_vessel_failure_at` at **both ends at this interval's mean temperature**, keep the increase if positive (steps 1–3);
   - [`advance_action_integral`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/decomposition.rs#@@L:crates/boon-lay/src/fuel_failure/decomposition.rs:fn=advance_action_integral@@) → [`thermal_decomposition_failure_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/decomposition.rs#@@L:crates/boon-lay/src/fuel_failure/decomposition.rs:fn=thermal_decomposition_failure_fraction@@) (step 4);
   - **branch** grain-boundary corrosion enabled → [`advance_grain_boundary_exposure`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/grain_boundary.rs#@@L:crates/boon-lay/src/fuel_failure/grain_boundary.rs:fn=advance_grain_boundary_exposure@@);
   - → [`total_failure_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/mod.rs#@@L:crates/boon-lay/src/fuel_failure/mod.rs:fn=total_failure_fraction@@).
<!-- /walk-in-words -->

*Generated by rust-analyzer (`kovan-cli code-walk`); a hop it cannot follow is marked, and a hop filled by hand is labelled:*

<!-- code-walk: from=crates/boon-lay/src/fuel_failure/history.rs::run to=crates/boon-lay/src/fuel_failure/mod.rs::total_failure_fraction -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `history.rs::AccidentHistory::run` to `mod.rs::total_failure_fraction`: 2 hops, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`history.rs::AccidentHistory::run`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L609) — Walk a whole temperature history and return the final state.

<!-- snippet-check: crates/boon-lay/src/fuel_failure/history.rs:609 fn run -->
<!-- snippet-check: crates/boon-lay/src/fuel_failure/history.rs:611 step -->

```rust,ignore
{{#include ../../../../../crates/boon-lay/src/fuel_failure/history.rs:609:612}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`history.rs::AccidentHistory::step`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L552) · called at [L611](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L611) — Advance one interval (pages -482-, -483-, -492-, -496-) and return the new state.

<!-- snippet-check: crates/boon-lay/src/fuel_failure/history.rs:552 fn step -->
<!-- snippet-check: crates/boon-lay/src/fuel_failure/history.rs:582 total_failure_fraction -->

```rust,ignore
{{#include ../../../../../crates/boon-lay/src/fuel_failure/history.rs:552:552}}
    // …
{{#include ../../../../../crates/boon-lay/src/fuel_failure/history.rs:580:583}}
    // … (the rest of the function: follow the link above)
```

**3.** → [`mod.rs::total_failure_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/mod.rs#L249) · called at [L582](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L582) — The combination of the three failure populations (page -480-).

<!-- snippet-check: crates/boon-lay/src/fuel_failure/mod.rs:249 fn total_failure_fraction -->

```rust,ignore
{{#include ../../../../../crates/boon-lay/src/fuel_failure/mod.rs:249:258}}
```

Unresolved calls inside the functions on this chain:

- in [`history.rs::AccidentHistory::step`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L552):
  - UNRESOLVED(closure): `rate` at [L601](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L601) (→ [`crates/boon-lay/src/fuel_failure/history.rs:585`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L585)) — call through a closure or fn-typed binding `rate`
  - UNRESOLVED(closure): `rate` at [L602](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L602) (→ [`crates/boon-lay/src/fuel_failure/history.rs:585`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L585)) — call through a closure or fn-typed binding `rate`
  - UNRESOLVED(closure): `rate` at [L603](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L603) (→ [`crates/boon-lay/src/fuel_failure/history.rs:585`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L585)) — call through a closure or fn-typed binding `rate`
<!-- /code-walk -->

**The checks.**

- **Step-size independence** (the report's own falsifiable claim, p. -482-):
  300 h at 1600 °C in 1, 12, 300 and 3000 steps gives $\phi_{\text{total}}$
  agreeing to **2·10⁻¹² relative** (recorded 2026-09-24). A driver that
  recomputed FKOR or $\zeta$ from total time, or accumulated $\phi_2$, would
  fail it.
- **Fig. 6, eight SiC varieties at 1600 °C:** inverting Eq (1) on each
  digitised curve must return one common stress. Rank order 8/8; the common
  stress agrees to 9.0–11.2 % relative s.d.; but the per-curve residual runs
  **systematically** with $m_{oo}$ from −0.37 to +0.39 decades. Two
  hypotheses were tested and neither removes it. Recorded, not tuned.
- **Fig. 7, the FRJ2-K11/03 heating test** (the only case in the report with
  a complete input set; 100 h at 1400 °C, 100 h at 1500 °C, then 1600 °C to
  1000 h). With one free scale (the unstated particle geometry) fixed over
  0–300 h, the chain's stress matches the report's curve to **4.7 %**
  relative s.d. through all three stages. **Then it drifts**, to a factor
  **1.87** by 977 h. The report's curve keeps rising as about $\sqrt t$ after
  the Booth release has saturated in the chain, and the printed equations do
  not say why (re-digitised and recorded 2026-09-28, in
  [`history.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L127-L230)).
- **Fig. 7 against the measured ⁸⁵Kr** (this compares the *report's* curves
  with the experiment, not this code): the without-corrosion curve
  under-predicts by **+0.50** decades, the with-corrosion curve
  over-predicts by **1.61**. That reproduces the report's own reading, and it
  is the one place where measured data enters this rung.

**Re-run 2026-10-04** (this track's run, `develop` atop `5e802df3a4`,
`--release`): every fuel-failure test passed, including
`the_step_length_does_not_matter_at_constant_temperature`, the Fig. 6 and
Fig. 7 tests, which pin the numbers above inside stated bands. These tests
assert rather than print, so the re-run confirms the recorded values hold
within those bands; it does not re-derive them digit by digit. The two
disagreements (Fig. 6's trend with $m_{oo}$, Fig. 7's late drift) are open
as gh:#295.

**Predict.** A transient holds at 1600 °C for 200 h. Which of $\phi_1$ and
$\phi_2$ do you expect to dominate? At 2200 °C?

---

## Step 6. The HTR-10 numbers, and what they are not

**The question.** What does boon-lay fuel failure say for HTR-10?

**The shortest answer.** Something, under accident conditions; almost
nothing about normal operation; and none of it is validated.

PANAMA-I was built for German TRISO fuel and claims good agreement over
1600–2500 °C. HTR-10's fuel is German-lineage, which makes applying it
defensible, but it is an **extrapolation**. Two inputs are not published for
HTR-10 and are taken by name from the report's HTR-Module case: the SiC
strength ($\sigma_{oo} = 834$ MPa, $m_{oo} = 8.02$) and the fast fluence
($1.4\cdot10^{25}$ m⁻²), as `STAND_IN_*` constants
([`htr10/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/htr10/mod.rs#@@L:crates/boon-lay/src/fuel_failure/htr10/mod.rs:const=KERNEL_RADIUS_UM@@)).
$T_B$ is an input too: HTR-10 publishes a maximum fuel temperature but no
average, and 776 °C is the HTR-Module average.

As recorded 2026-09-24 (`T_B = 776 °C`, 200 h isothermal, hourly steps):

| accident $T$ | $\phi_1$ | $\phi_2$ (14b) |
|---|---|---|
| 1200 °C | 4.53·10⁻⁹ | ~0 |
| 1600 °C | 3.10·10⁻⁵ | 3.4·10⁻¹⁵ |
| 2000 °C | 4.79·10⁻³ | 2.78·10⁻⁴ |
| 2200 °C | 6.07·10⁻² | 0.977 |

**Re-measured 2026-10-04** (this track's run, `develop` atop `5e802df3a4`,
`--release`; the accident test now prints its table): $\phi_1$ =
4.528·10⁻⁹, 3.103·10⁻⁵, 4.794·10⁻³, 6.069·10⁻² and $\phi_2$ = 0, 3.442·10⁻¹⁵,
2.779·10⁻⁴, 0.9770 at 1200, 1600, 2000, 2200 °C; FKOR 1.0005 to 1.1953.
**Unchanged** to the recorded three figures, so the 2026-09-24 table stands.
At the end of irradiation: 2.776·10⁻¹⁵ (700 °C), 1.214·10⁻¹² (776 °C),
4.988·10⁻⁹ (900 °C), 1.577·10⁻⁶ (1000 °C), also unchanged.

What the table shows is the model's own structure: $\phi_2$ overtakes
$\phi_1$ between 2000 and 2200 °C, as the report says it should. The
1600 °C value happens to land next to an old $3\cdot10^{-5}$ placeholder;
**that is a coincidence of two different quantities, not agreement.**

At the end of normal irradiation $\phi_1$ is 10⁻¹⁵ to 10⁻⁶ over 700–1000 °C.
That does **not** make it the in-service failure fraction TRISO-ATOPS needs
for normal operation: in-service failure is a manufacturing and irradiation
population, which the report takes as an input. The extended page
[HTR-10](../../deep-dives/triso-atops/htr10.html) has the full story, including the German qualification
data the crate stores and does not yet check (gh:#383).

---

## Step 7. Which particles count as "failed"? The TRISO-ATOPS classes

**The question.** Release (rung 7) needs more than one number. A particle
whose SiC cracked but whose IPyC held behaves differently from one with a
bare kernel.

**The shortest answer.** TRISO-ATOPS describes the fuel with four
fractions, and boon-lay fuel failure plugs into exactly one of them.

| Field | Population | Who supplies it |
|---|---|---|
| `f_hm` | heavy-metal contamination outside the coatings | input (fuel qualification) |
| `f_sic` | as-manufactured defective SiC | input |
| `f_inc` | in-service failure (full) | input, **or** `1 − (1 − φ₁)(1 − φ₂)` from an accident history |
| `f_inc_sic` | in-service SiC-only failure | input; $\phi_2$ is deliberately **not** routed here |

**The code walk.**

*In words, read from the source by hand:*

<!-- walk-in-words: from=crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::with_fuel_failure_incremental to=crates/boon-lay/src/fuel_failure/history.rs::in_service_failure_fraction -->
1. [`FailureFractions::with_fuel_failure_incremental`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs#@@L:crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs:fn=with_fuel_failure_incremental@@): replaces `f_inc`, leaves the other three alone;
2. → [`FailureProgress::in_service_failure_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L429-L441).
<!-- /walk-in-words -->

*Generated by rust-analyzer (`kovan-cli code-walk`); a hop it cannot follow is marked, and a hop filled by hand is labelled:*

<!-- code-walk: from=crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::with_fuel_failure_incremental to=crates/boon-lay/src/fuel_failure/history.rs::in_service_failure_fraction -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `source_terms.rs::FailureFractions::with_fuel_failure_incremental` to `history.rs::FailureProgress::in_service_failure_fraction`: 1 hop, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`source_terms.rs::FailureFractions::with_fuel_failure_incremental`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs#L106) — Replace `incremental` with a value computed by **boon-lay fuel failure** (`crate::fuel_failure`, boon-lay's own implementation of the PANAMA-I formulas, not the PANAMA code), leaving the other three untouched.

<!-- snippet-check: crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs:106 fn with_fuel_failure_incremental -->
<!-- snippet-check: crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs:112 in_service_failure_fraction -->

```rust,ignore
{{#include ../../../../../crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs:106:113}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`history.rs::FailureProgress::in_service_failure_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L441) · called at [L112](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs#L112) — The **in-service** failure fraction: `φ₁` and `φ₂` combined, with the as-manufactured population excluded.

<!-- snippet-check: crates/boon-lay/src/fuel_failure/history.rs:441 fn in_service_failure_fraction -->

```rust,ignore
{{#include ../../../../../crates/boon-lay/src/fuel_failure/history.rs:441:447}}
```
<!-- /code-walk -->

It is an *added* route: a run that sets all four by hand
is unaffected, and a test pins that.

**What is not here: oxidation failure.** In air ingress, measured failures
come from SiC oxidation (the KORA tests: about 20 of 16 400 particles,
1.2·10⁻³, after 140 h at 1400 °C in air; IAEA-TECDOC-978 Table 5-7 =
Kugeler 2017 Table 9). **boon-lay has no model of that.** `sembawang`'s
bounding air-ingress case adds the KORA number by hand as an empirical term
([`htr10_air_ingress_kora_bound.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/sembawang/examples/htr10_air_ingress_kora_bound.rs),
gh:#435, #438). A mechanistic oxidation-attack model is the open epic
gh:#441; [rung 6](./chemistry.md) has the pieces that exist.

## Deliberate liberties on this rung

- **Thin shell only**, as in the report.
- **Kr release = failure.** Fig. 7 puts a failure fraction and a ⁸⁵Kr release
  fraction on one axis; the comparison inherits that assumption.
- **HTR-10 inputs taken from another reactor** (`STAND_IN_*`, $T_B$).
- **Grain-boundary corrosion off** by the report's default.
- **No oxidation failure** (KORA, gh:#441).

## Use, modify, create

- **Use:** `cargo test --release -p boon-lay --lib the_accident_sweep -- --nocapture`
  prints the HTR-10 accident table.
- **Modify:** set `T_B` to 700 °C and to 900 °C in that test. How far does
  the 1600 °C value move? Write your guess first.
- **Create:** build a heat-up to 1620 °C over 50 h followed by a cool-down
  over 100 h as a list of `AccidentStep`s, and compare $\phi_{\text{total}}$
  with a flat 150 h hold at 1620 °C.

**Next:** [rung 6, air and steam](./chemistry.md).
