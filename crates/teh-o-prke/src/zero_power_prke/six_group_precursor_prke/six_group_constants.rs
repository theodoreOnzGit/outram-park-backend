use std::f64::consts::LN_2;

use uom::ConstZero;
use uom::si::f64::*;
use uom::si::time::second;
use uom::si::ratio::ratio;

use crate::zero_power_prke::six_group_precursor_prke::DecayConstant;

/// different nuclides or fuels have different delayed groups
#[derive(Debug, Clone, Copy)]
pub enum FissioningNuclideType {
    /// ~~chooses the U233 group of delayed constants~~ **CORRECTED
    /// 2026-11-10 (gh:#1319):** chooses U-233's delayed fractions `beta_i`
    /// but U-235's decay constants `lambda_i` (see
    /// [`FissioningNuclideType::get_decay_constant_array`]).
    U233,
    /// chooses the U235 group of delayed constants
    U235,
    /// ~~chooses the Pu239 group of delayed constants~~ **CORRECTED
    /// 2026-11-10 (gh:#1319):** chooses Pu-239's delayed fractions `beta_i`
    /// but U-235's decay constants `lambda_i` (see
    /// [`FissioningNuclideType::get_decay_constant_array`]).
    Pu239,
}

impl FissioningNuclideType {
    /// returns a new decay constant array based on nuclide
    /// (for this version, all decay constant arrays are the same)
    ///
    /// **Known defect, gh:#1319 (open, 2026-11-10).** All three nuclides
    /// return [`new_decay_constant_array`], which is U-235 thermal data, so a
    /// U-233 or Pu-239 PRKE pairs its own `beta_i` with U-235's `lambda_i`.
    /// Not yet fixed because no per-nuclide six-group source for U-233 is in
    /// the workspace (`reference-data/endf/` has no U-233 tape; the only
    /// independent six-group Pu-239 set is JENDL-3.3, MAT 9437 MF=1 MT=455,
    /// whose `lambda_i` differ from these by -10.5 % to +10.7 %). The source
    /// choice is asked on the issue.
    pub fn get_decay_constant_array(&self) -> [DecayConstant; 6] {
        match self {
            FissioningNuclideType::U233 => {
                return new_decay_constant_array();
            }
            FissioningNuclideType::U235 => {
                return new_decay_constant_array();
            }
            FissioningNuclideType::Pu239 => {
                return new_decay_constant_array();
            }
        }
    }

    /// returns a delayed fraction array based on nuclide
    pub fn get_delayed_fraction_array(&self) -> [Ratio; 6] {
        match self {
            FissioningNuclideType::U233 => {
                return new_u233_delayed_neutron_fraction_array();
            }
            FissioningNuclideType::U235 => {
                return new_u235_delayed_neutron_fraction_array();
            }
            FissioningNuclideType::Pu239 => {
                return new_pu239_delayed_neutron_fraction_array();
            }
        }
    }
}

/// produces a new decay constant array
pub fn new_decay_constant_array() -> [DecayConstant; 6] {
    // from table 5.1 (not sure where from, duderstadt?)
    // need to double check
    //
    // Checked 2026-11-10 (gh:#1319), source still NOT identified. These are
    // Keepin's U-235 thermal half-lives rounded to two significant figures
    // (Keepin 1965, *Physics of Nuclear Kinetics*, Table 4-7: 55.72, 22.72,
    // 6.22, 2.30, 0.610, 0.230 s, as cited in
    // `nee_soon/src/htr10_th_transient/kinetics.rs`; not re-checked against
    // Keepin directly). DOE-HDBK-1019/1-93 Table 3 lists 55.7, 22.7, 6.2,
    // 2.3, 0.61, 0.23 s, so it is not the source of the 56 and 23 s values.
    let mut decay_constant_array: [DecayConstant; 6] = [
        Frequency::ZERO,
        Frequency::ZERO,
        Frequency::ZERO,
        Frequency::ZERO,
        Frequency::ZERO,
        Frequency::ZERO,
    ];

    let half_life_array: [Time; 6] = [
        Time::new::<second>(56.0),
        Time::new::<second>(23.0),
        Time::new::<second>(6.2),
        Time::new::<second>(2.3),
        Time::new::<second>(0.61),
        Time::new::<second>(0.23),
    ];

    // crude, but gets the job done
    //
    // lambda (decay constant) = ln(2)/(half life)
    decay_constant_array[0] = LN_2 / half_life_array[0];
    decay_constant_array[1] = LN_2 / half_life_array[1];
    decay_constant_array[2] = LN_2 / half_life_array[2];
    decay_constant_array[3] = LN_2 / half_life_array[3];
    decay_constant_array[4] = LN_2 / half_life_array[4];
    decay_constant_array[5] = LN_2 / half_life_array[5];

    // done!
    decay_constant_array
}

/// produces a new delayed fraction for u233
pub fn new_u233_delayed_neutron_fraction_array() -> [Ratio; 6] {
    let delayed_neutron_array: [Ratio; 6] = [
        Ratio::new::<ratio>(0.00023),
        Ratio::new::<ratio>(0.00078),
        Ratio::new::<ratio>(0.00064),
        Ratio::new::<ratio>(0.00074),
        Ratio::new::<ratio>(0.00014),
        Ratio::new::<ratio>(0.00008),
    ];

    delayed_neutron_array
}

/// produces a new delayed fraction for u235
pub fn new_u235_delayed_neutron_fraction_array() -> [Ratio; 6] {
    let delayed_neutron_array: [Ratio; 6] = [
        Ratio::new::<ratio>(0.00021),
        Ratio::new::<ratio>(0.00142),
        Ratio::new::<ratio>(0.00128),
        Ratio::new::<ratio>(0.00257),
        Ratio::new::<ratio>(0.00075),
        Ratio::new::<ratio>(0.00027),
    ];

    delayed_neutron_array
}

/// produces a new delayed fraction for pu239
pub fn new_pu239_delayed_neutron_fraction_array() -> [Ratio; 6] {
    let delayed_neutron_array: [Ratio; 6] = [
        Ratio::new::<ratio>(0.00007),
        Ratio::new::<ratio>(0.00063),
        Ratio::new::<ratio>(0.00044),
        Ratio::new::<ratio>(0.00069),
        Ratio::new::<ratio>(0.00018),
        Ratio::new::<ratio>(0.00009),
    ];

    delayed_neutron_array
}
