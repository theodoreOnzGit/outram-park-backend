use petir::mathf::RealMath;
use uom::si::f64::*;
use uom::si::volume::cubic_meter;

use crate::constants::p_crit_water;
use crate::constants::t_crit_water;
use crate::dynamic_viscosity::mu_ph_eqm;
use crate::prelude::functional_programming::ph_flash_eqm::cp_ph_eqm;
use crate::prelude::functional_programming::ph_flash_eqm::cv_ph_eqm;
use crate::prelude::functional_programming::ph_flash_eqm::lambda_ph_eqm;
use crate::prelude::functional_programming::ph_flash_eqm::ph_flash_region;
use crate::prelude::functional_programming::ph_flash_eqm::w_ph_eqm_hem;
use crate::prelude::functional_programming::ph_flash_eqm::w_ph_wood_wallis;
use crate::prelude::functional_programming::ph_flash_eqm::x_ph_flash;
use crate::prelude::functional_programming::pt_flash_eqm::FwdEqnRegion;
use crate::region_2_vapour::*;
use crate::region_4_vap_liq_equilibrium::sat_pressure_4;
use crate::region_4_vap_liq_equilibrium::sat_temp_4;
use crate::steam_turbine_equations::choked_flow::single_phase_basic_choked_flow::get_critical_pressure_pure_vapour_ph_stagnation_properties;
use crate::steam_turbine_equations::choked_flow::single_phase_basic_choked_flow::get_critical_pressure_ratio_ideal_gas_using_throat_ph;
use crate::steam_turbine_equations::choked_flow::*;
impl super::TampinesSteamTableCV {
    /// Returns the pressure of the control volume.
    pub fn get_pressure(&self) -> Pressure {
        self.pressure
    }

    /// Returns the thermodynamic temperature of the control volume.
    pub fn get_temperature(&self) -> ThermodynamicTemperature {
        self.temperature
    }

    /// Returns the specific volume of the fluid in the control volume.
    pub fn get_specific_volume(&self) -> SpecificVolume {
        self.specific_volume
    }

    /// Returns the specific enthalpy of the fluid in the control volume.
    pub fn get_specific_enthalpy(&self) -> AvailableEnergy {
        self.specific_enthalpy
    }

    /// Returns the specific entropy of the fluid in the control volume.
    pub fn get_specific_entropy(&self) -> SpecificHeatCapacity {
        self.specific_entropy
    }

    /// Returns the total volume of the control volume.
    pub fn get_volume(&self) -> Volume {
        self.volume
    }
    /// returns the mass within the control volume
    pub fn get_mass(&self) -> Mass {
        return self.volume / self.specific_volume;
    }

    /// returns viscosity (important for Reynold's number)
    pub fn get_viscosity(&self) -> DynamicViscosity {
        let p = self.pressure;
        let h = self.specific_enthalpy;

        return mu_ph_eqm(p, h);
    }

    /// Returns the **frozen** speed of sound of the control volume
    /// (`w_ph_wood_wallis`).
    ///
    /// Outside the vapour–liquid dome this is the ordinary single-phase speed
    /// of sound. **Inside the dome it is Wood's (Wallis) frozen mixture speed**,
    /// which assumes no mass exchange between the phases while the wave
    /// passes. It is not the homogeneous-equilibrium speed, which is lower,
    /// often by an order of magnitude: for that, use
    /// [`Self::get_equilibrium_speed_of_sound`]. An earlier version of this doc
    /// said only "returns speed of sound" (corrected 2026-10-16, gh #901).
    pub fn get_speed_of_sound(&self) -> Velocity {
        let p = self.pressure;
        let h = self.specific_enthalpy;

        return w_ph_wood_wallis(p, h);
    }

    /// Returns the **homogeneous-equilibrium (HEM)** speed of sound of the
    /// control volume, [`w_ph_eqm_hem`].
    ///
    /// Inside the vapour–liquid dome the phases are assumed to stay in
    /// equilibrium as the wave passes (liquid flashes, vapour condenses), and
    /// the speed comes from Kieffer (1977) eq. 28, with a finite-difference
    /// fallback. Outside the dome it equals [`Self::get_speed_of_sound`]. This
    /// is the speed a homogeneous-equilibrium flow model must use for its
    /// characteristics, Courant number and choking criterion. Valid where the
    /// `(p, h)` flash is valid; not clamped.
    pub fn get_equilibrium_speed_of_sound(&self) -> Velocity {
        w_ph_eqm_hem(self.pressure, self.specific_enthalpy)
    }

    /// Mach number `v / c` of a flow speed `v`, using the **frozen** speed of
    /// [`Self::get_speed_of_sound`] (so in the dome it is not the HEM Mach
    /// number).
    pub fn get_mach_number(&self, v: Velocity) -> Ratio {
        v / self.get_speed_of_sound()
    }

    /// returns the specific heat ratio cp/cv of steam
    pub fn get_specific_heat_ratio(&self) -> Ratio {
        let p = self.pressure;
        let h = self.specific_enthalpy;

        let cp = cp_ph_eqm(p, h);
        let cv = cv_ph_eqm(p, h);

        cp / cv
    }

    /// returns cp
    pub fn get_cp(&self) -> SpecificHeatCapacity {
        let p = self.pressure;
        let h = self.specific_enthalpy;

        cp_ph_eqm(p, h)
    }
    /// returns cv
    pub fn get_cv(&self) -> SpecificHeatCapacity {
        let p = self.pressure;
        let h = self.specific_enthalpy;

        cv_ph_eqm(p, h)
    }

    /// returns thermal thermal_conductivity of steam
    pub fn get_thermal_conductivity(&self) -> ThermalConductivity {
        let p = self.pressure;
        let h = self.specific_enthalpy;

        lambda_ph_eqm(p, h)
    }

    /// returns critical pressure ratio for choked flow
    /// ie to accelerate the flow to Mach 1
    ///
    /// P*/P0 = (2/(k+1))^(k/(k-1))
    ///
    /// This is under ideal gas assumption (may not work)
    ///
    /// Note that this is done using throat properties
    pub fn get_critical_pressure_ratio_ideal_gas(&self) -> Ratio {
        let p = self.pressure;
        let h = self.specific_enthalpy;

        get_critical_pressure_ratio_ideal_gas_using_throat_ph(p, h)
    }

    /// Returns critical pressure ratio for choked flow using isentropic relations
    /// This accounts for real gas behavior of steam
    ///
    /// The critical pressure P* is found where the flow reaches Mach 1 during
    /// isentropic expansion from stagnation conditions (P0, h0)
    /// This only works for vapour
    pub fn get_critical_pressure_ratio_pure_vapour(&self) -> Ratio {
        let p0 = self.pressure;

        // Find critical pressure where Mach = 1
        // This requires iterative solution
        let p_star = self.get_critical_pressure_pure_vapour();

        p_star / p0
    }

    /// Critical pressure and mass flux for choked flow, assuming `self` holds
    /// stagnation properties.
    ///
    /// Routes to the correct HEM solver based on the stagnation region:
    /// - Region 4 (two-phase) → in-dome solver
    /// - Region 1 (subcooled liquid) or liquid-like supercritical → subcooled solver
    /// - Region 2/5 (superheated vapour) or vapour-like supercritical → superheated solver
    ///
    /// Validated against Zaloudek (1961) HEM reference curves for x_t = 0.0–1.00.
    #[inline]
    pub fn get_crit_pressure_and_massflux(&self) -> (Pressure, MassFlux) {
        let p0 = self.pressure;
        let h0 = self.specific_enthalpy;
        get_critical_pressure_and_mass_flux_multiphase_ph(p0, h0)
    }

    /// finds pressure where mach number = 1 during isentropic expansion
    /// for vapour liquid eqm and subcooled liquid
    /// it should work vapour as well, just that the vapour algorithm
    /// tends to use ideal gas critical pressure to bound the search
    /// this one does not
    ///
    ///
    /// Note: I tried mechanical equilibrium where slip ratio = 1
    /// that is both liquid and vapour move at same velocity
    /// this does NOT work.
    /// because no matter how low i go in terms of pressure, the
    /// vle velocity never reaches close to mach 1
    ///
    pub fn get_critical_pressure_vle(&self) -> Pressure {
        let (pressure, _mass_flux) = self.get_crit_pressure_and_massflux();

        return pressure;
    }

    /// Finds the pressure where Mach number = 1 during isentropic expansion
    /// This only works for superheated vapour
    pub fn get_critical_pressure_pure_vapour(&self) -> Pressure {
        let p0 = self.pressure;
        let s0 = self.specific_entropy;
        let h0 = self.specific_enthalpy;

        get_critical_pressure_pure_vapour_ph_stagnation_properties(p0, h0, Some(s0))
    }

    /// Returns the mass density (kg/m^3) of the fluid in the control
    /// volume, i.e. the reciprocal of `get_specific_volume`.
    pub fn get_rho(&self) -> MassDensity {
        self.get_specific_volume().recip()
    }

    /// Returns the IAPWS-IF97 forward-equation region (1-5) the control
    /// volume's current `(pressure, specific_enthalpy)` state falls in, via
    /// a `(p,h)` flash.
    pub fn get_region(&self) -> FwdEqnRegion {
        let p = self.pressure;
        let h = self.specific_enthalpy;
        let region = ph_flash_region(p, h);

        return region;
    }

    /// get metastable steam state, (region 2 only)
    ///
    /// if not region 2, then returns a None value
    pub fn get_metastable_steam_specific_volume(&self) -> Option<SpecificVolume> {
        let p = self.pressure;
        let t = self.temperature;
        let h = self.specific_enthalpy;
        let region = ph_flash_region(p, h);

        match region {
            FwdEqnRegion::Region2 => {
                let v = v_tp_2_metastable(t, p);
                return Some(v);
            }
            FwdEqnRegion::Region1 => None,
            FwdEqnRegion::Region3 => None,
            FwdEqnRegion::Region4 => None,
            FwdEqnRegion::Region5 => None,
        }
    }

    /// get metastable steam state, (region 2 only)
    ///
    /// if not region 2, then returns a None value
    pub fn get_metastable_steam_specific_enthalpy(&self) -> Option<AvailableEnergy> {
        let p = self.pressure;
        let t = self.temperature;
        let h = self.specific_enthalpy;
        let region = ph_flash_region(p, h);

        match region {
            FwdEqnRegion::Region2 => {
                let h = h_tp_2_metastable(t, p);
                return Some(h);
            }
            FwdEqnRegion::Region1 => None,
            FwdEqnRegion::Region3 => None,
            FwdEqnRegion::Region4 => None,
            FwdEqnRegion::Region5 => None,
        }
    }

    /// get metastable steam state, (region 2 only)
    ///
    /// if not region 2, then returns a None value
    pub fn get_metastable_steam_internal_energy(&self) -> Option<AvailableEnergy> {
        let p = self.pressure;
        let t = self.temperature;
        let h = self.specific_enthalpy;
        let region = ph_flash_region(p, h);

        match region {
            FwdEqnRegion::Region2 => {
                let u = u_tp_2_metastable(t, p);
                return Some(u);
            }
            FwdEqnRegion::Region1 => None,
            FwdEqnRegion::Region3 => None,
            FwdEqnRegion::Region4 => None,
            FwdEqnRegion::Region5 => None,
        }
    }

    /// get metastable steam state, (region 2 only)
    ///
    /// if not region 2, then returns a None value
    pub fn get_metastable_steam_specific_entropy(&self) -> Option<SpecificHeatCapacity> {
        let p = self.pressure;
        let t = self.temperature;
        let h = self.specific_enthalpy;
        let region = ph_flash_region(p, h);

        match region {
            FwdEqnRegion::Region2 => {
                let s = s_tp_2_metastable(t, p);
                return Some(s);
            }
            FwdEqnRegion::Region1 => None,
            FwdEqnRegion::Region3 => None,
            FwdEqnRegion::Region4 => None,
            FwdEqnRegion::Region5 => None,
        }
    }
    /// get metastable steam state, (region 2 only)
    ///
    /// if not region 2, then returns a None value
    pub fn get_metastable_steam_cp(&self) -> Option<SpecificHeatCapacity> {
        let p = self.pressure;
        let t = self.temperature;
        let h = self.specific_enthalpy;
        let region = ph_flash_region(p, h);

        match region {
            FwdEqnRegion::Region2 => {
                let cp = cp_tp_2_metastable(t, p);
                return Some(cp);
            }
            FwdEqnRegion::Region1 => None,
            FwdEqnRegion::Region3 => None,
            FwdEqnRegion::Region4 => None,
            FwdEqnRegion::Region5 => None,
        }
    }
    /// get metastable steam state, (region 2 only)
    ///
    /// if not region 2, then returns a None value
    pub fn get_metastable_steam_cv(&self) -> Option<SpecificHeatCapacity> {
        let p = self.pressure;
        let t = self.temperature;
        let h = self.specific_enthalpy;
        let region = ph_flash_region(p, h);

        match region {
            FwdEqnRegion::Region2 => {
                let cv = cv_tp_2_metastable(t, p);
                return Some(cv);
            }
            FwdEqnRegion::Region1 => None,
            FwdEqnRegion::Region3 => None,
            FwdEqnRegion::Region4 => None,
            FwdEqnRegion::Region5 => None,
        }
    }
    /// get metastable steam state, (region 2 only)
    ///
    /// if not region 2, then returns a None value
    pub fn get_metastable_steam_speed_of_sound(&self) -> Option<Velocity> {
        let p = self.pressure;
        let t = self.temperature;
        let h = self.specific_enthalpy;
        let region = ph_flash_region(p, h);

        match region {
            FwdEqnRegion::Region2 => {
                let c = w_tp_2_metastable(t, p);
                return Some(c);
            }
            FwdEqnRegion::Region1 => None,
            FwdEqnRegion::Region3 => None,
            FwdEqnRegion::Region4 => None,
            FwdEqnRegion::Region5 => None,
        }
    }

    /// get the steam quality, only if the region is in region 4
    /// region 4 is the vapour liquid equilibrium
    pub fn get_quality(&self) -> f64 {
        let p = self.pressure;
        let h = self.specific_enthalpy;

        let x = x_ph_flash(p, h);
        x
    }
    /// get the saturation temperature based on pressure
    /// provided pressure is less than p_crit
    pub fn try_new_tsat_based_on_pressure(&self) -> Option<ThermodynamicTemperature> {
        let p_crit = p_crit_water();

        if self.pressure > p_crit {
            return None;
        }

        if self.pressure == p_crit {
            return Some(t_crit_water());
        }

        let tsat = sat_temp_4(self.pressure);

        return Some(tsat);
    }

    /// get the saturation pressure based on temperature
    /// provided temperature is less than t_crit
    pub fn try_new_psat_based_on_temperature(&self) -> Option<Pressure> {
        let t_crit = t_crit_water();

        if self.temperature > t_crit {
            return None;
        }

        if self.temperature == t_crit {
            return Some(p_crit_water());
        }

        let psat = sat_pressure_4(self.temperature);

        return Some(psat);
    }

    /// get the saturation temperature based on pressure
    /// provided pressure is less than p_crit
    pub fn try_get_tsat(p: Pressure) -> Option<ThermodynamicTemperature> {
        let p_crit = p_crit_water();

        if p > p_crit {
            return None;
        }

        if p == p_crit {
            return Some(t_crit_water());
        }

        let tsat = sat_temp_4(p);

        return Some(tsat);
    }

    /// get the saturation pressure based on temperature
    /// provided temperature is less than t_crit
    pub fn try_get_psat(t: ThermodynamicTemperature) -> Option<Pressure> {
        let t_crit = t_crit_water();

        if t > t_crit {
            return None;
        }

        if t == t_crit {
            return Some(p_crit_water());
        }

        let psat = sat_pressure_4(t);

        return Some(psat);
    }

    /// just a convenience function to get ref volume
    /// 1m3
    pub fn get_ref_vol() -> Volume {
        Volume::new::<cubic_meter>(1.0)
    }

    /// Critical mass flux for choked flow, assuming `self` holds stagnation properties.
    ///
    /// Convenience wrapper around [`get_crit_pressure_and_massflux`] that returns
    /// only the mass flux.
    pub fn get_stagnation_critical_mass_flux(&self) -> MassFlux {
        let (_critical_pressure, mass_flux) = self.get_crit_pressure_and_massflux();
        mass_flux
    }
}

#[cfg(test)]
mod tests {
    use super::super::TampinesSteamTableCV;
    use crate::interfaces::functional_programming::ph_flash_eqm::{ph_flash_region, s_ph_eqm};
    use crate::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion;
    use crate::region_1_subcooled_liquid::h_tp_1;
    use crate::region_2_vapour::h_tp_2;
    use crate::region_4_vap_liq_equilibrium::{sat_temp_4, w_ps_eqm_region4_kieffer};
    use uom::si::f64::{Pressure, Volume};
    use uom::si::pressure::bar;
    use uom::si::velocity::meter_per_second;
    use uom::si::volume::cubic_meter;

    /// V&V: `get_equilibrium_speed_of_sound` at water-steam dome states against
    /// Kieffer (1977) Fig. 9 (gh #901, epic #896 step M2).
    ///
    /// Methodology: water at 1 bar, `h = (1−x)h' + x h''` with the IF97
    /// saturated enthalpies at `T_sat(1 bar)`, at `x = 0.1214` and `x = 0.3527`.
    /// Reference: Kieffer, S. W. (1977), "Sound speed in liquid-gas mixtures:
    /// water-air and water-steam", *J. Geophys. Res.* 82(20), 2895–2904,
    /// Fig. 9, digitised as 128.59 m/s and 248.83 m/s (the same points
    /// `region_4_vap_liq_equilibrium/tests.rs` uses). Pass criteria: the state
    /// is Region 4; the getter equals the Kieffer eq. 28 closure
    /// [`w_ps_eqm_region4_kieffer`] evaluated directly, bit for bit (routing);
    /// `|log10(c/c_fig9)| < 0.1` (the existing closure test's tolerance for a
    /// digitised log-scale figure); and `c` is strictly below the frozen
    /// Wood–Wallis speed of `get_speed_of_sound`.
    ///
    /// Results (2026-10-16, `cargo test --release -p tampines-steam-tables
    /// --lib equilibrium_speed_of_sound -- --nocapture`): x = 0.1214 →
    /// **128.46 m/s** (−0.1 % vs Fig. 9's 128.59; frozen Wood–Wallis 473.01
    /// m/s); x = 0.3527 → **247.75 m/s** (−0.4 % vs 248.83; frozen 472.30 m/s).
    /// The equilibrium speed is a quarter to a half of the frozen speed here,
    /// which is the gap a KNP flux or Courant number would get wrong if it used
    /// `get_speed_of_sound`. The figure is digitised from a log-scale plot, so
    /// this is agreement within that reading accuracy (a few percent). The
    /// CoolProp twin (`single_cv_and_flash.rs`) gives 128.46 / 247.74 m/s at
    /// the same states from an independent EOS and closure.
    #[test]
    fn equilibrium_speed_of_sound_is_kieffer_in_the_dome() {
        let p = Pressure::new::<bar>(1.0);
        let t_sat = sat_temp_4(p);
        let h_liq = h_tp_1(t_sat, p);
        let h_vap = h_tp_2(t_sat, p);
        for &(x, c_fig9) in &[
            (0.12139772_f64, 128.59231566_f64),
            (0.35266992, 248.83097247),
        ] {
            let h = x * h_vap + (1.0 - x) * h_liq;
            assert_eq!(ph_flash_region(p, h), FwdEqnRegion::Region4);
            let cv = TampinesSteamTableCV::new_from_ph(p, h, Volume::new::<cubic_meter>(1.0));

            let c_eq = cv.get_equilibrium_speed_of_sound();
            let c_kieffer = w_ps_eqm_region4_kieffer(p, s_ph_eqm(p, h));
            let c_frozen = cv.get_speed_of_sound();
            let c = c_eq.get::<meter_per_second>();
            println!(
                "MEASURE IF97 HEM 1 bar x = {x:.4}: equilibrium {c:.2} m/s vs Kieffer Fig. 9 {c_fig9:.2} m/s ({:+.1} %), frozen {:.2} m/s",
                100.0 * (c / c_fig9 - 1.0),
                c_frozen.get::<meter_per_second>()
            );
            assert_eq!(c_eq, c_kieffer);
            assert!(c > 0.0);
            assert!(
                (c / c_fig9).r_log10().abs() < 0.1,
                "x = {x}: {c} m/s vs {c_fig9} m/s"
            );
            assert!(
                c_eq < c_frozen,
                "equilibrium speed must be below the frozen speed"
            );
        }
    }
}
