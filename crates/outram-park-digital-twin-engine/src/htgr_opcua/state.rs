//! The plant snapshot the HTGR OPC-UA node map reads from.
//!
//! [`HtgrPlantSnapshot`] is a flat, `Copy` block of `f64` scalars **in SI base
//! and coherent SI derived units** — watts, kelvin, pascals, kilograms per
//! second, seconds, joules per kilogram. It is the wire-side twin of the
//! `htgr_sim_v1` example's GUI snapshot (`examples/htgr_sim_v1/app/state.rs`,
//! `HtgrSnapshot`), which carries the same quantities in *display* units
//! (megawatts, kilopascals, megapascals, pcm).
//!
//! ## Why a second struct, in SI
//!
//! An OPC-UA client sees a bare `Double`. It cannot tell 10 from 10 MPa from
//! 1.0e7 Pa. The GUI snapshot deliberately mixes scales for readability — it
//! carries megawatts, kilopascals *and* megapascals side by side — and
//! publishing that mixture would hand a client three different pressure scales
//! with nothing but a field name to disambiguate them. Normalising once, here,
//! at the boundary is the cheapest place to remove that ambiguity, and it is
//! done in one audited function per unit rather than at 30 call sites.
//!
//! ## Mapping from the GUI snapshot
//!
//! Wiring the example to this module is a field-by-field copy through the
//! conversion helpers below. The full mapping:
//!
//! | `HtgrSnapshot` field (GUI) | `HtgrPlantSnapshot` field (SI) | Conversion |
//! |---|---|---|
//! | `external_reactivity_dollars` | `external_reactivity_dollar` | none (dimensionless) |
//! | `helium_flow_setpoint_kg_per_s` | `helium_flow_setpoint_kg_per_s` | none |
//! | `reactor_power_mw` | `reactor_power_w` | [`megawatts_to_watts`] |
//! | `prompt_power_mw` | `prompt_power_w` | [`megawatts_to_watts`] |
//! | `delayed_power_mw` | `delayed_power_w` | [`megawatts_to_watts`] |
//! | `fuel_temperature_k` | `fuel_temperature_k` | none |
//! | `reactivity_margin_dollars` | `reactivity_margin_dollar` | none (dimensionless) |
//! | `delayed_neutron_fraction_pcm` | `delayed_neutron_fraction_ratio` | [`pcm_to_ratio`] |
//! | `core_inlet_temp_k` | `core_inlet_temp_k` | none |
//! | `core_outlet_temp_k` | `core_outlet_temp_k` | none |
//! | `helium_mass_flow_kg_per_s` | `helium_mass_flow_kg_per_s` | none |
//! | `ihx_duty_mw` | `ihx_duty_w` | [`megawatts_to_watts`] |
//! | `ihx_outlet_temp_k` | `ihx_outlet_temp_k` | none |
//! | `helium_residence_time_s` | `helium_residence_time_s` | none |
//! | `primary_pressure_drop_kpa` | `primary_pressure_drop_pa` | [`kilopascals_to_pascals`] |
//! | `circulator_power_mw` | `circulator_power_w` | [`megawatts_to_watts`] |
//! | `helium_cp_j_per_kg_k` | `helium_cp_j_per_kg_k` | none |
//! | `steam_pressure_mpa` | `steam_pressure_pa` | [`megapascals_to_pascals`] |
//! | `sg_steam_outlet_temp_k` | `sg_steam_outlet_temp_k` | none |
//! | `steam_enthalpy_j_per_kg` | `steam_enthalpy_j_per_kg` | none |
//! | `turbine_inlet_temp_k` | `turbine_inlet_temp_k` | none |
//! | `turbine_power_mw` | `turbine_power_w` | [`megawatts_to_watts`] |
//! | `steam_quality_after_turbine` | `steam_quality_after_turbine` | none (dimensionless) |
//! | `condenser_pressure_kpa` | `condenser_pressure_pa` | [`kilopascals_to_pascals`] |
//! | `secondary_mass_flow_kg_per_s` | `secondary_mass_flow_kg_per_s` | none |
//! | `secondary_residence_time_s` | `secondary_residence_time_s` | none |
//! | `feedwater_enthalpy_j_per_kg` | `feedwater_enthalpy_j_per_kg` | none |
//! | `condensate_enthalpy_j_per_kg` | `condensate_enthalpy_j_per_kg` | none |
//! | `feed_pump_power_mw` | `feed_pump_power_w` | [`megawatts_to_watts`] |
//! | `net_cycle_power_mw` | `net_cycle_power_w` | [`megawatts_to_watts`] |
//! | `condenser_duty_mw` | `condenser_duty_w` | [`megawatts_to_watts`] |
//! | `cooling_water_outlet_temp_k` | `cooling_water_outlet_temp_k` | none |
//! | `sim_time_s` | `sim_time_s` | none |
//!
//! Every field of the GUI snapshot appears exactly once, and every field of
//! this snapshot is published by the node map — see
//! `node_map::tests::signal_and_control_accessors_cover_every_snapshot_field`.
//!
//! ## Status
//!
//! These are the outputs of an **offline demonstration model**, not
//! measurements and not a validated HTGR model. Per-quantity caveats (which
//! numbers rest on real property libraries, which rest on illustrative plant
//! data, and which do not vary at all during a run) are carried in the node
//! descriptions — see [`super::node_map`].

use std::sync::{Arc, RwLock};

/// Scalar snapshot of the HTGR demonstration plant in **SI units**, as
/// published over OPC-UA.
///
/// Every field is a bare `f64` in the unit named by its suffix:
///
/// | Suffix | Unit |
/// |---|---|
/// | `_w` | watt (W) |
/// | `_k` | kelvin (K) |
/// | `_pa` | pascal (Pa) |
/// | `_kg_per_s` | kilogram per second (kg/s) |
/// | `_s` | second (s) |
/// | `_j_per_kg` | joule per kilogram (J/kg) |
/// | `_j_per_kg_k` | joule per kilogram kelvin (J/(kg K)) |
/// | `_dollar` | reactivity in dollars (`rho/beta`, dimensionless) |
/// | `_ratio` | dimensionless ratio |
///
/// The struct is `Copy` (33 `f64`s, 264 bytes), so a server thread can lift a
/// whole consistent snapshot out from behind its lock in one move rather than
/// reading fields one at a time and publishing a torn mixture of two timesteps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HtgrPlantSnapshot {
    // ---- Control inputs (writable by an OPC-UA client) ----
    /// User-commanded external reactivity, in **dollars** (`rho/beta`;
    /// dimensionless). Multiply by `delayed_neutron_fraction_ratio` to recover
    /// the absolute reactivity `rho`. Envelope in this simulator: -2 to +1 $,
    /// where +1 $ is exactly prompt critical.
    pub external_reactivity_dollar: f64,
    /// User-commanded helium circulator mass-flow setpoint \[kg/s\].
    /// Envelope in this simulator: 10 to 150 kg/s.
    pub helium_flow_setpoint_kg_per_s: f64,

    // ---- Kinetics ----
    /// Total reactor thermal power, prompt + delayed \[W\]. Nominal operating
    /// point of this demonstration model is 2.0e8 W (200 MWth).
    pub reactor_power_w: f64,
    /// Prompt-excursion-layer power `P_p` before the delayed increment is added
    /// back \[W\].
    pub prompt_power_w: f64,
    /// Delayed-neutron power increment `S*dt` added this step \[W\]. Small
    /// compared with `reactor_power_w` — it is a per-step increment, not the
    /// delayed power fraction.
    pub delayed_power_w: f64,
    /// Lumped fuel temperature \[K\] from the adiabatic prompt-excursion
    /// feedback. Single whole-core node; typical range 800-1500 K.
    pub fuel_temperature_k: f64,
    /// Reactivity margin `rho_ext - beta + alpha_f (T_f - T_ref)` expressed in
    /// **dollars** (dimensionless). Negative is subcritical on prompt neutrons.
    pub reactivity_margin_dollar: f64,
    /// Effective total delayed-neutron fraction `beta = sum(beta_i)` as a
    /// dimensionless ratio (not pcm). 0.0065 in this model; multiply by 1e5
    /// for pcm.
    pub delayed_neutron_fraction_ratio: f64,

    // ---- Primary helium loop ----
    /// Core inlet helium temperature \[K\]. Typical range 550-800 K.
    pub core_inlet_temp_k: f64,
    /// Core outlet helium temperature \[K\]. Typical range 700-1300 K.
    pub core_outlet_temp_k: f64,
    /// Helium mass flow \[kg/s\], floored at 1 kg/s by the loop model.
    pub helium_mass_flow_kg_per_s: f64,
    /// Helium loop transport residence time `m/m_dot` \[s\].
    pub helium_residence_time_s: f64,
    /// Frictional pressure drop around the helium loop \[Pa\]. Order 1.8e4 Pa
    /// at the nominal 85 kg/s.
    pub primary_pressure_drop_pa: f64,
    /// Circulator hydraulic power \[W\]. Order 5.6e5 W at nominal flow.
    pub circulator_power_w: f64,
    /// Helium isobaric specific heat at the current bulk mean temperature
    /// \[J/(kg K)\]. Near 5189 J/(kg K) at 7 MPa across the loop's range.
    pub helium_cp_j_per_kg_k: f64,

    // ---- Intermediate heat exchanger ----
    /// IHX duty transferred from helium to the steam side \[W\].
    pub ihx_duty_w: f64,
    /// Helium-side IHX outlet temperature \[K\] — what the core inlet relaxes
    /// toward once the return transport lag has played out.
    pub ihx_outlet_temp_k: f64,

    // ---- Secondary steam cycle ----
    /// Live steam pressure at the steam-generator outlet \[Pa\]. 1.0e7 Pa
    /// (10 MPa) in this model, and **held fixed** — see [`super::node_map`].
    pub steam_pressure_pa: f64,
    /// Steam-generator steam outlet temperature \[K\], from an IAPWS-IF97
    /// `(p, h)` flash. Typical range 580-900 K.
    pub sg_steam_outlet_temp_k: f64,
    /// Steam-generator / turbine-inlet steam specific enthalpy \[J/kg\].
    /// Around 3.4e6 J/kg at the controller's target.
    pub steam_enthalpy_j_per_kg: f64,
    /// Turbine inlet steam temperature \[K\]. Equal to `sg_steam_outlet_temp_k`
    /// in the current model — no steam-line loss is modelled between them.
    pub turbine_inlet_temp_k: f64,
    /// Turbine mechanical power \[W\]. Around 7.1e7 W at 200 MW IHX duty.
    pub turbine_power_w: f64,
    /// Steam quality at the turbine exhaust, dimensionless in `[0, 1]`
    /// (0 = saturated liquid, 1 = dry saturated vapour).
    pub steam_quality_after_turbine: f64,
    /// Condenser back-pressure \[Pa\]. 7.0e3 Pa in this model, and **held
    /// fixed**.
    pub condenser_pressure_pa: f64,
    /// Secondary (feedwater/steam) mass flow \[kg/s\], moved by the feedwater
    /// controller within 5-200 kg/s.
    pub secondary_mass_flow_kg_per_s: f64,
    /// Secondary loop transport residence time `m/m_dot` \[s\].
    pub secondary_residence_time_s: f64,
    /// Feedwater specific enthalpy entering the steam generator \[J/kg\] —
    /// condensate plus real feed-pump work.
    pub feedwater_enthalpy_j_per_kg: f64,
    /// Condensate (hotwell saturated-liquid) specific enthalpy \[J/kg\].
    pub condensate_enthalpy_j_per_kg: f64,
    /// Feed-pump power \[W\].
    pub feed_pump_power_w: f64,
    /// Net cycle power \[W\]: turbine output less feed-pump work.
    pub net_cycle_power_w: f64,
    /// Heat rejected in the condenser \[W\].
    pub condenser_duty_w: f64,
    /// Cooling-water outlet temperature from the condenser \[K\].
    pub cooling_water_outlet_temp_k: f64,

    // ---- Diagnostics ----
    /// Accumulated simulation time \[s\]. Simulated, not wall-clock.
    pub sim_time_s: f64,
}

impl Default for HtgrPlantSnapshot {
    /// The nominal start-up point, exactly the `HtgrSnapshot::default()` of the
    /// `htgr_sim_v1` example converted to SI. A server may publish this before
    /// the physics thread has produced its first step.
    fn default() -> Self {
        Self {
            external_reactivity_dollar: 0.0,
            helium_flow_setpoint_kg_per_s: 85.0,
            reactor_power_w: megawatts_to_watts(200.0),
            prompt_power_w: megawatts_to_watts(200.0),
            delayed_power_w: 0.0,
            fuel_temperature_k: 900.0,
            reactivity_margin_dollar: 0.0,
            delayed_neutron_fraction_ratio: pcm_to_ratio(650.0),
            core_inlet_temp_k: 573.0,
            core_outlet_temp_k: 573.0,
            helium_mass_flow_kg_per_s: 85.0,
            helium_residence_time_s: 0.0,
            primary_pressure_drop_pa: 0.0,
            circulator_power_w: 0.0,
            helium_cp_j_per_kg_k: 5193.0,
            ihx_duty_w: 0.0,
            ihx_outlet_temp_k: 573.0,
            steam_pressure_pa: megapascals_to_pascals(10.0),
            sg_steam_outlet_temp_k: 500.0,
            steam_enthalpy_j_per_kg: 1.0e6,
            turbine_inlet_temp_k: 500.0,
            turbine_power_w: 0.0,
            steam_quality_after_turbine: 0.0,
            condenser_pressure_pa: kilopascals_to_pascals(7.0),
            secondary_mass_flow_kg_per_s: 80.0,
            secondary_residence_time_s: 0.0,
            feedwater_enthalpy_j_per_kg: 1.0e6,
            condensate_enthalpy_j_per_kg: 1.63e5,
            feed_pump_power_w: 0.0,
            net_cycle_power_w: 0.0,
            condenser_duty_w: 0.0,
            cooling_water_outlet_temp_k: 298.15,
            sim_time_s: 0.0,
        }
    }
}

/// The snapshot as shared between the physics thread (writer) and the OPC-UA
/// server thread (reader).
///
/// `RwLock` rather than `Mutex` per the workspace design rules: the server may
/// serve many concurrent reads while the physics thread writes once per
/// timestep. Callers should clone the whole `HtgrPlantSnapshot` out under one
/// read guard rather than holding the lock across a set of per-node reads.
pub type SharedHtgrSnapshot = Arc<RwLock<HtgrPlantSnapshot>>;

/// Convert a power in megawatts to watts (`1 MW = 1e6 W`).
#[inline]
pub fn megawatts_to_watts(megawatts: f64) -> f64 {
    megawatts * 1.0e6
}

/// Convert a pressure in kilopascals to pascals (`1 kPa = 1e3 Pa`).
#[inline]
pub fn kilopascals_to_pascals(kilopascals: f64) -> f64 {
    kilopascals * 1.0e3
}

/// Convert a pressure in megapascals to pascals (`1 MPa = 1e6 Pa`).
#[inline]
pub fn megapascals_to_pascals(megapascals: f64) -> f64 {
    megapascals * 1.0e6
}

/// Convert a reactivity or delayed-neutron fraction in pcm (per cent mille,
/// `1e-5`) to a dimensionless ratio.
///
/// Divides by `1e5` rather than multiplying by `1e-5`, which is not a
/// cosmetic choice: `1e5` is exactly representable as a double and `1e-5` is
/// not, so `650.0 * 1.0e-5` gives `0.006500000000000001` while
/// `650.0 / 1.0e5` gives `0.0065`. The published `beta_eff` would otherwise
/// differ from the model's own `0.0065` in the last bit for no physical
/// reason. Found by
/// [`tests::si_conversions_are_exact_at_the_nominal_operating_point`], not by
/// inspection.
#[inline]
pub fn pcm_to_ratio(pcm: f64) -> f64 {
    pcm / 1.0e5
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The unit conversions must be exact at the scales this interface uses, or
    /// a client reading `steam_pressure_pa` would see a number that is right to
    /// six digits and wrong in the seventh for no physical reason.
    ///
    /// **Methodology.** Convert the nominal operating values of the
    /// `htgr_sim_v1` model (200 MW, 10 MPa, 7 kPa, 650 pcm) and compare with
    /// the hand-computed SI values. Pass criterion: bit-exact equality, which
    /// holds because each factor is a power of ten exactly representable in
    /// binary floating point up to 1e22.
    ///
    /// **Results (2026-08-12).** All four conversions are now bit-exact:
    /// 200 MW -> 2.0e8 W, 10 MPa -> 1.0e7 Pa, 7 kPa -> 7.0e3 Pa,
    /// 650 pcm -> 6.5e-3. The pcm case failed on first run: [`pcm_to_ratio`]
    /// originally multiplied by `1e-5`, which is not exactly representable as a
    /// double, and returned `0.006500000000000001` against the literal
    /// `0.0065` — a 1-ulp error that would have made the published `beta_eff`
    /// differ from the model's own value in the last bit. The implementation
    /// was changed to divide by the exactly representable `1e5`. The test was
    /// right and the code was wrong; the criterion was not relaxed to make it
    /// pass.
    #[test]
    fn si_conversions_are_exact_at_the_nominal_operating_point() {
        assert_eq!(megawatts_to_watts(200.0), 2.0e8);
        assert_eq!(megapascals_to_pascals(10.0), 1.0e7);
        assert_eq!(kilopascals_to_pascals(7.0), 7.0e3);
        assert_eq!(pcm_to_ratio(650.0), 6.5e-3);
    }

    /// The default snapshot must be the example GUI snapshot's default,
    /// converted — not an independently invented operating point, which would
    /// make a freshly started server publish a plant state the simulator never
    /// occupies.
    ///
    /// **Methodology.** Compare the SI default field-by-field against the
    /// `htgr_sim_v1` `HtgrSnapshot::default()` values converted by hand.
    /// Pass criterion: exact equality on the converted quantities.
    ///
    /// **Results (2026-08-12).** All checked fields matched: 200 MWth ->
    /// 2.0e8 W, 650 pcm -> 6.5e-3, 10 MPa -> 1.0e7 Pa, 7 kPa -> 7.0e3 Pa,
    /// 85 kg/s and 573 K carried across unchanged.
    #[test]
    fn default_snapshot_is_the_gui_default_in_si() {
        let s = HtgrPlantSnapshot::default();
        assert_eq!(s.reactor_power_w, 2.0e8);
        assert_eq!(s.delayed_neutron_fraction_ratio, 6.5e-3);
        assert_eq!(s.steam_pressure_pa, 1.0e7);
        assert_eq!(s.condenser_pressure_pa, 7.0e3);
        assert_eq!(s.helium_flow_setpoint_kg_per_s, 85.0);
        assert_eq!(s.core_inlet_temp_k, 573.0);
        assert_eq!(s.sim_time_s, 0.0);
    }

    /// Every default field must be finite. A NaN or infinity in the start-up
    /// snapshot would be served to a client as a valid `Double` and would
    /// poison any trend it fed.
    ///
    /// **Methodology.** Read every published node off the default snapshot via
    /// the node map's accessors and check `is_finite()`.
    ///
    /// **Results (2026-08-12).** All 33 published values finite.
    #[test]
    fn default_snapshot_is_entirely_finite() {
        use super::super::node_map::{HtgrControl, HtgrSignal};
        let s = HtgrPlantSnapshot::default();
        for signal in HtgrSignal::ALL {
            assert!(signal.read(&s).is_finite(), "{signal:?} is not finite");
        }
        for control in HtgrControl::ALL {
            assert!(control.read(&s).is_finite(), "{control:?} is not finite");
        }
    }
}
