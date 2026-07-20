# TRISO-ATOPS release model — derivation, mapped onto the Rust port

> ⚠️ **Unverified until validated.** All code in this workspace is **unverified
> and untrusted** unless a specific V&V case demonstrates otherwise. Not for
> nuclear facility operation, reactor control, safety-critical, or licensing
> decisions.

This is the **Rust-port view** of the TRISO-ATOPS derivation. It mirrors the
step-by-step physics build-up in the crate-root
[`TRISO_ATOPS_DERIVATION.md`](../TRISO_ATOPS_DERIVATION.md) — which is the
**Python-model view**, with every step tied to the upstream Python function —
and points each step at the `boon_lay::triso_atops_fork` module, type, or
function that implements it. Read the crate-root file for the full physics
derivation (equations, limits, references); read this file to find *where in the
Rust code* each step lives. The same narrative renders in the module-level
rustdoc (`cargo doc -p boon-lay --no-deps`).

For the Python→Rust module map, provenance, units convention, and verification
results, see the companion [`triso-atops-fork.md`](triso-atops-fork.md).

## Derivation → Rust map

| Derivation step (crate-root md) | Physics | Rust location |
|---|---|---|
| **1. First principles** — $\partial C/\partial t = D\nabla^2 C$, $dN/dt = -\lambda N$, $\lambda = \ln 2 / t_{1/2}$ | diffusion + decay laws | `nuclide_model::TrisoAtopsNuclide::decay_constant` (the $\lambda$); the diffusion law is the premise of `release_models` |
| **2. Equivalent sphere** — Booth idealisation; group dispatch; $a_\text{booth} = \sqrt{2\,a_\text{grain}\,r}$ | reduce multi-shell TRISO to one sphere per group | `nuclide_model::ElementGroup` (group partition); `release_models::rb_fail` (computes `a_booth`, dispatches) |
| **3. Effective coefficient `D'`** — $D' = D/a^2$; Arrhenius $D(T) = D_0 e^{-Q/RT}$; two-term branches; valid-range clamp | temperature-dependent diffusion | `diffusion::diffusion_coefficient`, `diffusion::diffusion_coefficient_sic_ag`, `diffusion::GAS_CONSTANT_J_PER_MOL_K` |
| **4. Stable-species fractional release** — $f = 1 - \frac{6}{\pi^2}\sum n^{-2} e^{-n^2\pi^2 D' t}$; short-time $6\sqrt{D't/\pi} - 3D't$ | full sphere release series | `release_models::steady_state::booth_longlived` |
| **5a. Short-lived R/B** — $\langle R/B\rangle = \frac{3}{\mu}(\coth\mu - 1/\mu)$, $\mu = \sqrt{\lambda a^2/D}$ | steady release-to-birth with decay | `release_models::steady_state::booth_shortlived_fast_diffuse` |
| **5b. Silver breakthrough** — Daynes–Barrer membrane time-lag $\times\,3/r$ | permeation through SiC | `release_models::steady_state::breakthrough_model` (barrier `D` from `diffusion_coefficient_sic_ag`) |
| **5c. Volatile empirical R/B** — $\exp(n\ln\frac{1}{\lambda} + B/T + C)$ | noble-gas / halogen fit | `release_models::steady_state::rb_fail_noble_gases` |
| **5d. Graphite attenuation** — $Af = 1/(1 - S)$, $S = \sum_\text{odd}\frac{4}{i\pi}\sin\frac{i\pi}{2} e^{-(i\pi)^2 D_\text{graph} t/4a^2}$ | graphite hold-up factor | `release_models::steady_state::attenuation_factor` |
| **6(i) R/B dispatch** | group → model | `release_models::rb_fail` |
| **6(ii) Release rate `R`** — birth rate $A$ (short-lived) or $A/(1 - e^{-\lambda t})$ (long-lived) $\times$ failure fraction $\times \langle R/B\rangle$ | inventory → release rate | `activities::source_terms::release_rate`, `activities::FailureFractions` |
| **6(iii) Source `S` + graphite `G`** — $S = R/Af$, $G = R(1 - 1/Af)(1 - e^{-\lambda t})/\lambda$ | split release | `activities::source_terms::base_activities` |
| **6(iv) Loop pools `C`, `P`, `HPS`** — $\beta = \lambda + k_\text{plate} + k_\text{clean}$ balances | primary-loop activity | `activities::coolant_activity::{circulating, plate_out, clean_up}` (+ `*_steadystate`) |
| **6(v) Curie report** — $\times\,\lambda/3.7\times10^{10}$ | Bq/atoms → Ci | `activities::{becquerels_from_curies, curies_from_becquerels, activity_from_atom_count}`; `normal_operation::NodalActivities::to_curies` |
| **whole node chain (i)–(v)** | one nuclide, one node | `normal_operation::normal_operation_node` |
| **7. Transient** — replace $Dt \to \int D\,dt'$, $D't \to \int D'\,dt'$ | accident (time-varying $T$) | `diffusion::integrate_diffusion_over_time`; `release_models::transient::{booth_transient, breakthrough_model_transient, rf_graph}`; `release_models::release_fraction_transient` |

## Types that carry the physics

- `DecayConstant` (= `uom` `Frequency`, s⁻¹) — the $\lambda$ of Step 1.
- `ReleaseFraction` (= `uom` `Ratio`, `[0,1]`) — every $\langle R/B\rangle$ and
  release fraction of Steps 4–7.
- `diffusion::KernelGraphiteDiffusion` — the $(D, D_\text{graph})$ pair of
  Step 3; `Area` carries the transient $\int D\,dt'$ of Step 7.
- `activities::Activity` (= `uom` `Frequency`, Bq) — activity as "decays per
  second", the anchor for $A = \lambda N$ (Step 6v).
- `normal_operation::{PlantConstants, NodeState, NodalActivities, NodalActivitiesCurie}`
  — the inputs and outputs of the assembled node chain (Step 6).

## Quirks preserved by the port

The port reproduces the upstream behaviour exactly (a *verification* port matches
its reference, quirks included). The ones flagged during this derivation — the
`clean_up_steadystate` `HPS_parent` omission, the Booth short-time expansion
being a check-only approximation, temperature clamping vs. extrapolation, and the
nominal `1e-5` / `1e8` placeholders — are listed in §8 of the crate-root
[`TRISO_ATOPS_DERIVATION.md`](../TRISO_ATOPS_DERIVATION.md#8-upstream-quirks-and-approximations-flagged-during-this-derivation)
and, where they touch a specific function, in that function's rustdoc (e.g.
`coolant_activity::clean_up_steadystate`).

## References

See §9 of the crate-root [`TRISO_ATOPS_DERIVATION.md`](../TRISO_ATOPS_DERIVATION.md#9-references):
Anderson et al. 1989 (NP-MHTGR, the source of the equations); IAEA Live Chart of
Nuclides (half-lives); Booth 1957 (equivalent sphere); Crank 1975 (sphere and
membrane solutions); the TRISO-ATOPS User Manual.
