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
    /// ~~chooses the U235 group of delayed constants~~ **CHANGED
    /// 2026-11-10 (gh:#1319, maintainer decision: ENDF/B-VIII.0):** chooses
    /// the ENDF/B-VIII.0 U-235 thermal six-group set, `lambda_i` and `beta_i`
    /// both from the evaluation ([`ENDF_B_VIII0_U235`]). The previous
    /// textbook set is still available as
    /// [`FissioningNuclideType::textbook_decay_constant_array`] and
    /// [`FissioningNuclideType::textbook_delayed_fraction_array`].
    U235,
    /// ~~chooses the Pu239 group of delayed constants~~ **CORRECTED
    /// 2026-11-10 (gh:#1319):** chooses Pu-239's delayed fractions `beta_i`
    /// but U-235's decay constants `lambda_i` (see
    /// [`FissioningNuclideType::get_decay_constant_array`]).
    Pu239,
}

impl FissioningNuclideType {
    /// returns a new decay constant array based on nuclide
    /// ~~(for this version, all decay constant arrays are the same)~~
    ///
    /// **CHANGED 2026-11-10 (gh:#1319):** `U235` returns the ENDF/B-VIII.0
    /// set ([`ENDF_B_VIII0_U235`]); before, it returned
    /// [`new_decay_constant_array`] (rounded Keepin U-235 thermal half-lives,
    /// source unidentified).
    ///
    /// **Known defect, gh:#1319 (still open for U-233 and Pu-239).** `U233`
    /// and `Pu239` still return [`new_decay_constant_array`], the old U-235
    /// set, so they pair their own `beta_i` with U-235's `lambda_i`. The
    /// maintainer chose ENDF/B-VIII.0 (2026-11-10), but the ENDF/B-VIII.0
    /// Pu-239 tape is not in `reference-data/endf/` (only JENDL-3.3 Pu-239,
    /// MAT 9437, whose `lambda_i` differ from these by -10.5 % to +10.7 %),
    /// and U-233 is outside that decision (no U-233 tape here either).
    pub fn get_decay_constant_array(&self) -> [DecayConstant; 6] {
        match self {
            FissioningNuclideType::U233 => {
                return new_decay_constant_array();
            }
            FissioningNuclideType::U235 => {
                return ENDF_B_VIII0_U235.decay_constant_array();
            }
            FissioningNuclideType::Pu239 => {
                return new_decay_constant_array();
            }
        }
    }

    /// returns a delayed fraction array based on nuclide
    ///
    /// **CHANGED 2026-11-10 (gh:#1319):** `U235` returns the ENDF/B-VIII.0
    /// thermal `beta_i` ([`ENDF_B_VIII0_U235`], total 0.0065230); before, it
    /// returned [`new_u235_delayed_neutron_fraction_array`] (total 0.0065).
    /// `U233` and `Pu239` are unchanged.
    pub fn get_delayed_fraction_array(&self) -> [Ratio; 6] {
        match self {
            FissioningNuclideType::U233 => {
                return new_u233_delayed_neutron_fraction_array();
            }
            FissioningNuclideType::U235 => {
                return ENDF_B_VIII0_U235.delayed_fraction_array();
            }
            FissioningNuclideType::Pu239 => {
                return new_pu239_delayed_neutron_fraction_array();
            }
        }
    }

    /// The textbook decay constants this crate used for every nuclide before
    /// gh:#1319: [`new_decay_constant_array`] (rounded Keepin U-235 thermal
    /// half-lives, source unidentified). Kept so a pre-2026-11-10 result can
    /// be reproduced, and as the arithmetic fixture of the six-group
    /// bit-identity pin.
    pub fn textbook_decay_constant_array(&self) -> [DecayConstant; 6] {
        new_decay_constant_array()
    }

    /// The textbook delayed fractions this crate used before gh:#1319:
    /// [`new_u233_delayed_neutron_fraction_array`],
    /// [`new_u235_delayed_neutron_fraction_array`] (total 0.0065) or
    /// [`new_pu239_delayed_neutron_fraction_array`].
    pub fn textbook_delayed_fraction_array(&self) -> [Ratio; 6] {
        match self {
            FissioningNuclideType::U233 => new_u233_delayed_neutron_fraction_array(),
            FissioningNuclideType::U235 => new_u235_delayed_neutron_fraction_array(),
            FissioningNuclideType::Pu239 => new_pu239_delayed_neutron_fraction_array(),
        }
    }
}

/// One nuclide's six-group delayed-neutron data, as an evaluation gives it,
/// at one incident energy.
///
/// The group delayed fractions are `beta_i = p_i nu_d(E) / nu(E)`: the
/// group abundance `p_i` (ENDF MF=5/MT=455) times the delayed fraction of
/// all fission neutrons, `nu_d(E)` (MF=1/MT=455) over the total `nu(E)`
/// (MF=1/MT=452). The decay constants `lambda_i` are MF=1/MT=455's
/// energy-independent (`LDG=0`) list. Stored as the tape's numbers, not
/// derived ones, so a test can compare each with the evaluation exactly:
/// `njoy-outram-park-fork`'s `tests/teh_o_prke_delayed_data_vs_endf8.rs`
/// re-reads the tapes and asserts bit equality.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SixGroupDelayedNeutronData {
    /// Precursor decay constants `lambda_i` \[1/s\], in tape order.
    pub decay_constants_per_s: [f64; 6],
    /// Group abundances `p_i` (sum 1 to the tape's rounding).
    pub abundances: [f64; 6],
    /// Delayed neutrons per fission `nu_d(E)` at [`Self::incident_energy_ev`].
    pub nu_delayed: f64,
    /// Total neutrons per fission `nu(E)` at [`Self::incident_energy_ev`].
    pub nu_total: f64,
    /// The incident neutron energy \[eV\] `nu_d` and `nu` are taken at.
    pub incident_energy_ev: f64,
}

impl SixGroupDelayedNeutronData {
    /// `lambda_i` as `uom` decay constants.
    pub fn decay_constant_array(&self) -> [DecayConstant; 6] {
        self.decay_constants_per_s
            .map(|l| Frequency::new::<uom::si::frequency::hertz>(l))
    }

    /// `beta_i = p_i (nu_d / nu)`.
    pub fn delayed_fraction_array(&self) -> [Ratio; 6] {
        let delayed_share = self.nu_delayed / self.nu_total;
        self.abundances
            .map(|p| Ratio::new::<ratio>(p * delayed_share))
    }
}

/// **ENDF/B-VIII.0 U-235**, thermal (0.0253 eV), six groups (gh:#1319).
///
/// Source: ENDF/B-VIII.0 neutron sublibrary, U-235, MAT 9228 (D.A. Brown et
/// al., *Nuclear Data Sheets* 148, 1 (2018)), MF=1/MT=455 (`LDG=0`, `NNF=6`),
/// MF=5/MT=455 (abundances, energy-independent on the tape) and MF=1/MT=452.
/// Read from `reference-data/endf/n-092_U_235-ENDF8.0.endf` on 2026-11-10;
/// pinned to that tape by `njoy-outram-park-fork`'s
/// `tests/teh_o_prke_delayed_data_vs_endf8.rs`. `nu_d` is the tape's lin-lin
/// interpolation at 0.0253 eV between 0.01585 (1e-5 eV) and 0.0167 (50 keV).
///
/// `beta = 0.0065230`; mean precursor life `sum(beta_i/lambda_i)/beta` is
/// about 11.06 s (the old textbook set: 13.05 s).
pub const ENDF_B_VIII0_U235: SixGroupDelayedNeutronData = SixGroupDelayedNeutronData {
    decay_constants_per_s: [0.013336, 0.032739, 0.12078, 0.30278, 0.84949, 2.853],
    abundances: [0.03500755, 0.1806982, 0.1725102, 0.3867821, 0.1585752, 0.0664266],
    nu_delayed: 0.01585000042993,
    nu_total: 2.42985,
    incident_energy_ev: 0.0253,
};

/// **ENDF/B-VIII.0 U-238**, at 2 MeV incident energy, six groups (gh:#1319).
///
/// Source: ENDF/B-VIII.0 neutron sublibrary, U-238, MAT 9237 (Brown et al.,
/// NDS 148 (2018)), the same three sections as [`ENDF_B_VIII0_U235`], read
/// from `reference-data/endf/n-092_U_238.endf` on 2026-11-10 and pinned by
/// the same test.
///
/// **Energy choice.** U-238 has no thermal fission, so a fast energy is
/// needed; 2 MeV is about the mean of the fission spectrum. `nu_d` is flat
/// at 0.044 below 4 MeV on the tape, so the choice moves only `nu(E)`:
/// `beta` is 0.01739 at 1 MeV, 0.01680 here, 0.01518 at 4 MeV. A
/// spectrum-weighted value needs the fission cross section and a spectrum,
/// which is a caller's model, not this table's.
///
/// No [`FissioningNuclideType`] variant: one would need U-238 I-135/Xe-135
/// yields in `fission_product_poisons`. Use the two arrays with
/// `PointKinetics::new`.
pub const ENDF_B_VIII0_U238: SixGroupDelayedNeutronData = SixGroupDelayedNeutronData {
    decay_constants_per_s: [0.01363, 0.031334, 0.12334, 0.32373, 0.90597, 3.0487],
    abundances: [0.01393795, 0.1127969, 0.1310273, 0.3851429, 0.2539916, 0.1031033],
    nu_delayed: 0.044,
    nu_total: 2.61936,
    incident_energy_ev: 2.0e6,
};

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
