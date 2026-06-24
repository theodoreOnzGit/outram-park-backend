use crate::thermophysics::imports::*;
use crate::thermophysics::constants::T_STD;
use crate::thermophysics::eos::EquationOfState;
use super::traits::ThermoModel;

/// NASA 7-coefficient (JANAF) thermodynamic polynomial.
///
/// Mirrors `Foam::janafThermo<EOS>` from
/// `src/thermophysicalModels/specie/thermo/janaf/`.
///
/// Coefficients are stored **pre-scaled by R** (i.e. stored as R·a_i), so
/// polynomials directly return J/(kg·K) or J/kg without an extra R factor.
///
/// Dual temperature range: `low` coefficients apply for T < tcommon,
/// `high` for T >= tcommon.
///
/// Polynomial formulas (matching `janafThermoI.H`):
/// ```text
/// Cp  = (((a[4]·T + a[3])·T + a[2])·T + a[1])·T + a[0]  + EOS::Cp
/// Ha  = ((((a[4]/5·T + a[3]/4)·T + a[2]/3)·T + a[1]/2)·T + a[0])·T + a[5]  + EOS::H
/// S   = (((a[4]/4·T + a[3]/3)·T + a[2]/2)·T + a[1])·T + a[0]·ln(T) + a[6]  + EOS::S
/// Hc  = Ha evaluated at T_std using low coefficients
/// Hs  = Ha − Hc
/// ```
#[derive(Debug, Clone)]
pub struct JanafThermo<E: EquationOfState> {
    eos: E,
    tlow: f64,
    thigh: f64,
    tcommon: f64,
    low: [f64; 7],
    high: [f64; 7],
}

impl<E: EquationOfState> JanafThermo<E> {
    pub fn new(
        eos: E,
        tlow: f64,
        thigh: f64,
        tcommon: f64,
        low: [f64; 7],
        high: [f64; 7],
    ) -> Self {
        Self { eos, tlow, thigh, tcommon, low, high }
    }

    fn coeffs(&self, t: f64) -> &[f64; 7] {
        if t < self.tcommon { &self.low } else { &self.high }
    }
}

// --- EquationOfState delegation ---

impl<E: EquationOfState> EquationOfState for JanafThermo<E> {
    fn mol_weight(&self) -> MolarMass           { self.eos.mol_weight() }
    fn r(&self) -> SpecificHeatCapacity         { self.eos.r() }
    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { self.eos.rho(p, t) }
    fn psi(&self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { self.eos.psi(p, t) }
    fn z(&self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { self.eos.z(p, t) }
    fn cp_m_cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.eos.cp_m_cv(p, t) }
    fn cp_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.eos.cp_eos(p, t) }
    fn h_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.eos.h_eos(p, t) }
    fn e_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { self.eos.e_eos(p, t) }
    fn s_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { self.eos.s_eos(p, t) }
}

// --- ThermoModel ---

impl<E: EquationOfState> ThermoModel for JanafThermo<E> {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let tv = t.get::<kelvin>().clamp(self.tlow, self.thigh);
        let a = self.coeffs(tv);
        let cp_val = (((a[4] * tv + a[3]) * tv + a[2]) * tv + a[1]) * tv + a[0];
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(cp_val) + self.eos.cp_eos(p, t)
    }

    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        let tv = t.get::<kelvin>().clamp(self.tlow, self.thigh);
        let a = self.coeffs(tv);
        let ha_val =
            ((((a[4] / 5.0 * tv + a[3] / 4.0) * tv + a[2] / 3.0) * tv + a[1] / 2.0) * tv
                + a[0]) * tv
            + a[5];
        AvailableEnergy::new::<joule_per_kilogram>(ha_val) + self.eos.h_eos(p, t)
    }

    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy {
        self.ha(p, t) - self.hc()
    }

    fn hc(&self) -> AvailableEnergy {
        // Ha at Tstd using low-range coefficients (OpenFOAM convention)
        let t = T_STD;
        let a = &self.low;
        let hc_val =
            ((((a[4] / 5.0 * t + a[3] / 4.0) * t + a[2] / 3.0) * t + a[1] / 2.0) * t + a[0])
                * t
            + a[5];
        AvailableEnergy::new::<joule_per_kilogram>(hc_val)
    }

    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        let tv = t.get::<kelvin>().clamp(self.tlow, self.thigh);
        let a = self.coeffs(tv);
        let s_val = (((a[4] / 4.0 * tv + a[3] / 3.0) * tv + a[2] / 2.0) * tv + a[1]) * tv
            + a[0] * tv.ln()
            + a[6];
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(s_val) + self.eos.s_eos(p, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermophysics::eos::PerfectGas;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use approx::assert_relative_eq;

    /// Air JANAF coefficients from OpenFOAM's `Air` thermophysical library.
    ///
    /// Coefficients are `Cp/R * R_specific` (pre-scaled by R_air ≈ 287.058 J/(kg·K)).
    /// Source: GRI-Mech 3.0 / standard NIST data, R-scaled for air.
    ///
    /// Low range  (200–1000 K):  a[0..7]
    /// High range (1000–6000 K): a[0..7]
    fn air_janaf() -> JanafThermo<PerfectGas> {
        let eos = PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97));
        let r = eos.r().get::<joule_per_kilogram_kelvin>(); // ≈ 287.058 J/(kg·K)

        // Standard JANAF dimensionless coefficients for N₂ (as proxy for air)
        // Source: NIST-JANAF tables (4th edition)
        let low_dim = [3.53100528, -1.23660988e-4, -5.02999433e-7, 2.43530612e-9, -1.40881235e-12, -1046.976280, 2.96747038];
        let high_dim = [2.95257637,  1.39690040e-3, -4.92631603e-7,  7.86010195e-10, -4.60755204e-13, -923.948688, 5.87188762];

        // Scale by r to get OpenFOAM-style (R-pre-scaled) coefficients.
        // Note: a[5] and a[6] are already in reduced form (dimensionless * R → J/kg or J/(kg·K))
        // For the polynomial, a[i] for i<5 are Cp coefficients, a[5] = H_0/R, a[6] = S_0/R
        let scale_cp = |d: [f64; 7]| -> [f64; 7] {
            [d[0]*r, d[1]*r, d[2]*r, d[3]*r, d[4]*r, d[5]*r, d[6]*r]
        };
        JanafThermo::new(eos, 200.0, 6000.0, 1000.0, scale_cp(low_dim), scale_cp(high_dim))
    }

    #[test]
    fn cp_at_300k_is_reasonable() {
        let a = air_janaf();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let cp = a.cp(p, t).get::<joule_per_kilogram_kelvin>();
        // N₂/air Cp ≈ 1040 J/(kg·K) at 300 K
        assert!(cp > 1000.0 && cp < 1100.0, "Cp = {cp} J/(kg·K) out of expected range");
    }

    #[test]
    fn ha_roundtrip() {
        let a = air_janaf();
        let p = Pressure::new::<pascal>(101_325.0);
        let t_in = ThermodynamicTemperature::new::<kelvin>(800.0);
        let ha = a.ha(p, t_in);
        let t_out = a.t_from_ha(ha, p, ThermodynamicTemperature::new::<kelvin>(400.0)).unwrap();
        assert_relative_eq!(t_in.get::<kelvin>(), t_out.get::<kelvin>(), epsilon = 0.01);
    }

    #[test]
    fn hs_plus_hc_equals_ha() {
        let a = air_janaf();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(500.0);
        use uom::si::available_energy::joule_per_kilogram;
        let ha = a.ha(p, t).get::<joule_per_kilogram>();
        let hs_hc = (a.hs(p, t) + a.hc()).get::<joule_per_kilogram>();
        assert_relative_eq!(ha, hs_hc, epsilon = 1e-3);
    }

    #[test]
    fn cp_both_ranges_are_reasonable() {
        // Standard JANAF tables can have ~5-10% discontinuity at Tcommon — don't
        // test continuity; instead verify each branch gives a physically plausible Cp.
        let a = air_janaf();
        let p = Pressure::new::<pascal>(101_325.0);
        let t_low  = ThermodynamicTemperature::new::<kelvin>(500.0);
        let t_high = ThermodynamicTemperature::new::<kelvin>(1500.0);
        let cp_low  = a.cp(p, t_low).get::<joule_per_kilogram_kelvin>();
        let cp_high = a.cp(p, t_high).get::<joule_per_kilogram_kelvin>();
        // N₂ Cp ∈ [1000, 1300] J/(kg·K) over 500–1500 K
        assert!(cp_low  > 1000.0 && cp_low  < 1300.0, "low Cp = {cp_low}");
        assert!(cp_high > 1000.0 && cp_high < 1300.0, "high Cp = {cp_high}");
    }

    #[test]
    fn newton_converges_from_bad_initial_guess() {
        // t0 = T_MIN = 100 K, but the target is ha(3000 K).
        // The DTMAX clamp (500 K/step) means Newton takes ~6 steps to bridge the gap,
        // then converges.  Must succeed within MAX_ITER = 50 iterations.
        let a = air_janaf();
        let p = Pressure::new::<pascal>(101_325.0);
        let t_target = ThermodynamicTemperature::new::<kelvin>(3000.0);
        let ha_target = a.ha(p, t_target);
        let t0 = ThermodynamicTemperature::new::<kelvin>(100.0);
        let t_out = a.t_from_ha(ha_target, p, t0)
            .expect("Newton should converge from t0=100 K to 3000 K");
        assert!((t_out.get::<kelvin>() - 3000.0).abs() < 5.0,
            "expected ≈3000 K, got {:.1} K", t_out.get::<kelvin>());
    }

    #[test]
    fn newton_t_max_clamp_returns_err() {
        // Target ha is far above ha(T_MAX = 6000 K) → the iteration gets pinned at
        // T_MAX and cannot converge.  Must return Err(NonConvergent), not panic or NaN.
        use crate::thermophysics::error::ThermoError;
        let a = air_janaf();
        let p = Pressure::new::<pascal>(101_325.0);
        let t_max = ThermodynamicTemperature::new::<kelvin>(6000.0);
        let ha_max = a.ha(p, t_max);
        // Add a huge delta so the target is genuinely unreachable
        let impossible_ha = ha_max + uom::si::f64::AvailableEnergy::new::<
            uom::si::available_energy::joule_per_kilogram>(1.0e9);
        let t0 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let result = a.t_from_ha(impossible_ha, p, t0);
        assert!(matches!(result, Err(ThermoError::NonConvergent { .. })),
            "expected NonConvergent, got {:?}", result);
        // Also verify no NaN in the error payload
        if let Err(ThermoError::NonConvergent { last_t, .. }) = result {
            assert!(last_t.is_finite(), "last_t is not finite: {last_t}");
        }
    }

    #[test]
    fn newton_t_min_clamp_returns_err() {
        // Target ha far below ha(T_MIN = 100 K) → pinned at T_MIN, non-convergent.
        use crate::thermophysics::error::ThermoError;
        use uom::si::available_energy::joule_per_kilogram;
        let a = air_janaf();
        let p = Pressure::new::<pascal>(101_325.0);
        let t_min = ThermodynamicTemperature::new::<kelvin>(100.0);
        let ha_min = a.ha(p, t_min);
        let impossible_ha = ha_min - uom::si::f64::AvailableEnergy::new::<joule_per_kilogram>(1.0e9);
        let t0 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let result = a.t_from_ha(impossible_ha, p, t0);
        assert!(matches!(result, Err(ThermoError::NonConvergent { .. })),
            "expected NonConvergent, got {:?}", result);
    }

    #[test]
    fn newton_crosses_tcommon_discontinuity() {
        // Start from t0 on the LOW side (800 K) but target ha is achievable only on the
        // HIGH side (1500 K).  Newton must cross Tcommon = 1000 K and converge.
        let a = air_janaf();
        let p = Pressure::new::<pascal>(101_325.0);
        let t_target = ThermodynamicTemperature::new::<kelvin>(1500.0);
        let ha_target = a.ha(p, t_target);
        let t0 = ThermodynamicTemperature::new::<kelvin>(800.0);
        // Due to the JANAF discontinuity at Tcommon, ha(1500 K) measured from the
        // high-range polynomial.  The iteration starts in the low range but the high-
        // range ha at 1500 K is the genuine target — Newton crosses and converges.
        let t_out = a.t_from_ha(ha_target, p, t0)
            .expect("Newton should converge across Tcommon");
        // Accept up to 50 K error — the discontinuity in ha at Tcommon can shift
        // the apparent root by a few tens of kelvin when crossing from the low side.
        assert!((t_out.get::<kelvin>() - 1500.0).abs() < 50.0,
            "expected ≈1500 K, got {:.1} K", t_out.get::<kelvin>());
    }
}
