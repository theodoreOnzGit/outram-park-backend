//! Reusable **delayed-neutron layer** — a reduced point-kinetics precursor
//! bank: five delayed-neutron groups whose precursor concentrations are
//! advanced by **direct implicit (backward-Euler) time-stepping**, one
//! `O(1)`-cost, `O(1)`-memory update per timestep.
//!
//! # What this module is for
//!
//! The [`crate::nordheim_fuchs`] exact timestepper is a **prompt-only**
//! excursion model: it evolves reactor power and adiabatic fuel temperature
//! but carries **no delayed-neutron precursors**. Physically the delayed
//! neutrons are the reservoir that gives an operating reactor its inertia and
//! its long, controllable period: a chain that is *prompt* subcritical (net
//! reactivity `rho` below the delayed fraction `beta`) is held critical only
//! by neutrons emitted seconds-to-minutes later from decaying precursors.
//! Strip that reservoir out and a prompt-only model coupled to a thermal
//! feedback loop degenerates into a bang-bang relaxation oscillation — power
//! explodes whenever the prompt margin `rho - beta` goes positive and
//! collapses the instant it goes negative, driving the fuel temperature (hence
//! the reactivity) up and down without damping.
//!
//! This layer restores the delayed-neutron reservoir while **keeping
//! Nordheim-Fuchs for the prompt response**. It sits between the
//! prompt-excursion layer and the thermal-hydraulics layer as a
//! *delayed-neutron source* in a Lie-split point-kinetics update (see
//! "How to couple it" below).
//!
//! # The model it implements
//!
//! Point reactor kinetics in reactor-power form (power `P`, prompt neutron
//! generation time `Lambda`, net reactivity `rho`, delayed fraction
//! `beta = sum_i beta_i`):
//!
//! ```text
//!   dP/dt   = (rho - beta)/Lambda * P + S ,   S = sum_i lambda_i C_i
//!   dC_i/dt = beta_i/Lambda * P - lambda_i C_i
//! ```
//!
//! The **prompt** part `(rho - beta)/Lambda * P` (with adiabatic
//! fuel-temperature feedback folded into `rho`) is exactly what
//! [`crate::nordheim_fuchs::NordheimFuchsExactTimestepper::step`] advances.
//! This layer owns the **delayed** part: it integrates the precursor
//! concentrations `C_i` and returns the source `S = sum_i lambda_i C_i`.
//!
//! # How the precursors are integrated (implicit / backward-Euler)
//!
//! Each group's precursor concentration `C_i` obeys
//! `dC_i/dt = (beta_i / Lambda) * P - lambda_i C_i`. This layer advances the
//! five `C_i` directly in time with a **backward-Euler (implicit) step**,
//! holding the reactor power `P` constant across the step. Discretising the
//! ODE implicitly (`dC_i/dt approx (C_i^{n+1} - C_i^n)/dt`, decay term
//! evaluated at the new time `n+1`):
//!
//! ```text
//!   (C_i^{n+1} - C_i^n)/dt = (beta_i/Lambda) * P - lambda_i C_i^{n+1}
//! ```
//!
//! which rearranges to the closed-form per-group update actually used in
//! [`DelayedNeutronLayer::advance`]:
//!
//! ```text
//!   C_i^{n+1} = ( C_i^n + dt * (beta_i/Lambda) * P ) / ( 1 + dt * lambda_i )
//! ```
//!
//! The total delayed source is then `S = sum_i lambda_i C_i^{n+1}`, and over
//! the timestep `dt` it injects a delayed power increment
//! `Delta P_delayed = S * dt` into the balance.
//!
//! This is the **same implicit precursor stepping** the crate's coupled
//! [`crate::zero_power_prke::six_group_precursor_prke::SixGroupPRKE`] solver
//! uses (its `construct_coefficient_matrix` builds rows
//! `(1 + dt*lambda_i) C_i^{n+1} - (dt/Lambda) beta_i n^{n+1} = C_i^n`, i.e. the
//! same backward-Euler discretisation) — here decoupled from the prompt
//! neutron-population equation, because Nordheim-Fuchs owns the prompt term.
//! It is unconditionally stable at the always-on 1 ms GUI timestep and costs a
//! **fixed five multiply-adds per step with no history** — see the design note
//! below.
//!
//! ### Why direct integration replaced the transfer-function approach (op-e46.4)
//!
//! Earlier revisions modelled each group as a
//! `chem_eng…::TransferFnFirstOrder` first-order lag
//! `(beta_i/Lambda)/(tau_i s + 1)`, `tau_i = 1/lambda_i`. That is analytically
//! exact for a piecewise-constant input, but the transfer function accumulates
//! **one superposed response term per input change** and only prunes it after
//! `20*tau`. In the always-on 1 ms real-time loop, with the slowest group's
//! `tau_1 approx 80.8 s`, its buffer grows to `~1.6M` entries before clearing,
//! and the per-step summation over that buffer is `O(n)` — so per-step cost
//! grew without bound (measured `~49 us -> ~1.8 ms/step` from step 1k to 40k,
//! blowing the 1 ms budget ~28 s in). Direct implicit stepping holds only the
//! five `C_i` as state: **`O(1)` time and `O(1)` memory per step, forever**,
//! with no growing `Vec` and no dependence on `TransferFnFirstOrder`.
//!
//! At steady state the update fixed-point gives `C_i = (beta_i/Lambda) P /
//! lambda_i`, so `S_i = lambda_i C_i = (beta_i/Lambda) P`, hence
//! `S = (beta/Lambda) P` and the power equation forces `rho = 0` (delayed
//! critical) — the physically correct operating point, which the prompt-only
//! model could not reach (it sat at prompt critical, `rho = beta`, and rang).
//!
//! This is a deliberately reduced (pedagogical / real-time-simulator) model,
//! **not** a full spatially-resolved kinetics solve. It is intended for
//! education, capability building, and V&V demonstrations — not for
//! licensing, safety, or operational analysis.
//!
//! # Timestep selection (why 1 ms, not 25 microseconds)
//!
//! The delayed-neutron precursors are **slow** compared with the 1 ms
//! real-time GUI timestep. Each group's half-life is
//! `t_half,i = ln(2) / lambda_i`; for the five-group thermal-U-235 set this
//! layer ships (see [`DelayedNeutronLayer::u235_five_group`]):
//!
//! | i | `lambda_i` \[s^-1\] | `t_half,i` \[s\] | `t_half,i` at `dt = 1 ms` |
//! |---|---------------------|------------------|---------------------------|
//! | 1 | 0.012378            | 56.0             | ~56,000 steps             |
//! | 2 | 0.030137            | 23.0             | ~23,000 steps             |
//! | 3 | 0.111799            | 6.20             | ~6,200 steps              |
//! | 4 | 0.301369            | 2.30             | ~2,300 steps              |
//! | 5 | 1.633286            | 0.424 (merged)   | ~424 steps                |
//!
//! Even the **fastest** group (the merged short-lived group, half-life
//! `~0.42 s`) is resolved by `~424` timesteps of 1 ms; the slowest spans tens
//! of thousands. Every precursor timescale is three-plus orders of magnitude
//! longer than 1 ms, so a 1 ms step samples the precursor dynamics with a very
//! large margin.
//!
//! The **25 microsecond** timestep the earlier coupled solver used was **not**
//! set by the precursors at all — it was set by the fast **prompt** neutron
//! kinetics (prompt neutron generation time `Lambda ~ 2.31e-4 s`, and an
//! explicit prompt-power update needs `dt << Lambda` for stability). In
//! `fhr_sim_v2` the prompt term is owned entirely by the closed-form
//! [`crate::nordheim_fuchs::NordheimFuchsExactTimestepper`] (no `dt << Lambda`
//! restriction), and **this** layer integrates *only* the delayed precursors,
//! whose `0.42–56 s` timescale imposes no such fine-step requirement. Combined
//! with the unconditionally-stable implicit (backward-Euler) update above,
//! 1 ms is more than adequate and removes the previous fine-timestep cost.
//!
//! # How to couple it (Lie-split point kinetics)
//!
//! Per timestep `dt`, with the prompt-excursion layer as the prompt
//! propagator:
//!
//! 1. set the prompt layer's power to the current total power and its
//!    reactivity/feedback, then advance it one step → prompt power `P_p`
//!    (this applies the `(rho - beta)/Lambda * P` term and adiabatic
//!    feedback);
//! 2. `let dp_delayed = layer.advance(P_p, dt);` — updates the precursor
//!    lags and returns the delayed power increment `S * dt`;
//! 3. total power `P = P_p + dp_delayed`; feed `P` back to the prompt layer
//!    for the next step and to the thermal-hydraulics layer.
//!
//! The delayed increment keeps the reactor alive through the prompt-subcritical
//! operating regime and, because the `S_i` lag `P`, supplies the inertia that
//! damps the fuel-temperature feedback loop.
//!
//! # Standard delayed-neutron data
//!
//! [`DelayedNeutronLayer::u235_five_group`] bakes in a documented five-group
//! reduced set for thermal U-235 (see that constructor). For any other fuel or
//! group structure, build the layer from explicit `(beta_i, lambda_i)` pairs
//! with [`DelayedNeutronLayer::new`].

use uom::si::f64::*;
use uom::si::frequency::hertz;
use uom::si::power::megawatt;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::ConstZero;

use crate::teh_o_prke_error::TehOPrkeError;

/// Number of delayed-neutron groups this layer models. Fixed at five precursor
/// groups (see the module documentation for why the six-group standard data is
/// reduced to five).
pub const NUM_DELAYED_GROUPS: usize = 5;

/// One delayed-neutron precursor group: its delayed fraction `beta_i`, its
/// decay constant `lambda_i`, and its running precursor concentration `C_i`
/// (in power form, MW — see [`DelayedNeutronGroup::precursor_mw`]) advanced by
/// the implicit backward-Euler step in [`DelayedNeutronLayer::advance`]. The
/// group's delayed-neutron source contribution is `S_i = lambda_i C_i`.
#[derive(Debug, Clone)]
struct DelayedNeutronGroup {
    /// Delayed-neutron fraction `beta_i` (dimensionless) for this group.
    delayed_fraction: Ratio,
    /// Decay constant `lambda_i` \[s^-1\] for this group.
    decay_constant: Frequency,
    /// Precursor concentration `C_i` in **power form** (units MW): the state
    /// variable integrated implicitly each step so that `S_i = lambda_i C_i`
    /// has units MW/s and the delayed increment `S_i * dt` has units MW. Zero
    /// at construction; the only per-group state carried between steps (this
    /// is why the layer is `O(1)` in memory — see the module doc, op-e46.4).
    precursor_mw: f64,
}

/// A reusable delayed-neutron layer: five precursor groups, integrated by
/// direct implicit (backward-Euler) time-stepping, that turn a prompt-only
/// kinetics model into proper point kinetics by supplying the delayed-neutron
/// source `S = sum_i lambda_i C_i`.
///
/// Construct it with [`DelayedNeutronLayer::u235_five_group`] (documented
/// thermal-U-235 data) or [`DelayedNeutronLayer::new`] (arbitrary
/// `(beta_i, lambda_i)` data), then call [`DelayedNeutronLayer::advance`] once
/// per timestep with the prompt power to get the delayed power increment. See
/// the module-level documentation for the model and the coupling recipe.
#[derive(Debug, Clone)]
pub struct DelayedNeutronLayer {
    /// The five precursor groups (each holds its own `C_i` state).
    groups: [DelayedNeutronGroup; NUM_DELAYED_GROUPS],
    /// Prompt neutron generation time `Lambda` \[s\] used to scale each
    /// group's production gain `beta_i/Lambda`.
    prompt_generation_time: Time,
    /// Total delayed-neutron fraction `beta = sum_i beta_i`.
    total_delayed_fraction: Ratio,
    /// Running simulation time, advanced by `dt` on every [`Self::advance`]
    /// call (kept for reporting/diagnostics; the implicit update itself is
    /// time-invariant and needs only `dt`, not absolute time).
    elapsed_time: Time,
    /// The delayed power increment `S * dt` \[MW\] from the most recent
    /// [`Self::advance`] call, retained for reporting.
    last_delayed_increment: Power,
}

impl DelayedNeutronLayer {
    /// Builds a delayed-neutron layer from explicit per-group data.
    ///
    /// # Parameters
    /// - `prompt_generation_time`: the prompt neutron generation time
    ///   `Lambda` \[s\] of the reactor whose precursors these are; it sets
    ///   each group's source gain `beta_i/Lambda`. Must be strictly positive.
    /// - `groups`: five `(beta_i, lambda_i)` pairs. `beta_i` is dimensionless
    ///   ([`Ratio`]); `lambda_i` is a [`Frequency`] (`s^-1`) and must be
    ///   strictly positive so the group has a finite decay timescale
    ///   `1/lambda_i`.
    ///
    /// # Errors
    /// Returns [`TehOPrkeError::NonPositivePromptNeutronGenerationTime`] if
    /// `Lambda <= 0`, or [`TehOPrkeError::NonPositiveDelayedDecayConstant`] if
    /// any `lambda_i <= 0`.
    pub fn new(
        prompt_generation_time: Time,
        groups: [(Ratio, Frequency); NUM_DELAYED_GROUPS],
    ) -> Result<Self, TehOPrkeError> {
        let lambda_gen = prompt_generation_time.get::<second>();
        if lambda_gen <= 0.0 {
            return Err(TehOPrkeError::NonPositivePromptNeutronGenerationTime(
                lambda_gen,
            ));
        }

        let mut total_delayed_fraction = Ratio::ZERO;
        let mut built: Vec<DelayedNeutronGroup> = Vec::with_capacity(NUM_DELAYED_GROUPS);
        for (beta_i, lambda_i) in groups.into_iter() {
            let lambda_hz = lambda_i.get::<hertz>();
            if lambda_hz <= 0.0 {
                return Err(TehOPrkeError::NonPositiveDelayedDecayConstant(lambda_hz));
            }

            total_delayed_fraction += beta_i;
            built.push(DelayedNeutronGroup {
                delayed_fraction: beta_i,
                decay_constant: lambda_i,
                // precursors start empty; they build up under power via the
                // implicit update in `advance`.
                precursor_mw: 0.0,
            });
        }

        let groups: [DelayedNeutronGroup; NUM_DELAYED_GROUPS] = built.try_into().map_err(|_| {
            TehOPrkeError::GenericStringError(
                "DelayedNeutronLayer::new: expected exactly 5 groups".to_string(),
            )
        })?;

        Ok(Self {
            groups,
            prompt_generation_time,
            total_delayed_fraction,
            elapsed_time: Time::ZERO,
            last_delayed_increment: Power::ZERO,
        })
    }

    /// Builds the layer with a **documented five-group reduced set for thermal
    /// U-235**, for a reactor of prompt neutron generation time `Lambda`.
    ///
    /// # Data provenance
    /// The six-group thermal-U-235 delayed-neutron data used elsewhere in this
    /// crate (`zero_power_prke::six_group_precursor_prke::six_group_constants`,
    /// half-lives 56.0, 23.0, 6.2, 2.3, 0.61, 0.23 s; group fractions 0.00021,
    /// 0.00142, 0.00128, 0.00257, 0.00075, 0.00027; total `beta = 0.0065`) is
    /// collapsed here to **five** groups by merging the two shortest-lived
    /// groups (half-lives 0.61 s and 0.23 s). The merged group keeps their
    /// combined fraction `beta = 0.00075 + 0.00027 = 0.00102` and takes a
    /// fraction-weighted effective decay constant
    /// `lambda = (0.00075*lambda_5 + 0.00027*lambda_6) / 0.00102`, preserving
    /// the total delayed fraction (`beta = 0.0065`) and the aggregate fast
    /// delayed-neutron emission rate. `lambda_i = ln(2) / t_half,i`.
    ///
    /// Resulting five groups (`beta_i`, `lambda_i` \[s^-1\]):
    ///
    /// | i | `beta_i`  | `t_half` \[s\] | `lambda_i` \[s^-1\] |
    /// |---|-----------|---------------|---------------------|
    /// | 1 | 0.00021   | 56.0          | 0.012378            |
    /// | 2 | 0.00142   | 23.0          | 0.030137            |
    /// | 3 | 0.00128   | 6.2           | 0.111799            |
    /// | 4 | 0.00257   | 2.3           | 0.301369            |
    /// | 5 | 0.00102   | (merged)      | 1.633286            |
    ///
    /// These are round, order-of-magnitude illustrative constants suitable for
    /// education and V&V, consistent with the workspace data policy — not
    /// design data for any specific licensed reactor.
    ///
    /// # Panics
    /// If `prompt_generation_time <= 0`.
    pub fn u235_five_group(prompt_generation_time: Time) -> Self {
        let groups = [
            (
                Ratio::new::<ratio>(0.00021),
                Frequency::new::<hertz>(0.012378),
            ),
            (
                Ratio::new::<ratio>(0.00142),
                Frequency::new::<hertz>(0.030137),
            ),
            (
                Ratio::new::<ratio>(0.00128),
                Frequency::new::<hertz>(0.111799),
            ),
            (
                Ratio::new::<ratio>(0.00257),
                Frequency::new::<hertz>(0.301369),
            ),
            (
                Ratio::new::<ratio>(0.00102),
                Frequency::new::<hertz>(1.633286),
            ),
        ];
        Self::new(prompt_generation_time, groups)
            .expect("DelayedNeutronLayer::u235_five_group: Lambda > 0 and all lambda_i > 0")
    }

    /// Total delayed-neutron fraction `beta = sum_i beta_i` (dimensionless).
    pub fn total_delayed_neutron_fraction(&self) -> Ratio {
        self.total_delayed_fraction
    }

    /// Prompt neutron generation time `Lambda` \[s\] this layer was built with.
    pub fn prompt_generation_time(&self) -> Time {
        self.prompt_generation_time
    }

    /// The per-group decay constants `lambda_i` \[s^-1\], in construction order.
    pub fn decay_constants(&self) -> [Frequency; NUM_DELAYED_GROUPS] {
        core::array::from_fn(|i| self.groups[i].decay_constant)
    }

    /// The per-group delayed-neutron fractions `beta_i` (dimensionless), in
    /// construction order.
    pub fn delayed_fractions(&self) -> [Ratio; NUM_DELAYED_GROUPS] {
        core::array::from_fn(|i| self.groups[i].delayed_fraction)
    }

    /// The delayed power increment `S * dt` produced by the most recent
    /// [`Self::advance`] call. Zero before the first call.
    pub fn last_delayed_increment(&self) -> Power {
        self.last_delayed_increment
    }

    /// Advances the precursor bank by one implicit (backward-Euler) timestep
    /// `dt` and returns the **delayed power increment** `Delta P_delayed =
    /// S * dt` to add to the power balance this step, where
    /// `S = sum_i lambda_i C_i` is the delayed-neutron source.
    ///
    /// Each group's precursor concentration `C_i` (in power form, MW) is
    /// updated in place with the closed-form backward-Euler solution for a
    /// power `P` held constant across the step:
    ///
    /// ```text
    ///   C_i^{n+1} = ( C_i^n + dt * (beta_i/Lambda) * P ) / ( 1 + dt * lambda_i )
    /// ```
    ///
    /// Their `S_i = lambda_i C_i^{n+1}` are summed into the source `S`, and the
    /// returned increment is `S * dt`. This is `O(1)` in time and memory — five
    /// multiply-adds, no growing history (see the module doc, op-e46.4). See
    /// the module documentation for the Lie-split coupling recipe.
    ///
    /// # Parameters
    /// - `reactor_power`: the reactor power `P` driving precursor production
    ///   this step (in the recommended coupling, the prompt layer's freshly
    ///   advanced power `P_p`). Must be finite and non-negative for a
    ///   physical result.
    /// - `dt`: the timestep. The backward-Euler update is unconditionally
    ///   stable, so any positive `dt` is safe.
    pub fn advance(&mut self, reactor_power: Power, dt: Time) -> Power {
        self.elapsed_time += dt;

        let lambda_gen = self.prompt_generation_time.get::<second>();
        let p_mw = reactor_power.get::<megawatt>();
        let dt_s = dt.get::<second>();

        // sum_i S_i, with S_i = lambda_i * C_i in MW/s
        let mut source_mw_per_s = 0.0;
        for group in self.groups.iter_mut() {
            let lambda_i = group.decay_constant.get::<hertz>();
            let beta_i = group.delayed_fraction.get::<ratio>();

            // implicit (backward-Euler) precursor update, same discretisation
            // as SixGroupPRKE's coefficient matrix, decoupled from the prompt
            // equation (Nordheim-Fuchs owns that):
            //   C_i^{n+1} = (C_i^n + dt*(beta_i/Lambda)*P) / (1 + dt*lambda_i)
            let production = (beta_i / lambda_gen) * p_mw; // MW/s
            group.precursor_mw = (group.precursor_mw + dt_s * production) / (1.0 + dt_s * lambda_i);

            source_mw_per_s += lambda_i * group.precursor_mw;
        }

        // delayed power increment over the step: S * dt  [MW]
        let increment = Power::new::<megawatt>(source_mw_per_s * dt_s);
        self.last_delayed_increment = increment;
        increment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    /// U-235 illustrative prompt generation time used across this crate.
    fn lambda() -> Time {
        Time::new::<second>(2.31e-4)
    }

    /// The five-group U-235 dataset sums to the standard total delayed
    /// fraction `beta = 0.0065`.
    #[test]
    fn u235_five_group_total_beta_is_0_0065() {
        let layer = DelayedNeutronLayer::u235_five_group(lambda());
        assert_relative_eq!(
            layer.total_delayed_neutron_fraction().get::<ratio>(),
            0.0065,
            epsilon = 1e-9
        );
    }

    /// Methodology: hold the reactor power at a constant `P` and step the layer
    /// for many precursor time constants. Each group's source `S_i` is a
    /// first-order lag of `(beta_i/Lambda) P`, so at steady state
    /// `S = sum_i S_i -> (beta/Lambda) P` and the per-step delayed increment
    /// settles to `S * dt = (beta/Lambda) P dt`.
    ///
    /// Result (data 2026-07-15): with `P = 30 MW`, `Lambda = 2.31e-4 s`,
    /// `dt = 0.05 s`, stepped to `t = 1000 s` (> 12 x the longest time constant
    /// `tau_1 = 80.8 s`), the delayed increment settles to
    /// `(0.0065/2.31e-4) * 30 * 0.05 = 42.21 MW` within `< 0.1 %`.
    #[test]
    fn constant_power_source_settles() {
        let mut layer = DelayedNeutronLayer::u235_five_group(lambda());
        let p = Power::new::<megawatt>(30.0);
        let dt = Time::new::<second>(0.05);

        let mut inc = Power::ZERO;
        for _ in 0..20_000 {
            inc = layer.advance(p, dt);
        }

        let expected = (0.0065 / 2.31e-4) * 30.0 * 0.05;
        assert_relative_eq!(inc.get::<megawatt>(), expected, max_relative = 1e-3);
    }

    /// Methodology: after a step increase in power, the delayed source must
    /// *lag* — it cannot instantly reach its new steady value because the
    /// precursors take time to build up. Confirm the increment right after a
    /// power step is well below its eventual steady value.
    ///
    /// Result (data 2026-07-15): power stepped 0 -> 30 MW; one `dt = 0.05 s`
    /// after the step the delayed increment is a small fraction of the steady
    /// `42.21 MW`, confirming the reservoir builds up gradually (the inertia
    /// that damps the feedback loop).
    #[test]
    fn power_step_source_lags() {
        let mut layer = DelayedNeutronLayer::u235_five_group(lambda());
        let p = Power::new::<megawatt>(30.0);
        let dt = Time::new::<second>(0.05);

        let first = layer.advance(p, dt);
        let steady = (0.0065 / 2.31e-4) * 30.0 * 0.05;
        assert!(
            first.get::<megawatt>() < 0.5 * steady,
            "one step after a power step the delayed source {} MW should still \
             be well below its steady value {steady} MW",
            first.get::<megawatt>()
        );
    }

    /// Rejects a non-positive decay constant.
    #[test]
    fn rejects_non_positive_decay_constant() {
        let groups = [
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.01)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.03)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.1)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.3)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.0)),
        ];
        assert!(matches!(
            DelayedNeutronLayer::new(lambda(), groups),
            Err(TehOPrkeError::NonPositiveDelayedDecayConstant(_))
        ));
    }

    /// Rejects a non-positive prompt generation time.
    #[test]
    fn rejects_non_positive_lambda() {
        let groups = [
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.01)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.03)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.1)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.3)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(1.6)),
        ];
        assert!(matches!(
            DelayedNeutronLayer::new(Time::new::<second>(0.0), groups),
            Err(TehOPrkeError::NonPositivePromptNeutronGenerationTime(_))
        ));
    }

    /// Performance regression for op-e46.4: the implicit precursor update must
    /// be **O(1) per step** — constant time and constant memory regardless of
    /// how many steps have been taken. The former `TransferFnFirstOrder`-based
    /// layer accumulated one superposed response term per input change (pruned
    /// only after `20*tau`, ~1.6M entries for the slowest group at 1 ms) and
    /// summed over that buffer every step, so its per-step cost grew linearly
    /// (measured ~49 us -> ~1.8 ms/step from step 1k to 40k).
    ///
    /// # Methodology
    /// Advance the layer for many steps at the 1 ms GUI timestep with a
    /// changing power input (so the old implementation *would* have grown its
    /// buffer). Time an early block of steps and a late block of equal size,
    /// and assert the late block is not meaningfully slower than the early one
    /// (allowing generous headroom for scheduler/measurement noise). A truly
    /// O(1) update makes the two blocks ~equal; an O(n) update makes the late
    /// block dramatically slower.
    ///
    /// The struct also carries no `Vec`/history at all (only five `f64`
    /// precursor states), so there is nothing that can grow with step count —
    /// this test guards against a regression that reintroduces one.
    ///
    /// # Results (data 2026-07-15, release build)
    /// Representative: ~0.7–1.0 ns/step, flat across blocks — early and late
    /// 100k-step blocks time within noise of each other (late/early ratio well
    /// under the 5x guard), versus the old layer's unbounded linear growth.
    /// Exact numbers print with `--nocapture`.
    #[test]
    fn precursor_update_is_o1_per_step() {
        let mut layer = DelayedNeutronLayer::u235_five_group(lambda());
        let dt = Time::new::<second>(0.001); // 1 ms GUI timestep

        let block = 100_000;
        // vary the power each step so an input-change-accumulating implementation
        // would keep growing its internal buffer.
        let power_for = |k: usize| Power::new::<megawatt>(30.0 + (k % 97) as f64 * 0.1);

        // warm up so the first timed block isn't paying one-time costs.
        for k in 0..block {
            layer.advance(power_for(k), dt);
        }

        let t_early = std::time::Instant::now();
        for k in 0..block {
            std::hint::black_box(layer.advance(std::hint::black_box(power_for(k)), dt));
        }
        let early = t_early.elapsed();

        // take many more steps so an O(n) implementation would have a much
        // larger internal buffer by the time we measure the late block.
        for k in 0..(20 * block) {
            layer.advance(power_for(k), dt);
        }

        let t_late = std::time::Instant::now();
        for k in 0..block {
            std::hint::black_box(layer.advance(std::hint::black_box(power_for(k)), dt));
        }
        let late = t_late.elapsed();

        let early_ns = early.as_nanos() as f64;
        let late_ns = late.as_nanos() as f64;
        let ns_per_step = late_ns / block as f64;
        eprintln!(
            "op-e46.4 O(1) check: early block {early:.2?} ({:.2} ns/step), \
             late block {late:.2?} ({ns_per_step:.2} ns/step), late/early ratio {:.2}",
            early_ns / block as f64,
            late_ns / early_ns.max(1.0)
        );

        // O(1): the late block (after 20x more steps) must not be dramatically
        // slower than the early block. 5x is generous headroom for timing
        // noise; the old O(n) implementation would blow past it by orders of
        // magnitude. Guard the ratio only when the early block is long enough
        // to time meaningfully (avoid dividing by a near-zero measurement).
        if early_ns > 1.0e5 {
            assert!(
                late_ns < 5.0 * early_ns,
                "per-step cost must be O(1): late block {late_ns:.0} ns should not \
                 be >5x the early block {early_ns:.0} ns after 20x more steps"
            );
        }
    }
}
