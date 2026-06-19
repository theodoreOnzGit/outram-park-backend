
use uom::si::ratio::ratio;
use uom::si::f64::*;

/// six factor formula to calculate keff and 
/// reactivity
///
/// keff = P_TNL * P_FNL * eta * f * p * epsilon
///
/// epsilon = fast fission factor 
/// p = resonance escape probability 
/// f  = thermal utilisation factor 
/// eta = fuel reproduction factor 
///
/// P_TNL = probability of thermal non leakage 
/// P_FNL = probability of fast non leakage
#[derive(Debug,Clone,Copy)]
pub struct SixFactorFormulaFeedback{

    /// thermal non leakage probability
    pub p_tnl: Ratio,

    /// fast non leakage probability
    pub p_fnl: Ratio,

    /// fast fission factor 
    pub epsilon: Ratio,

    /// resonance escape probability 
    pub p: Ratio,

    /// thermal utilisation factor
    pub f: Ratio,

    /// fuel reproduction factor 
    pub eta: Ratio,

}

impl SixFactorFormulaFeedback {
    /// calculates the k_eff given the 
    /// stored data of the six factor formula
    #[inline]
    pub fn calc_keff(&self) -> Ratio {
        return 
            self.epsilon 
            * self.p_fnl 
            * self.p 
            * self.p_tnl
            * self.f
            * self.eta;
    }

    /// calculates reactivity given the six factor 
    /// formula 
    /// rho = (keff - 1)/keff
    #[inline]
    pub fn calc_rho(&self) -> Ratio {
        let keff = self.calc_keff();
        // what if keff = 0 tho? 
        // we would just get negative infinity 
        // I'll just give a large negative number
        // arbitrarily large

        if keff.get::<ratio>() <= 0.0 {

            return Ratio::new::<ratio>(-1.0e30);
        }


        return (keff-Ratio::new::<ratio>(1.0))/keff;
    }


    /// fuel temperature feedback, should impact 
    /// resonance escape probability
    /// there should be a set fuel temperature to  
    /// resonance esc probability map or function
    pub fn fuel_temp_feedback(&mut self,
        t: ThermodynamicTemperature,
        resonance_esc_feedback: fn(ThermodynamicTemperature) -> Ratio ) 
        {

            // this is the user defined resonance 
            // escape probability feedback due to fuel 
            // temperature
            let p_chg = resonance_esc_feedback(t);
            self.p *= p_chg.get::<ratio>();

        }
    /// void (average density) feedback
    /// for moderator
    ///
    /// this would affect the thermal utilisation factor
    /// however, resonance escape probability may also be impacted
    /// as more neutrons may exist in the resonance region
    ///
    /// note that temperature feedbacks for moderator are accounted 
    /// for in moderator density feedback by and large
    ///
    pub fn moderator_density_feedback(&mut self,
        rho: MassDensity,
        mod_void_feedback: fn(MassDensity) -> Ratio,
        resonance_esc_feedback: fn(MassDensity) -> Ratio,
        thermal_non_leakage_feedback: fn(MassDensity) -> Ratio,
        fast_non_leakage_feedback: fn(MassDensity) -> Ratio,
        ) 
        {

            // this is the user defined 
            // thermal utilisation 
            // feedback due to 
            // moderator density
            let f_chg = mod_void_feedback(rho);
            self.f *= f_chg.get::<ratio>();
            // this is the user defined resonance 
            // escape probability feedback due to 
            // moderator density
            let p_chg = resonance_esc_feedback(rho);
            self.p *= p_chg.get::<ratio>();
            let p_fnl_chg = fast_non_leakage_feedback(rho);
            let p_tnl_chg = thermal_non_leakage_feedback(rho);
            self.p_tnl *= p_tnl_chg.get::<ratio>();
            self.p_fnl *= p_fnl_chg.get::<ratio>();

        }

    /// void (average density) feedback for reflector 
    ///
    /// this would affect the thermal utilisation factor
    /// however, resonance escape probability may also be impacted
    /// as more neutrons may exist in the resonance region
    ///
    /// moreover, this impacts the non leakage probability in 
    /// both fast and thermal regions
    ///
    /// reflectors can also act as moderators, but the reflector 
    /// is outside the core region
    pub fn reflector_density_feedback(&mut self,
        rho: MassDensity,
        mod_void_feedback: fn(MassDensity) -> Ratio,
        resonance_esc_feedback: fn(MassDensity) -> Ratio,
        thermal_non_leakage_feedback: fn(MassDensity) -> Ratio,
        fast_non_leakage_feedback: fn(MassDensity) -> Ratio,
        ) 
        {

            // this is the user defined 
            // thermal utilisation 
            // feedback due to 
            // moderator density
            let f_chg = mod_void_feedback(rho);
            self.f *= f_chg.get::<ratio>();
            // this is the user defined resonance 
            // escape probability feedback due to 
            // moderator density
            let p_chg = resonance_esc_feedback(rho);
            self.p *= p_chg.get::<ratio>();
            let p_fnl_chg = fast_non_leakage_feedback(rho);
            let p_tnl_chg = thermal_non_leakage_feedback(rho);
            self.p_tnl *= p_tnl_chg.get::<ratio>();
            self.p_fnl *= p_fnl_chg.get::<ratio>();

        }

    /// control rod feedback
    ///
    /// reflects feedback due to degree of control rod insertion 
    /// this is a ratio of 0 to 1 usually.
    ///
    /// this affects thermal utilisation factor usually
    pub fn control_rod_feedback(&mut self,
        rod_insertion_ratio: Ratio,
        ctrl_rod_feedback: fn(Ratio) -> Ratio,
        ){

        let f_chg = ctrl_rod_feedback(rod_insertion_ratio);
        self.f *= f_chg.get::<ratio>();

    }

    /// generic leakage feedback
    /// due to core expansion or some other factor
    pub fn leakage_feedback(&mut self,
        rho: MassDensity,
        thermal_non_leakage_feedback: fn(MassDensity) -> Ratio,
        fast_non_leakage_feedback: fn(MassDensity) -> Ratio,
        ) 
        {

            let p_fnl_chg = fast_non_leakage_feedback(rho);
            let p_tnl_chg = thermal_non_leakage_feedback(rho);
            self.p_tnl *= p_tnl_chg.get::<ratio>();
            self.p_fnl *= p_fnl_chg.get::<ratio>();

        }

    /// reactor poison feedback
    ///
    /// reflects feedback due to 
    /// reactor poison concentration 
    ///
    /// this affects thermal utilisation factor usually
    ///
    /// it is up to the user to decide to use this for 
    /// Xenon, Samarium or some other poison
    pub fn reactor_poison_feedback(&mut self,
        reactor_poison_concentration: MassConcentration,
        reactor_poison_conc_feedback: fn(MassConcentration) -> Ratio,
        ){

        let f_chg = reactor_poison_conc_feedback(reactor_poison_concentration);
        self.f *= f_chg.get::<ratio>();

    }
    /// burnable absorber/poison feedback
    ///
    /// reflects feedback due to 
    /// burnable absorber/poison concentration
    ///
    /// this affects thermal utilisation factor usually
    ///
    /// it is up to the user to decide to use this for 
    /// Xenon, Samarium or some other poison
    pub fn burnable_absorber_posion_feedback(&mut self,
        burnable_poison_concentration: MassConcentration,
        poison_conc_feedback: fn(MassConcentration) -> Ratio,
        ){

        let f_chg = poison_conc_feedback(burnable_poison_concentration);
        self.f *= f_chg.get::<ratio>();

    }


    /// fuel depletion and fuel breeding 
    pub fn fuel_depletion_and_breeding_feedback(&mut self,
        fuel_concentration: MassConcentration, 
        eta_feedback: fn(MassConcentration) -> Ratio,
        fast_fission_factor_feedback: fn(MassConcentration) -> Ratio,
        resonance_esc_feedback: fn(MassConcentration) -> Ratio,
        thermal_utilisation_feedback: fn(MassConcentration) -> Ratio,)
        {

            let f_chg = thermal_utilisation_feedback(fuel_concentration);
            self.f *= f_chg.get::<ratio>();

            let eta_chg = eta_feedback(fuel_concentration);
            self.eta *= eta_chg.get::<ratio>();

            let epsilon_chg = fast_fission_factor_feedback(fuel_concentration);
            self.epsilon *= epsilon_chg.get::<ratio>();

            let p_chg = resonance_esc_feedback(fuel_concentration);
            self.p *= p_chg.get::<ratio>();
        }

    /// burnup feedback 
    ///
    /// similar to fuel depletion, but we use 
    /// burnup units for convenience
    ///
    /// as in MWd/ton heavy metal 
    ///
    /// this is energy per unit mass
    ///
    pub fn fuel_burnup_feedback(&mut self,
        burnup: AvailableEnergy, 
        eta_feedback: fn(AvailableEnergy) -> Ratio,
        fast_fission_factor_feedback: fn(AvailableEnergy) -> Ratio,
        resonance_esc_feedback: fn(AvailableEnergy) -> Ratio,
        thermal_utilisation_feedback: fn(AvailableEnergy) -> Ratio,)
        {

            let f_chg = thermal_utilisation_feedback(burnup);
            self.f *= f_chg.get::<ratio>();

            let eta_chg = eta_feedback(burnup);
            self.eta *= eta_chg.get::<ratio>();

            let epsilon_chg = fast_fission_factor_feedback(burnup);
            self.epsilon *= epsilon_chg.get::<ratio>();

            let p_chg = resonance_esc_feedback(burnup);
            self.p *= p_chg.get::<ratio>();
        }

}

impl Default for SixFactorFormulaFeedback {
    fn default() -> Self {
        let ratio_one = Ratio::new::<ratio>(1.0);
        Self { 
            p_tnl: ratio_one, 
            p_fnl: ratio_one, 
            epsilon: ratio_one, 
            p: ratio_one, 
            f: ratio_one, 
            eta: ratio_one 
        }
    }
}

/// fission product poisoning 
/// includes but not limited to xenon-iodine 135 poisoning
/// 
pub mod fission_product_poisons;

