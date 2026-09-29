//! Reactor-kinetics slot for the HTGR ~~scaffold~~ demo (**CHANGED 2026-09-28**,
//! maintainer: "no longer a scaffold, but a demo" -- an offline, unvalidated
//! demonstration; see `main.rs`).
//!
//! Composes two **real** workspace pieces, matching the recommended OUTRAM PARK
//! kinetics architecture (see `teh_o_prke::nordheim_fuchs`'s module doc):
//!
//! 1. **Prompt Excursion Layer** --
//!    [`teh_o_prke::nordheim_fuchs::NordheimFuchsExactTimestepper`]
//!    (re-exported as [`nee_soon::NordheimFuchsExactTimestepper`]). Closed-form
//!    prompt power + adiabatic fuel-temperature feedback.
//! 2. **Delayed-neutron precursor bank** --
//!    [`teh_o_prke::delayed_neutron_layer::DelayedNeutronLayer`], the reduced
//!    five-group U-235 precursor bank (five first-order transfer functions).
//!    This supplies the precursor inertia that a prompt-only model omits, so
//!    the kinetics is **not prompt-only** -- deliberately avoiding
//!    `fhr_sim_v2`'s prompt-only oscillation mistake by construction.
//!
//! ## Read this first: what the opening state is (gh:#387, maintainer 2026-09-29)
//!
//! - **One delayed-neutron fraction everywhere: `beta_eff = 7.26e-3`**
//!   (Chen et al. 2009 Table 1, [`HtgrKinetics::HTR10_EFFECTIVE_DELAYED_FRACTION`]).
//!   The prompt layer, the delayed bank (the U-235 five-group relative
//!   abundances rescaled to that total) and the rods-to-dollars conversion
//!   all use it. ~~The delayed bank summed to bare U-235's 0.0065, the rods
//!   were converted at 0.0065 (x1.117 too large) and steady state needed
//!   `rho_net = +76 pcm`~~ -- fixed 2026-09-29; at 0 $ with the precursors at
//!   equilibrium the power is now stationary
//!   (`tests::zero_net_dollars_with_equilibrium_precursors_is_stationary`).
//! - **The delayed-neutron precursors start EMPTY.** The bank is built with
//!   no precursors and fills under power, on the groups' own time constants;
//!   the longest group's ~56 s half-life sets the scale. Measured
//!   (`tests::how_long_the_empty_precursor_bank_takes_to_fill`, 10 MW held):
//!   the inventory reaches 1 % of equilibrium in **0.13 s** and comes within
//!   1 % of equilibrium only after **245 s** (analytic 245.0 s). **Consequence:** from a cold
//!   start at net 0 $ the power first DROPS, because the prompt layer sees
//!   `rho - beta = -beta` with no delayed source yet (measured 2026-09-28,
//!   with the old 0.0065 bank: 10 MW -> 0.255 MW in 20 s with the bed held;
//!   not re-measured since the single-beta change; pending validation work).
//!   **Early-transient numbers
//!   -- the first few minutes after the simulator opens -- are the
//!   precursors filling, not plant behaviour, and must not be read as
//!   such.** `DelayedNeutronLayer::seed_at_equilibrium` exists for a caller
//!   that wants a steady opening instead; the plant does not use it.
//! - **Decay heat is seeded at equilibrium** (`DecayHeat::new_at_equilibrium`):
//!   a conservative choice, since it means more decay heat early than a
//!   fresh core would have.
//! - **The rod worth and the feedback reference are demo-grade.** The bank
//!   worth is cold-clean (20 degC, TECDOC-1382) while the feedback zero is an
//!   illustrative 950 K seed, so the temperature defect between them is not
//!   in the budget (gh:#387 section 1-2). Filed for future validation as
//!   **gh:#408**; not changed here.
//!
//! ## Lie-split coupling (the important part)
//!
//! Each timestep the two layers are combined with an operator (Lie) split, per
//! the `DelayedNeutronLayer` coupling recipe:
//!
//! 1. Advance the prompt model -> prompt power `P_p`.
//! 2. `let increment = delayed.advance(P_p, dt);` -- the delayed power
//!    increment `S*dt` (`S = sum_i lambda_i C_i` is the delayed-neutron
//!    source).
//! 3. Total power `P = P_p + increment`, fed **back** into the prompt model
//!    (`prompt.power = P`) so the next step's prompt dynamics and the fuel
//!    heating both see the full point-kinetics power, not the prompt part
//!    alone. That feedback is the precursor inertia that damps the
//!    fuel-temperature feedback loop.
//!
//! ## Fission-product decay heat (3rd real piece, added 2026-08-14)
//!
//! [`teh_o_prke::decay_heat::DecayHeat`] -- the **23-group fit of the 1978
//! draft ANS Standard** (England *et al.*, via Tobias Table 16), integrated in
//! its exact piecewise-constant-source form. Decay heat is what makes a reactor
//! impossible to switch off, so a simulator that omits it cannot depict a
//! shutdown at all: before this, dropping the rods took core power to zero and
//! the graphite simply cooled.
//!
//! **The prompt term is scaled so the energy is not counted twice.** The
//! group fit accounts for the 13.18 MeV/fission of U-235 thermal fission that
//! emerges *later* as fission-product decay, out of the nominal 200 MeV. So the
//! core's thermal source is
//!
//! ```text
//! P_thermal = prompt_power_fraction * P_fission + P_decay
//! ```
//!
//! with `prompt_power_fraction = 1 - 13.183/200 = 0.9341`. At equilibrium the
//! two terms sum back to `P_fission` by construction (see
//! [`HtgrKinetics::core_thermal_power`]) -- that is a property of the model,
//! not a tuned constant, and it is what makes the steady state unchanged while
//! the shutdown transient becomes right. The bank is seeded with
//! `DecayHeat::new_at_equilibrium`, so the simulator opens with its fission
//! products already saturated rather than clean.
//!
//! ## Nodalisation
//!
//! **One node.** This is point kinetics: the whole core is a single amplitude,
//! with one lumped fuel temperature behind the feedback. There is no spatial
//! flux shape, so control-rod worth cannot depend on rod position or on which
//! of the ten side-reflector rods moves, and there is no way to represent a
//! local power peak or the axial power shape of a pebble bed. The obvious
//! refinement is a coarse axial nodal-diffusion solve, which is a different
//! crate's job (`bedok`), not this simulator's.
//!
//! ## The fuel-temperature feedback got a heat sink (2026-08-14) -- SUPERSEDED 2026-09-28
//!
//! **Superseded by "The fuel node IS the kernel now" below (gh:#360):** the
//! coolant sink described here was sized for the bed, not for this node, and
//! is gone. Kept as the record of why the node needed a sink at all.
//!
//! Until 2026-08-14 the prompt layer's fuel temperature was **adiabatic**: it
//! integrated `dT_f/dt = P/C_f` and never cooled, whatever the helium was
//! doing. The consequence was not subtle -- after any power rise the feedback
//! reactivity stuck at its most negative value forever, because the temperature
//! it was computed from could only ever climb.
//!
//! **The fix is a sink, not a replacement.** It is tempting to overwrite the
//! node with [`super::pebble_bed`]'s graphite temperature each step, but that
//! would be a mistake: Nordheim-Fuchs integrates the prompt power *and* its
//! temperature feedback together in **closed form**, and that exactness is
//! exactly what keeps the stiff feedback term from being stiff here.
//! Substituting an externally integrated node, reset discontinuously once per
//! plant step, throws the closed form away and reintroduces the stiffness this
//! layer exists to avoid.
//!
//! So the closed form keeps the feedback, and
//! [`HtgrKinetics::apply_coolant_heat_removal`] adds only what was missing --
//! the heat the coolant carried off, over the same graphite heat capacity the
//! bed uses. Fast, stiff coupling stays analytic; the slow sink (the bed's
//! ~184 s time constant, against a 0.1 s plant step) is a plain Lie split.
//! The fuel node and the pebble bed then see the same power in and the same
//! heat out over the same capacity, so they track each other physically rather
//! than being reconciled by force.
//!
//! **This makes the kinetics genuinely coupled**, so it is now stepped
//! *inside* the plant's outer-corrector loop (see [`super::HtgrPlant::step`]).
//! [`HtgrKinetics`] is `Clone` precisely so the corrector can rewind it.
//!
//! ## The Doppler feedback moved onto the fuel kernel (2026-09-22) -- SUPERSEDED 2026-09-28
//!
//! **Superseded (gh:#360):** `KernelDopplerChannel` is removed; the fuel
//! node is the kernel, so the fuel share rides the closed form directly. The
//! record below is kept for its reasoning about the split fraction, which
//! still applies.
//!
//! ~~"A real Doppler feedback wants the fuel kernel temperature, which needs
//! the intra-pebble split described in [`super::pebble_bed`]."~~
//! ~~"**UPDATED 2026-09-22 -- the split now exists, and this module still does
//! not use it.** The feedback here still reads the bed node. That is a
//! deliberate hold ... It wants its own change, with its own before/after."~~
//! **CORRECTED 2026-09-22 -- that change is this one, and the hold is
//! discharged.** `KernelDopplerChannel` (removed 2026-09-28, gh:#360) carries the **fuel share** of the
//! published isothermal coefficient on the peak UO2 kernel temperature, which
//! [`super::pebble_bed::PebbleBedPorousMediaNode`] resolves from a two-zone
//! pebble; the graphite and reflector share stays on the bed node. The two
//! shares are constrained to **sum to the published isothermal coefficient**,
//! so no reactivity is invented or double-counted.
//!
//! Three things make this attributable rather than a rewrite, and they are
//! worth reading before any number in this module is quoted:
//!
//! 1. **The closed form is untouched.** The split is algebraically one extra
//!    external reactivity term (the derivation is on
//!    `KernelDopplerChannel` (removed 2026-09-28, gh:#360)), so Nordheim-Fuchs keeps ownership of the
//!    stiff feedback exactly as the section above insists it must, and every
//!    previously recorded `alpha_iso` number stays valid.
//! 2. **The design point is neutral by construction**, so the steady state is
//!    unchanged and everything the channel moves is transient.
//! 3. **The split fraction is an INPUT and is bounded.** It is a ratio taken
//!    from Hu *et al.*, not a fitted value; `f = 0` reproduces the
//!    pre-2026-09-22 model exactly and `f = 1` puts the whole coefficient on
//!    the kernel. Both bounds are measured -- see
//!    `KernelDopplerChannel::HU_FUEL_SHARE_OF_ISOTHERMAL` (removed 2026-09-28, gh:#360) and
//!    [`tests::the_kernel_doppler_split_is_ablated_across_its_full_range`].
//!
//! **What this buys that the bed node could not:** a *prompt* feedback
//! temperature. The kernel follows power essentially instantly while the bed
//! relaxes on ~184 s, so before this the simulator's only feedback temperature
//! was a slow one -- it could depict where a transient settles but not the
//! prompt arrest that gets it there.
//!
//! What this still does not buy: the bed is one node, so the kernel is the
//! peak kernel of a **core-average** pebble. There is no power peaking factor,
//! no axial or radial shape and no burnup, so a real HTR-10 peak-power pebble
//! runs hotter than this one.
//!
//! ## The fuel node IS the kernel now, and the energy balance is a chain (2026-09-28, gh:#360)
//!
//! **Maintainer direction 2026-09-28** (gh:#360 comment and the refinement
//! that followed): the kinetics fuel node was a *shadow copy of the bed* —
//! same 9 MJ/K capacity, heated by the full fission power *and* the decay
//! heat, cooled by the bed's heat-to-helium — and it drifted **234.9 K above
//! the bed** at t = 1500 s (two defects: the `(1 - f_prompt) P` share counted
//! twice, and the passive RCCS loss never charged to it), giving ~-4.5 $ of
//! spurious feedback. The sections above that describe the fuel node as
//! "tracking the bed" through a coolant sink are **superseded** by this one.
//!
//! The model is now a proper chain:
//!
//! ```text
//!  f_prompt P + P_decay
//!         |
//!         v
//!  [FUEL NODE T_f]  --(T_f - T_bed)/R_fb-->  [BED T_bed] --hA--> [helium] --> loop
//!  TRISO kernels+coatings                     graphite    \--(ZBS conduction + radiation)--> [reflector] --> RPV --> RCCS
//!  C_fuel ~0.27 MJ/K (Nordheim-Fuchs node)   ~8.9 MJ/K
//! ```
//!
//! - **The Nordheim-Fuchs node is the fuel node**, with the TRISO particles'
//!   own heat capacity ([`super::pebble_bed::fuel_node_heat_capacity`]),
//!   not the bed's. Its temperature is the **inventory-averaged kernel
//!   temperature**.
//! - **Sources are deposited in the fuel, once.** The closed form carries
//!   capacity `C_fuel / f_prompt`, so its adiabatic heating is exactly
//!   `f_prompt P / C_fuel` — the promptly released share — and the decay heat
//!   is added separately. No `(1 - f_prompt) P` double count.
//! - **The fuel loses heat only to the bed**, by conduction through the
//!   resolved-pebble resistance `R_fb`
//!   ([`super::pebble_bed::FuelBedCoupling`]), driven by `T_f - T_bed`. Never
//!   directly to helium, never to the RCCS. The sink is applied as the exact
//!   exponential relaxation toward the bed over each 1 ms substep, and the
//!   energy it moves is handed to the bed as its source
//!   ([`HtgrKinetics::heat_to_bed`]) — so what leaves the fuel is exactly what
//!   the bed receives.
//! - **The bed** loses to the helium (convection) and, in parallel, to the
//!   reflector (the ZBS leg of `decay_heat_removal`, which carries both
//!   solid conduction and pebble-to-pebble radiation). The RCCS loss is
//!   charged at the reflector -> RPV -> RCCS end of that chain.
//! - **Reactivity** is `alpha_fuel (T_f - T_f,ref) + alpha_mod (T_bed - T_bed,ref)`
//!   with `alpha_fuel + alpha_mod = alpha_iso` (the published isothermal
//!   coefficient) split by the Hu *et al.* ratio. The fuel term lives in the
//!   Nordheim-Fuchs closed form (prompt, stiff — where it belongs); the
//!   moderator term is an external reactivity evaluated at the bed
//!   temperature once per plant step (the bed moves on ~180 s). At steady
//!   state `T_f - T_bed = R_fb (f_prompt P + P_decay)` by construction: there
//!   is no second lumped node left to drift. ~~`KernelDopplerChannel`~~
//!   (the 2026-09-22 add-on that put `alpha_D (dT - dT_ref)` on top of the
//!   old shadow node) is **removed** — its job is what the fuel node now does
//!   natively.
//!
//! What this still does not buy: one bed node, so the fuel node is the
//! average kernel of a **core-average** pebble — no peaking factor, no axial
//! shape, no burnup. And the matrix graphite's heat capacity is all in the
//! bed, so after a power step the kernel reaches its new steady offset on
//! `R_fb C_fuel` (~0.1-0.3 s) rather than on the pebble's ~60 s conduction
//! time: the fuel temperature leads a fully resolved pebble somewhat.
//!
//! This slot is wired to the real `teh-o-prke` API (bead `op-wqk.9.2`). What
//! remains ~~scaffold-level~~ illustrative is only the *plant-scale illustrative parameters*
//! below, not the kinetics wiring.

use nee_soon::NordheimFuchsExactTimestepper;
use teh_o_prke::decay_heat::{DecayHeat, FissioningNuclide};
use teh_o_prke::delayed_neutron_layer::DelayedNeutronLayer;

use uom::si::f64::{
    HeatCapacity, Power, Ratio, TemperatureCoefficient, TemperatureInterval, ThermalResistance,
    ThermodynamicTemperature, Time,
};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::thermal_resistance::kelvin_per_watt;
use uom::si::temperature_interval;
use uom::si::power::{megawatt, watt};
use uom::si::ratio::ratio;
use uom::si::time::second;

/// The HTGR kinetics slot: prompt excursion layer + five-group delayed-neutron
/// bank, coupled by a Lie split (see the module doc).
///
/// Reactivity is driven in **dollars** (`rho/beta`) from the GUI and converted
/// to the prompt layer's dimensionless `rho_ext = dollars * beta`.
#[derive(Clone)]
pub struct HtgrKinetics {
    /// Nordheim-Fuchs prompt-excursion timestepper.
    pub prompt: NordheimFuchsExactTimestepper,
    /// Five-group U-235 delayed-neutron precursor bank.
    pub delayed: DelayedNeutronLayer,
    /// 23-group fission-product decay-heat bank (1978 draft ANS Standard).
    pub decay: DecayHeat,
    /// Prompt-layer power `P_p` from the most recent step (before the delayed
    /// increment is fed back), kept for display.
    prompt_power: Power,
    /// Delayed power increment `S*dt` from the most recent step, kept for
    /// display.
    delayed_increment: Power,
    /// Total reactor power `P = P_p + increment` from the most recent step.
    total_power: Power,
    /// Xe-135 poisoning channel, `None` when disabled. See [`XenonChannel`].
    xenon: Option<XenonChannel>,
    /// How the published isothermal coefficient is split between the fuel
    /// node (inside the closed form) and the bed (external). Always present;
    /// the split is ablated with [`HtgrKinetics::set_fuel_share`]. See
    /// [`FeedbackSplit`].
    feedback_split: FeedbackSplit,
    /// The fuel node's **real** heat capacity `C_fuel` \[J/K\], refreshed at
    /// the fuel temperature once per plant step. The closed form carries
    /// `C_fuel / f_prompt` (see the module doc); the decay heat and the sink to
    /// the bed use this.
    fuel_heat_capacity: HeatCapacity,
    /// Core-level fuel-to-bed conduction resistance \[K/W\] used on the most
    /// recent step, from the bed's resolved pebble
    /// ([`super::pebble_bed::FuelBedCoupling::resistance`]).
    fuel_to_bed_resistance: ThermalResistance,
    /// The bed temperature the most recent step ran against — the sink's
    /// target and the moderator channel's temperature.
    bed_temperature: ThermodynamicTemperature,
    /// Heat conducted from the fuel to the bed, averaged over the most recent
    /// step \[W\]. This is the bed's source term.
    heat_to_bed: Power,
    /// Cumulative energy ledger of the fuel node, for the conservation tests.
    ledger: FuelNodeLedger,
    /// The external (rod) reactivity the most recent [`Self::step`] was
    /// handed, in the kinetics' dollars \[$\] -- recorded so the reactivity
    /// budget shows what the kinetics actually integrated (after the
    /// protection system's scram demand), not the operator's command.
    last_external_reactivity_dollars: f64,
}

/// Cumulative energy ledger of the fuel node \[J\], summed over every substep
/// since construction. `deposited_prompt + deposited_decay - conducted_to_bed`
/// equals the fuel node's stored-energy change `sum C_fuel dT` to rounding.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FuelNodeLedger {
    /// Promptly released fission energy deposited, `sum f_prompt P dt`.
    pub deposited_prompt: f64,
    /// Decay heat deposited, `sum P_decay dt`.
    pub deposited_decay: f64,
    /// Energy conducted to the bed, `sum C_fuel (T_f - T_f')`.
    pub conducted_to_bed: f64,
    /// Stored-energy change, `sum C_fuel dT` over all three operators.
    pub stored: f64,
}

/// ~~The **kernel Doppler channel** — the fuel share of HTR-10's published
/// isothermal coefficient, applied to the UO2 kernel rather than to the bed,
/// as one extra external term `alpha_D (dT - dT_ref)` on top of a lumped node
/// that tracked the bed.~~ **REPLACED 2026-09-28 (gh:#360)** by this split:
/// the fuel node *is* the kernel now, so the fuel share rides the
/// Nordheim-Fuchs node directly and only the moderator share is external.
///
/// ```text
/// rho = alpha_fuel (T_f - T_f,ref) + alpha_mod (T_bed - T_bed,ref)
/// alpha_fuel = f alpha_iso,   alpha_mod = (1 - f) alpha_iso
/// ```
///
/// `f` is [`Self::HU_FUEL_SHARE_OF_ISOTHERMAL`] by default — an input from a
/// published ratio, not a fit, and bounded. The two references are the design
/// point: `T_bed,ref` the bed's seed temperature (950 K) and
/// `T_f,ref = T_bed,ref + R_fb P_rated`, so the rated design point is neutral.
///
/// **Why the old algebra's reassurance no longer applies.** The retired
/// channel argued that `alpha_iso (T_f - T_ref)` was untouched and the
/// kernel term merely added on top. That was true algebra on a node that was
/// itself wrong: `T_f` drifted 235 K above the bed (gh:#360), so
/// `alpha_iso (T_f - T_bed)` carried ~-4.5 $ nobody intended. With the fuel
/// node a real node of the energy chain, `T_f - T_bed = R_fb Q_fb` at steady
/// state and the whole feedback is on physical temperatures.
#[derive(Debug, Clone, Copy)]
pub struct FeedbackSplit {
    /// `f`, the fuel share, in `(0, 1]`.
    fuel_share: f64,
    /// `T_bed,ref` — the bed temperature at which the moderator term is zero.
    bed_reference_temperature: ThermodynamicTemperature,
}

impl FeedbackSplit {
    /// The fuel share `f` of the isothermal coefficient, **dimensionless**.
    ///
    /// # What is published, and what this does with it
    ///
    /// Hu *et al.* (2006) section 2.1 gives the only split of HTR-10's
    /// temperature coefficient this workspace has:
    ///
    /// | Channel | Hu *et al.* (2006), as printed |
    /// |---|---|
    /// | Fuel | `-1.93e-5 $/degC` |
    /// | Moderator | `-1.49e-5 $/degC` |
    /// | Reflector | `+7.08e-6 $/degC` |
    ///
    /// **Those magnitudes are not usable and this constant does not use
    /// them.** [`HtgrKinetics::HTR10_TEMPERATURE_COEFFICIENT_PER_K`] records
    /// why in full: summed and converted at `beta = 7.26e-3` they give about
    /// `-2e-7 dk/k per degC`, three orders of magnitude below Chen's total, so
    /// a printed exponent is wrong somewhere — and reading it as `10^-2` makes
    /// the *fuel term alone* equal Chen's *total*, which cannot be right
    /// either. That doc comment's instruction, "do not use it until the
    /// exponent is settled against INET (1998)", stands and is obeyed here.
    ///
    /// **What is used is the RATIO, which the disputed exponent cannot
    /// touch.** A common factor — whatever power of ten it is — cancels out of
    ///
    /// ```text
    /// f = 1.93 / (1.93 + 1.49 - 0.708) = 0.71165
    /// ```
    ///
    /// so `f` is identical under every reading of the exponent. The reflector
    /// enters with its published **positive** sign, which is why the
    /// denominator is a difference; folding it into the graphite channel is
    /// right on timescale, since the reflector is the slowest mass in the core,
    /// slower even than the bed.
    ///
    /// The *magnitude* then comes entirely from Chen's published isothermal
    /// total, and the two channels are constrained to sum back to it. **No
    /// reactivity is created or double-counted by this split** — that
    /// constraint is what
    /// [`tests::the_split_channels_sum_to_the_published_isothermal_coefficient`]
    /// pins.
    ///
    /// # What would settle it properly
    ///
    /// A fuel-only Doppler coefficient computed for an HTR-10 fuel zone —
    /// `outram-mc-libs` has the pieces (`examples/htr10_fuel_zone_kinf.rs`,
    /// URR and DBRC both on by default since 2026-09-20), and a `k_inf` sweep
    /// over kernel temperature at fixed graphite temperature would give it
    /// directly. That is hours of Monte-Carlo, not an inline calculation, and
    /// it is the right way to retire this constant. Until then the ratio is
    /// **an input, not a derivation**, and any conclusion that turns on it must
    /// be reported as a conclusion about this number.
    pub const HU_FUEL_SHARE_OF_ISOTHERMAL: f64 = 0.711_651_917_404_129_8;

    /// The HTR-10 split: [`Self::HU_FUEL_SHARE_OF_ISOTHERMAL`], moderator
    /// referenced to `bed_reference_temperature`.
    pub fn htr10_published(bed_reference_temperature: ThermodynamicTemperature) -> Self {
        Self {
            fuel_share: Self::HU_FUEL_SHARE_OF_ISOTHERMAL,
            bed_reference_temperature,
        }
    }

    /// `alpha_fuel = f alpha_iso` \[1/K\] — carried by the Nordheim-Fuchs node.
    pub fn fuel_coefficient(&self) -> TemperatureCoefficient {
        use uom::si::temperature_coefficient::per_kelvin;
        TemperatureCoefficient::new::<per_kelvin>(
            self.fuel_share * HtgrKinetics::HTR10_TEMPERATURE_COEFFICIENT_PER_K,
        )
    }

    /// `alpha_mod = (1 - f) alpha_iso` \[1/K\] — applied to the bed.
    pub fn moderator_coefficient(&self) -> TemperatureCoefficient {
        use uom::si::temperature_coefficient::per_kelvin;
        TemperatureCoefficient::new::<per_kelvin>(
            (1.0 - self.fuel_share) * HtgrKinetics::HTR10_TEMPERATURE_COEFFICIENT_PER_K,
        )
    }

    /// The moderator channel's reactivity in dollars at bed temperature
    /// `bed`, `alpha_mod (T_bed - T_bed,ref) / beta`.
    pub fn moderator_reactivity_dollars(&self, bed: ThermodynamicTemperature, beta: f64) -> f64 {
        use uom::si::temperature_coefficient::per_kelvin;
        use uom::si::thermodynamic_temperature::kelvin;
        if beta <= 0.0 {
            return 0.0;
        }
        self.moderator_coefficient().get::<per_kelvin>()
            * (bed.get::<kelvin>() - self.bed_reference_temperature.get::<kelvin>())
            / beta
    }
}

/// Xe-135 poisoning channel for the HTGR core.
///
/// ## What is published here and what is a model input
///
/// This distinction matters more than usual, so it is stated first.
///
/// **Published, and carried by [`teh_o_prke`]:** the I-135 and Xe-135 fission
/// yields (Lamarsh Table 7.5 -- 0.0639 and 0.00237 for U-235 thermal) and the
/// decay constants (Lamarsh Table 7.6 -- `lambda_I = 2.87e-5 Hz`,
/// `lambda_Xe = 2.09e-5 Hz`). Those set the **shape** of the transient: how
/// fast xenon builds after a shutdown, when it peaks, and how fast it decays.
/// That shape is the physics this channel exists to represent.
///
/// **A model input, NOT derived:** the reactivity **magnitude**. Converting a
/// xenon number density into reactivity needs the core's absorption cross
/// sections, and no published set exists for HTR-10 in this workspace. The
/// route `teh-o-prke` offers ([`Xenon135Poisoning::simplified_poison_concentration_feedback`])
/// is not usable here either: it hardcodes an enrichment of 0.199 and depends
/// on constants its own source comments mark as *"AI guestimate"* and
/// *"tentatively got from AI, but need to cite"*. Importing those would put
/// unverified numbers underneath a headline result.
///
/// So the magnitude is set by [`EQUILIBRIUM_XENON_WORTH_DOLLARS`], an explicit
/// input scaled so that the equilibrium inventory at the seeding power is worth
/// exactly that. **Any conclusion about how much xenon changes the peak power
/// is a conclusion about that input**, and must be reported as such.
///
/// ## Why the fission rate needs no invented constant
///
/// The chain is driven by a fission-rate density, which follows from published
/// quantities alone: `P_fission / (E_fission * V_core)`, with the HTR-10 core
/// volume of 5 m^3 (IAEA-TECDOC-1382, carried in
/// [`outram_park_digital_twin_engine::htr10::design`]) and the nominal
/// 200 MeV/fission this crate already uses for the decay-heat split.
///
/// ## What is deliberately omitted
///
/// **Xenon burnout by neutron absorption.** The true chain loses xenon to
/// `sigma_a * phi * X` as well as to decay, and that term needs a thermal flux,
/// which needs a cross section this workspace does not have for HTR-10.
/// Omitting it makes the modelled xenon build-up after shutdown **slightly
/// faster and larger** than reality, because burnout is what holds the
/// equilibrium inventory down while the reactor is at power. The direction of
/// that error is stated so a reader can bound it: this channel over-states
/// xenon's negative reactivity, so it under-states the recriticality peak.
///
/// At HTR-10's low power density (3.3 MW in 5 m^3) burnout is a modest
/// correction -- decay dominates -- but it is not nothing, and this is a
/// scoping model, not a validated one.
#[derive(Clone, Debug)]
pub struct XenonChannel {
    /// Iodine-135 number density \[atoms/cm^3\].
    iodine_per_cm3: f64,
    /// Xenon-135 number density \[atoms/cm^3\].
    xenon_per_cm3: f64,
    /// Equilibrium xenon inventory at the seeding power \[atoms/cm^3\], used
    /// to normalise the reactivity so that it equals
    /// [`EQUILIBRIUM_XENON_WORTH_DOLLARS`] there.
    equilibrium_per_cm3: f64,
}

/// Reactivity worth of the **equilibrium** Xe-135 inventory, in dollars.
///
/// **This is a model input, not a measurement.** See [`XenonChannel`] for why
/// it cannot be derived here. A value of 1 dollar means the saturated xenon
/// inventory is worth one delayed-neutron fraction.
///
/// Order-of-magnitude justification, stated so the choice is arguable rather
/// than arbitrary: equilibrium xenon in a thermal power reactor is commonly
/// quoted around 2.6-3 % dk/k at high flux, which at
/// `beta = 7.26e-3` would be 3.6-4.1 dollars. HTR-10 at the test condition runs
/// at about 0.66 MW/m^3, far below a power reactor, and xenon worth falls with
/// flux; 1 dollar is a deliberately conservative placeholder in that light.
///
/// **Vary this and re-run before quoting any xenon effect.** The paired
/// with/without comparison this constant supports is a sensitivity study.
pub const EQUILIBRIUM_XENON_WORTH_DOLLARS: f64 = 1.0;

impl XenonChannel {
    /// I-135 decay constant \[1/s\]. Lamarsh Table 7.6, via `teh-o-prke`.
    const LAMBDA_IODINE_PER_S: f64 = 2.87e-5;
    /// Xe-135 decay constant \[1/s\]. Lamarsh Table 7.6, via `teh-o-prke`.
    const LAMBDA_XENON_PER_S: f64 = 2.09e-5;
    /// I-135 yield per U-235 thermal fission. Lamarsh Table 7.5.
    const YIELD_IODINE: f64 = 0.0639;
    /// Xe-135 direct yield per U-235 thermal fission. Lamarsh Table 7.5.
    const YIELD_XENON: f64 = 0.00237;
    /// Energy per fission \[J\], the same 200 MeV this module's decay-heat
    /// split uses.
    const JOULES_PER_FISSION: f64 = 200.0 * 1.602_176_634e-13;

    /// Fission-rate density \[fissions/(cm^3 s)\] for a given fission power.
    fn fission_rate_per_cm3_s(power: Power) -> f64 {
        // Published HTR-10 core volume, 5 m^3 (IAEA-TECDOC-1382 via
        // `Htr10DesignPoint::iaea_benchmark`).
        let core_volume_cm3 = super::pebble_bed::design()
            .core_volume
            .get::<uom::si::volume::cubic_meter>()
            * 1.0e6;
        power.get::<watt>() / (Self::JOULES_PER_FISSION * core_volume_cm3)
    }

    /// Seed the chain at equilibrium for `power`.
    ///
    /// At equilibrium, with burnout omitted (see the type docs):
    /// `I = gamma_I * F / lambda_I` and
    /// `X = (gamma_I + gamma_X) * F / lambda_X`, the latter because every
    /// iodine atom eventually becomes a xenon atom.
    #[cfg(test)] // test-only: no GUI/headless control reaches it (2026-09-29 dead-code pass)
    pub fn new_at_equilibrium(power: Power) -> Self {
        let f = Self::fission_rate_per_cm3_s(power);
        let iodine = Self::YIELD_IODINE * f / Self::LAMBDA_IODINE_PER_S;
        let xenon = (Self::YIELD_IODINE + Self::YIELD_XENON) * f / Self::LAMBDA_XENON_PER_S;
        Self {
            iodine_per_cm3: iodine,
            xenon_per_cm3: xenon,
            // Guard against a zero-power seed making the normaliser singular.
            equilibrium_per_cm3: if xenon > 0.0 { xenon } else { 1.0 },
        }
    }

    /// Advance the I-135 / Xe-135 chain by `dt`, driven by `fission_power`.
    ///
    /// Backward Euler on both species, which is unconditionally stable and
    /// matters here because the plant step (0.1 s) is many orders of magnitude
    /// shorter than the xenon time constants (about 9.2 h and 13.3 h), so an
    /// explicit scheme would be wasting its stability margin for nothing.
    pub fn advance(&mut self, dt: Time, fission_power: Power) {
        let dt_s = dt.get::<second>();
        if dt_s <= 0.0 {
            return;
        }
        let f = Self::fission_rate_per_cm3_s(fission_power);

        // I: dI/dt = gamma_I F - lambda_I I
        self.iodine_per_cm3 = (self.iodine_per_cm3 + dt_s * Self::YIELD_IODINE * f)
            / (1.0 + dt_s * Self::LAMBDA_IODINE_PER_S);

        // X: dX/dt = gamma_X F + lambda_I I - lambda_X X
        // Burnout (- sigma_a phi X) is deliberately absent; see the type docs.
        let source = Self::YIELD_XENON * f + Self::LAMBDA_IODINE_PER_S * self.iodine_per_cm3;
        self.xenon_per_cm3 =
            (self.xenon_per_cm3 + dt_s * source) / (1.0 + dt_s * Self::LAMBDA_XENON_PER_S);
    }

    /// Xenon reactivity in dollars: negative, and proportional to inventory.
    pub fn reactivity_dollars(&self) -> f64 {
        -EQUILIBRIUM_XENON_WORTH_DOLLARS * self.xenon_per_cm3 / self.equilibrium_per_cm3
    }
}

impl HtgrKinetics {
    /// Construct the kinetics slot with illustrative graphite-moderated
    /// pebble-bed parameters. **Not** any specific licensed design -- round,
    /// order-of-magnitude numbers only, per this workspace's data policy.
    ///
    /// - `Lambda` = [`Self::HTR10_PROMPT_GENERATION_TIME_S`] -- **published**,
    /// - `beta` = [`Self::HTR10_EFFECTIVE_DELAYED_FRACTION`] -- **published**,
    /// - ~~`C_f` = the pebble bed's own lumped graphite heat capacity,
    ///   [`super::pebble_bed::bed_heat_capacity`] (about 9.0 MJ/K)~~
    ///   **CHANGED 2026-09-28 (gh:#360)** — `C_f` is the TRISO particles' own
    ///   heat capacity, [`super::pebble_bed::fuel_node_heat_capacity`]
    ///   (~0.27 MJ/K), because this node is the fuel now, not a copy of the
    ///   bed. The closed form carries `C_f / f_prompt` (module doc).
    /// - `alpha_f` = ~~[`Self::HTR10_TEMPERATURE_COEFFICIENT_PER_K`]~~ its
    ///   **fuel share** (`f alpha_iso`, [`FeedbackSplit`]) since 2026-09-28;
    ///   the moderator share rides the bed. The total is still
    ///   [`Self::HTR10_TEMPERATURE_COEFFICIENT_PER_K`] -- **published**, and
    ///   3.5x the magnitude of the -4e-5 /K it replaced.
    ///   This is the coefficient that arrests a LOFC transient, so the old
    ///   value materially under-stated HTR-10's inherent safety margin,
    /// - ~~reference/initial fuel temperature = the pebble bed's own
    ///   design-point temperature~~ **CHANGED 2026-09-28** -- the bed's
    ///   design-point temperature **plus the fuel's steady offset above it at
    ///   rated power**, `R_fb P_rated`, so the design point stays neutral with
    ///   the fuel a real node. Original reasoning, still the principle:
    ///   **the pebble bed's own design-point temperature**, [`super::pebble_bed::PebbleBedPorousMediaNode::new`]
    ///   (about 950 K), *not* a separately chosen 900 K. This matters now that
    ///   the fuel node has a coolant sink and therefore tracks the bed: the
    ///   feedback is `alpha_f (T_f - T_ref)`, so if `T_ref` sits below the
    ///   temperature the bed actually runs at, the "negative" feedback comes
    ///   out **positive** at the design point and the reactor climbs above
    ///   rated power for no physical reason. While the two nodes were
    ///   decoupled this inconsistency was invisible, because the adiabatic
    ///   fuel node never sat anywhere near the bed temperature anyway.
    ///   Deriving both from the bed makes the design point neutral by
    ///   construction,
    /// - `reference_power` seeds the initial prompt power.
    ///
    /// The delayed bank is built with the **same** `Lambda`, so its per-group
    /// source gains `beta_i/Lambda` are consistent with the prompt layer.
    /// Effective delayed-neutron fraction of the HTR-10, dimensionless.
    ///
    /// **7.26e-3**, Chen et al. (2009) Table 1; the same figure appears in
    /// Hu et al. (2006) section 2.1, both attributing it to INET (1998). The
    /// two papers agree on this one, which is why it is the least hedged
    /// constant here.
    ///
    /// Replaces an illustrative 0.0065, which was 10.5 % low.
    pub const HTR10_EFFECTIVE_DELAYED_FRACTION: f64 = 7.26e-3;

    /// Prompt neutron generation time of the HTR-10 \[s\].
    ///
    /// **1.68e-3 s**, Chen et al. (2009) Table 1.
    ///
    /// **The two sources disagree by a factor of ten and this is not an
    /// extraction artefact** — Hu et al. (2006) section 2.1 prints
    /// `1.68 x 10^-4 s`, Chen et al. (2009) Table 1 prints `1.68 x 10^-3 s`,
    /// and both cite INET (1998). Chen's value is used here for two reasons:
    /// it is the one whose post-test analysis reproduces the measured power
    /// transient, and ~1e-3 s is the physically expected magnitude for a
    /// graphite-moderated thermal system (a large migration area gives a long
    /// prompt generation time; 1.68e-4 s is LWR-like). **Not resolved against
    /// a primary source — INET (1998) has not been read.**
    ///
    /// Sensitivity is low for this transient: the excursion is arrested by
    /// thermal feedback over hundreds of seconds, far slower than either
    /// candidate `Lambda`.
    pub const HTR10_PROMPT_GENERATION_TIME_S: f64 = 1.68e-3;

    /// Isothermal temperature coefficient of reactivity \[K^-1\].
    ///
    /// **-1.4e-4 dk/k per degC**, Chen et al. (2009) Table 1, used as the
    /// single lumped feedback coefficient of the post-test THERMIX analysis.
    /// Per degC and per K are numerically identical for a *coefficient*, so
    /// this is used directly as a `per_kelvin` quantity.
    ///
    /// **Why this and not the IAEA-TECDOC-1382 Table 4-33 values** already
    /// transcribed in `tampines::pebble_bed::feedback` (-7.37e-5 to
    /// -9.15e-5 /K): those are benchmark states spanning 20-250 degC, and
    /// their magnitude grows with temperature (-7.49e-5 over 20-120 degC
    /// against -9.15e-5 over 120-250 degC). This transient runs at 212-650 degC,
    /// above all of them. -1.4e-4 is the value evaluated for the test
    /// condition, and it is the one to use here.
    ///
    /// **A second, unresolved discrepancy.** Hu et al. (2006) section 2.1
    /// gives a *split* the workspace otherwise lacks entirely — fuel
    /// -1.93e-5, moderator -1.49e-5, reflector **+7.08e-6**, all as
    /// `$ per degC`. Those do not reconcile with Chen's total: summed and
    /// converted at `beta = 7.26e-3` they give about -2e-7 dk/k per degC,
    /// three orders of magnitude small. Read as `10^-2 $/degC` the fuel term
    /// alone would give -1.401e-4 dk/k per degC, matching Chen exactly, which
    /// suggests a printed exponent is wrong — but the rendered PDF really does
    /// read `10^-5`, so this is recorded rather than silently corrected. The
    /// split is what a two-channel Doppler/graphite model would need; do not
    /// use it until the exponent is settled against INET (1998).
    pub const HTR10_TEMPERATURE_COEFFICIENT_PER_K: f64 = -1.4e-4;

    pub fn new_htr10_published(reference_power: Power) -> Self {
        let prompt_generation_time = Time::new::<second>(Self::HTR10_PROMPT_GENERATION_TIME_S);

        // Seeded SATURATED, not clean: the simulator opens at its operating
        // point, where a real core has been running long enough for the
        // fission-product inventory to have reached equilibrium. Starting the
        // groups cold would show zero decay heat at t=0 and then a spurious
        // several-minute climb to equilibrium that no operator would ever see.
        let decay = DecayHeat::new_at_equilibrium(FissioningNuclide::U235Thermal, reference_power);
        let f_prompt = decay.prompt_power_fraction().get::<ratio>();

        // The design point, read from the bed rather than restated: its seed
        // temperature, and the fuel's steady offset above it at rated power
        // through the resolved-pebble coupling. At equilibrium the thermal
        // source is `f_prompt P + P_decay = P`, so the offset is `R P`.
        let bed_reference = super::pebble_bed::PebbleBedPorousMediaNode::new().pebble_temperature();
        let coupling = super::pebble_bed::FuelBedCoupling::at_design_point();
        let fuel_reference = bed_reference + coupling.resistance * reference_power;
        let fuel_heat_capacity = super::pebble_bed::fuel_node_heat_capacity(fuel_reference);
        let feedback_split = FeedbackSplit::htr10_published(bed_reference);

        let prompt = NordheimFuchsExactTimestepper::new(
            prompt_generation_time,
            Ratio::new::<ratio>(Self::HTR10_EFFECTIVE_DELAYED_FRACTION),
            // `C_fuel / f_prompt`: the closed form heats with the full
            // point-kinetics power, so this makes its adiabatic heating
            // exactly the promptly released share. See the module doc.
            fuel_heat_capacity / f_prompt,
            feedback_split.fuel_coefficient(),
            fuel_reference,
            fuel_reference,
            reference_power,
        )
        .expect("published HTR-10 kinetics parameters must satisfy NordheimFuchs preconditions");

        // ONE beta (gh:#387): the U-235 five-group shape, rescaled so the
        // groups sum to the prompt layer's published beta_eff. Built EMPTY --
        // see the module doc's "Read this first".
        let delayed = DelayedNeutronLayer::u235_five_group_with_total_fraction(
            prompt_generation_time,
            Ratio::new::<ratio>(Self::HTR10_EFFECTIVE_DELAYED_FRACTION),
        )
        .expect("published HTR-10 beta_eff is in (0, 1) and Lambda > 0");

        Self {
            prompt,
            delayed,
            decay,
            prompt_power: reference_power,
            delayed_increment: Power::new::<watt>(0.0),
            total_power: reference_power,
            // Xenon is OFF by default, so this constructor reproduces the
            // simulator's behaviour before the channel existed. Callers opt
            // in with `enable_xenon_at_equilibrium`.
            xenon: None,
            feedback_split,
            fuel_heat_capacity,
            fuel_to_bed_resistance: coupling.resistance,
            bed_temperature: bed_reference,
            heat_to_bed: reference_power,
            ledger: FuelNodeLedger::default(),
            last_external_reactivity_dollars: 0.0,
        }
    }

    /// ~~Cool the reactivity-feedback fuel node by the heat the coolant
    /// actually carried away over `dt`: `T_f <- T_f - Q_removed dt / C_f`~~
    /// **REPLACED 2026-09-28 (gh:#360).** The fuel node's only sink is
    /// conduction to the bed through the resolved-pebble resistance, driven
    /// by `T_f - T_bed` — not the bed's heat-to-helium, which was a sink sized
    /// for a different node.
    ///
    /// Applied as the **exact** solution of `C dT/dt = -(T - T_bed)/R` over
    /// `dt` with `T_bed` held (the bed moves on ~180 s against a 1 ms substep):
    ///
    /// ```text
    /// T_f' = T_bed + (T_f - T_bed) exp(-dt / (R C_fuel))
    /// ```
    ///
    /// Unconditionally stable at any `dt`, and the energy it removes,
    /// `C_fuel (T_f - T_f')`, is returned so the caller hands the bed exactly
    /// that. Why a separate smooth operator rather than a term inside the
    /// closed form is the same argument the retired coolant sink made, and it
    /// still holds: the stiff power-temperature coupling stays analytic; the
    /// sink, `R C_fuel ~ 0.1-0.3 s` against a 1 ms substep, is well resolved
    /// as a Lie split.
    fn transfer_heat_to_bed(&mut self, dt: Time) -> f64 {
        use uom::si::thermodynamic_temperature::kelvin;
        let c = self.fuel_heat_capacity.get::<joule_per_kelvin>();
        let r = self.fuel_to_bed_resistance.get::<kelvin_per_watt>();
        if !(c > 0.0 && r > 0.0) {
            return 0.0;
        }
        let t_f = self.prompt.fuel_temperature.get::<kelvin>();
        let t_b = self.bed_temperature.get::<kelvin>();
        let relaxed = t_b + (t_f - t_b) * (-dt.get::<second>() / (r * c)).exp();
        self.prompt.fuel_temperature = ThermodynamicTemperature::new::<kelvin>(relaxed);
        let moved = c * (t_f - relaxed);
        self.ledger.conducted_to_bed += moved;
        self.ledger.stored -= moved;
        moved
    }

    /// The fuel node's temperature -- the **inventory-averaged kernel
    /// temperature** since 2026-09-28 (gh:#360). ~~"the Nordheim-Fuchs node,
    /// now with a coolant heat sink"~~ -- it is still the Nordheim-Fuchs node,
    /// but a node of the energy chain now, losing heat only to the bed.
    pub fn fuel_temperature(&self) -> ThermodynamicTemperature {
        self.prompt.fuel_temperature
    }

    /// Advance the kinetics by one timestep with the Lie-split coupling.
    ///
    /// `external_reactivity_dollars` is the user-commanded reactivity in
    /// dollars (`rho/beta`); it is converted to the prompt layer's
    /// dimensionless `rho_ext = dollars * beta` and held constant over the
    /// step.
    ///
    /// # This is a multi-rate sub-model, like the steam generator
    ///
    /// `dt` is subdivided into whole pieces no longer than
    /// [`super::KINETICS_SUBSTEP_S`], and [`Self::advance_one`] is run on each.
    /// **This is not decoration -- it is the difference between a right and a
    /// wrong answer at the plant timestep.** The prompt layer relaxes on
    /// `Lambda / beta = 1e-3 / 0.0065 = 0.154 s`, which is the *only* timescale
    /// in this plant comparable to [`super::PLANT_TIMESTEP_S`] = 0.1 s. The
    /// Lie split between the prompt layer and the precursor bank is first-order
    /// accurate in the step, so at `dt / tau = 0.65` it is badly resolved.
    ///
    /// Measured 2026-08-13 on the flow-ramp transient of
    /// `super::tests::the_plant_outer_correctors_converge`, which drives the
    /// reactor deeply subcritical through its own temperature feedback -- the
    /// hardest case for this split, because the power is decaying fast:
    ///
    /// | Kinetics substep | Reactor power at 60 s | vs the 1 ms reference |
    /// |---|---|---|
    /// | 0.1 s (none -- one piece per plant step) | 0.02703 MW | **-84.5%** |
    /// | 0.01 s | 0.14071 MW | **-19.3%** |
    /// | **0.001 s (shipped)** | **0.17428 MW** | **+0.0000%** |
    ///
    /// The exact agreement in the last row is structural rather than a
    /// convergence result -- both runs then integrate the kinetics at 1 ms, and
    /// the kinetics is decoupled from everything the plant timestep governs.
    /// See [`super::KINETICS_SUBSTEP_S`].
    ///
    /// The plant's outer correctors cannot fix this, because the kinetics is
    /// **not coupled** to anything the corrector loop iterates (see
    /// [`super::HtgrPlant::step`]) -- it depends only on the commanded
    /// reactivity and its own adiabatic fuel temperature. Sub-stepping is the
    /// only remedy, and it is nearly free: one `atanh` and five first-order
    /// transfer-function updates per substep, against three coupled array
    /// solves for the steam generator.
    ///
    /// Subdividing to a **maximum** substep rather than a fixed count means a
    /// caller already stepping finer than [`super::KINETICS_SUBSTEP_S`] -- the
    /// 1 ms reference leg of the accuracy test, for instance -- pays nothing
    /// extra.
    /// ~~`coolant_heat_removal` is the heat the coolant is currently carrying
    /// off the fuel node, applied **inside** the substep loop~~ **CHANGED
    /// 2026-09-28 (gh:#360)** -- the fuel node's sink is conduction to the bed
    /// (`bed_temperature`, `fuel_to_bed_resistance`), still applied inside the
    /// substep loop for the reason recorded here: a sink applied once per
    /// plant step leaves the feedback reading a temperature 0.1 s stale on the
    /// cooling side (measured 2026-08-14 on the old sink: -1.27 % power drift
    /// against the 1 ms reference).
    ///
    /// - `bed_temperature` -- the corrector's current estimate of the bed
    ///   temperature. The sink's target and the moderator channel's
    ///   temperature, both held over the step (the bed moves on ~180 s).
    /// - `fuel_to_bed_resistance` -- the bed's most recent coupling
    ///   ([`super::pebble_bed::FuelBedCoupling::resistance`]); `None` keeps
    ///   the previous step's.
    ///
    /// After the call, [`Self::heat_to_bed`] is the average fuel-to-bed heat
    /// over the step -- the bed's source term.
    pub fn step(
        &mut self,
        dt: Time,
        external_reactivity_dollars: f64,
        bed_temperature: ThermodynamicTemperature,
        fuel_to_bed_resistance: Option<ThermalResistance>,
    ) {
        use uom::si::thermodynamic_temperature::kelvin;
        if let Some(r) = fuel_to_bed_resistance {
            self.fuel_to_bed_resistance = r;
        }
        self.bed_temperature = bed_temperature;
        self.last_external_reactivity_dollars = external_reactivity_dollars;
        // The fuel capacity moves with temperature (UO2, SiC, carbon cp); it is
        // refreshed once per plant step at the start-of-step fuel temperature,
        // and the closed form's `C_fuel / f_prompt` with it.
        let f_prompt = self.decay.prompt_power_fraction().get::<ratio>();
        self.fuel_heat_capacity =
            super::pebble_bed::fuel_node_heat_capacity(self.prompt.fuel_temperature);
        self.prompt.fuel_heat_capacity = self.fuel_heat_capacity / f_prompt;
        let beta = self.prompt.delayed_neutron_fraction.get::<ratio>();
        let rho_moderator_dollars = self
            .feedback_split
            .moderator_reactivity_dollars(bed_temperature, beta);

        let dt_s = dt.get::<second>();
        let pieces = if dt_s > super::KINETICS_SUBSTEP_S {
            (dt_s / super::KINETICS_SUBSTEP_S).ceil().max(1.0)
        } else {
            1.0
        };
        let sub = Time::new::<second>(dt_s / pieces);
        let c = self.fuel_heat_capacity.get::<joule_per_kelvin>();
        let mut conducted = 0.0;
        for _ in 0..(pieces as usize) {
            // The xenon channel is an EXTERNAL reactivity contribution, added
            // to whatever the rods are worth. It is evaluated from the
            // concentration carried into this substep, then advanced, so the
            // kinetics never sees a xenon worth that depends on the power it
            // is about to produce.
            let rho_xenon_dollars = self.xenon_reactivity_dollars();
            let before = self.prompt.fuel_temperature.get::<kelvin>();
            self.advance_one(
                sub,
                external_reactivity_dollars + rho_xenon_dollars + rho_moderator_dollars,
            );
            let deposited = c * (self.prompt.fuel_temperature.get::<kelvin>() - before);
            self.ledger.deposited_prompt += deposited;
            self.ledger.stored += deposited;
            self.apply_decay_heat(sub);
            conducted += self.transfer_heat_to_bed(sub);
            self.advance_xenon(sub);
        }
        self.heat_to_bed = Power::new::<watt>(if dt_s > 0.0 { conducted / dt_s } else { 0.0 });
    }

    /// Heat conducted from the fuel node to the bed, averaged over the most
    /// recent [`Self::step`] -- the bed's source term (before the passive
    /// loss is taken off it).
    pub fn heat_to_bed(&self) -> Power {
        self.heat_to_bed
    }

    /// Cumulative energy ledger of the fuel node. See [`FuelNodeLedger`].
    pub fn ledger(&self) -> FuelNodeLedger {
        self.ledger
    }

    /// Rebuild the feedback split with fuel share `fuel_share` -- **the
    /// ablation entry point**. `f` is clamped to `[1e-3, 1]`:
    /// `teh_o_prke`'s Nordheim-Fuchs rejects a non-negative coefficient
    /// (its closed form divides by it), so `f = 0` -- the whole coefficient on
    /// the bed -- is approached, not reached; `f = 1e-3` leaves 0.1 % on the
    /// fuel node. ~~`0.0` restores the pre-2026-09-22 model exactly~~ (that
    /// model -- a bed-tracking shadow node -- no longer exists to restore).
    #[cfg(test)] // test-only: no GUI/headless control reaches it (2026-09-29 dead-code pass)
    pub fn set_fuel_share(&mut self, fuel_share: f64) {
        self.feedback_split.fuel_share = fuel_share.clamp(1.0e-3, 1.0);
        self.prompt.fuel_feedback_coefficient = self.feedback_split.fuel_coefficient();
    }

    /// The feedback split, for display and for tests.
    #[cfg(test)] // test-only: no GUI/headless control reaches it (2026-09-29 dead-code pass)
    pub fn feedback_split(&self) -> &FeedbackSplit {
        &self.feedback_split
    }

    /// The fuel channel's reactivity in dollars, `alpha_fuel (T_f - T_f,ref) / beta`.
    pub fn fuel_feedback_reactivity_dollars(&self) -> f64 {
        use uom::si::temperature_coefficient::per_kelvin;
        use uom::si::thermodynamic_temperature::kelvin;
        let beta = self.prompt.delayed_neutron_fraction.get::<ratio>();
        self.prompt.fuel_feedback_coefficient.get::<per_kelvin>()
            * (self.prompt.fuel_temperature.get::<kelvin>()
                - self.prompt.fuel_reference_temperature.get::<kelvin>())
            / beta
    }

    /// The moderator (bed) channel's reactivity in dollars at the most recent
    /// step's bed temperature.
    pub fn moderator_feedback_reactivity_dollars(&self) -> f64 {
        let beta = self.prompt.delayed_neutron_fraction.get::<ratio>();
        self.feedback_split
            .moderator_reactivity_dollars(self.bed_temperature, beta)
    }

    /// The external (rod) reactivity the most recent step integrated \[$\],
    /// in the kinetics' own dollars (converted to `rho` with
    /// [`Self::kinetics_delayed_neutron_fraction`]).
    pub fn external_reactivity_dollars(&self) -> f64 {
        self.last_external_reactivity_dollars
    }

    /// **The net reactivity the kinetics integrates** \[$\]: external (rods)
    /// + fuel/Doppler + moderator + xenon, every term in the kinetics' own
    /// dollars. This is the sum the Nordheim-Fuchs step sees -- `rho = beta
    /// (external + xenon + moderator) + alpha_fuel (T_f - T_ref)` -- divided
    /// by the same `beta`. Evaluated at the end of the most recent step
    /// (the xenon term at the end-of-step inventory), so it is a budget of the
    /// current state, not an average over the step.
    pub fn net_reactivity_dollars(&self) -> f64 {
        self.external_reactivity_dollars()
            + self.fuel_feedback_reactivity_dollars()
            + self.moderator_feedback_reactivity_dollars()
            + self.xenon_reactivity_dollars()
    }

    /// The delayed-neutron fraction the kinetics convert every dollar term
    /// with (the Nordheim-Fuchs stepper's `beta`, 7.26e-3 published). Since
    /// 2026-09-29 (gh:#387) it equals the delayed layer's `sum(beta_i)`
    /// ([`Self::delayed_neutron_fraction`]) and the rod-worth conversion uses
    /// it; ~~the layer summed to 0.0065 and the rods used that~~.
    pub fn kinetics_delayed_neutron_fraction(&self) -> Ratio {
        self.prompt.delayed_neutron_fraction
    }

    /// Reactivity worth of the current Xe-135 inventory, in dollars.
    ///
    /// Zero when the xenon channel is disabled, which is the default and
    /// reproduces this simulator's behaviour before xenon existed.
    pub fn xenon_reactivity_dollars(&self) -> f64 {
        let Some(xenon) = self.xenon.as_ref() else {
            return 0.0;
        };
        xenon.reactivity_dollars()
    }

    /// Advance the iodine/xenon chain over `dt`, driven by the fission power
    /// produced in this substep.
    fn advance_xenon(&mut self, dt: Time) {
        let fission_power = self.total_power;
        if let Some(xenon) = self.xenon.as_mut() {
            xenon.advance(dt, fission_power);
        }
    }

    /// Enable the Xe-135 channel, seeded at equilibrium for `power`.
    ///
    /// Seeding at equilibrium rather than clean is the physically right
    /// opening state for a reactor that has been at power: the HTR-10 LOFC
    /// ATWS test was run on a core that had been operating, so its xenon was
    /// saturated when the circulator tripped.
    #[cfg(test)] // test-only: no GUI/headless control reaches it (2026-09-29 dead-code pass)
    pub fn enable_xenon_at_equilibrium(&mut self, power: Power) {
        self.xenon = Some(XenonChannel::new_at_equilibrium(power));
    }

    /// Heat the fuel node by the fission-product decay heat generated over
    /// `dt`, at the **real** fuel capacity (the closed form's `C / f_prompt`
    /// is only for the prompt share).
    ///
    /// Decay heat is deposited in the fuel, where the fission products are
    /// (maintainer direction 2026-09-28, gh:#360 comment, point 1). History:
    /// this method was added 2026-08-17 (GitHub issue #22) because the old
    /// shadow node was charged a coolant sink sized for heat it never
    /// received; ~~its measured "+0.04 K after it"~~ held only post-scram --
    /// at power the same node drifted 234.9 K above the bed (gh:#360), half of
    /// it because the closed form ALSO heated with the full fission power.
    /// Both halves are gone: the closed form heats with `f_prompt P` and the
    /// sink is conduction to the bed.
    ///
    /// Call every substep, right after [`Self::advance_one`] has refreshed
    /// [`Self::decay_heat_power`].
    fn apply_decay_heat(&mut self, dt: Time) {
        let c = self.fuel_heat_capacity.get::<joule_per_kelvin>();
        if c <= 0.0 {
            return;
        }
        let energy = self.decay_heat_power().get::<watt>() * dt.get::<second>();
        self.prompt.fuel_temperature +=
            TemperatureInterval::new::<temperature_interval::kelvin>(energy / c);
        self.ledger.deposited_decay += energy;
        self.ledger.stored += energy;
    }

    /// One Lie-split kinetics substep. See [`Self::step`], which is the entry
    /// point callers should use -- it subdivides `dt` for accuracy.
    fn advance_one(&mut self, dt: Time, external_reactivity_dollars: f64) {
        let beta = self.prompt.delayed_neutron_fraction.get::<ratio>();
        self.prompt
            .set_external_reactivity(Ratio::new::<ratio>(external_reactivity_dollars * beta));

        // 1. Prompt substep -> P_p.
        self.prompt.step(dt);
        let prompt_power = self.prompt.power;

        // 2. Delayed substep -> increment S*dt.
        let increment = self.delayed.advance(prompt_power, dt);

        // 3. Total power, fed back into the prompt model so the next step's
        //    dynamics and the fuel heating see the full point-kinetics power.
        let total = prompt_power + increment;
        self.prompt.power = total;

        // 4. Fission-product decay-heat bank, driven by the FISSION power
        //    only. Feeding the decay heat back in here would count the same
        //    energy twice -- `DecayHeat::advance_timestep` says so explicitly.
        //    Integrated on the kinetics substep because the fastest of the 23
        //    groups has a ~45 ms time constant, which the 0.1 s plant step
        //    would not resolve.
        self.decay.advance_timestep(total, dt);

        self.prompt_power = prompt_power;
        self.delayed_increment = increment;
        self.total_power = total;
    }

    /// Fission-product decay-heat power, summed over all 23 groups.
    ///
    /// Non-zero after shutdown -- this is the term that keeps heating the
    /// graphite when the chain reaction has stopped.
    pub fn decay_heat_power(&self) -> Power {
        self.decay.total_decay_heat_power()
    }

    /// **The heat source the core actually sees**: the promptly-released part
    /// of the fission power plus the fission-product decay heat.
    ///
    /// ```text
    /// P_thermal = prompt_power_fraction * P_fission + P_decay
    /// ```
    ///
    /// The prompt term is scaled by
    /// [`DecayHeat::prompt_power_fraction`] (0.9341 for U-235 thermal at
    /// 200 MeV/fission) because the decay groups already account for the
    /// 13.18 MeV/fission that emerges later; adding the two unscaled would
    /// overstate core power by about 7%.
    ///
    /// At equilibrium the two terms sum back to [`Self::total_power`], so the
    /// steady state is unchanged by introducing decay heat. After a trip the
    /// first term collapses with the flux while the second decays over hours,
    /// which is the whole point.
    ///
    /// This -- not [`Self::total_power`] -- is what should be handed to
    /// [`super::pebble_bed`].
    pub fn core_thermal_power(&self) -> Power {
        self.total_power * self.decay.prompt_power_fraction() + self.decay_heat_power()
    }

    /// Prompt-excursion-layer power `P_p` (before the delayed increment is
    /// added back).
    pub fn prompt_power(&self) -> Power {
        self.prompt_power
    }

    /// Delayed-neutron power increment `S*dt` from the most recent step.
    pub fn delayed_power(&self) -> Power {
        self.delayed_increment
    }

    /// Total reactor power `P = P_p + increment`.
    pub fn total_power(&self) -> Power {
        self.total_power
    }

    /// Effective total delayed-neutron fraction `beta = sum(beta_i)` reported
    /// by the delayed-neutron layer (dimensionless): 7.26e-3, the same `beta`
    /// as the prompt layer (gh:#387).
    pub fn delayed_neutron_fraction(&self) -> Ratio {
        self.delayed.total_delayed_neutron_fraction()
    }

    /// Current reactivity margin `r = rho_ext - beta + alpha_f*(T_f - T_ref)`
    /// expressed in **dollars** (`r/beta`), for display.
    pub fn reactivity_margin_dollars(&self) -> f64 {
        let beta = self.prompt.delayed_neutron_fraction.get::<ratio>();
        self.prompt.reactivity_margin().get::<ratio>() / beta
    }
}

/// Convenience: power in megawatts (for snapshot scalars / plots).
pub fn power_in_megawatts(p: Power) -> f64 {
    p.get::<megawatt>()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The reactivity budget sums to the net, in the kinetics' own terms**
    /// (panel reactivity budget, 2026-09-29).
    ///
    /// Methodology: a published-HTR-10 kinetics with the xenon channel on,
    /// stepped 20 s at 0.1 s against a bed held at 1000 K with an external
    /// +0.5 $; every step require `external + fuel + moderator + xenon =
    /// net` to 1e-12 absolute (dollars), the recorded external equal to what
    /// was handed in, and `pcm = $ x beta_kinetics x 1e5` for each term with
    /// `beta_kinetics = 7.26e-3`.
    #[test]
    fn the_reactivity_budget_sums_to_the_net_in_kinetics_dollars() {
        let mut k = HtgrKinetics::new_htr10_published(Power::new::<megawatt>(10.0));
        k.enable_xenon_at_equilibrium(Power::new::<megawatt>(10.0));
        let beta = k.kinetics_delayed_neutron_fraction().get::<ratio>();
        assert!((beta - 7.26e-3).abs() < 1e-15);
        for _ in 0..200 {
            k.step(
                Time::new::<second>(0.1),
                0.5,
                ThermodynamicTemperature::new::<kelvin>(1000.0),
                None,
            );
            let terms = [
                k.external_reactivity_dollars(),
                k.fuel_feedback_reactivity_dollars(),
                k.moderator_feedback_reactivity_dollars(),
                k.xenon_reactivity_dollars(),
            ];
            assert_eq!(terms[0], 0.5);
            let net = k.net_reactivity_dollars();
            assert!((terms.iter().sum::<f64>() - net).abs() < 1e-12);
            for t in terms {
                let pcm = crate::app::state::reactivity_pcm(t, beta);
                assert!((pcm - t * beta * 1e5).abs() < 1e-12 * pcm.abs().max(1.0));
            }
        }
        assert!(
            k.xenon_reactivity_dollars() < 0.0,
            "the xenon channel must be on"
        );
    }
    use uom::si::heat_capacity::joule_per_kelvin;
    use uom::si::power::megawatt;
    use uom::si::thermodynamic_temperature::kelvin;

    fn rated() -> Power {
        Power::new::<megawatt>(10.0)
    }

    fn design_bed() -> ThermodynamicTemperature {
        super::super::pebble_bed::PebbleBedPorousMediaNode::new().pebble_temperature()
    }

    /// V&V: introducing decay heat must NOT move the steady state.
    ///
    /// **Methodology.** The decay bank is seeded at equilibrium for 10 MWth, so
    /// by construction `prompt_power_fraction * P_fission + P_decay` must sum
    /// back to `P_fission`. This is the property that makes the split
    /// self-consistent rather than a tuned constant: if it failed, adding
    /// decay heat would silently rescale the whole plant. Pass criterion:
    /// [`HtgrKinetics::core_thermal_power`] within 0.1% of the rated power at
    /// t = 0, and the decay fraction within the physically expected 6-7% band
    /// for U-235 thermal at 200 MeV/fission (13.183/200 = 6.59%).
    ///
    /// **Results (2026-08-14).** Printed below; the equilibrium sum reproduces
    /// the rated power to round-off and the decay share is 6.59%, exactly
    /// `1 - prompt_power_fraction`.
    #[test]
    fn decay_heat_at_equilibrium_does_not_move_the_steady_state() {
        let k = HtgrKinetics::new_htr10_published(rated());
        let thermal = k.core_thermal_power().get::<megawatt>();
        let decay = k.decay_heat_power().get::<megawatt>();
        let share = decay / thermal;
        println!(
            "equilibrium: fission {:.6} MW, decay {:.6} MW ({:.3}%), core thermal {:.6} MW",
            k.total_power().get::<megawatt>(),
            decay,
            share * 100.0,
            thermal
        );
        assert!(
            (thermal - 10.0).abs() / 10.0 < 1.0e-3,
            "core thermal power {thermal} MW must equal the rated 10 MW at equilibrium"
        );
        assert!(
            (0.06..0.07).contains(&share),
            "decay share {share} is outside the expected 6-7% for U-235 thermal"
        );
    }

    /// V&V: after a deep shutdown the core must still be producing decay heat.
    ///
    /// **Methodology.** The kinetics is driven hard subcritical (-10 $, far
    /// below prompt-critical in the negative direction) and advanced for 60 s
    /// of simulated time. Fission power must collapse; decay heat must NOT,
    /// because the 23-group bank has decay constants spanning fifteen orders of
    /// magnitude and the long groups barely move in a minute. Pass criterion:
    /// fission power falls below 1% of rated while core thermal power stays
    /// above 1% of rated -- i.e. the reactor cannot be switched off.
    ///
    /// **Results (2026-08-14).** Printed below. This is the behaviour the
    /// simulator could not depict at all before decay heat was wired in: rods
    /// in took core power to zero and the graphite simply cooled.
    #[test]
    fn decay_heat_survives_a_shutdown() {
        let mut k = HtgrKinetics::new_htr10_published(rated());
        let dt = Time::new::<second>(0.1);
        for _ in 0..600 {
            // The bed held at its design point: this isolates the decay-heat
            // behaviour from the thermal-hydraulics, which is the point of the
            // test. (CHANGED 2026-09-28: the fuel node's sink is conduction to
            // the bed now, not a coolant-removal argument.)
            k.step(dt, -10.0, design_bed(), None);
        }
        let fission = k.total_power().get::<megawatt>();
        let decay = k.decay_heat_power().get::<megawatt>();
        let thermal = k.core_thermal_power().get::<megawatt>();
        println!(
            "60 s after a -10 $ trip: fission {fission:.6} MW, decay {decay:.6} MW, \
             core thermal {thermal:.6} MW"
        );
        assert!(
            fission < 0.1,
            "fission power {fission} MW should have collapsed after a deep trip"
        );
        assert!(
            thermal > 0.1,
            "core thermal power {thermal} MW must stay up on decay heat -- a reactor \
             cannot be switched off"
        );
    }

    /// V&V: the fuel node relaxes to the bed **exactly**, and hands the bed
    /// exactly the energy it gives up (gh:#360, 2026-09-28).
    ///
    /// Replaces ~~`the_fuel_node_cools_smoothly_rather_than_stiffly`~~, which
    /// tested the retired coolant sink.
    ///
    /// **Methodology.** Displace the fuel node 50 K above a bed held at the
    /// design point, then apply only the fuel-to-bed operator for 2 s in 1 ms
    /// steps. Pass criteria: (1) monotone, no overshoot below the bed (a stiff
    /// explicit sink would oscillate); (2) the trajectory matches
    /// `T_bed + 50 exp(-t / (R C))` to 1e-9 K; (3) the energy handed to the bed
    /// equals `C x (drop)` to 1e-9 relative.
    ///
    /// **Results (2026-09-28).** `C_fuel = 2.6830e5 J/K`, `R = 1.0250e-6 K/W`,
    /// so the fuel node's time constant `R C = 0.2750 s`; 49.965 K relaxed in
    /// 2 s, worst deviation from the exact exponential 8e-13 K, energy to the
    /// bed equal to `C x drop` (1.3406e7 J) to round-off.
    /// constant at the design point.
    #[test]
    fn the_fuel_node_relaxes_to_the_bed_exactly() {
        use uom::si::thermodynamic_temperature::kelvin as k_unit;
        let mut k = HtgrKinetics::new_htr10_published(rated());
        let bed = design_bed();
        k.bed_temperature = bed;
        k.prompt.fuel_temperature =
            ThermodynamicTemperature::new::<k_unit>(bed.get::<k_unit>() + 50.0);
        let c = k.fuel_heat_capacity.get::<joule_per_kelvin>();
        let r = k.fuel_to_bed_resistance.get::<kelvin_per_watt>();
        let tau = r * c;
        let dt = Time::new::<second>(1.0e-3);
        let mut moved = 0.0;
        let mut previous = k.fuel_temperature().get::<k_unit>();
        let mut worst = 0.0f64;
        for i in 1..=2000 {
            moved += k.transfer_heat_to_bed(dt);
            let now = k.fuel_temperature().get::<k_unit>();
            assert!(now <= previous && now >= bed.get::<k_unit>(), "step {i}");
            let exact = bed.get::<k_unit>() + 50.0 * (-(i as f64) * 1.0e-3 / tau).exp();
            worst = worst.max((now - exact).abs());
            previous = now;
        }
        let drop = bed.get::<k_unit>() + 50.0 - previous;
        println!(
            "fuel node: C = {c:.4e} J/K, R = {r:.4e} K/W, tau = R C = {tau:.4} s; \
             drop {drop:.4} K in 2 s, worst deviation from the exponential {worst:.2e} K, \
             energy to bed {moved:.4e} J vs C x drop {:.4e} J",
            c * drop
        );
        assert!(worst < 1e-9);
        assert!((moved - c * drop).abs() < 1e-9 * moved);
    }

    /// V&V: the two feedback channels must **sum back to the published
    /// isothermal coefficient**, at every fuel share.
    ///
    /// # Why this is the load-bearing check
    ///
    /// Splitting one published coefficient into two channels is the step where
    /// reactivity gets created or destroyed by accident. `alpha_iso` is
    /// Chen *et al.* (2009)'s **isothermal** value -- fuel and graphite moving
    /// together -- so the only split that conserves it is one where the two
    /// parts add back up.
    ///
    /// **Methodology (rewritten 2026-09-28 for [`FeedbackSplit`]).** Over fuel
    /// shares 1e-3, 0.25, `HU_FUEL_SHARE`, 0.75, 1 and two out-of-range values
    /// (-0.5, 1.5) set through [`HtgrKinetics::set_fuel_share`], form
    /// `alpha_fuel + alpha_mod` and compare to `alpha_iso`; check the
    /// Nordheim-Fuchs node carries `alpha_fuel`; check the clamp lands on
    /// `[1e-3, 1]` (Nordheim-Fuchs rejects a non-negative coefficient, so
    /// `f = 0` is approached, not reached).
    ///
    /// **Results (2026-09-28).** Sum exact (0.0 /K) at every share; -0.5 and
    /// 1e-3 both clamp to `alpha_fuel = -1.4e-7 /K`, 1.5 to `-1.4e-4 /K`; at
    /// the shipped default `alpha_fuel = -9.96313e-5 /K` on the fuel node and
    /// `alpha_mod = -4.03687e-5 /K` on the bed -- the same numbers the retired
    /// kernel channel recorded on 2026-09-22, since the ratio is unchanged.
    #[test]
    fn the_split_channels_sum_to_the_published_isothermal_coefficient() {
        use uom::si::temperature_coefficient::per_kelvin;

        let alpha_iso = HtgrKinetics::HTR10_TEMPERATURE_COEFFICIENT_PER_K;
        let mut worst = 0.0f64;
        let mut k = HtgrKinetics::new_htr10_published(rated());
        for share in [
            -0.5,
            1.0e-3,
            0.25,
            FeedbackSplit::HU_FUEL_SHARE_OF_ISOTHERMAL,
            0.75,
            1.0,
            1.5,
        ] {
            k.set_fuel_share(share);
            let split = *k.feedback_split();
            let a_f = split.fuel_coefficient().get::<per_kelvin>();
            let a_m = split.moderator_coefficient().get::<per_kelvin>();
            worst = worst.max((a_f + a_m - alpha_iso).abs());
            let clamped = share.clamp(1.0e-3, 1.0);
            assert!((a_f - clamped * alpha_iso).abs() < 1e-18, "share {share}");
            assert_eq!(
                k.prompt.fuel_feedback_coefficient.get::<per_kelvin>(),
                a_f,
                "the closed form must carry the fuel share"
            );
            println!("f = {share:>6} -> alpha_fuel = {a_f:+.5e} /K, alpha_mod = {a_m:+.5e} /K");
        }
        println!("worst |alpha_fuel + alpha_mod - alpha_iso| = {worst:.3e} /K");
        assert!(worst < 1e-18);
    }

    /// V&V: the rated design point is **neutral**, and a **thermal fixed
    /// point** of the fuel node (gh:#360, 2026-09-28).
    ///
    /// **Methodology.** Build the kinetics at 10 MWth with the bed at its
    /// 950 K design temperature. Check (1) both feedback channels read zero at
    /// t = 0; (2) the fuel node opens exactly `R P` above the bed; (3) over one
    /// 1 ms substep the fuel temperature moves by less than 1e-3 K -- against
    /// the +0.037 K one substep's deposit alone would give, so source
    /// `f_prompt P + P_decay` and sink `(T_f - T_bed)/R` balance at the design
    /// point -- and the heat handed to the bed equals the core thermal power
    /// to 1 %. The tolerances are derived, not chosen after the fact: the Lie
    /// split (deposit, then relax) puts the discrete fixed point
    /// `dt/(2 R C) = 0.18 %` above the continuous one, and power itself falls
    /// ~0.4 % in the first millisecond (see below), so a 1e-3 criterion on the
    /// heat -- the first one written -- was tighter than the scheme can meet
    /// and was widened for that stated reason.
    ///
    /// **What this deliberately does NOT assert: that power holds at 0 $.**
    /// It does not, and not because of this change: the delayed bank starts
    /// with **empty precursor groups** (~~and its five-group `beta` sums to
    /// 6.5e-3 against the prompt layer's 7.26e-3~~ -- one beta since
    /// 2026-09-29, gh:#387), so at zero inserted reactivity the prompt layer
    /// sees `rho - beta = -beta` with no delayed source yet, and power falls
    /// (measured 2026-09-28: 10 MW -> 0.255 MW in 20 s with the bed held; not
    /// re-measured since the single-beta change -- pending validation work).
    /// With the precursors at equilibrium it holds:
    /// `zero_net_dollars_with_equilibrium_precursors_is_stationary`.
    /// That is the pre-existing opening transient behind gh:#317/#318, recorded
    /// here so the next reader does not mistake it for a fuel-node defect.
    ///
    /// **Results (2026-09-28).** Fuel opens at 960.2501 K = 950 K + 10.2501 K
    /// (`R P` exactly); both channels 0 $; over one 1 ms substep the fuel moved
    /// -1.4e-4 K and the heat to the bed was 10.018 MW.
    /// V&V (gh:#387): **one beta, and near 0 $ the plant is stationary; the
    /// remaining offset is the kinetics' own Lie-split bias, not a beta
    /// mismatch.**
    ///
    /// **Methodology.** Build the kinetics at 10 MW; check the prompt layer's
    /// `beta` equals the delayed bank's `sum(beta_i)` (both 7.26e-3); seed the
    /// precursors at their 10 MW equilibrium; step 600 s at the 0.1 s plant
    /// step (1 ms kinetics substeps) with 0 $ external and the bed held at its
    /// 950 K design temperature. The fuel feedback is then the only thing that
    /// moves, so the net reactivity the plant settles at is the offset the
    /// discrete scheme needs to hold power.
    ///
    /// **Instrument, derived before the run's criterion was set.** The Lie
    /// split (prompt substep, then the delayed source from the prompt power)
    /// loses `exp(-a)(1 + a) ~ 1 - a^2/2` per substep, `a = beta dt / Lambda`,
    /// which a steady state must make up with `rho = beta^2 dt / (2 Lambda)`,
    /// i.e. `beta dt / (2 Lambda)` dollars = **2.16e-3 $** (1.57 pcm) at
    /// `dt = 1 ms`. Pass: settled net within 25 % of that (the `a^3` terms and
    /// the backward-Euler precursor lag are the tolerance), power changing by
    /// less than 1e-4 relative over the last 60 s, and the offset under 5 % of
    /// the old beta-mismatch bias (+76 pcm = +0.1047 $ at 7.26e-3).
    ///
    /// ~~Pass: power within 1e-3 relative of 10 MW throughout~~ -- the first
    /// criterion, written before the split bias was worked out; it failed
    /// (0.55 % in 60 s), which is what exposed the bias. Recorded rather than
    /// quietly replaced.
    ///
    /// **Results (2026-09-29):** net settles at **+2.37e-3 $** (1.72 pcm, +10 %
    /// of the analytic split bias) with the power at 9.866 MW (-1.34 %, the
    /// fuel cooling ~0.14 K to supply that reactivity); power changed 7e-5
    /// relative over the last 60 s. Against the old bookkeeping, where the
    /// same run needed +0.105 $, the offset is 44x smaller.
    #[test]
    fn zero_net_dollars_with_equilibrium_precursors_is_stationary() {
        let mut k = HtgrKinetics::new_htr10_published(rated());
        let beta_p = k.kinetics_delayed_neutron_fraction().get::<ratio>();
        let beta_d = k.delayed_neutron_fraction().get::<ratio>();
        assert!((beta_p - 7.26e-3).abs() < 1e-15 && (beta_d - beta_p).abs() < 1e-15);
        k.delayed.seed_at_equilibrium(rated());
        let bed = design_bed();
        let mut p_540 = 0.0;
        for i in 0..6000 {
            k.step(Time::new::<second>(0.1), 0.0, bed, None);
            if i == 5399 {
                p_540 = k.total_power().get::<megawatt>();
            }
        }
        let p_600 = k.total_power().get::<megawatt>();
        let net = k.net_reactivity_dollars();
        let split_bias = beta_p * super::super::KINETICS_SUBSTEP_S
            / (2.0 * HtgrKinetics::HTR10_PROMPT_GENERATION_TIME_S);
        let old_bias_dollars = (beta_p - 6.5e-3) / beta_p;
        println!(
            "0 $, equilibrium precursors, bed held 600 s: P {p_600:.6} MW, net {net:+.4e} $ \
             (split bias {split_bias:.4e} $, old beta-mismatch bias {old_bias_dollars:.4e} $), \
             last-60-s change {:.2e}",
            (p_600 - p_540).abs() / p_600
        );
        assert!(
            (net - split_bias).abs() < 0.25 * split_bias,
            "{net:e} vs {split_bias:e}"
        );
        assert!((p_600 - p_540).abs() / p_600 < 1e-4);
        assert!(net.abs() < 0.05 * old_bias_dollars);
    }

    /// **How long the EMPTY precursor bank takes to fill** (maintainer
    /// direction 2026-09-29, gh:#387: state it upfront, measure it).
    ///
    /// **Methodology.** The plant's own bank (U-235 shape, `sum(beta_i) =
    /// 7.26e-3`, `Lambda = 1.68e-3 s`), built empty, advanced at a held 10 MW
    /// in 1 ms steps (the kinetics substep). Record when the summed inventory
    /// first reaches 1 % of its equilibrium, and when it first comes within
    /// 1 % of it. Cross-check the second against the analytic deficit
    /// `sum_i w_i exp(-lambda_i t)`, `w_i = (beta_i/lambda_i) / sum_j
    /// (beta_j/lambda_j)`, within 1 s.
    ///
    /// **Results (2026-09-29):** printed below, and quoted in the module doc.
    #[test]
    fn how_long_the_empty_precursor_bank_takes_to_fill() {
        let k = HtgrKinetics::new_htr10_published(rated());
        let mut layer = k.delayed.clone();
        assert_eq!(layer.precursor_inventory(), 0.0);
        let mut eq = layer.clone();
        eq.seed_at_equilibrium(rated());
        let target = eq.precursor_inventory();
        let dt = Time::new::<second>(1e-3);
        let (mut t, mut reach_1pct, mut within_1pct) = (0.0, None, None);
        while within_1pct.is_none() && t < 2000.0 {
            layer.advance(rated(), dt);
            t += 1e-3;
            let f = layer.precursor_inventory() / target;
            if reach_1pct.is_none() && f >= 0.01 {
                reach_1pct = Some(t);
            }
            if f >= 0.99 {
                within_1pct = Some(t);
            }
        }
        let (a, b) = (reach_1pct.unwrap(), within_1pct.unwrap());
        let lam: Vec<f64> = layer
            .decay_constants()
            .iter()
            .map(|l| l.get::<uom::si::frequency::hertz>())
            .collect();
        let bet: Vec<f64> = layer
            .delayed_fractions()
            .iter()
            .map(|x| x.get::<ratio>())
            .collect();
        let norm: f64 = (0..5).map(|i| bet[i] / lam[i]).sum();
        let deficit = |t: f64| -> f64 {
            (0..5)
                .map(|i| bet[i] / lam[i] / norm * (-lam[i] * t).exp())
                .sum()
        };
        let (mut lo, mut hi) = (0.0, 2000.0);
        for _ in 0..100 {
            let mid = 0.5 * (lo + hi);
            if deficit(mid) > 0.01 {
                lo = mid
            } else {
                hi = mid
            }
        }
        println!(
            "empty bank at 10 MW: 1 % of equilibrium inventory at {a:.3} s; within 1 % of \
             equilibrium at {b:.1} s (analytic {hi:.1} s)"
        );
        assert!((b - hi).abs() < 1.0, "{b} vs {hi}");
    }

    #[test]
    fn the_design_point_is_neutral() {
        use uom::si::thermodynamic_temperature::kelvin as k_unit;
        let mut k = HtgrKinetics::new_htr10_published(rated());
        let bed = design_bed();
        let t0 = k.fuel_temperature().get::<k_unit>();
        let offset0 = t0 - bed.get::<k_unit>();
        let r = k.fuel_to_bed_resistance.get::<kelvin_per_watt>();
        println!(
            "design point: fuel {t0:.4} K = bed {:.2} K + {offset0:.4} K (R P = {:.4} K); \
             fuel channel {:+.3e} $, moderator channel {:+.3e} $",
            bed.get::<k_unit>(),
            r * 1.0e7,
            k.fuel_feedback_reactivity_dollars(),
            k.moderator_feedback_reactivity_dollars()
        );
        assert!((offset0 - r * 1.0e7).abs() < 1e-9);
        assert!(k.fuel_feedback_reactivity_dollars().abs() < 1e-12);
        assert!(k.moderator_feedback_reactivity_dollars().abs() < 1e-12);

        k.step(Time::new::<second>(1.0e-3), 0.0, bed, None);
        let moved = k.fuel_temperature().get::<k_unit>() - t0;
        let q_bed = k.heat_to_bed().get::<megawatt>();
        println!(
            "one 1 ms substep at the design point: fuel moved {moved:+.3e} K, heat to bed \
             {q_bed:.6} MW (rated thermal 10 MW)"
        );
        assert!(moved.abs() < 1e-3);
        assert!((q_bed - 10.0).abs() / 10.0 < 1e-2);
    }

    /// V&V **and ablation**: what the fuel share is worth across its range.
    ///
    /// [`FeedbackSplit::HU_FUEL_SHARE_OF_ISOTHERMAL`] is an **input** (a
    /// published ratio, not a derivation), so its contribution must be
    /// measured. Rewritten 2026-09-28: with the fuel a real node, the fuel
    /// channel's worth at a steady power `x P_rated` (bed held at 950 K) is
    /// `f alpha_iso R P_rated (x - 1) / beta` -- the moderator channel is zero
    /// with the bed held.
    ///
    /// **Methodology.** For shares 1e-3, 0.5, shipped, 1 and powers 0.5-2x
    /// rated, set the fuel node to its steady offset `R x P` above the bed and
    /// read [`HtgrKinetics::fuel_feedback_reactivity_dollars`]. Pass: monotone
    /// in `f`, zero at rated, shipped between the bounds.
    ///
    /// **Results (2026-09-28)**, fuel-channel worth [$]:
    ///
    /// | power | f = 0.001 | f = 0.5 | f = 0.712 (shipped) | f = 1 |
    /// |---|---|---|---|---|
    /// | 0.5x | +0.0001 | +0.0494 | +0.0703 | +0.0988 |
    /// | 1.0x | 0 | 0 | 0 | 0 |
    /// | 1.5x | -0.0001 | -0.0494 | -0.0703 | -0.0988 |
    /// | 2.0x | -0.0002 | -0.0988 | -0.1407 | -0.1977 |
    ///
    /// Smaller than the retired
    /// channel's 2026-09-22 table (+0.1609 $ at 0.5x shipped) because the
    /// fuel node is the **average** kernel, 10.25 K above the bed at rated,
    /// where the retired channel used the **peak** kernel centre, 23.45 K
    /// above it. That is a change of instrument, stated so the two tables are
    /// not read as the same quantity: the average kernel is the temperature a
    /// whole-core Doppler coefficient applies to.
    #[test]
    fn the_fuel_share_is_ablated_across_its_range() {
        use uom::si::thermodynamic_temperature::kelvin as k_unit;
        let bed = design_bed();
        let shares = [1.0e-3, 0.5, FeedbackSplit::HU_FUEL_SHARE_OF_ISOTHERMAL, 1.0];
        println!("fuel channel worth [$]    f=0.001    f=0.5   f=0.712     f=1.0");
        for x in [0.5, 1.0, 1.5, 2.0] {
            let mut row = Vec::new();
            for share in shares {
                let mut k = HtgrKinetics::new_htr10_published(rated());
                k.set_fuel_share(share);
                let r = k.fuel_to_bed_resistance.get::<kelvin_per_watt>();
                k.prompt.fuel_temperature =
                    ThermodynamicTemperature::new::<k_unit>(bed.get::<k_unit>() + r * x * 1.0e7);
                row.push(k.fuel_feedback_reactivity_dollars());
            }
            println!(
                "  {x:>4.1}x rated        {:>8.4} {:>9.4} {:>9.4} {:>9.4}",
                row[0], row[1], row[2], row[3]
            );
            let ascending = row.windows(2).all(|w| w[1] >= w[0]);
            let descending = row.windows(2).all(|w| w[1] <= w[0]);
            assert!(ascending || descending, "row {row:?}");
            if (x - 1.0f64).abs() < 1e-12 {
                assert!(row.iter().all(|v| v.abs() < 1e-12));
            }
        }
    }

    /// V&V: the fuel node's energy ledger closes against its **true**
    /// enthalpy (gh:#360, 2026-09-28).
    ///
    /// **Methodology.** Hold the bed at 950 K, insert +0.3 $ for 30 s then
    /// -2 $ for 30 s (a heat-up and a cool-down, so the capacity moves both
    /// ways). The ledger records `sum f_prompt P dt + sum P_decay dt - sum Q_fb dt`
    /// using the per-step capacity; compare that with the exact enthalpy change
    /// `integral C_fuel(T) dT` from the start to the end temperature
    /// (trapezoid, 2000 intervals). Pass: agreement to 1e-4 of the gross energy
    /// throughput.
    ///
    /// **Results (2026-09-28).** Fuel 960.250 K -> 950.467 K; deposited prompt
    /// 8.0478e7 J, decay 2.7336e7 J, to bed 1.1044e8 J; ledger net
    /// -2.6214e6 J vs exact enthalpy change -2.6211e6 J -- residual 1.4e-6 of
    /// the throughput (the per-step frozen capacity; well inside 1e-4).
    #[test]
    fn the_fuel_node_ledger_closes_against_the_true_enthalpy() {
        use uom::si::thermodynamic_temperature::kelvin as k_unit;
        let mut k = HtgrKinetics::new_htr10_published(rated());
        let bed = design_bed();
        let t0 = k.fuel_temperature().get::<k_unit>();
        let dt = Time::new::<second>(0.1);
        for i in 0..600 {
            let rho = if i < 300 { 0.3 } else { -2.0 };
            k.step(dt, rho, bed, None);
        }
        let t1 = k.fuel_temperature().get::<k_unit>();
        let n = 2000;
        let mut exact = 0.0;
        for j in 0..n {
            let a = t0 + (t1 - t0) * j as f64 / n as f64;
            let b = t0 + (t1 - t0) * (j + 1) as f64 / n as f64;
            let ca =
                super::super::pebble_bed::fuel_node_heat_capacity(ThermodynamicTemperature::new::<
                    k_unit,
                >(a))
                .get::<joule_per_kelvin>();
            let cb =
                super::super::pebble_bed::fuel_node_heat_capacity(ThermodynamicTemperature::new::<
                    k_unit,
                >(b))
                .get::<joule_per_kelvin>();
            exact += 0.5 * (ca + cb) * (b - a);
        }
        let l = k.ledger();
        let net = l.deposited_prompt + l.deposited_decay - l.conducted_to_bed;
        let throughput = l.deposited_prompt + l.deposited_decay + l.conducted_to_bed;
        println!(
            "fuel {t0:.3} K -> {t1:.3} K; deposited prompt {:.4e} J, decay {:.4e} J, to bed \
             {:.4e} J; ledger net {net:.4e} J vs exact enthalpy change {exact:.4e} J \
             (residual {:.3e} of throughput)",
            l.deposited_prompt,
            l.deposited_decay,
            l.conducted_to_bed,
            (net - exact).abs() / throughput
        );
        assert!((net - l.stored).abs() < 1e-6 * throughput);
        assert!((net - exact).abs() < 1e-4 * throughput);
    }
}
