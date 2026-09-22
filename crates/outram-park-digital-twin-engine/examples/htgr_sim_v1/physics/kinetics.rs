//! Reactor-kinetics slot for the HTGR scaffold.
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
//! ## The fuel-temperature feedback got a heat sink (2026-08-14)
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
//! ## The Doppler feedback moved onto the fuel kernel (2026-09-22)
//!
//! ~~"A real Doppler feedback wants the fuel kernel temperature, which needs
//! the intra-pebble split described in [`super::pebble_bed`]."~~
//! ~~"**UPDATED 2026-09-22 -- the split now exists, and this module still does
//! not use it.** The feedback here still reads the bed node. That is a
//! deliberate hold ... It wants its own change, with its own before/after."~~
//! **CORRECTED 2026-09-22 -- that change is this one, and the hold is
//! discharged.** [`KernelDopplerChannel`] carries the **fuel share** of the
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
//!    [`KernelDopplerChannel`]), so Nordheim-Fuchs keeps ownership of the
//!    stiff feedback exactly as the section above insists it must, and every
//!    previously recorded `alpha_iso` number stays valid.
//! 2. **The design point is neutral by construction**, so the steady state is
//!    unchanged and everything the channel moves is transient.
//! 3. **The split fraction is an INPUT and is bounded.** It is a ratio taken
//!    from Hu *et al.*, not a fitted value; `f = 0` reproduces the
//!    pre-2026-09-22 model exactly and `f = 1` puts the whole coefficient on
//!    the kernel. Both bounds are measured -- see
//!    [`KernelDopplerChannel::HU_FUEL_SHARE_OF_ISOTHERMAL`] and
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
//! This slot is wired to the real `teh-o-prke` API (bead `op-wqk.9.2`). What
//! remains scaffold-level is only the *plant-scale illustrative parameters*
//! below, not the kinetics wiring.

use nee_soon::NordheimFuchsExactTimestepper;
use teh_o_prke::decay_heat::{DecayHeat, FissioningNuclide};
use teh_o_prke::delayed_neutron_layer::DelayedNeutronLayer;

use uom::si::f64::{
    Power, Ratio, TemperatureCoefficient, TemperatureInterval, ThermalResistance,
    ThermodynamicTemperature, Time,
};
use uom::si::heat_capacity::joule_per_kelvin;
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
    /// The fuel share of the temperature feedback, carried on the UO2 kernel
    /// instead of the bed node. Always present (this is physics the model has,
    /// not an opt-in); ablated by constructing it with a zero fuel share. See
    /// [`KernelDopplerChannel`].
    kernel_doppler: KernelDopplerChannel,
}

/// The **kernel Doppler channel** — the fuel share of HTR-10's published
/// isothermal coefficient, applied to the UO2 kernel rather than to the bed.
///
/// # The algebra that makes this one extra term instead of a rewrite
///
/// Splitting the feedback into a fuel channel on the kernel and a
/// graphite/reflector channel on the bed reads as a change to both. It is not.
/// Write `T_kernel = T_f + dT(P)`, with `dT` the kernel-above-node rise from
/// the resolved pebble and `T_f` the Nordheim-Fuchs node (which tracks the
/// bed — see [`HtgrKinetics::apply_coolant_heat_removal`]):
///
/// ```text
/// rho = alpha_D (T_kernel - T_kernel,ref)  +  alpha_m (T_f - T_ref)
///     = alpha_D (T_f + dT - T_ref - dT_ref) + alpha_m (T_f - T_ref)
///     = (alpha_D + alpha_m)(T_f - T_ref)   +  alpha_D (dT - dT_ref)
///     =  alpha_iso        (T_f - T_ref)    +  alpha_D (dT - dT_ref)
///             ^ unchanged, still inside the closed form     ^ THIS CHANNEL
/// ```
///
/// Because the split is constrained to **sum to the published isothermal
/// coefficient**, the first term is exactly what the Nordheim-Fuchs timestepper
/// already applies. So the closed form is not touched at all — its exactness,
/// which is the reason this module refuses to overwrite its node (see
/// [`HtgrKinetics::apply_coolant_heat_removal`]), is preserved — and the whole
/// rewiring is **one external reactivity term**, added alongside the xenon
/// channel. Two consequences worth stating:
///
/// - **Every reactivity number recorded for the `alpha_iso` term stays valid.**
///   The change is additive and attributable, which is what the hold noted in
///   this module's doc comment was waiting for.
/// - **The design point is neutral by construction.** `dT_ref` is the offset
///   at the bed's own design point and rated power, so this term is *zero*
///   there and the steady state is unchanged. Everything it moves is transient.
///
/// ## One bookkeeping detail, stated because it looks like a bug
///
/// `T_f` above is the Nordheim-Fuchs node, while `dT` is the kernel's rise
/// above the **bed** node, and those two are not the same number -- they track
/// each other but diverged by as much as 10.2 K post-scram before the
/// decay-heat source was added to the fuel node (2026-08-17). Mixing them
/// looks like an error, so here is why it is not one.
///
/// Write the ideal two-channel model against the bed throughout:
///
/// ```text
/// rho_ideal = alpha_D (T_bed + dT - T_ref - dT_ref) + alpha_m (T_bed - T_ref)
///           = alpha_iso (T_bed - T_ref) + alpha_D (dT - dT_ref)
/// ```
///
/// and this implementation gives
/// `alpha_iso (T_f - T_ref) + alpha_D (dT - dT_ref)`. **The added term is
/// identical in both**; the whole difference is `alpha_iso (T_f - T_bed)`,
/// which is the *pre-existing* gap between the kinetics node and the bed node
/// and is exactly what this module already ran on before 2026-09-22. So this
/// change neither introduces that discrepancy nor inherits any of it into the
/// new channel -- it is orthogonal, and closing it would mean replacing the
/// closed-form node with an externally integrated one, which the section on
/// [`HtgrKinetics::apply_coolant_heat_removal`] explains at length is the
/// thing not to do.
///
/// # Why the kernel is the right temperature, and why it must be prompt
///
/// A UO2 kernel is a quarter of a millimetre across and follows a power change
/// essentially instantly; the graphite is 5.3 tonnes and takes minutes. That
/// separation *is* HTR-10's self-limiting response — prompt negative Doppler
/// arrests the excursion, then the slow graphite channel decides where the core
/// settles — and it is the case `tampines::pebble_bed::feedback`'s module doc
/// is built around. Before this channel the simulator had **no prompt feedback
/// temperature at all**: the only node the feedback could see relaxed on the
/// bed's ~184 s time constant.
///
/// So `dT` is evaluated at the **instantaneous** power on every kinetics
/// substep, from a resistance refreshed once per plant step
/// ([`super::pebble_bed::PebbleBedPorousMediaNode::kernel_offset_resistance`]).
/// Holding the *temperature* fixed across a plant step instead would have added
/// a 0.1 s lag to the one channel whose whole purpose is to be prompt.
///
/// # The split is an INPUT, and it is bounded
///
/// `alpha_iso = -1.4e-4 /K` ([`HtgrKinetics::HTR10_TEMPERATURE_COEFFICIENT_PER_K`],
/// Chen *et al.* 2009) is **isothermal**: fuel and moderator moving together,
/// so it is the *sum* of the two channels and not either one. What fraction
/// belongs to the fuel is not published for HTR-10 in a form this workspace can
/// resolve — see [`Self::HU_FUEL_SHARE_OF_ISOTHERMAL`] for exactly what is and
/// is not known, and what would settle it.
///
/// **The uncertainty is bounded rather than open**, which is why this is usable
/// at all: the fraction lies in `[0, 1]`, `f = 0` reproduces this simulator's
/// pre-2026-09-22 behaviour exactly, and `f = 1` puts the whole coefficient on
/// the kernel. Both bounds are runnable —
/// [`tests::the_kernel_doppler_split_is_ablated_across_its_full_range`]
/// measures them and records what the choice is worth.
#[derive(Debug, Clone, Copy)]
pub struct KernelDopplerChannel {
    /// `alpha_D = f * alpha_iso` \[1/K\] — the fuel share, applied to the
    /// kernel. Zero disables the channel and reproduces the pre-2026-09-22
    /// model exactly.
    doppler_coefficient: TemperatureCoefficient,
    /// `R_kernel` \[K/W of per-pebble power\], refreshed once per plant step
    /// from the bed's resolved pebble solve. `None` when that solve was out of
    /// range, which disables the term for that step — see
    /// [`Self::reactivity_dollars`].
    offset_resistance: Option<ThermalResistance>,
    /// `dT_ref` — the kernel-above-node offset at the design point and rated
    /// power. Subtracting it is what makes the design point neutral.
    reference_offset: TemperatureInterval,
    /// `beta`, to convert `dk/k` into the dollars this module's reactivity
    /// interface speaks. Stored rather than re-read so the channel cannot
    /// disagree with the prompt layer about `beta`.
    delayed_fraction: f64,
}

impl KernelDopplerChannel {
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

    /// Build the channel for the published HTR-10 point, with the fuel share
    /// defaulting to [`Self::HU_FUEL_SHARE_OF_ISOTHERMAL`].
    ///
    /// The reference offset is solved from the *same* resolved pebble the bed
    /// uses, at the bed's design-point temperature and the core-average pebble
    /// power, so the design point is neutral by construction rather than by a
    /// second constant that could drift. See
    /// [`Self::with_fuel_share`] for the ablation entry point.
    pub fn new_htr10_published(delayed_fraction: f64) -> Self {
        Self::with_fuel_share(delayed_fraction, Self::HU_FUEL_SHARE_OF_ISOTHERMAL)
    }

    /// Build the channel with an explicit fuel share `f` — **the ablation
    /// knob**, per this workspace's rule that a calibrated or input-valued
    /// parameter must be turn-off-able and measured.
    ///
    /// - `f = 0` — the whole isothermal coefficient stays on the bed node.
    ///   Reproduces this simulator's behaviour before 2026-09-22 **exactly**
    ///   (the term is identically zero, not merely small).
    /// - `f = 1` — the whole coefficient rides the kernel.
    /// - `f = ` [`Self::HU_FUEL_SHARE_OF_ISOTHERMAL`] — the shipped default.
    ///
    /// `f` is clamped to `[0, 1]`: outside that range one of the two channels
    /// changes sign, which would mean a *positive* feedback on either the fuel
    /// or the graphite, and no reading of the literature supports that for this
    /// core.
    pub fn with_fuel_share(delayed_fraction: f64, fuel_share: f64) -> Self {
        use uom::si::temperature_coefficient::per_kelvin;
        use uom::si::thermodynamic_temperature::kelvin;

        let fuel_share = fuel_share.clamp(0.0, 1.0);
        let alpha_d = TemperatureCoefficient::new::<per_kelvin>(
            fuel_share * HtgrKinetics::HTR10_TEMPERATURE_COEFFICIENT_PER_K,
        );

        // The design point: the bed's own seed temperature and the published
        // core-average pebble power. Read from the bed module rather than
        // restated, for the same reason `new_htr10_published` reads its
        // reference temperature there -- two copies of an operating point
        // drift, silently.
        let design_point = super::pebble_bed::PebbleBedPorousMediaNode::new().pebble_temperature();
        let rated_pebble_power = super::pebble_bed::core_average_pebble_power();
        let reference_offset = super::pebble_bed::resolved_pebble_profile(
            design_point,
            rated_pebble_power,
        )
        .map(|p| {
            TemperatureInterval::new::<temperature_interval::kelvin>(
                p.peak_kernel_centre.get::<kelvin>() - design_point.get::<kelvin>(),
            )
        })
        // A design point outside the pebble correlation window would be a
        // construction-time defect, not a transient excursion -- but a zero
        // reference merely makes the channel measure the offset from zero
        // instead of from rated, which is a wrong steady state rather than a
        // panic in a GUI. The bed's own seed is inside the window and
        // `tests::the_design_point_is_neutral` pins that it resolves.
        .unwrap_or_else(|| TemperatureInterval::new::<temperature_interval::kelvin>(0.0));

        Self {
            doppler_coefficient: alpha_d,
            offset_resistance: None,
            reference_offset,
            delayed_fraction,
        }
    }

    /// Hand the channel the kernel-above-node resistance from the bed's most
    /// recent resolved pebble solve.
    ///
    /// Called once per plant step by [`super::HtgrPlant::step_with_correctors`],
    /// which is the same predictor-corrector treatment `core_heat_to_helium`
    /// gets: on the first corrector this is the previous step's value, and it
    /// tightens as the loop iterates.
    pub fn set_offset_resistance(&mut self, resistance: Option<ThermalResistance>) {
        self.offset_resistance = resistance;
    }

    /// The kernel-above-node offset `dT` at core thermal power
    /// `core_thermal_power`, or zero when no resistance is available.
    ///
    /// `core_thermal_power` is the **thermal** power — promptly-released
    /// fission plus decay heat, i.e. [`HtgrKinetics::core_thermal_power`] —
    /// because that is the heat the pebble actually conducts, and it is the
    /// quantity the bed's own resistance was solved against. Using the raw
    /// fission power here would over-state the offset at power and, worse,
    /// take it to zero after a scram when decay heat is still keeping the
    /// kernels hot.
    pub fn kernel_offset(&self, core_thermal_power: Power) -> TemperatureInterval {
        let Some(resistance) = self.offset_resistance else {
            return TemperatureInterval::new::<temperature_interval::kelvin>(0.0);
        };
        let pebble_power = core_thermal_power / super::pebble_bed::pebble_count();
        resistance * pebble_power
    }

    /// This channel's reactivity in **dollars** at core thermal power
    /// `core_thermal_power`.
    ///
    /// ```text
    /// rho_$ = alpha_D * (dT(P) - dT_ref) / beta
    /// ```
    ///
    /// Negative on a power rise (the kernel runs further above the bed) and
    /// positive on a power fall, which is the prompt Doppler response.
    ///
    /// **Returns exactly zero** when the bed had no resolved profile for the
    /// step — the fallback is the model that was in service before the pebble
    /// was resolved, not a fabricated offset. See
    /// [`super::pebble_bed::PebbleBedPorousMediaNode::kernel_offset_resistance`].
    pub fn reactivity_dollars(&self, core_thermal_power: Power) -> f64 {
        use uom::si::temperature_coefficient::per_kelvin;

        if self.delayed_fraction <= 0.0 {
            return 0.0;
        }
        let offset = self.kernel_offset(core_thermal_power);
        let excess_k = offset.get::<temperature_interval::kelvin>()
            - self.reference_offset.get::<temperature_interval::kelvin>();
        let dk_over_k = self.doppler_coefficient.get::<per_kelvin>() * excess_k;
        dk_over_k / self.delayed_fraction
    }

    /// The fuel-share Doppler coefficient `alpha_D` this channel carries
    /// \[1/K\]. The graphite/reflector remainder `alpha_iso - alpha_D` stays
    /// inside the Nordheim-Fuchs closed form.
    pub fn doppler_coefficient(&self) -> TemperatureCoefficient {
        self.doppler_coefficient
    }

    /// The design-point kernel-above-node offset this channel measures from.
    pub fn reference_offset(&self) -> TemperatureInterval {
        self.reference_offset
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

    /// Current Xe-135 number density \[atoms/cm^3\].
    pub fn xenon_per_cm3(&self) -> f64 {
        self.xenon_per_cm3
    }
}

impl HtgrKinetics {
    /// Construct the kinetics slot with illustrative graphite-moderated
    /// pebble-bed parameters. **Not** any specific licensed design -- round,
    /// order-of-magnitude numbers only, per this workspace's data policy.
    ///
    /// - `Lambda` = [`Self::HTR10_PROMPT_GENERATION_TIME_S`] -- **published**,
    /// - `beta` = [`Self::HTR10_EFFECTIVE_DELAYED_FRACTION`] -- **published**,
    /// - `C_f` = the pebble bed's own lumped graphite heat capacity,
    ///   [`super::pebble_bed::bed_heat_capacity`] (about 9.0 MJ/K). This is
    ///   *derived* from the published pebble count, diameter and graphite
    ///   density, so the feedback sees the same thermal mass the thermal
    ///   hydraulics does. It used to be a flat 1e8 J/K sized for the old
    ///   200 MWth prismatic plant, which at 10 MWth would have made the
    ///   temperature feedback almost inert.
    /// - `alpha_f` = [`Self::HTR10_TEMPERATURE_COEFFICIENT_PER_K`] --
    ///   **published**, and 3.5x the magnitude of the -4e-5 /K it replaced.
    ///   This is the coefficient that arrests a LOFC transient, so the old
    ///   value materially under-stated HTR-10's inherent safety margin,
    /// - reference/initial fuel temperature = **the pebble bed's own
    ///   design-point temperature**, [`super::pebble_bed::PebbleBedPorousMediaNode::new`]
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
        use uom::si::f64::{TemperatureCoefficient, ThermodynamicTemperature};
        use uom::si::temperature_coefficient::per_kelvin;

        let prompt_generation_time = Time::new::<second>(Self::HTR10_PROMPT_GENERATION_TIME_S);

        // Reference AND initial fuel temperature both taken from the pebble
        // bed's design point, so `T_f - T_ref` is zero there and the
        // temperature feedback neither holds the reactor down nor pushes it up
        // at rated conditions. Same principle as `C_f` above: derive it from
        // the bed rather than choosing a second number that can disagree.
        let design_point_temperature =
            super::pebble_bed::PebbleBedPorousMediaNode::new().pebble_temperature();

        let prompt = NordheimFuchsExactTimestepper::new(
            prompt_generation_time,
            Ratio::new::<ratio>(Self::HTR10_EFFECTIVE_DELAYED_FRACTION),
            super::pebble_bed::bed_heat_capacity(),
            TemperatureCoefficient::new::<per_kelvin>(Self::HTR10_TEMPERATURE_COEFFICIENT_PER_K),
            design_point_temperature,
            design_point_temperature,
            reference_power,
        )
        .expect("published HTR-10 kinetics parameters must satisfy NordheimFuchs preconditions");

        let delayed = DelayedNeutronLayer::u235_five_group(prompt_generation_time);

        // Seeded SATURATED, not clean: the simulator opens at its operating
        // point, where a real core has been running long enough for the
        // fission-product inventory to have reached equilibrium. Starting the
        // groups cold would show zero decay heat at t=0 and then a spurious
        // several-minute climb to equilibrium that no operator would ever see.
        let decay = DecayHeat::new_at_equilibrium(FissioningNuclide::U235Thermal, reference_power);

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
            // The kernel Doppler channel is ON by default -- per this
            // workspace's rule that correct physics is the default setting and
            // not an opt-in. A feedback term behind an off-by-default flag is
            // a term the recorded transients would never have been measured
            // with. `set_kernel_fuel_share` is the explicit, visible ablation.
            kernel_doppler: KernelDopplerChannel::new_htr10_published(
                Self::HTR10_EFFECTIVE_DELAYED_FRACTION,
            ),
        }
    }

    /// Cool the reactivity-feedback fuel node by the heat the coolant actually
    /// carried away over `dt`.
    ///
    /// # Why this, and not "set the fuel temperature to the bed temperature"
    ///
    /// The obvious way to couple the feedback to the core is to overwrite the
    /// prompt layer's fuel temperature with [`super::pebble_bed`]'s graphite
    /// temperature each step. **That is the wrong move, and it is worth saying
    /// why**: the Nordheim-Fuchs timestepper integrates the prompt power *and*
    /// its adiabatic temperature feedback together in **closed form**, and that
    /// exactness is precisely what keeps this feedback from being stiff.
    /// Reactivity feedback is the stiff term in point kinetics -- `alpha_f`
    /// couples power to temperature and back on the prompt timescale. Replacing
    /// the closed-form node with an externally integrated one, reset
    /// discontinuously once per plant step, throws that away and reintroduces
    /// the stiffness Nordheim-Fuchs is in this simulator to avoid.
    ///
    /// So the closed form keeps ownership of the feedback. All that was ever
    /// actually *missing* from it is a heat sink: `NordheimFuchsExactTimestepper`
    /// is adiabatic, so its fuel temperature could only ever climb, and after
    /// any power rise the feedback reactivity stuck at its most negative value
    /// forever. This applies the sink as a separate, **smooth** operator:
    ///
    /// ```text
    /// T_f <- T_f - Q_removed * dt / C_f
    /// ```
    ///
    /// with `C_f` the same graphite heat capacity the bed carries. That is a
    /// Lie split on the *sink only*, and the sink is slow -- the bed's time
    /// constant is about 184 s against a 0.1 s plant step -- so it adds no
    /// stiffness of its own. The fast, stiff part stays inside the closed form.
    ///
    /// The result is that the fuel node and the pebble bed see the same power
    /// in and the same heat out, over the same heat capacity, so they track
    /// each other physically instead of being reconciled by force.
    ///
    /// Call this **after** [`Self::step`] and after the bed has been advanced,
    /// with the heat that actually crossed the pebble surface.
    pub fn apply_coolant_heat_removal(&mut self, heat_removed: Power, dt: Time) {
        let c_f = self.prompt.fuel_heat_capacity;
        if c_f.get::<joule_per_kelvin>() <= 0.0 {
            return;
        }
        let drop = heat_removed * dt / c_f;
        self.prompt.fuel_temperature -= drop;
    }

    /// The fuel temperature the reactivity feedback is currently computed
    /// against -- the Nordheim-Fuchs node, now with a coolant heat sink (see
    /// [`Self::apply_coolant_heat_removal`]).
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
    /// `coolant_heat_removal` is the heat the coolant is currently carrying
    /// off the fuel node, applied **inside** the substep loop rather than as
    /// one lump afterwards. That matters: the fuel temperature drives the
    /// reactivity feedback, so a sink applied only at the end of the plant
    /// step leaves the feedback reading a temperature that is a whole 0.1 s
    /// stale on the cooling side. Measured 2026-08-14, applying it per plant
    /// step instead of per substep drifted reactor power **-1.27%** from the
    /// 1 ms reference on the flow-ramp transient of
    /// `super::tests::the_plant_outer_correctors_converge`, against a 1%
    /// tolerance; at substep resolution the drift is well inside it.
    pub fn step(
        &mut self,
        dt: Time,
        external_reactivity_dollars: f64,
        coolant_heat_removal: Power,
    ) {
        let dt_s = dt.get::<second>();
        let pieces = if dt_s > super::KINETICS_SUBSTEP_S {
            (dt_s / super::KINETICS_SUBSTEP_S).ceil().max(1.0)
        } else {
            1.0
        };
        let sub = Time::new::<second>(dt_s / pieces);
        for _ in 0..(pieces as usize) {
            // The xenon channel is an EXTERNAL reactivity contribution, added
            // to whatever the rods are worth. It is evaluated from the
            // concentration carried into this substep, then advanced, so the
            // kinetics never sees a xenon worth that depends on the power it
            // is about to produce.
            let rho_xenon_dollars = self.xenon_reactivity_dollars();
            // The kernel Doppler channel, evaluated on EVERY substep from the
            // power carried into it -- same "before, not after" treatment as
            // xenon above, so the feedback never sees an offset that depends
            // on the power it is about to produce. Evaluating it per substep
            // rather than per plant step is the whole point of the channel:
            // it is the only feedback temperature in this model that responds
            // promptly, and a once-per-0.1 s evaluation would give it the very
            // lag it exists to remove. See `KernelDopplerChannel`.
            let rho_kernel_dollars = self
                .kernel_doppler
                .reactivity_dollars(self.core_thermal_power());
            self.advance_one(
                sub,
                external_reactivity_dollars + rho_xenon_dollars + rho_kernel_dollars,
            );
            self.apply_decay_heat(sub);
            self.apply_coolant_heat_removal(coolant_heat_removal, sub);
            self.advance_xenon(sub);
        }
    }

    /// Hand the kernel Doppler channel the bed's kernel-above-node resistance
    /// for this plant step. See
    /// [`KernelDopplerChannel::set_offset_resistance`].
    pub fn set_kernel_offset_resistance(&mut self, resistance: Option<ThermalResistance>) {
        self.kernel_doppler.set_offset_resistance(resistance);
    }

    /// Rebuild the kernel Doppler channel with an explicit fuel share --
    /// **the ablation entry point**. `0.0` restores the pre-2026-09-22 model
    /// exactly; `1.0` puts the whole isothermal coefficient on the kernel.
    ///
    /// Rebuilding rather than mutating keeps `alpha_D` and the reference
    /// offset derived together from one fuel share, so they cannot disagree.
    /// The current offset resistance is carried across, so an ablation swapped
    /// in mid-run does not lose a step of coupling.
    pub fn set_kernel_fuel_share(&mut self, fuel_share: f64) {
        let carried = self.kernel_doppler.offset_resistance;
        self.kernel_doppler =
            KernelDopplerChannel::with_fuel_share(Self::HTR10_EFFECTIVE_DELAYED_FRACTION, fuel_share);
        self.kernel_doppler.set_offset_resistance(carried);
    }

    /// The kernel Doppler channel, for display and for tests.
    pub fn kernel_doppler(&self) -> &KernelDopplerChannel {
        &self.kernel_doppler
    }

    /// Reactivity worth of the current kernel Doppler channel, in dollars, at
    /// the power most recently produced. Zero at the design point by
    /// construction, negative above it.
    pub fn kernel_doppler_reactivity_dollars(&self) -> f64 {
        self.kernel_doppler
            .reactivity_dollars(self.core_thermal_power())
    }

    /// The kernel's current temperature rise above the bed node, from the
    /// channel's resistance and the current thermal power. Zero when the bed
    /// had no resolved profile.
    pub fn kernel_offset(&self) -> TemperatureInterval {
        self.kernel_doppler.kernel_offset(self.core_thermal_power())
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
    pub fn enable_xenon_at_equilibrium(&mut self, power: Power) {
        self.xenon = Some(XenonChannel::new_at_equilibrium(power));
    }

    /// Disable the Xe-135 channel.
    pub fn disable_xenon(&mut self) {
        self.xenon = None;
    }

    /// Current Xe-135 number density, or zero if the channel is disabled.
    pub fn xenon_number_density_per_cm3(&self) -> f64 {
        self.xenon
            .as_ref()
            .map(|x| x.xenon_per_cm3())
            .unwrap_or(0.0)
    }

    /// Heat this node by the fission-product decay heat generated over `dt`.
    ///
    /// # Why this exists (GitHub issue #22)
    ///
    /// [`Self::apply_coolant_heat_removal`] charges this node the SAME
    /// coolant sink rate the real pebble bed sees, and that rate is sized
    /// against the bed's FULL thermal source -- fission power plus decay
    /// heat (see [`Self::core_thermal_power`]). Until this method existed,
    /// this node's own heating came only from [`Self::advance_one`]'s
    /// prompt-plus-delayed fission power, with no decay-heat term: it was
    /// being charged for a removal rate sized for heat it never received.
    ///
    /// This is NOT the same concern [`Self::advance_one`]'s decay-bank
    /// comment raises -- that one is about the decay bank's own DRIVING
    /// INPUT needing to stay fission-power-only so it does not double-count
    /// itself internally, and is unaffected here: [`Self::decay_heat_power`]
    /// is read as an OUTPUT after [`DecayHeat::advance_timestep`] has
    /// already run for this substep, not fed back into it.
    ///
    /// Without this term, the node drifted increasingly colder than the
    /// bed after any trip -- prompt fission collapses toward zero on scram
    /// while the coolant sink does not, since the bed (and hence the sink)
    /// still has decay heat. Measured **-10.2 K by 300 s post-scram** before
    /// this fix, **+0.04 K after it**, in
    /// `super::tests::kinetics_fuel_node_tracks_the_bed_node_after_a_scram`,
    /// and reported as GitHub issue #22's "totally wrong energy balance"
    /// (the schematic compared this node against a helium temperature).
    ///
    /// Call every substep, right after [`Self::advance_one`] has refreshed
    /// [`Self::decay_heat_power`] -- same resolution
    /// [`Self::apply_coolant_heat_removal`] needs and for the same reason
    /// (a term applied only once per plant step lags the reactivity feedback
    /// by a whole 0.1 s on the heating side too).
    fn apply_decay_heat(&mut self, dt: Time) {
        let c_f = self.prompt.fuel_heat_capacity;
        if c_f.get::<joule_per_kelvin>() <= 0.0 {
            return;
        }
        let rise = self.decay_heat_power() * dt / c_f;
        self.prompt.fuel_temperature += rise;
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
    /// by the delayed-neutron layer (dimensionless).
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
    use uom::si::heat_capacity::joule_per_kelvin;
    use uom::si::power::megawatt;
    use uom::si::thermodynamic_temperature::kelvin;

    fn rated() -> Power {
        Power::new::<megawatt>(10.0)
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
            // No coolant removal: this isolates the decay-heat behaviour from
            // the thermal-hydraulics, which is the point of the test.
            k.step(dt, -10.0, Power::new::<watt>(0.0));
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

    /// V&V: the fuel-temperature feedback node must now COOL, and must do so
    /// without becoming stiff.
    ///
    /// **Methodology.** Two checks on the same run:
    ///
    /// 1. **The sink works.** With no reactivity inserted, applying a steady
    ///    heat removal must bring the fuel temperature down. Before
    ///    2026-08-14 the Nordheim-Fuchs node was adiabatic and this was
    ///    impossible -- it could only climb.
    /// 2. **It is not stiff.** The removal is applied at the plant timestep
    ///    (0.1 s) and the temperature trajectory must be monotone and smooth,
    ///    with no step-to-step reversal. A stiff explicit coupling shows up as
    ///    exactly that: alternating over- and under-shoot. The check is that
    ///    the temperature decreases at every step.
    ///
    /// **Results (2026-08-14).** Printed below. The drop per step matches
    /// `Q dt / C_f` analytically, and no reversal occurs -- the sink is a
    /// plain first-order operator, and the fast feedback stays inside the
    /// closed form where it belongs.
    #[test]
    fn the_fuel_node_cools_smoothly_rather_than_stiffly() {
        let mut k = HtgrKinetics::new_htr10_published(rated());
        let dt = Time::new::<second>(0.1);
        let removal = Power::new::<megawatt>(10.0);
        let c_f = k.prompt.fuel_heat_capacity.get::<joule_per_kelvin>();

        let start = k.fuel_temperature().get::<kelvin>();
        let mut previous = start;
        let mut reversals = 0;
        for _ in 0..600 {
            k.apply_coolant_heat_removal(removal, dt);
            let now = k.fuel_temperature().get::<kelvin>();
            if now > previous + 1e-12 {
                reversals += 1;
            }
            previous = now;
        }
        let end = previous;

        // Analytical: 600 steps of 0.1 s at 10 MW over C_f.
        let expected_drop = 1.0e7 * 60.0 / c_f;
        println!(
            "fuel node: {start:.3} K -> {end:.3} K over 60 s at 10 MW removal \
             (C_f = {c_f:.4e} J/K, analytical drop {expected_drop:.3} K), \
             {reversals} step reversals"
        );
        assert!(
            end < start,
            "the fuel node must cool -- it used to be adiabatic"
        );
        assert_eq!(
            reversals, 0,
            "a stiff coupling would show step-to-step reversals; found {reversals}"
        );
        assert!(
            ((start - end) - expected_drop).abs() / expected_drop < 1e-6,
            "measured drop {:.3} K departs from the analytical {expected_drop:.3} K",
            start - end
        );
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
    /// parts add back up. If they did not, the model would have a different
    /// total temperature coefficient from the one it cites, and every
    /// transient in this simulator would be measuring an unpublished number
    /// while claiming a published one.
    ///
    /// The graphite share is never written down anywhere: it is whatever the
    /// closed form already applies minus the kernel channel's `alpha_D`. This
    /// test is therefore checking an *identity the code relies on*, not an
    /// arithmetic restatement -- `alpha_D` is built from a clamped fuel share
    /// inside [`KernelDopplerChannel::with_fuel_share`], and a clamp that
    /// misbehaved would break the sum without breaking anything else visible.
    ///
    /// **Methodology.** Over fuel shares 0, 0.25, `HU_FUEL_SHARE`, 0.75, 1 and
    /// two deliberately out-of-range values (-0.5, 1.5), form
    /// `alpha_D + (alpha_iso - alpha_D)` and compare to `alpha_iso`. Pass
    /// criterion: exact to 1e-18 /K (this is floating-point addition of two
    /// numbers built from one, so anything looser would hide a real error).
    /// Also check the clamp: out-of-range shares must land on the endpoints,
    /// because a share outside [0, 1] gives one channel a *positive*
    /// coefficient, which no reading of the literature supports for this core.
    ///
    /// **Results (2026-09-22).** Every share reproduced
    /// `alpha_iso = -1.4e-4 /K` to **0.0** -- exactly, not approximately.
    /// `f = -0.5` clamped to `alpha_D = -0.0` and `f = 1.5` to
    /// `alpha_D = -1.40000e-4 /K` (leaving `alpha_m = +0.0`). At the shipped
    /// default the split is **`alpha_D = -9.96313e-5 /K` on the kernel,
    /// `alpha_m = -4.03687e-5 /K` on the bed**.
    ///
    /// **Interpretation.** The split is a redistribution, not a change of
    /// magnitude. Whatever the fuel share turns out to be, this simulator's
    /// *total* temperature coefficient remains the published one -- so the
    /// split can only move **when** feedback arrives, never how much there is
    /// in total. That is exactly the property that makes the unresolved
    /// fraction (see [`KernelDopplerChannel::HU_FUEL_SHARE_OF_ISOTHERMAL`])
    /// tolerable as an input.
    #[test]
    fn the_split_channels_sum_to_the_published_isothermal_coefficient() {
        use uom::si::temperature_coefficient::per_kelvin;

        let alpha_iso = HtgrKinetics::HTR10_TEMPERATURE_COEFFICIENT_PER_K;
        let beta = HtgrKinetics::HTR10_EFFECTIVE_DELAYED_FRACTION;
        let mut worst = 0.0f64;

        for share in [
            -0.5,
            0.0,
            0.25,
            KernelDopplerChannel::HU_FUEL_SHARE_OF_ISOTHERMAL,
            0.75,
            1.0,
            1.5,
        ] {
            let channel = KernelDopplerChannel::with_fuel_share(beta, share);
            let alpha_d = channel.doppler_coefficient().get::<per_kelvin>();
            let alpha_m = alpha_iso - alpha_d;
            worst = worst.max((alpha_d + alpha_m - alpha_iso).abs());

            let clamped = share.clamp(0.0, 1.0);
            assert!(
                (alpha_d - clamped * alpha_iso).abs() < 1e-18,
                "share {share} must clamp to {clamped}; alpha_D = {alpha_d:e}"
            );
            println!(
                "f = {share:>5} -> alpha_D = {alpha_d:+.5e} /K (kernel), \
                 alpha_m = {alpha_m:+.5e} /K (bed)"
            );
        }

        println!("worst |alpha_D + alpha_m - alpha_iso| = {worst:.3e} /K");
        assert!(
            worst < 1e-18,
            "the split must conserve the published coefficient; worst {worst:e} /K"
        );
    }

    /// V&V: the kernel Doppler channel must be **exactly zero at the design
    /// point**, so introducing it does not move the steady state.
    ///
    /// # Why this matters more than it looks
    ///
    /// This is the same property [`super::super::kinetics`]'s reference
    /// temperature was chosen for (see
    /// [`HtgrKinetics::new_htr10_published`]): a feedback term that is
    /// non-zero at rated conditions silently re-rates the plant, and it does
    /// so in a direction that looks like physics. The channel measures the
    /// kernel offset **from its design-point value**, so at rated power and
    /// the bed's design temperature the term vanishes identically -- which is
    /// what makes every difference this change produces attributable to the
    /// *transient*, not to a shifted operating point.
    ///
    /// It also pins that the design point resolves at all: the reference
    /// offset is solved through the pebble correlation window, and a
    /// construction that fell outside it would silently reference zero and put
    /// a large spurious negative reactivity on the plant at rated power.
    ///
    /// **Methodology.** Build the channel at the shipped fuel share, hand it
    /// the resistance the bed's resolved pebble gives at the design point and
    /// core-average pebble power, and evaluate at exactly the rated thermal
    /// power. Pass criterion: |rho| < 1e-12 $ and the reference offset is
    /// strictly positive (a zero would mean the solve failed and the fallback
    /// fired).
    ///
    /// **Results (2026-09-22).** Reference offset **23.453 K**, reactivity at
    /// the design point **-4.876e-17 $** -- zero to floating-point round-off,
    /// as the algebra requires. At 1.5x rated the same channel is worth
    /// **-0.1609 $**, and at half rated **+0.1609 $**, so the term is live and
    /// signed correctly (negative on a power rise) rather than merely small.
    ///
    /// **Interpretation.** The offset is 23.45 K at rated, not the 21.30 K
    /// `one_node`'s resolved-pebble test reports, and the difference is not a
    /// discrepancy: that test solves at a bed temperature of 815.15 K, this at
    /// the bed's 950 K design point, and A3 graphite conducts *worse* hot, so
    /// the same power drives a larger rise. The two agree on the physics and
    /// disagree on the operating point, which is the correct behaviour for a
    /// temperature-dependent conductivity.
    #[test]
    fn the_design_point_is_neutral() {
        use uom::si::power::watt;

        let beta = HtgrKinetics::HTR10_EFFECTIVE_DELAYED_FRACTION;
        let mut channel = KernelDopplerChannel::new_htr10_published(beta);

        let design_point =
            super::super::pebble_bed::PebbleBedPorousMediaNode::new().pebble_temperature();
        let rated_pebble = super::super::pebble_bed::core_average_pebble_power();
        let profile = super::super::pebble_bed::resolved_pebble_profile(design_point, rated_pebble)
            .expect("the design point must sit inside the pebble correlation window");
        let rise_k =
            profile.peak_kernel_centre.get::<kelvin>() - design_point.get::<kelvin>();
        channel.set_offset_resistance(Some(ThermalResistance::new::<
            uom::si::thermal_resistance::kelvin_per_watt,
        >(rise_k / rated_pebble.get::<watt>())));

        let rated = super::super::pebble_bed::design().thermal_power;
        let at_design = channel.reactivity_dollars(rated);
        let hot = channel.reactivity_dollars(rated * 1.5);
        let cold = channel.reactivity_dollars(rated * 0.5);

        println!(
            "reference offset {:.3} K; rho at rated {at_design:+.3e} $, \
             at 1.5x {hot:+.4} $, at 0.5x {cold:+.4} $",
            channel.reference_offset().get::<temperature_interval::kelvin>()
        );

        assert!(
            channel.reference_offset().get::<temperature_interval::kelvin>() > 1.0,
            "a zero reference offset means the design-point solve fell back"
        );
        assert!(
            at_design.abs() < 1e-12,
            "the design point must be neutral; got {at_design:e} $"
        );
        assert!(hot < 0.0, "a power RISE must give negative reactivity");
        assert!(cold > 0.0, "a power FALL must give positive reactivity");
    }

    /// V&V **and ablation**: what the unresolved fuel share is actually worth,
    /// measured across its whole admissible range.
    ///
    /// # Why this test is the point of the whole change
    ///
    /// [`KernelDopplerChannel::HU_FUEL_SHARE_OF_ISOTHERMAL`] is an **input**,
    /// not a derivation -- a ratio read off Hu *et al.* (2006) because no
    /// fuel-only Doppler coefficient for HTR-10 exists in this workspace. This
    /// workspace's rules are explicit that a parameter of that kind must be
    /// ablatable and its contribution measured, otherwise it is
    /// indistinguishable from curve-fitting. This is that measurement.
    ///
    /// The uncertainty is **bounded**, which is what makes the input usable at
    /// all: the share must lie in `[0, 1]`, `f = 0` is exactly the model this
    /// simulator ran before 2026-09-22, and `f = 1` puts the entire published
    /// coefficient on the kernel. The true answer is inside those two runs.
    ///
    /// **Methodology.** At a fixed bed design point and the resistance the
    /// resolved pebble gives there, evaluate the channel's reactivity across
    /// the power range a transient reaches (0.5x to 2x rated) at fuel shares
    /// 0, 0.5, the shipped default, and 1. Pass criteria: `f = 0` is
    /// identically zero everywhere (a *bit-exact* reproduction of the old
    /// model, not an approximation); the worth is monotone in `f`; and the
    /// shipped default sits between the bounds.
    ///
    /// **Results (2026-09-22)**, kernel Doppler worth in dollars:
    ///
    /// | Power | `f = 0` | `f = 0.5` | `f = 0.712` (shipped) | `f = 1` |
    /// |---|---|---|---|---|
    /// | 0.5x rated | 0.0000 | +0.1131 | +0.1609 | +0.2261 |
    /// | 1.0x rated | -0.0000 | -0.0000 | -0.0000 | -0.0000 |
    /// | 1.5x rated | -0.0000 | -0.1131 | -0.1609 | -0.2261 |
    /// | 2.0x rated | -0.0000 | -0.2261 | -0.3219 | -0.4523 |
    ///
    /// **Interpretation -- and this is the number to quote.** The *entire*
    /// span of the unresolved fraction is worth **0.4523 $ at double rated
    /// power**; at 1.5x it spans 0.2261 $, of which **0.0652 $** separates the
    /// shipped default from the `f = 1` bound. Against HTR-10's
    /// `beta = 7.26e-3` that is real prompt reactivity -- a fifth of a dollar
    /// between the bounds at 1.5x is not noise -- so the fraction is worth
    /// settling, and any conclusion this simulator produces about a *prompt*
    /// excursion is a conclusion about this input and must be reported that
    /// way.
    ///
    /// What it is **not** worth is the steady state: every share is zero at
    /// rated, so no operating point, no settled temperature and no recorded
    /// steady-state number in this simulator depends on the fraction at all.
    /// The ablation therefore bounds the exposure precisely: transients yes,
    /// operating points no.
    #[test]
    fn the_kernel_doppler_split_is_ablated_across_its_full_range() {
        use uom::si::power::watt;

        let beta = HtgrKinetics::HTR10_EFFECTIVE_DELAYED_FRACTION;
        let design_point =
            super::super::pebble_bed::PebbleBedPorousMediaNode::new().pebble_temperature();
        let rated_pebble = super::super::pebble_bed::core_average_pebble_power();
        let profile = super::super::pebble_bed::resolved_pebble_profile(design_point, rated_pebble)
            .expect("the design point must sit inside the pebble correlation window");
        let resistance = ThermalResistance::new::<uom::si::thermal_resistance::kelvin_per_watt>(
            (profile.peak_kernel_centre.get::<kelvin>() - design_point.get::<kelvin>())
                / rated_pebble.get::<watt>(),
        );
        let rated = super::super::pebble_bed::design().thermal_power;

        let shares = [
            0.0,
            0.5,
            KernelDopplerChannel::HU_FUEL_SHARE_OF_ISOTHERMAL,
            1.0,
        ];
        let fractions = [0.5, 1.0, 1.5, 2.0];

        println!("kernel Doppler worth [$]   f=0.0      f=0.5    f=0.712      f=1.0");
        for fraction in fractions {
            let mut row = Vec::new();
            for share in shares {
                let mut channel = KernelDopplerChannel::with_fuel_share(beta, share);
                channel.set_offset_resistance(Some(resistance));
                row.push(channel.reactivity_dollars(rated * fraction));
            }
            println!(
                "  {fraction:>4.1}x rated        {:>8.4} {:>9.4} {:>10.4} {:>10.4}",
                row[0], row[1], row[2], row[3]
            );

            // f = 0 must be EXACTLY the old model, not approximately.
            assert_eq!(
                row[0], 0.0,
                "f = 0 must reproduce the pre-2026-09-22 model bit-exactly"
            );
            // Monotone in f, in whichever direction the sign runs.
            let ascending = row.windows(2).all(|w| w[1] >= w[0]);
            let descending = row.windows(2).all(|w| w[1] <= w[0]);
            assert!(
                ascending || descending,
                "the worth must be monotone in the fuel share; row {row:?}"
            );
            // The shipped default must sit between the two bounds.
            assert!(
                (row[2].abs() - row[3].abs()) <= 1e-12 && row[2].abs() >= row[0].abs(),
                "the shipped share must lie between f = 0 and f = 1; row {row:?}"
            );
        }
    }

}
