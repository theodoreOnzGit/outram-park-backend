use std::f64::consts::LN_2;
use uom::si::time::{day, hour, second, year};
use uom::si::{f64::*, ratio::ratio};
use uom::ConstZero;
/// this struct helps to manage decay heat calculations
///
/// similar to how delayed neutron precursors are handled,
/// I have some groups for decay heat precursors. 
/// These are loosely coupled (semi-implicit)
///
///
/// i think this is slightly buggy, need to change code
///
/// the precursors are energy units, not power...
#[derive(Clone, Debug, Copy)]
pub struct DecayHeat {
    decay_heat_precursor1: Energy,
    pub decay_heat_precursor1_half_life: Time,
    decay_heat_precursor2: Energy,
    pub decay_heat_precursor2_half_life: Time,
    decay_heat_precursor3: Energy,
    pub decay_heat_precursor3_half_life: Time,
    decay_heat_precursor4: Energy,
    pub decay_heat_precursor4_half_life: Time,
    decay_heat_precursor5: Energy,
    pub decay_heat_precursor5_half_life: Time,
    decay_heat_precursor6: Energy,
    pub decay_heat_precursor6_half_life: Time,
    decay_heat_precursor7: Energy,
    pub decay_heat_precursor7_half_life: Time,
}

impl DecayHeat {
    pub fn add_decay_heat_precursor1(&mut self, 
        decay_precursor_power: Power,
        timestep: Time){

        self.decay_heat_precursor1 += decay_precursor_power * timestep;
    }
    pub fn add_decay_heat_precursor2(&mut self, 
        decay_precursor_power: Power,
        timestep: Time){

        self.decay_heat_precursor2 += decay_precursor_power * timestep;
    }
    pub fn add_decay_heat_precursor3(&mut self, 
        decay_precursor_power: Power,
        timestep: Time){

        self.decay_heat_precursor3 += decay_precursor_power * timestep;
    }
    pub fn add_decay_heat_precursor4(&mut self, 
        decay_precursor_power: Power,
        timestep: Time){

        self.decay_heat_precursor4 += decay_precursor_power * timestep;
    }
    pub fn add_decay_heat_precursor5(&mut self, 
        decay_precursor_power: Power,
        timestep: Time){

        self.decay_heat_precursor5 += decay_precursor_power * timestep;
    }
    pub fn add_decay_heat_precursor6(&mut self, 
        decay_precursor_power: Power,
        timestep: Time){

        self.decay_heat_precursor6 += decay_precursor_power * timestep;
    }
    pub fn add_decay_heat_precursor7(&mut self, 
        decay_precursor_power: Power,
        timestep: Time){

        self.decay_heat_precursor7 += decay_precursor_power * timestep;
    }


    /// basically 
    ///
    /// (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
    pub fn calc_decay_heat_power_1(&mut self,
        timestep: Time) -> Power {

        let decay_constant: Frequency = 
            LN_2/self.decay_heat_precursor1_half_life;
        // (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
        //
        // E_decay^(t + Delta t) (1 + lambda_i * Delta t) = E_decay^t
        //

        let e_decay_t = self.decay_heat_precursor1;
        let coeff = Ratio::new::<ratio>(1.0) + decay_constant * timestep;

        let e_decay_t_plus_delta_t = e_decay_t / coeff;

        self.decay_heat_precursor1 = e_decay_t_plus_delta_t;
        
        return (e_decay_t_plus_delta_t - e_decay_t)/timestep;

        

    }
    /// basically 
    ///
    /// (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
    pub fn calc_decay_heat_power_2(&mut self,
        timestep: Time) -> Power {

        let decay_constant: Frequency = 
            LN_2/self.decay_heat_precursor2_half_life;
        // (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
        //
        // E_decay^(t + Delta t) (1 + lambda_i * Delta t) = E_decay^t
        //

        let e_decay_t = self.decay_heat_precursor2;
        let coeff = Ratio::new::<ratio>(1.0) + decay_constant * timestep;

        let e_decay_t_plus_delta_t = e_decay_t / coeff;

        self.decay_heat_precursor2 = e_decay_t_plus_delta_t;

        return (e_decay_t_plus_delta_t - e_decay_t)/timestep;

        

    }
    /// basically 
    ///
    /// (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
    pub fn calc_decay_heat_power_3(&mut self,
        timestep: Time) -> Power {

        let decay_constant: Frequency = 
            LN_2/self.decay_heat_precursor3_half_life;
        // (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
        //
        // E_decay^(t + Delta t) (1 + lambda_i * Delta t) = E_decay^t
        //

        let e_decay_t = self.decay_heat_precursor3;
        let coeff = Ratio::new::<ratio>(1.0) + decay_constant * timestep;

        let e_decay_t_plus_delta_t = e_decay_t / coeff;

        self.decay_heat_precursor3 = e_decay_t_plus_delta_t;

        return (e_decay_t_plus_delta_t - e_decay_t)/timestep;

        

    }
    /// basically 
    ///
    /// (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
    pub fn calc_decay_heat_power_4(&mut self,
        timestep: Time) -> Power {

        let decay_constant: Frequency = 
            LN_2/self.decay_heat_precursor4_half_life;
        // (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
        //
        // E_decay^(t + Delta t) (1 + lambda_i * Delta t) = E_decay^t
        //

        let e_decay_t = self.decay_heat_precursor4;
        let coeff = Ratio::new::<ratio>(1.0) + decay_constant * timestep;

        let e_decay_t_plus_delta_t = e_decay_t / coeff;

        self.decay_heat_precursor4 = e_decay_t_plus_delta_t;

        return (e_decay_t_plus_delta_t - e_decay_t)/timestep;

        

    }
    /// basically 
    ///
    /// (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
    pub fn calc_decay_heat_power_5(&mut self,
        timestep: Time) -> Power {

        let decay_constant: Frequency = 
            LN_2/self.decay_heat_precursor5_half_life;
        // (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
        //
        // E_decay^(t + Delta t) (1 + lambda_i * Delta t) = E_decay^t
        //

        let e_decay_t = self.decay_heat_precursor5;
        let coeff = Ratio::new::<ratio>(1.0) + decay_constant * timestep;

        let e_decay_t_plus_delta_t = e_decay_t / coeff;

        self.decay_heat_precursor5 = e_decay_t_plus_delta_t;

        return (e_decay_t_plus_delta_t - e_decay_t)/timestep;

        

    }
    /// basically 
    ///
    /// (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
    pub fn calc_decay_heat_power_6(&mut self,
        timestep: Time) -> Power {

        let decay_constant: Frequency = 
            LN_2/self.decay_heat_precursor6_half_life;
        // (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
        //
        // E_decay^(t + Delta t) (1 + lambda_i * Delta t) = E_decay^t
        //

        let e_decay_t = self.decay_heat_precursor6;
        let coeff = Ratio::new::<ratio>(1.0) + decay_constant * timestep;

        let e_decay_t_plus_delta_t = e_decay_t / coeff;

        self.decay_heat_precursor6 = e_decay_t_plus_delta_t;

        return (e_decay_t_plus_delta_t - e_decay_t)/timestep;

        

    }
    /// basically 
    ///
    /// (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
    pub fn calc_decay_heat_power_7(&mut self,
        timestep: Time) -> Power {

        let decay_constant: Frequency = 
            LN_2/self.decay_heat_precursor7_half_life;
        // (E_decay^(t + Delta t) - E_decay^t)/ Delta t = - lambda_i E_decay^(t + Delta t)
        //
        // E_decay^(t + Delta t) (1 + lambda_i * Delta t) = E_decay^t
        //

        let e_decay_t = self.decay_heat_precursor7;
        let coeff = Ratio::new::<ratio>(1.0) + decay_constant * timestep;

        let e_decay_t_plus_delta_t = e_decay_t / coeff;

        self.decay_heat_precursor7 = e_decay_t_plus_delta_t;

        return (e_decay_t_plus_delta_t - e_decay_t)/timestep;

        

    }

}

/// default is half lives of 
///
/// 0.2 hrs
/// 8 hrs 
/// 30 days 
///
/// then fission power ratio is:
/// 30 days -> 2%
/// 8 hrs -> 4% 
/// 0.2 hrs -> 4%
impl Default for DecayHeat {
    fn default() -> Self {

        Self { 
            decay_heat_precursor1: Energy::ZERO, 
            decay_heat_precursor1_half_life: Time::new::<hour>(0.2), 
            decay_heat_precursor2: Energy::ZERO, 
            decay_heat_precursor2_half_life: Time::new::<hour>(8.0), 
            decay_heat_precursor3: Energy::ZERO, 
            decay_heat_precursor3_half_life: Time::new::<day>(30.0),
            decay_heat_precursor4: Energy::ZERO, 
            decay_heat_precursor4_half_life: Time::new::<year>(1.0),
            decay_heat_precursor5: Energy::ZERO, 
            decay_heat_precursor5_half_life: Time::new::<year>(30.0),
            decay_heat_precursor6: Energy::ZERO, 
            decay_heat_precursor6_half_life: Time::new::<year>(1000.0),
            decay_heat_precursor7: Energy::ZERO, 
            decay_heat_precursor7_half_life: Time::new::<second>(1.0),
        }
    }
}


