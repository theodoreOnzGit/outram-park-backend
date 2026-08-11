use uom::{si::{f64::*, ratio::ratio}, ConstZero};

use crate::tuas_lib_error::TuasLibError;

use super::pipe_correlations::*;
/// contains information Nusselt Prandtl Reynold's
/// correlation
/// usually in the form:
///
/// Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e
///
/// a is the constant 
/// b is the reynolds_prandtl_coefficient
/// c is the reynolds_power,
/// d is the prandtl_power,
/// e is the prandtl_correction_factor_power
#[derive(Clone,Copy,Debug, PartialEq)]
pub struct NusseltPrandtlReynoldsData {

    /// reynolds number input
    pub reynolds: Ratio,

    /// bulk fluid prandtl number
    pub prandtl_bulk: Ratio,

    /// wall prandtl number based on wall tmeperature
    pub prandtl_wall: Ratio,
    /// a in 
    /// Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e
    pub constant: Ratio,
    /// b in 
    /// Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e
    pub reynolds_prandtl_coefficient: Ratio,
    /// c in 
    /// Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e
    pub reynolds_power: f64,
    /// d in 
    /// Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e
    pub prandtl_power: f64,
    /// power for prandtl number correction factor
    /// e in 
    /// Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e
    pub prandtl_correction_factor_power: f64,
}

impl NusseltPrandtlReynoldsData {

    /// obtains nusselt based on:
    /// Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e
    #[inline]
    pub fn custom_reynolds_prandtl(&self) 
    -> Result<Ratio,TuasLibError>{
        let reynolds: Ratio =  self.reynolds;
        let prandtl_bulk: Ratio = self.prandtl_bulk;
        let prandtl_wall: Ratio = self.prandtl_wall;
        let a: Ratio = self.constant;
        let b: Ratio = self.reynolds_prandtl_coefficient;
        let c: f64 = self.reynolds_power;
        let d: f64 = self.prandtl_power;
        let e: f64 = self.prandtl_correction_factor_power;

        let nusselt: Ratio = a + 
        b * reynolds.get::<ratio>().powf(c) 
        * prandtl_bulk.get::<ratio>().powf(d) 
        * (prandtl_bulk/prandtl_wall).get::<ratio>().powf(e);

        return Ok(nusselt);
    }

    /// obtains nusselt based on:
    /// Nu = 0.04179 * reynolds^0.836 * prandtl^0.333
    ///
    /// ignores the coefficients, a,b,c,d,e in the struct
    #[inline]
    pub fn ciet_version_2_heater_uncorrected(&self) -> 
    Result<Ratio, TuasLibError>{

        let reynolds = self.reynolds;
        let prandtl = self.prandtl_bulk;
        let reynolds_power_0_836 = reynolds.value.powf(0.836);
        let prandtl_power_0_333 = prandtl.value.powf(0.333333333333333);

        let nusselt = Ratio::new::<ratio>(
            0.04179 * reynolds_power_0_836 * prandtl_power_0_333);

        Ok(nusselt)
    }


    /// ciet heater correlation for version 2, 
    ///
    /// Nu = 0.04179 * reynolds^0.836 * Pr_bulk^0.333
    /// * (Pr_bulk/Pr_wall)^0.11
    ///
    /// ignores the coefficients, a,b,c,d,e in the struct
    ///
    /// If reynolds number is negative, doesn't matter, just 
    /// take the absolute value of reynolds number
    #[inline]
    pub fn ciet_version_2_heater_prandtl_corrected(&self) -> 
    Result<Ratio, TuasLibError>{
        let nusselt_uncorrected 
        =  {
            let ref this = self;

            let reynolds = this.reynolds.abs();
            let prandtl = this.prandtl_bulk;
            let reynolds_power_0_836 = reynolds.value.powf(0.836);
            let prandtl_power_0_333 = prandtl.value.powf(0.333333333333333);

            let nusselt_uncorrected = Ratio::new::<ratio>(
                0.04179 * reynolds_power_0_836 * prandtl_power_0_333);

            nusselt_uncorrected
        };
        // nusselt number check ok

        let prandtl_wall = self.prandtl_wall;
        let prandtl_bulk = self.prandtl_bulk;

        let prandtl_bulk_to_wall_ratio = prandtl_bulk/prandtl_wall;

        let correction_factor: f64 
        = prandtl_bulk_to_wall_ratio.get::<ratio>().powf(0.11);

        return Ok(nusselt_uncorrected*correction_factor);

    }
}

impl Default for NusseltPrandtlReynoldsData {
    fn default() -> Self {
        NusseltPrandtlReynoldsData{
            reynolds: Ratio::default(),
            prandtl_bulk: Ratio::default(),
            prandtl_wall: Ratio::default(),
            constant: Ratio::default(),
            reynolds_prandtl_coefficient: Ratio::default(),
            reynolds_power: 0.0,
            prandtl_power: 0.0,
            prandtl_correction_factor_power: 0.0,
        }
    }
}


/// Input data for the Wakao particle-to-fluid Nusselt number correlation
/// in a packed bed of spheres.
///
/// Both members are dimensionless (`uom` [`Ratio`]), and both are formed
/// on the **particle (pebble) diameter**.
///
/// # References
///
/// Wakao, N., & Funazkri, T. (1978). Effect
/// of fluid dispersion coefficients on particle-to-fluid mass
/// transfer coefficients in packed beds: correlation of
/// Sherwood numbers. Chemical Engineering Science, 33(10), 1375-1384.
/// (the mass-transfer / Sherwood form)
///
/// Wakao, N., Kaguei, S., & Funazkri, T. (1979). Effect of fluid
/// dispersion coefficients on particle-to-fluid heat transfer
/// coefficients in packed beds: correlation of Nusselt numbers.
/// Chemical Engineering Science, 34(3), 325-336.
/// DOI: 10.1016/0009-2509(79)85064-2
/// (the heat-transfer / Nusselt form, which is what [`WakaoData::get`]
/// evaluates)
#[derive(Clone,Copy,Debug, PartialEq)]
pub struct WakaoData {
    /// Reynolds number, dimensionless.
    ///
    /// Based on the **particle (sphere/pebble) diameter** `d` and the
    /// **superficial** velocity `u` (volumetric flow divided by the
    /// *empty* bed cross-section, not the interstitial velocity):
    ///
    /// Re = rho u d / mu
    ///
    /// Using interstitial rather than superficial velocity inflates Re
    /// by 1/porosity (roughly a factor of 2.5 for a typical randomly
    /// packed bed), so be explicit about which one is supplied.
    pub reynolds: Ratio,
    /// Prandtl number of the fluid, dimensionless.
    ///
    /// Pr = c_p mu / k. Either the bulk-fluid or the film Prandtl number
    /// may be supplied; the correlation carries no wall-correction term,
    /// so only one Prandtl number is used.
    pub prandtl_bulk: Ratio,
}

impl Default for WakaoData {
    fn default() -> Self {
        return Self {
            reynolds: Ratio::ZERO,
            prandtl_bulk: Ratio::ZERO,
        };
    }
}

impl WakaoData {
    /// Returns the particle-to-fluid Nusselt number for a packed bed of
    /// spheres, using the published Wakao correlation:
    ///
    /// Nu = 2 + 1.1 * Pr^(1/3) * Re^0.6
    ///
    /// # Physical quantity
    ///
    /// `Nu = h d / k_f`, the dimensionless particle-to-fluid convective
    /// heat transfer coefficient, where `h` is the heat transfer
    /// coefficient (W m^-2 K^-1), `d` the **particle (pebble) diameter**
    /// (m), and `k_f` the fluid thermal conductivity (W m^-1 K^-1).
    /// The returned value is a `uom` [`Ratio`] (dimensionless).
    ///
    /// Both `Nu` and `Re` are formed on the **particle/pebble diameter**,
    /// and `Re` uses the **superficial** velocity (see
    /// [`WakaoData::reynolds`]). `Pr = c_p mu / k` is dimensionless; either
    /// the bulk or the film Prandtl number may be used, as the correlation
    /// has no wall-correction term.
    ///
    /// The additive `2` is the exact conduction limit for an isolated
    /// sphere in a stagnant infinite medium, so `Nu -> 2` as `Re -> 0`.
    ///
    /// # Valid range
    ///
    /// As published, roughly `15 <= Re <= 8500`, the range over which the
    /// 1979 heat-transfer correlation was regressed against packed-bed
    /// data. Outside that range the expression still evaluates (and still
    /// tends to 2 as `Re -> 0`), but the answer is an extrapolation. This
    /// function does **not** range-check: it never returns `Err` for an
    /// out-of-range `Re`, so the caller is responsible for knowing whether
    /// it is extrapolating. The `Result` return exists only for signature
    /// compatibility with the other correlations in this module.
    ///
    /// # History — corrected 2026-08-11 (bead `op-4542`)
    ///
    /// **Before 2026-08-11 this function had the Reynolds and Prandtl
    /// exponents transposed**, computing
    ///
    /// `Nu = 2 + 1.1 * Re^0.3333 * Pr^0.6`  (WRONG, pre-2026-08-11)
    ///
    /// instead of the published form above. This is not a rounding
    /// difference — at `Re = 1000, Pr = 0.7` the corrected form returns
    /// about 5.85 times the old value. Any result, calibration, or
    /// tuned heat transfer coefficient produced with this function before
    /// that date should be regarded as invalid. Downstream users who
    /// notice a behaviour change here are seeing the fix, not a
    /// regression.
    ///
    /// # References
    ///
    /// Wakao, N., Kaguei, S., & Funazkri, T. (1979). Effect of fluid
    /// dispersion coefficients on particle-to-fluid heat transfer
    /// coefficients in packed beds: correlation of Nusselt numbers.
    /// Chemical Engineering Science, 34(3), 325-336.
    /// DOI: 10.1016/0009-2509(79)85064-2
    ///
    /// Wakao, N., & Funazkri, T. (1978). Effect of fluid dispersion
    /// coefficients on particle-to-fluid mass transfer coefficients in
    /// packed beds: correlation of Sherwood numbers. Chemical
    /// Engineering Science, 33(10), 1375-1384.
    /// (the identically-shaped mass-transfer form)
    ///
    /// Neither primary paper was consulted at page level when this
    /// correction was made; the equation form, its coefficients and its
    /// stated `Re` range are transcribed from the secondary literature
    /// (including Wang, X. (2018) PhD dissertation, ref. [45], catalogued
    /// in this workspace as `wang2018coupled`). A human should confirm
    /// them against the primary source before this correlation is
    /// promoted past Prototype in the V&V pipeline.
    #[inline]
    pub fn get(&self)
    -> Result<Ratio,TuasLibError>{
        let reynolds: Ratio =  self.reynolds;
        let prandtl_bulk: Ratio = self.prandtl_bulk;
        let a: Ratio = Ratio::new::<ratio>(2.0);
        let b: Ratio = Ratio::new::<ratio>(1.1);
        // Prandtl exponent is 1/3, Reynolds exponent is 0.6.
        // Do NOT transpose these -- see the op-4542 note above.
        let prandtl_power: f64 = 1.0/3.0;
        let reynolds_power: f64 = 0.6;

        let nusselt: Ratio = a +
        b * prandtl_bulk.get::<ratio>().powf(prandtl_power)
        * reynolds.get::<ratio>().powf(reynolds_power);

        return Ok(nusselt);
    }
}


/// contains data for gnielinski 
/// correlation of various
#[derive(Clone,Copy,Debug, PartialEq)]
pub struct GnielinskiData {
    /// reynolds number based on hydraulic_diameter
    pub reynolds: Ratio,
    /// bulk fluid prandtl number
    pub prandtl_bulk: Ratio,
    /// wall prandtl number based on wall temperature
    pub prandtl_wall: Ratio,
    /// friction factor, set by user
    pub darcy_friction_factor: Ratio,
    /// pipe length to diameter ratio 
    pub length_to_diameter: Ratio
}

impl Default for GnielinskiData {
    fn default() -> Self {
        Self {
            reynolds: Ratio::ZERO,
            prandtl_bulk: Ratio::ZERO,
            prandtl_wall: Ratio::ZERO,
            darcy_friction_factor: Ratio::ZERO,
            length_to_diameter: Ratio::new::<ratio>(1.0),
        }
    }
}

impl GnielinskiData {


    /// Gnielinski correlation but for developing flows 
    ///
    /// suitable for laminar, turbulent and transition flows
    ///
    /// for this, only bulk prandtl number and wall prandtl 
    /// numbers are used to calculate Nusselt rather than film 
    /// prandtl number
    #[inline]
    pub fn get_nusselt_for_developing_flow_bulk_fluid_prandtl(&self) 
    -> Result<Ratio,TuasLibError>{
        let reynolds: Ratio =  self.reynolds;
        let prandtl_bulk: Ratio = self.prandtl_bulk;
        let prandtl_wall: Ratio = self.prandtl_wall;
        let darcy_friction_factor = self.darcy_friction_factor;
        let length_to_diameter = self.length_to_diameter;

        let nusselt_value = 
        gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing_bulk_fluid_prandtl(
            reynolds.get::<ratio>(),
            prandtl_bulk.get::<ratio>(),
            prandtl_wall.get::<ratio>(),
            darcy_friction_factor.get::<ratio>(),
            length_to_diameter.get::<ratio>(),
        );

        return Ok(
            Ratio::new::<ratio>(nusselt_value)
        );

    }
    /// Gnielinski correlation but for developing flows 
    ///
    /// suitable for laminar, turbulent and transition flows
    ///
    /// for this, film prandtl numbers, bulk prandtl number and wall prandtl 
    /// numbers are used to calculate Nusselt number
    /// prandtl_film: Ratio = (prandtl_wall + prandtl_bulk)/2.0;
    #[inline]
    pub fn get_nusselt_for_developing_flow(&self) 
    -> Result<Ratio,TuasLibError>{
        let reynolds: Ratio =  self.reynolds;
        let prandtl_bulk: Ratio = self.prandtl_bulk;
        let prandtl_wall: Ratio = self.prandtl_wall;
        let prandtl_film: Ratio = (prandtl_wall + prandtl_bulk)/2.0;
        let darcy_friction_factor = self.darcy_friction_factor;
        let length_to_diameter = self.length_to_diameter;

        let nusselt_value = 
        gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing(
            reynolds.get::<ratio>(),
            prandtl_bulk.get::<ratio>(),
            prandtl_film.get::<ratio>(),
            prandtl_wall.get::<ratio>(),
            darcy_friction_factor.get::<ratio>(),
            length_to_diameter.get::<ratio>(),
        );

        return Ok(
            Ratio::new::<ratio>(nusselt_value)
        );

    }


    /// Custom Gnielinski correlation but for developing flows 
    ///
    /// suitable for laminar, turbulent and transition flows
    /// the transition regime is around Re = 2300 - 4000 
    /// this is taken from the Re for transition in pipes 
    /// IT MAY NOT BE APPLICABLE IN THIS CASE
    ///
    /// for the prandtl number of the film, I just took 
    /// Pr_film = 0.5 * (prandtl_number_wall + prandtl_number_bulk_fluid)
    #[inline]
    pub fn get_nusselt_for_custom_developing_flow_prandtl_film(&self,
        correlation_coefficient_c: Ratio,
        reynolds_exponent_m: f64) 
    -> Result<Ratio,TuasLibError>{
        let reynolds: Ratio =  self.reynolds;
        let prandtl_bulk: Ratio = self.prandtl_bulk;
        let prandtl_wall: Ratio = self.prandtl_wall;
        let length_to_diameter = self.length_to_diameter;

        let prandtl_film_estimate = 0.5 * (prandtl_wall + prandtl_bulk);


        let nusselt_value = 
        custom_gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing(
            correlation_coefficient_c,
            reynolds_exponent_m,
            prandtl_film_estimate,
            prandtl_bulk,
            prandtl_wall,
            reynolds,
            length_to_diameter,
        );

        return Ok(
            Ratio::new::<ratio>(nusselt_value)
        );

    }
    /// Custom Gnielinski correlation but for developing flows 
    ///
    /// suitable for laminar, turbulent and transition flows
    /// the transition regime is around Re = 2300 - 4000 
    /// this is taken from the Re for transition in pipes 
    /// IT MAY NOT BE APPLICABLE IN THIS CASE
    ///
    /// the custom gnielinski correlation has an extra prandtl_film_estimate 
    /// argument in it based on Du's original correlation:
    /// Nu = C (Re^m - 280.0) Pr^0.4 ( 1.0 + (D_e/l)^(2/3) ) ( Pr_f / Pr_w )^0.25
    ///
    /// Du did not mention which Pr to use 
    /// I'm going to assume this is Pr_film 
    ///
    /// Nu = C (Re^m - 280.0) Pr_film^0.4 ( 1.0 + (D_e/l)^(2/3) ) ( Pr_f / Pr_w )^0.25
    /// 
    /// Now, since this unidentified prandtl number (just thought to be 
    /// prandtl film) is an extra argument, 
    /// I could assume it is the same as prandtl bulk:
    /// 
    /// Nu = C (Re^m - 280.0) Pr_f^0.4 ( 1.0 + (D_e/l)^(2/3) ) ( Pr_f / Pr_w )^0.25
    ///
    /// This is what this function does
    #[inline]
    pub fn get_nusselt_for_custom_developing_flow_prandtl_bulk(&self,
        correlation_coefficient_c: Ratio,
        reynolds_exponent_m: f64) 
    -> Result<Ratio,TuasLibError>{
        let reynolds: Ratio =  self.reynolds;
        let prandtl_bulk: Ratio = self.prandtl_bulk;
        let prandtl_wall: Ratio = self.prandtl_wall;
        let length_to_diameter = self.length_to_diameter;

        let prandtl_film_estimate = prandtl_bulk;


        let nusselt_value = 
        custom_gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing(
            correlation_coefficient_c,
            reynolds_exponent_m,
            prandtl_film_estimate,
            prandtl_bulk,
            prandtl_wall,
            reynolds,
            length_to_diameter,
        );

        return Ok(
            Ratio::new::<ratio>(nusselt_value)
        );

    }

}

