# V&V status: what has been checked, against what

**Verification** asks whether the code computes what it means to compute.
**Validation** asks whether that matches the physical world. Nothing in
`boon-lay` is **validated** against measured release or failure data. What
follows is verification, plus one comparison with a published workflow.

Every number below names the file that records it. Numbers are quoted as
recorded, with the date they were taken.

## Lagrangian engine against closed-form diffusion

**CRP-6 Case 1, bare kernel, Walk-on-Spheres against Crank.** Cs release from
a 212.5 µm-radius UO₂ kernel at 200 h, uniform start, perfect-sink surface.
`N = 40 000` atoms, seed `0x0C1A5EED`. Pass if `|MC − Crank| < 0.02` (the 1σ
binomial error is about 0.0025). Measured 2026-07-23
([record](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/crp6_case1_kernel_release_vs_crank.md)):

| Case | T | MC | Crank | abs. error |
|---|---|---|---|---|
| 1a | 1200 °C | 0.5332 | 0.5337 | 0.0006 |
| 1b | 1600 °C | 1.0000 | 1.0000 | 0.0000 |

This is a **single-layer** case. The multilayer release through the coatings
has no full CRP-6 record yet.

**Interface rule gives a uniform equilibrium.** Across a tenfold diffusivity
contrast, the time a walker spends in the inner region should equal its
volume fraction, 0.1250. Measured 0.1216 (error 0.0034, threshold 0.02),
`3·10⁶` steps
([record](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/interface_uniform_equilibrium_density.md)).
This is the check that the `D`-linear transmission rule, not a `√D` rule, is
the right one for this walk.

## TRISO-ATOPS port

**Against its own analytical limits.** The Booth functions `booth_longlived`
and `booth_transient` equal the Crank series at CRP-6 Case 1a/1b to
`2.2·10⁻¹⁶` (1a: 0.5337290191), added 2026-09-29. Together with the row above
this closes the loop on one benchmark: random walk, closed form and port
agree. The same test measured the `1.216·10⁻⁴` truncation floor at small
`D't` (gh:#385)
([test](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/tests/triso_atops_booth_vs_crp6.rs)).

**Code to code against upstream Python** (INL TRISO-ATOPS at `de374c8`). The
reference values come from *running* upstream, not from transcribing its
manual. Three passes, the last on 2026-09-21: **14 130 cases in 51 tests**.
The worst group is `accident_case.nodal_kernel` at `2.44·10⁻⁹` relative (180
cases). That is above the `1·10⁻⁹` group tolerance and is admitted only by a
per-case widening for an ill-conditioned Ag-110m row. Methodology, results,
and the two deliberate divergences from upstream are in
[`docs/triso-atops-code-to-code.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/triso-atops-code-to-code.md)
(summary in the
[README](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/README.md)).

Porting turned up **ten upstream defects**, listed in
[`docs/triso-atops-fork.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/triso-atops-fork.md)
and filed for upstream as gh:#219 (not yet reported to INL). One example:
upstream assigns `parent_decay` with `==` instead of `=`, so its short-lived
parent test never runs. Where the port diverges, the divergence can be
selected, and is not silent.

**The MHTGR workflow of Stoyer et al. (2026), gh:#413.** The paper's two
MHTGR cases, run through the port and through upstream on the same cited
inputs
([record](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/mhtgr_stoyer/README.md)):

- Port against upstream: worst relative difference `5.6·10⁻¹²` in normal
  operation.
- Against the paper's normal-operation tables, with `k_plate` as printed
  (`7.5·10⁻⁵ /s`): graphite agrees 40/40, but circulating only 11/64 and HPS
  only 10/24.
- With `k_plate = 7.5·10⁻⁴ /s` (upstream's own default): every printed value
  within 2 %. On this evidence the table's value looks like a misprint. The
  committed inputs keep the printed value, and the other run is labelled a
  diagnostic.
- Accident (heat-up) releases on the maintainer's digitisation of Fig. 5,
  densely resampled (reported 2026-09-30): final over paper is about 0.90
  across all nuclides (Case A range 0.84–0.94). That common factor of about
  0.90 is **not diagnosed**. Case B has outliers (Cs-134 0.67, Kr-85 0.68).

## boon-lay fuel failure against the PANAMA-I report

These check the transcription against the report's **printed output**. They
are not a comparison with the PANAMA code, which this project does not have.
All are recorded in
[`docs/panama-i-units-and-open-questions.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/panama-i-units-and-open-questions.md).

- **Table 1** (irradiated SiC strength and modulus): reproduced 16/16, with
  `T_B` in kelvin. 0/16 in °C. This is how the unit was settled.
- **Table 2**, a second closed-form check at a different fluence and
  temperature: reproduced.
- **Fig. 7**, the staged heating test FRJ2-K11/03, the only case in the report
  with a complete input set. The comparison uses **one free scale** (the
  particle geometry, which the report does not state). With that scale fixed
  over 0–300 h, the stress agrees within 4.7 % relative s.d. through all three
  temperature stages. After that it drifts, to a factor **1.87** by 977 h.
  The drift is not explained.
- **Fig. 6** (eight SiC varieties): the digitised frame is **unverified**, so
  results against it are not quoted as verification.
- **Thermal decomposition, Eqs (11)–(14b):** no figure or table in the report
  checks them. The checks are internal and algebraic only.

## Which numbers to quote

- For the **Lagrangian engine**: CRP-6 Case 1 and the interface equilibrium.
  Both are verification against exact solutions.
- For the **TRISO-ATOPS port**: the code-to-code result (it is a port, so
  upstream is the specification), and the MHTGR workflow with its undiagnosed
  0.90 factor stated alongside.
- For **boon-lay fuel failure**: Table 1 and Fig. 7 *with* the drift. Do not
  quote any HTR-10 number from it as validated. It is an extrapolation
  ([fuel failure](./fuel-failure.md)).
