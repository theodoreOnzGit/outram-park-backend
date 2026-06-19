use std::f64::consts::PI;

use uom::si::f64::*;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::centimeter;
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use crate::teh_o_prke_error::TehOPrkeError;

/// a struct for calculating fuel temperature feedback
/// using a rather simple heat balance equations
///
/// m c_p (dT_fuel/dt) = -hA(T_fuel-T_surr) + fission_power_source
///
/// uses explicit time stepping for simplicity
///
pub struct SimpleFuelTemperatureFeedback {
    /// rho V c_p = m c_p
    ///
    /// let's do c_p
    pub fuel_specific_heat_capacity: SpecificHeatCapacity,

    /// rho 
    pub fuel_density: MassDensity,

    /// volume 
    pub fuel_volume: Volume,

    /// T_fuel 
    pub fuel_temperature: ThermodynamicTemperature,
    /// convection heat transfer coefficient
    pub convection_heat_trf_coeff: HeatTransfer,
    /// convection heat transfer area 
    pub convection_heat_trf_area: Area,

    /// fuel temperature feedback coefficient
    /// can be expressed as alpha = -alpha_coefficient/sqrt(T(kelvin))
    ///
    /// typically around 10^(-4) dimensionless
    pub alpha_coefficient: Ratio,

}

impl SimpleFuelTemperatureFeedback {

    /// set initial fuel temperature 
    pub fn set_fuel_temperature(&mut self,
        temperature: ThermodynamicTemperature,)
        -> Result<(), TehOPrkeError>{
            self.fuel_temperature = temperature;

            Ok(())
    }

    /// get current fuel temperature
    pub fn get_fuel_temperature(&self)
        -> Result<ThermodynamicTemperature, TehOPrkeError>{
            Ok(self.fuel_temperature)
    }

    /// set fuel alpha_coefficient 
    /// that is reactivity feedback coefficient
    /// for thermal spectrum reactor
    ///
    /// it is typically around 10^(-4) dimensionless
    pub fn set_fuel_alpha_coefficient(&mut self,
        alpha_coefficient: Ratio,)
        -> Result<(), TehOPrkeError>{
            self.alpha_coefficient = alpha_coefficient;

            Ok(())
    }
    
    /// add fission heat 
    ///
    ///
    /// m c_p (T_new - T_old)/Delta t = fission power source 
    ///
    /// T_new = (fission power source) * (Delta t) / (m c_p) + T_old
    pub fn add_fission_heat(&mut self,
        fission_power: Power,
        timestep: Time) -> Result<(),TehOPrkeError>{

        let new_temperature = (fission_power * timestep) / 
            (self.fuel_volume * self.fuel_density * self.fuel_specific_heat_capacity)
            + self.fuel_temperature;

        self.fuel_temperature = new_temperature;
        Ok(())
    }

    /// remove heat due to convection
    /// m c_p (dT_fuel/dt) = -hA(T_fuel-T_surr) + fission_power_source
    ///
    /// implicit time stepping used
    ///
    /// m c_p (T_new - T_old)/Delta t = -hA(T_new-T_surr)
    /// T_new - T_old = -hA*(delta t)/(m c_p)*(T_new-T_surr)
    /// T_new - T_old + hA*(delta t)/(m c_p) T_new = hA*(delta t)/(m c_p)(T_surr)
    /// T_new  + hA*(delta t)/(m c_p) T_new = hA*(delta t)/(m c_p)(T_surr) + T_old
    /// T_new  ( 1+ hA*(delta t)/(m c_p) ) = hA*(delta t)/(m c_p)(T_surr) + T_old
    /// T_new  = 1/( 1+ hA*(delta t)/(m c_p) ) * [ hA*(delta t)/(m c_p)(T_surr) + T_old]
    /// 
    ///
    pub fn remove_convection_heat(&mut self,
        coolant_temperature: ThermodynamicTemperature,
        timestep: Time,) -> Result<(),TehOPrkeError>{

        let m_cp = self.fuel_volume * self.fuel_density * self.fuel_specific_heat_capacity;

        let h_a_delta_t_by_mcp: Ratio = 
            self.convection_heat_trf_coeff * 
            self.convection_heat_trf_area * 
            timestep / 
            m_cp;

        let numerator: ThermodynamicTemperature = 
            h_a_delta_t_by_mcp * coolant_temperature + self.fuel_temperature;

        let denominator: Ratio = Ratio::new::<ratio>(1.0) + h_a_delta_t_by_mcp;

        let new_temperature: ThermodynamicTemperature = 
            numerator/(denominator.get::<ratio>());


        self.fuel_temperature = new_temperature;

        Ok(())
    }

    /// obtain reactivity change compared to reference temperature 
    /// usually 300K
    pub fn obtain_fuel_temperature_delta_rho(&self,
        reference_temperature: ThermodynamicTemperature,) -> Result<Ratio, TehOPrkeError>{

        let alpha_coefficient = self.alpha_coefficient;
        let temperature = self.fuel_temperature;

        obtain_fuel_temperature_reactivity_feedback_thermal_spectrum(
            alpha_coefficient, temperature, reference_temperature)

    }
}

impl Default for SimpleFuelTemperatureFeedback {
    fn default() -> Self {
        // just use water
        let fuel_specific_heat_capacity: SpecificHeatCapacity = 
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(4.184);

        // use uranium oxide ish 
        let fuel_density: MassDensity = 
            MassDensity::new::<gram_per_cubic_centimeter>(10.85);

        // a cylinder 83 cm high and 30 cm diameter 
        // (arbitrary)

        let d = Length::new::<centimeter>(30.0);
        let l = Length::new::<centimeter>(83.0);
        let fuel_volume: Volume = PI / 4.0 * d * d * l;

        // fuel temperature is default 300K, you'll need to set it initially 
        // otherwise

        let fuel_temperature:ThermodynamicTemperature 
            = ThermodynamicTemperature::new::<kelvin>(300.0);

        // convection heat transfer coeff is 20 (W/m^2 K)
        let convection_heat_trf_coeff: HeatTransfer = 
             HeatTransfer::new::<watt_per_square_meter_kelvin>(20.0);

        // based on
        // a cylinder 83 cm high and 30 cm diameter 
        //
        // 2* PI/4.0 D^2
        let cylinder_circular_face_areas: Area = 2.0 *
            PI / 4.0 * d * d;


        let cylinder_curved_area: Area = PI * d * l;

        let convection_heat_trf_area = cylinder_curved_area +
            cylinder_circular_face_areas;

        // arbitrary
        let alpha_coefficient: Ratio = Ratio::new::<ratio>(5.0e-4);


        Self { 
            fuel_specific_heat_capacity,
            fuel_density, 
            fuel_volume, 
            fuel_temperature, 
            convection_heat_trf_coeff, 
            convection_heat_trf_area, 
            alpha_coefficient,
        }
    }
}


#[inline]
/// for thermal spectrum reactors,
/// the alpha  = d(rho)/dT 
///
/// can be expressed as alpha = -alpha_coefficient/sqrt(T(kelvin))
/// (see lamarsh)
///
/// alpha_coefficient is some value, usually on the order of 1*10^(-4)
/// 
pub fn obtain_fuel_temperature_feedback_coeff_thermal_spectrum(
    alpha_coefficient: Ratio,
    temperature: ThermodynamicTemperature) -> Result<Ratio, TehOPrkeError> {

    let alpha = -alpha_coefficient/
        (temperature.get::<kelvin>().sqrt());

    Ok(alpha)

}


#[inline]
/// for thermal spectrum reactors,
/// we can calculate reactivity based on alpha
///
/// can be expressed as d(rho)/dT = -alpha_coefficient/sqrt(T(kelvin))
///
/// alpha_coefficient is some value, usually on the order of 1*10^(-4)
///
/// now, if you want to have a reactivity with respect to some temperature,
/// we can analytically integrate:
///
/// rho - rho_ref = -alpha_coefficient * 2.0 (sqrt(T) - sqrt(T_ref))
///
/// we can define rho_ref as reactivity at some temperature T_ref
///
/// The function will return (rho - rho_ref), or delta_rho
/// 
///
pub fn obtain_fuel_temperature_reactivity_feedback_thermal_spectrum(
    alpha_coefficient: Ratio,
    temperature: ThermodynamicTemperature,
    reference_temperature: ThermodynamicTemperature) -> Result<Ratio, TehOPrkeError> {

    let delta_rho = -2.0 * alpha_coefficient *
        (temperature.get::<kelvin>().sqrt()
         - reference_temperature.get::<kelvin>().sqrt());

    Ok(delta_rho)

}


#[test]
pub fn fuel_temperature_feedback_test(){

    use approx::assert_abs_diff_eq;

    let mut fuel_temperature_feedback_struct = 
        SimpleFuelTemperatureFeedback::default();


    // temperature should be 300K

    let fuel_temperature_initial = ThermodynamicTemperature::new::<kelvin>(300.0);

    fuel_temperature_feedback_struct
        .set_fuel_temperature(fuel_temperature_initial).unwrap();

    let fuel_temperature_test = 
        fuel_temperature_feedback_struct.get_fuel_temperature().unwrap();


    assert_abs_diff_eq!(
        fuel_temperature_test.get::<kelvin>(),
        300.0,
        epsilon = 1e-11);

    // at 300K reference, we should get no reactivity feedback 

    let reactivity_at_reference: Ratio = 
        fuel_temperature_feedback_struct.obtain_fuel_temperature_delta_rho(
            ThermodynamicTemperature::new::<kelvin>(300.0)
            ).unwrap();

    assert_abs_diff_eq!(
        reactivity_at_reference.get::<ratio>(),
        0.0,
        epsilon = 1e-11);

    // now raise temperature to 600K
    let fuel_temperature_test_1 = ThermodynamicTemperature::new::<kelvin>(600.0);
    fuel_temperature_feedback_struct
        .set_fuel_temperature(fuel_temperature_test_1).unwrap();

    let reactivity_test_1: Ratio = 
        fuel_temperature_feedback_struct.obtain_fuel_temperature_delta_rho(
            ThermodynamicTemperature::new::<kelvin>(300.0)
            ).unwrap();

    // should be -0.007174389
    assert_abs_diff_eq!(
        reactivity_test_1.get::<ratio>(),
        -0.007174389,
        epsilon = 1e-5);

    // at 900K should have more negative feedback
    let fuel_temperature_test_2 = ThermodynamicTemperature::new::<kelvin>(900.0);
    fuel_temperature_feedback_struct
        .set_fuel_temperature(fuel_temperature_test_2).unwrap();

    let reactivity_test_2: Ratio = 
        fuel_temperature_feedback_struct.obtain_fuel_temperature_delta_rho(
            ThermodynamicTemperature::new::<kelvin>(300.0)
            ).unwrap();

    // should be -0.01267949
    assert_abs_diff_eq!(
        reactivity_test_2.get::<ratio>(),
        -0.01267949,
        epsilon = 1e-5);
}
