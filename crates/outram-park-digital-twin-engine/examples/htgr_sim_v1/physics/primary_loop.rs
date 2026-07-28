//! Helium primary loop.
//!
//! Models a **helium-cooled, graphite-moderated prismatic-block HTGR** primary
//! loop as a single lumped coolant node: reactor thermal power heats helium
//! flowing through the core (core inlet -> core outlet), and an intermediate
//! heat exchanger (IHX) rejects that heat to the steam secondary loop. The
//! helium then returns to the core inlet, closing the loop.
//!
//! ## What is real
//!
//! - **Helium properties are real and temperature-dependent.** `c_p` and
//!   density come from the CoolProp-derived Helmholtz EOS
//!   ([`outram_park_fork_coolprop::state_pt`], helium from Ortiz-Vega et al.)
//!   evaluated at the loop pressure and the current bulk mean helium
//!   temperature, re-evaluated every step -- not a frozen constant.
//! - **The loop is closed.** The core inlet temperature is *computed* as the
//!   IHX helium-side outlet, relaxed through the loop's transport lag; it is
//!   no longer pinned to a fixed number. Reducing secondary heat removal
//!   therefore raises the core inlet, which raises the core outlet -- the
//!   feedback a real primary loop has.
//! - **The IHX is pinch-limited** by an effectiveness-NTU model against the
//!   secondary saturation temperature, so duty cannot exceed what the
//!   temperature difference and `UA` actually support (see [`Self::step`]).
//! - **Pressure drop and pumping power** are a Darcy-Weisbach friction loss
//!   with a Haaland friction factor, from the real helium density and the
//!   loop's flow area/length/roughness.
//! - **Residence time** is the true `m/m_dot` transport time from the helium
//!   inventory `rho*A*L`, which is what drives the schematic's flow tracers.
//!
//! ## What is still illustrative
//!
//! The loop geometry (flow area, length, roughness), the IHX `UA`, the
//! circulator efficiency, and the transport time constant are **illustrative
//! prismatic-HTGR-scale values, not a specific plant's design data**, and this
//! remains a single-node lumped model rather than a nodalised fluid array.
//! It is a demonstration model, not a validated HTGR primary-loop model.

use outram_park_fork_coolprop::{state_pt, Fluid};
use uom::si::f64::{
    Mass, MassRate, Power, Pressure, SpecificHeatCapacity, ThermodynamicTemperature, Time,
};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

/// Loop helium pressure (illustrative prismatic-HTGR scale), 7 MPa.
const LOOP_PRESSURE_PA: f64 = 7.0e6;

/// Total flow area of the core coolant channels \[m^2\] (illustrative).
const FLOW_AREA_M2: f64 = 2.0;

/// Effective flow path length around the loop \[m\] (illustrative).
const LOOP_LENGTH_M: f64 = 40.0;

/// Hydraulic diameter of the coolant channels \[m\] (illustrative).
const HYDRAULIC_DIAMETER_M: f64 = 0.016;

/// Absolute wall roughness of the coolant channels \[m\] (illustrative,
/// machined graphite).
const ROUGHNESS_M: f64 = 1.5e-5;

/// Circulator isentropic/mechanical efficiency (illustrative), 0.80.
const CIRCULATOR_EFFICIENCY: f64 = 0.80;

/// IHX overall conductance `UA` \[W/K\] (illustrative), sized so the nominal
/// ~200 MWth is transferred at an HTGR-scale approach temperature.
const IHX_UA_W_PER_K: f64 = 6.0e5;

/// Thermal-inertia time constant of the lumped helium node in the core \[s\]
/// (illustrative), giving the core-outlet temperature a visible first-order
/// transient rather than an instantaneous jump.
const CORE_THERMAL_TIME_CONSTANT_S: f64 = 5.0;

/// Transport lag from the IHX helium outlet round to the core inlet \[s\]
/// (illustrative), so a change in secondary heat removal reaches the core
/// inlet with a delay rather than instantly.
const RETURN_TRANSPORT_TIME_CONSTANT_S: f64 = 8.0;

/// Floor on the commanded helium flow \[kg/s\], keeping the energy-balance
/// denominator and the residence time finite when the user drives the
/// circulator setpoint to zero.
const MIN_HELIUM_FLOW_KG_PER_S: f64 = 1.0;

/// Seed temperature for the loop at construction \[K\] (~300 degC), a typical
/// prismatic-HTGR core inlet, illustrative only.
const SEED_TEMPERATURE_K: f64 = 573.0;

/// Lumped helium primary-loop state.
pub struct HeliumPrimaryLoop {
    /// Isobaric specific heat of helium at the current bulk mean temperature
    /// (re-evaluated every step from the real EOS).
    c_p: SpecificHeatCapacity,
    /// Helium density at the current bulk mean temperature (real EOS).
    density: f64,
    /// Current (transient) core-inlet temperature -- the IHX helium outlet
    /// after the return transport lag.
    core_inlet_temperature: ThermodynamicTemperature,
    /// Current (transient) core-outlet helium temperature.
    core_outlet_temperature: ThermodynamicTemperature,
    /// Current helium mass flow (driven by the circulator setpoint).
    mass_flow: MassRate,
    /// Most recently computed IHX duty transferred to the secondary loop.
    ihx_duty: Power,
    /// Helium-side IHX outlet temperature (feeds the core inlet).
    ihx_outlet_temperature: ThermodynamicTemperature,
    /// Frictional pressure drop around the loop at the current flow.
    pressure_drop: Pressure,
    /// Circulator hydraulic power required to sustain that pressure drop.
    circulator_power: Power,
}

impl HeliumPrimaryLoop {
    /// Construct the loop at a nominal operating point: `nominal_flow` helium
    /// mass flow, all temperatures seeded at [`SEED_TEMPERATURE_K`], helium
    /// properties evaluated there.
    pub fn new(nominal_flow: MassRate) -> Self {
        let seed = ThermodynamicTemperature::new::<kelvin>(SEED_TEMPERATURE_K);
        let (c_p, density) = helium_properties(SEED_TEMPERATURE_K);

        Self {
            c_p,
            density,
            core_inlet_temperature: seed,
            core_outlet_temperature: seed,
            mass_flow: nominal_flow,
            ihx_duty: Power::new::<watt>(0.0),
            ihx_outlet_temperature: seed,
            pressure_drop: Pressure::new::<pascal>(0.0),
            circulator_power: Power::new::<watt>(0.0),
        }
    }

    /// Advance the loop by `dt`, absorbing reactor thermal power
    /// `reactor_power` with the helium flow commanded by `flow_setpoint`, and
    /// rejecting heat to a secondary side sitting at `secondary_sink_temperature`
    /// (the steam-side saturation temperature the IHX pinches against).
    ///
    /// The step, in order:
    ///
    /// 1. Helium `c_p` and density are re-evaluated from the real EOS at the
    ///    current bulk mean temperature `(T_in + T_out)/2`.
    /// 2. Core energy balance: steady-state outlet `T_in + Q/(m_dot c_p)`,
    ///    with the displayed outlet relaxed toward it over
    ///    [`CORE_THERMAL_TIME_CONSTANT_S`].
    /// 3. IHX, effectiveness-NTU with one isothermal (boiling) side, so
    ///    `C_min = m_dot c_p` and `eps = 1 - exp(-UA/C_min)`:
    ///    `Q_ihx = eps * m_dot * c_p * (T_out - T_sink)`. This is the pinch
    ///    limit -- the duty vanishes as the helium approaches the secondary
    ///    saturation temperature, and can never drive it below.
    /// 4. Helium leaves the IHX at `T_out - Q_ihx/(m_dot c_p)`, which the core
    ///    inlet relaxes toward over [`RETURN_TRANSPORT_TIME_CONSTANT_S`],
    ///    closing the loop.
    /// 5. Darcy-Weisbach pressure drop and circulator hydraulic power at the
    ///    current flow and density.
    pub fn step(
        &mut self,
        dt: Time,
        reactor_power: Power,
        flow_setpoint: MassRate,
        secondary_sink_temperature: ThermodynamicTemperature,
    ) {
        // Clamp the commanded flow to a small positive floor so the energy
        // balance denominator and the residence time never blow up.
        let flow_kg_s = flow_setpoint
            .get::<kilogram_per_second>()
            .max(MIN_HELIUM_FLOW_KG_PER_S);
        self.mass_flow = MassRate::new::<kilogram_per_second>(flow_kg_s);

        // 1. Real helium properties at the current bulk mean temperature.
        let t_in_k = self.core_inlet_temperature.get::<kelvin>();
        let t_out_k = self.core_outlet_temperature.get::<kelvin>();
        let (c_p, density) = helium_properties(0.5 * (t_in_k + t_out_k));
        self.c_p = c_p;
        self.density = density;
        let c_p_j = self.c_p.get::<joule_per_kilogram_kelvin>();

        let dt_s = dt.get::<second>();
        let capacity_rate = flow_kg_s * c_p_j; // C_min = m_dot c_p [W/K]

        // 2. Core energy balance with first-order thermal inertia.
        let t_out_ss_k = t_in_k + reactor_power.get::<watt>() / capacity_rate;
        let alpha_core = (dt_s / CORE_THERMAL_TIME_CONSTANT_S).clamp(0.0, 1.0);
        let t_out_next_k = t_out_k + alpha_core * (t_out_ss_k - t_out_k);
        self.core_outlet_temperature = ThermodynamicTemperature::new::<kelvin>(t_out_next_k);

        // 3. IHX duty, effectiveness-NTU against an isothermal secondary side.
        let t_sink_k = secondary_sink_temperature.get::<kelvin>();
        let ntu = IHX_UA_W_PER_K / capacity_rate;
        let effectiveness = 1.0 - (-ntu).exp();
        let duty_w = (effectiveness * capacity_rate * (t_out_next_k - t_sink_k)).max(0.0);
        self.ihx_duty = Power::new::<watt>(duty_w);

        // 4. Helium leaves the IHX cooled by that duty; the core inlet
        //    relaxes toward it through the return transport lag.
        let t_ihx_out_k = t_out_next_k - duty_w / capacity_rate;
        self.ihx_outlet_temperature = ThermodynamicTemperature::new::<kelvin>(t_ihx_out_k);
        let alpha_return = (dt_s / RETURN_TRANSPORT_TIME_CONSTANT_S).clamp(0.0, 1.0);
        let t_in_next_k = t_in_k + alpha_return * (t_ihx_out_k - t_in_k);
        self.core_inlet_temperature = ThermodynamicTemperature::new::<kelvin>(t_in_next_k);

        // 5. Friction pressure drop and circulator hydraulic power.
        self.update_hydraulics(flow_kg_s);
    }

    /// Darcy-Weisbach frictional pressure drop and the circulator hydraulic
    /// power needed to sustain it.
    ///
    /// `dp = f (L/D) (rho v^2 / 2)` with the bulk velocity `v = m_dot/(rho A)`
    /// and an explicit Haaland friction factor
    /// `1/sqrt(f) = -1.8 log10[(eps/D/3.7)^1.11 + 6.9/Re]`. Below `Re = 2300`
    /// the laminar `f = 64/Re` is used instead. Circulator power is
    /// `m_dot dp / (rho eta)`.
    fn update_hydraulics(&mut self, flow_kg_s: f64) {
        let rho = self.density;
        if !(rho > 0.0) {
            self.pressure_drop = Pressure::new::<pascal>(0.0);
            self.circulator_power = Power::new::<watt>(0.0);
            return;
        }

        let velocity = flow_kg_s / (rho * FLOW_AREA_M2);
        // Helium dynamic viscosity at HTGR conditions, ~3.5e-5 Pa s
        // (illustrative constant -- the transport property varies far less
        // than c_p over this range, and only enters through log(Re)).
        let mu = 3.5e-5;
        let reynolds = rho * velocity * HYDRAULIC_DIAMETER_M / mu;

        let friction_factor = if reynolds < 2300.0 {
            if reynolds > 0.0 {
                64.0 / reynolds
            } else {
                0.0
            }
        } else {
            let relative_roughness = ROUGHNESS_M / HYDRAULIC_DIAMETER_M;
            let inner = (relative_roughness / 3.7).powf(1.11) + 6.9 / reynolds;
            let inv_sqrt_f = -1.8 * inner.log10();
            if inv_sqrt_f > 0.0 {
                1.0 / (inv_sqrt_f * inv_sqrt_f)
            } else {
                0.0
            }
        };

        let dp = friction_factor
            * (LOOP_LENGTH_M / HYDRAULIC_DIAMETER_M)
            * 0.5
            * rho
            * velocity
            * velocity;
        self.pressure_drop = Pressure::new::<pascal>(dp);
        self.circulator_power = Power::new::<watt>(flow_kg_s * dp / (rho * CIRCULATOR_EFFICIENCY));
    }

    /// Helium inventory held in the loop, `rho * A * L` from the real density.
    pub fn helium_inventory(&self) -> Mass {
        Mass::new::<kilogram>(self.density * FLOW_AREA_M2 * LOOP_LENGTH_M)
    }

    /// Current (transient) core-inlet helium temperature -- the IHX helium
    /// outlet after the return transport lag.
    pub fn core_inlet_temperature(&self) -> ThermodynamicTemperature {
        self.core_inlet_temperature
    }

    /// Current (transient) core-outlet helium temperature.
    pub fn core_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.core_outlet_temperature
    }

    /// Helium-side IHX outlet temperature.
    pub fn ihx_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.ihx_outlet_temperature
    }

    /// Current helium mass flow.
    pub fn mass_flow(&self) -> MassRate {
        self.mass_flow
    }

    /// Most recently computed IHX duty transferred to the secondary loop.
    pub fn ihx_duty(&self) -> Power {
        self.ihx_duty
    }

    /// Isobaric specific heat of helium at the current bulk mean temperature.
    pub fn specific_heat(&self) -> SpecificHeatCapacity {
        self.c_p
    }

    /// Frictional pressure drop around the loop at the current flow.
    pub fn pressure_drop(&self) -> Pressure {
        self.pressure_drop
    }

    /// Circulator hydraulic power required to sustain [`Self::pressure_drop`].
    pub fn circulator_power(&self) -> Power {
        self.circulator_power
    }
}

/// Real helium isobaric specific heat and density at temperature `t_k` \[K\]
/// and the loop pressure, from the CoolProp-derived Helmholtz EOS.
///
/// Falls back to the ideal-gas-limit helium values (`c_p = 5193 J/(kg K)`,
/// density from `p/(R_specific T)` with `R_specific = 2077 J/(kg K)`) if the
/// `(p, T)` density solve fails to converge -- helium at HTGR conditions is
/// close to ideal, so the fallback is a physically sane bound rather than a
/// fabricated number, and it keeps a GUI frame from panicking on a transient.
fn helium_properties(t_k: f64) -> (SpecificHeatCapacity, f64) {
    /// Ideal-gas-limit helium `c_p` \[J/(kg K)\].
    const IDEAL_CP: f64 = 5193.0;
    /// Specific gas constant of helium \[J/(kg K)\].
    const R_SPECIFIC: f64 = 2077.0;

    let t = if t_k.is_finite() && t_k > 1.0 {
        t_k
    } else {
        SEED_TEMPERATURE_K
    };

    match state_pt(Fluid::Helium, t, LOOP_PRESSURE_PA) {
        Ok(state) if state.cp.is_finite() && state.cp > 0.0 && state.density > 0.0 => (
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(state.cp),
            state.density,
        ),
        _ => (
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(IDEAL_CP),
            LOOP_PRESSURE_PA / (R_SPECIFIC * t),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::power::megawatt;

    fn nominal_loop() -> HeliumPrimaryLoop {
        HeliumPrimaryLoop::new(MassRate::new::<kilogram_per_second>(85.0))
    }

    fn sink(t_k: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(t_k)
    }

    /// Methodology: helium `c_p` from the ported Helmholtz EOS is compared
    /// against the ideal-gas-limit value `5R/2M = 5193 J/(kg K)`, which real
    /// helium approaches to within a few percent at HTGR conditions (7 MPa,
    /// 573-1200 K). Pass criterion: within 10% of the ideal limit, and
    /// strictly positive density.
    ///
    /// Results (2026-07-28, CoolProp-fork helium EOS, Ortiz-Vega et al.), at
    /// 7 MPa:
    ///
    /// | T \[K\] | `c_p` \[J/(kg K)\] | vs ideal limit | `rho` \[kg/m^3\] |
    /// |---|---|---|---|
    /// | 573 | 5189.3 | -0.071% | 5.7900 |
    /// | 800 | 5189.3 | -0.071% | 4.1683 |
    /// | 1100 | 5189.8 | -0.061% | 3.0417 |
    ///
    /// `c_p` sits just *below* the ideal-gas limit across the range (real
    /// helium at 7 MPa is very slightly non-ideal), and the density falls as
    /// `1/T` would suggest. The values differ from the 5193 constant, which
    /// confirms the EOS path is live rather than silently falling through to
    /// the fallback.
    #[test]
    fn helium_properties_are_near_the_ideal_gas_limit() {
        for t_k in [573.0, 800.0, 1100.0] {
            let (c_p, density) = helium_properties(t_k);
            let cp_val = c_p.get::<joule_per_kilogram_kelvin>();
            assert!(
                (cp_val - 5193.0).abs() / 5193.0 < 0.10,
                "helium c_p {cp_val} at {t_k} K is not within 10% of the ideal limit"
            );
            assert!(density > 0.0, "helium density must be positive at {t_k} K");
        }
    }

    /// Methodology: the effectiveness-NTU IHX must respect the second law in
    /// both directions. The invariant is conditional on which way heat can
    /// flow, checked at every one of 2000 steps of 0.05 s at 200 MWth against
    /// a 585 K sink:
    ///
    /// - while the helium is **hotter** than the sink, the IHX may cool it but
    ///   never past the sink: `T_sink <= T_ihx_out <= T_out`;
    /// - while the helium is **colder** than the sink (which is the case at
    ///   startup, seeded at 573 K against a 585 K sink), the IHX transfers
    ///   nothing rather than heating the helium backwards, so
    ///   `T_ihx_out == T_out`.
    ///
    /// The loop must also end the run hotter than the sink, confirming the
    /// 200 MWth actually drives it through the crossover.
    ///
    /// Results (2026-07-28): both branches held at every step. The run begins
    /// in the no-transfer branch (helium seeded at 573 K, below the 585 K
    /// sink, zero duty) and crosses over as the core heats it. After 200 s of
    /// simulated time at 200 MWth it settles at `T_in = 741.5 K`,
    /// `T_out = 1194.9 K`, `T_ihx_out = 741.5 K` -- a 156.5 K approach above
    /// the sink. An earlier version of this test
    /// asserted the unconditional `T_ihx_out >= T_sink` and failed on the
    /// startup transient -- that assertion was wrong, not the model: a
    /// heat exchanger cannot raise its hot side to the cold side's
    /// temperature.
    #[test]
    fn ihx_respects_the_pinch_in_both_directions() {
        let mut loop_ = nominal_loop();
        let t_sink = sink(585.0);
        let t_sink_k = t_sink.get::<kelvin>();
        let mut saw_no_transfer_branch = false;
        let mut saw_transfer_branch = false;

        for _ in 0..2000 {
            loop_.step(
                Time::new::<second>(0.05),
                Power::new::<megawatt>(200.0),
                MassRate::new::<kilogram_per_second>(85.0),
                t_sink,
            );
            let t_out = loop_.core_outlet_temperature().get::<kelvin>();
            let t_ihx = loop_.ihx_outlet_temperature().get::<kelvin>();

            if t_out > t_sink_k {
                saw_transfer_branch = true;
                assert!(
                    t_ihx >= t_sink_k - 1e-9,
                    "IHX cooled the helium ({t_ihx} K) past the {t_sink_k} K sink"
                );
                assert!(
                    t_ihx <= t_out + 1e-9,
                    "IHX heated the helium ({t_ihx} K) above its inlet {t_out} K"
                );
            } else {
                saw_no_transfer_branch = true;
                assert!(
                    (t_ihx - t_out).abs() < 1e-9,
                    "IHX moved heat backwards: helium {t_out} K below the sink \
                     {t_sink_k} K but left the IHX at {t_ihx} K"
                );
            }
        }

        assert!(
            saw_no_transfer_branch,
            "startup should begin below the sink"
        );
        assert!(
            saw_transfer_branch,
            "200 MWth should drive the loop past the sink"
        );
        assert!(
            loop_.core_outlet_temperature().get::<kelvin>() > t_sink_k,
            "the loop should settle hotter than the secondary sink at load"
        );
    }

    /// The core inlet must be a computed loop variable, not a fixed constant:
    /// cutting secondary heat removal (raising the sink temperature) must
    /// raise the core inlet.
    #[test]
    fn core_inlet_responds_to_secondary_heat_removal() {
        let mut cold_sink = nominal_loop();
        let mut hot_sink = nominal_loop();
        for _ in 0..2000 {
            let dt = Time::new::<second>(0.05);
            let q = Power::new::<megawatt>(200.0);
            let m = MassRate::new::<kilogram_per_second>(85.0);
            cold_sink.step(dt, q, m, sink(560.0));
            hot_sink.step(dt, q, m, sink(700.0));
        }
        assert!(
            hot_sink.core_inlet_temperature().get::<kelvin>()
                > cold_sink.core_inlet_temperature().get::<kelvin>(),
            "a hotter secondary sink must raise the core inlet temperature"
        );
    }

    /// Methodology: once the transient has settled, the core temperature rise
    /// must follow the energy balance `dT = Q/(m_dot c_p)` using the *live*
    /// helium `c_p`. Pass criterion: within 2% of that balance.
    ///
    /// Results (2026-07-28): at 200 MWth and 85 kg/s the measured rise was
    /// `1194.9 - 741.5 = 453.4 K`, against `200e6/(85 x 5189.3) = 453.4 K`
    /// from the balance -- agreement to better than 0.1%.
    #[test]
    fn core_temperature_rise_matches_the_energy_balance() {
        let mut loop_ = nominal_loop();
        let power = Power::new::<megawatt>(200.0);
        let flow = MassRate::new::<kilogram_per_second>(85.0);
        for _ in 0..4000 {
            loop_.step(Time::new::<second>(0.05), power, flow, sink(585.0));
        }

        let measured = loop_.core_outlet_temperature().get::<kelvin>()
            - loop_.core_inlet_temperature().get::<kelvin>();
        let expected = power.get::<watt>()
            / (flow.get::<kilogram_per_second>()
                * loop_.specific_heat().get::<joule_per_kilogram_kelvin>());
        assert!(
            (measured - expected).abs() / expected < 0.02,
            "core rise {measured} K departs from the energy balance {expected} K"
        );
    }

    /// Methodology: the Darcy-Weisbach loss must increase monotonically with
    /// flow, as must the circulator power, and both must be strictly positive
    /// at a running flow. Pass criterion: strict inequalities between a
    /// 40 kg/s and a 120 kg/s case.
    ///
    /// Results (2026-07-28): at the nominal 85 kg/s settled point the loop
    /// showed `dp = 18.1 kPa` and `W_circulator = 0.557 MW` (0.28% of the
    /// 200 MWth heat load, a plausible fraction for a gas-cooled primary
    /// circulator), with a helium inventory of 276.1 kg. The 120 kg/s case
    /// exceeded the 40 kg/s case on both measures.
    #[test]
    fn pressure_drop_and_pump_power_increase_with_flow() {
        let mut slow = nominal_loop();
        let mut fast = nominal_loop();
        for _ in 0..200 {
            let dt = Time::new::<second>(0.05);
            let q = Power::new::<megawatt>(200.0);
            slow.step(
                dt,
                q,
                MassRate::new::<kilogram_per_second>(40.0),
                sink(585.0),
            );
            fast.step(
                dt,
                q,
                MassRate::new::<kilogram_per_second>(120.0),
                sink(585.0),
            );
        }
        assert!(slow.pressure_drop().get::<pascal>() > 0.0);
        assert!(fast.pressure_drop().get::<pascal>() > slow.pressure_drop().get::<pascal>());
        assert!(fast.circulator_power().get::<watt>() > slow.circulator_power().get::<watt>());
    }

    /// The helium inventory must be positive so the residence time driving the
    /// schematic's flow tracers is finite.
    #[test]
    fn helium_inventory_is_positive() {
        let loop_ = nominal_loop();
        assert!(loop_.helium_inventory().get::<kilogram>() > 0.0);
    }
}
