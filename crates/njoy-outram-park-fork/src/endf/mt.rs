//! ENDF MT reaction number type.
//!
//! [`MtReaction`] is an enum of all standard ENDF-6 MT (section/reaction)
//! numbers, with a human-readable name attached to each. Using it instead of a
//! bare `i32` gives compile-time checking on reaction-number arguments and makes
//! hover documentation legible in rust-analyzer.
//!
//! ## Usage
//!
//! ```
//! use njoy_outram_park_fork::endf::MtReaction;
//!
//! // From a known compile-time reaction:
//! let mt = MtReaction::Mt18Fission;
//! assert_eq!(i32::from(mt), 18);
//!
//! // From a runtime integer (e.g. config file, CLI argument):
//! let mt = MtReaction::try_from(102).unwrap();
//! assert_eq!(mt, MtReaction::Mt102Capture);
//!
//! // For display in logs:
//! println!("{mt}");   // "radiative capture n,γ (MT=102)"
//!
//! // Parsing raw ENDF data where any MT can appear:
//! let mt = MtReaction::from_any(451);   // always succeeds
//! assert_eq!(mt, MtReaction::Mt451GeneralInfo);
//! let mt = MtReaction::from_any(999);   // unknown → Unknown(999)
//! assert!(matches!(mt, MtReaction::Unknown(999)));
//! ```

use std::fmt;

/// Standard ENDF-6 MT reaction/section numbers.
///
/// Each variant carries its MT integer and a human-readable description.
/// Conversion to/from `i32`:
///
/// - `i32::from(mt)` — always succeeds (including `Unknown(n)` → n).
/// - `MtReaction::try_from(n)` — succeeds for any MT number listed here;
///   returns `Err(n)` for numbers that are not in the standard list.
/// - `MtReaction::from_any(n)` — always succeeds; maps unknown numbers to
///   [`Unknown`][MtReaction::Unknown] instead of returning an error. Use this
///   when deserialising raw ENDF data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MtReaction {
    // ── Composite / sum reactions ──────────────────────────────────────────
    /// MT=1: Total cross section (sum of all reactions).
    Mt1Total,
    /// MT=2: Elastic scattering (n,n).
    Mt2Elastic,
    /// MT=3: Nonelastic total (sum of everything except MT=2).
    Mt3Nonelastic,
    /// MT=4: Sum of all MT=51–91 inelastic levels.
    Mt4Inelastic,
    /// MT=5: Any reaction not listed elsewhere (n,anything).
    Mt5NAny,

    // ── Two-particle final-state reactions ────────────────────────────────
    /// MT=11: n,2n+d.
    Mt11N2nD,
    /// MT=16: n,2n.
    Mt16N2n,
    /// MT=17: n,3n.
    Mt17N3n,
    /// MT=18: Total fission (first + second + third + fourth chance).
    Mt18Fission,
    /// MT=19: First-chance fission.
    Mt19FirstChanceFission,
    /// MT=20: Second-chance fission.
    Mt20SecondChanceFission,
    /// MT=21: Third-chance fission.
    Mt21ThirdChanceFission,
    /// MT=22: n,n'+α — neutron + alpha in the exit channel.
    Mt22NnAlpha,
    /// MT=23: n,n'+3α.
    Mt23Nn3Alpha,
    /// MT=24: n,2n+α.
    Mt24N2nAlpha,
    /// MT=25: n,3n+α.
    Mt25N3nAlpha,
    /// MT=27: Absorption (MT=18 + MT=102 + MT=103 + …).
    Mt27Absorption,
    /// MT=28: n,n'+p — neutron + proton in the exit channel.
    Mt28NnProton,
    /// MT=29: n,n'+2α.
    Mt29Nn2Alpha,
    /// MT=30: n,2n+2α.
    Mt30N2n2Alpha,
    /// MT=32: n,n'+d.
    Mt32NnD,
    /// MT=33: n,n'+t.
    Mt33NnT,
    /// MT=34: n,n'+³He.
    Mt34NnHe3,
    /// MT=35: n,n'+d+2α.
    Mt35NnD2Alpha,
    /// MT=36: n,n'+t+2α.
    Mt36NnT2Alpha,
    /// MT=37: n,4n.
    Mt37N4n,
    /// MT=38: Fourth-chance fission.
    Mt38FourthChanceFission,
    /// MT=41: n,2n+p.
    Mt41N2nProton,
    /// MT=42: n,3n+p.
    Mt42N3nProton,
    /// MT=44: n,n'+2p.
    Mt44Nn2Proton,
    /// MT=45: n,n'+p+α.
    Mt45NnProtonAlpha,

    // ── Inelastic scattering to individual levels (MT=51–90) ─────────────
    /// MT=51: n,n' to the first excited level.
    Mt51NnLevel1,
    /// MT=52: n,n' to the second excited level.
    Mt52NnLevel2,
    /// MT=53: n,n' to the third excited level.
    Mt53NnLevel3,
    /// MT=54: n,n' to the fourth excited level.
    Mt54NnLevel4,
    /// MT=55: n,n' to the fifth excited level.
    Mt55NnLevel5,
    /// MT=91: n,n' continuum (all inelastic levels not listed individually).
    Mt91NnContinuum,

    // ── Particle emission reactions ───────────────────────────────────────
    /// MT=101: Absorption total (sum of MT=102–117).
    Mt101AbsorptionTotal,
    /// MT=102: Radiative capture, n+A → γ+(A+1).
    Mt102Capture,
    /// MT=103: n,p — proton in the exit channel.
    Mt103Np,
    /// MT=104: n,d — deuteron in the exit channel.
    Mt104Nd,
    /// MT=105: n,t — triton in the exit channel.
    Mt105Nt,
    /// MT=106: n,³He.
    Mt106NHe3,
    /// MT=107: n,α — alpha in the exit channel.
    Mt107NAlpha,
    /// MT=108: n,2α.
    Mt108N2Alpha,
    /// MT=109: n,3α.
    Mt109N3Alpha,
    /// MT=111: n,2p.
    Mt111N2Proton,
    /// MT=112: n,p+α.
    Mt112NProtonAlpha,
    /// MT=113: n,t+2α.
    Mt113NT2Alpha,
    /// MT=114: n,d+2α.
    Mt114ND2Alpha,
    /// MT=115: n,p+d.
    Mt115NProtonD,
    /// MT=116: n,p+t.
    Mt116NProtonT,
    /// MT=117: n,d+α.
    Mt117NDAlpha,

    // ── MF=2 special section ──────────────────────────────────────────────
    /// MT=151: Resonance parameters (MF=2 only).
    Mt151ResonanceParameters,

    // ── Energy / heating ─────────────────────────────────────────────────
    /// MT=301: Kinematic energy release (KERMA).
    Mt301Kerma,
    /// MT=444: Damage energy (DPA cross section).
    Mt444DamageEnergy,

    // ── MF=1 special sections ─────────────────────────────────────────────
    /// MT=451: General nuclide information (MF=1 only).
    Mt451GeneralInfo,
    /// MT=452: Total neutron multiplicity ν̄ (prompt + delayed).
    Mt452NubarTotal,
    /// MT=455: Delayed neutron multiplicity.
    Mt455NubarDelayed,
    /// MT=456: Prompt neutron multiplicity.
    Mt456NubarPrompt,
    /// MT=458: Energy release per fission event.
    Mt458EnergyRelease,
    /// MT=460: Delayed fission photon spectrum.
    Mt460DelayedFissionPhotons,

    // ── Catch-all ─────────────────────────────────────────────────────────
    /// An MT number not represented by any named variant above.
    ///
    /// Created by [`MtReaction::from_any`]; never returned by
    /// [`TryFrom<i32>`][MtReaction#impl-TryFrom<i32>-for-MtReaction].
    Unknown(i32),
}

impl MtReaction {
    /// Convert any `i32` to an [`MtReaction`], mapping unknown numbers to
    /// [`Unknown(n)`][MtReaction::Unknown] instead of returning an error.
    ///
    /// Use this when deserialising raw ENDF data where any MT number can appear.
    /// For validated user input, prefer [`try_from`][MtReaction::try_from] which
    /// returns `Err` for unrecognised numbers.
    pub fn from_any(n: i32) -> Self {
        Self::try_from(n).unwrap_or(MtReaction::Unknown(n))
    }

    /// Return the integer MT number.
    pub fn number(self) -> i32 {
        i32::from(self)
    }
}

// ── Conversions ────────────────────────────────────────────────────────────────

impl From<MtReaction> for i32 {
    fn from(mt: MtReaction) -> i32 {
        match mt {
            MtReaction::Mt1Total => 1,
            MtReaction::Mt2Elastic => 2,
            MtReaction::Mt3Nonelastic => 3,
            MtReaction::Mt4Inelastic => 4,
            MtReaction::Mt5NAny => 5,
            MtReaction::Mt11N2nD => 11,
            MtReaction::Mt16N2n => 16,
            MtReaction::Mt17N3n => 17,
            MtReaction::Mt18Fission => 18,
            MtReaction::Mt19FirstChanceFission => 19,
            MtReaction::Mt20SecondChanceFission => 20,
            MtReaction::Mt21ThirdChanceFission => 21,
            MtReaction::Mt22NnAlpha => 22,
            MtReaction::Mt23Nn3Alpha => 23,
            MtReaction::Mt24N2nAlpha => 24,
            MtReaction::Mt25N3nAlpha => 25,
            MtReaction::Mt27Absorption => 27,
            MtReaction::Mt28NnProton => 28,
            MtReaction::Mt29Nn2Alpha => 29,
            MtReaction::Mt30N2n2Alpha => 30,
            MtReaction::Mt32NnD => 32,
            MtReaction::Mt33NnT => 33,
            MtReaction::Mt34NnHe3 => 34,
            MtReaction::Mt35NnD2Alpha => 35,
            MtReaction::Mt36NnT2Alpha => 36,
            MtReaction::Mt37N4n => 37,
            MtReaction::Mt38FourthChanceFission => 38,
            MtReaction::Mt41N2nProton => 41,
            MtReaction::Mt42N3nProton => 42,
            MtReaction::Mt44Nn2Proton => 44,
            MtReaction::Mt45NnProtonAlpha => 45,
            MtReaction::Mt51NnLevel1 => 51,
            MtReaction::Mt52NnLevel2 => 52,
            MtReaction::Mt53NnLevel3 => 53,
            MtReaction::Mt54NnLevel4 => 54,
            MtReaction::Mt55NnLevel5 => 55,
            MtReaction::Mt91NnContinuum => 91,
            MtReaction::Mt101AbsorptionTotal => 101,
            MtReaction::Mt102Capture => 102,
            MtReaction::Mt103Np => 103,
            MtReaction::Mt104Nd => 104,
            MtReaction::Mt105Nt => 105,
            MtReaction::Mt106NHe3 => 106,
            MtReaction::Mt107NAlpha => 107,
            MtReaction::Mt108N2Alpha => 108,
            MtReaction::Mt109N3Alpha => 109,
            MtReaction::Mt111N2Proton => 111,
            MtReaction::Mt112NProtonAlpha => 112,
            MtReaction::Mt113NT2Alpha => 113,
            MtReaction::Mt114ND2Alpha => 114,
            MtReaction::Mt115NProtonD => 115,
            MtReaction::Mt116NProtonT => 116,
            MtReaction::Mt117NDAlpha => 117,
            MtReaction::Mt151ResonanceParameters => 151,
            MtReaction::Mt301Kerma => 301,
            MtReaction::Mt444DamageEnergy => 444,
            MtReaction::Mt451GeneralInfo => 451,
            MtReaction::Mt452NubarTotal => 452,
            MtReaction::Mt455NubarDelayed => 455,
            MtReaction::Mt456NubarPrompt => 456,
            MtReaction::Mt458EnergyRelease => 458,
            MtReaction::Mt460DelayedFissionPhotons => 460,
            MtReaction::Unknown(n) => n,
        }
    }
}

impl TryFrom<i32> for MtReaction {
    /// The unrecognised MT number is returned as the error payload.
    type Error = i32;

    fn try_from(n: i32) -> Result<Self, i32> {
        Ok(match n {
            1 => MtReaction::Mt1Total,
            2 => MtReaction::Mt2Elastic,
            3 => MtReaction::Mt3Nonelastic,
            4 => MtReaction::Mt4Inelastic,
            5 => MtReaction::Mt5NAny,
            11 => MtReaction::Mt11N2nD,
            16 => MtReaction::Mt16N2n,
            17 => MtReaction::Mt17N3n,
            18 => MtReaction::Mt18Fission,
            19 => MtReaction::Mt19FirstChanceFission,
            20 => MtReaction::Mt20SecondChanceFission,
            21 => MtReaction::Mt21ThirdChanceFission,
            22 => MtReaction::Mt22NnAlpha,
            23 => MtReaction::Mt23Nn3Alpha,
            24 => MtReaction::Mt24N2nAlpha,
            25 => MtReaction::Mt25N3nAlpha,
            27 => MtReaction::Mt27Absorption,
            28 => MtReaction::Mt28NnProton,
            29 => MtReaction::Mt29Nn2Alpha,
            30 => MtReaction::Mt30N2n2Alpha,
            32 => MtReaction::Mt32NnD,
            33 => MtReaction::Mt33NnT,
            34 => MtReaction::Mt34NnHe3,
            35 => MtReaction::Mt35NnD2Alpha,
            36 => MtReaction::Mt36NnT2Alpha,
            37 => MtReaction::Mt37N4n,
            38 => MtReaction::Mt38FourthChanceFission,
            41 => MtReaction::Mt41N2nProton,
            42 => MtReaction::Mt42N3nProton,
            44 => MtReaction::Mt44Nn2Proton,
            45 => MtReaction::Mt45NnProtonAlpha,
            51 => MtReaction::Mt51NnLevel1,
            52 => MtReaction::Mt52NnLevel2,
            53 => MtReaction::Mt53NnLevel3,
            54 => MtReaction::Mt54NnLevel4,
            55 => MtReaction::Mt55NnLevel5,
            91 => MtReaction::Mt91NnContinuum,
            101 => MtReaction::Mt101AbsorptionTotal,
            102 => MtReaction::Mt102Capture,
            103 => MtReaction::Mt103Np,
            104 => MtReaction::Mt104Nd,
            105 => MtReaction::Mt105Nt,
            106 => MtReaction::Mt106NHe3,
            107 => MtReaction::Mt107NAlpha,
            108 => MtReaction::Mt108N2Alpha,
            109 => MtReaction::Mt109N3Alpha,
            111 => MtReaction::Mt111N2Proton,
            112 => MtReaction::Mt112NProtonAlpha,
            113 => MtReaction::Mt113NT2Alpha,
            114 => MtReaction::Mt114ND2Alpha,
            115 => MtReaction::Mt115NProtonD,
            116 => MtReaction::Mt116NProtonT,
            117 => MtReaction::Mt117NDAlpha,
            151 => MtReaction::Mt151ResonanceParameters,
            301 => MtReaction::Mt301Kerma,
            444 => MtReaction::Mt444DamageEnergy,
            451 => MtReaction::Mt451GeneralInfo,
            452 => MtReaction::Mt452NubarTotal,
            455 => MtReaction::Mt455NubarDelayed,
            456 => MtReaction::Mt456NubarPrompt,
            458 => MtReaction::Mt458EnergyRelease,
            460 => MtReaction::Mt460DelayedFissionPhotons,
            _ => return Err(n),
        })
    }
}

// ── Display ────────────────────────────────────────────────────────────────────

impl fmt::Display for MtReaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (name, n) = match self {
            MtReaction::Mt1Total => ("total cross section", 1),
            MtReaction::Mt2Elastic => ("elastic scattering", 2),
            MtReaction::Mt3Nonelastic => ("nonelastic total", 3),
            MtReaction::Mt4Inelastic => ("inelastic total", 4),
            MtReaction::Mt5NAny => ("n,anything", 5),
            MtReaction::Mt11N2nD => ("n,2n+d", 11),
            MtReaction::Mt16N2n => ("n,2n", 16),
            MtReaction::Mt17N3n => ("n,3n", 17),
            MtReaction::Mt18Fission => ("total fission", 18),
            MtReaction::Mt19FirstChanceFission => ("first-chance fission", 19),
            MtReaction::Mt20SecondChanceFission => ("second-chance fission", 20),
            MtReaction::Mt21ThirdChanceFission => ("third-chance fission", 21),
            MtReaction::Mt22NnAlpha => ("n,n'+α", 22),
            MtReaction::Mt23Nn3Alpha => ("n,n'+3α", 23),
            MtReaction::Mt24N2nAlpha => ("n,2n+α", 24),
            MtReaction::Mt25N3nAlpha => ("n,3n+α", 25),
            MtReaction::Mt27Absorption => ("absorption", 27),
            MtReaction::Mt28NnProton => ("n,n'+p", 28),
            MtReaction::Mt29Nn2Alpha => ("n,n'+2α", 29),
            MtReaction::Mt30N2n2Alpha => ("n,2n+2α", 30),
            MtReaction::Mt32NnD => ("n,n'+d", 32),
            MtReaction::Mt33NnT => ("n,n'+t", 33),
            MtReaction::Mt34NnHe3 => ("n,n'+³He", 34),
            MtReaction::Mt35NnD2Alpha => ("n,n'+d+2α", 35),
            MtReaction::Mt36NnT2Alpha => ("n,n'+t+2α", 36),
            MtReaction::Mt37N4n => ("n,4n", 37),
            MtReaction::Mt38FourthChanceFission => ("fourth-chance fission", 38),
            MtReaction::Mt41N2nProton => ("n,2n+p", 41),
            MtReaction::Mt42N3nProton => ("n,3n+p", 42),
            MtReaction::Mt44Nn2Proton => ("n,n'+2p", 44),
            MtReaction::Mt45NnProtonAlpha => ("n,n'+p+α", 45),
            MtReaction::Mt51NnLevel1 => ("n,n' first excited level", 51),
            MtReaction::Mt52NnLevel2 => ("n,n' second excited level", 52),
            MtReaction::Mt53NnLevel3 => ("n,n' third excited level", 53),
            MtReaction::Mt54NnLevel4 => ("n,n' fourth excited level", 54),
            MtReaction::Mt55NnLevel5 => ("n,n' fifth excited level", 55),
            MtReaction::Mt91NnContinuum => ("n,n' continuum", 91),
            MtReaction::Mt101AbsorptionTotal => ("absorption total", 101),
            MtReaction::Mt102Capture => ("radiative capture n,γ", 102),
            MtReaction::Mt103Np => ("n,p", 103),
            MtReaction::Mt104Nd => ("n,d", 104),
            MtReaction::Mt105Nt => ("n,t", 105),
            MtReaction::Mt106NHe3 => ("n,³He", 106),
            MtReaction::Mt107NAlpha => ("n,α", 107),
            MtReaction::Mt108N2Alpha => ("n,2α", 108),
            MtReaction::Mt109N3Alpha => ("n,3α", 109),
            MtReaction::Mt111N2Proton => ("n,2p", 111),
            MtReaction::Mt112NProtonAlpha => ("n,p+α", 112),
            MtReaction::Mt113NT2Alpha => ("n,t+2α", 113),
            MtReaction::Mt114ND2Alpha => ("n,d+2α", 114),
            MtReaction::Mt115NProtonD => ("n,p+d", 115),
            MtReaction::Mt116NProtonT => ("n,p+t", 116),
            MtReaction::Mt117NDAlpha => ("n,d+α", 117),
            MtReaction::Mt151ResonanceParameters => ("resonance parameters", 151),
            MtReaction::Mt301Kerma => ("KERMA", 301),
            MtReaction::Mt444DamageEnergy => ("damage energy (DPA)", 444),
            MtReaction::Mt451GeneralInfo => ("general nuclide information", 451),
            MtReaction::Mt452NubarTotal => ("total neutron multiplicity ν̄", 452),
            MtReaction::Mt455NubarDelayed => ("delayed neutron multiplicity", 455),
            MtReaction::Mt456NubarPrompt => ("prompt neutron multiplicity", 456),
            MtReaction::Mt458EnergyRelease => ("energy release per fission", 458),
            MtReaction::Mt460DelayedFissionPhotons => ("delayed fission photons", 460),
            MtReaction::Unknown(n) => return write!(f, "unknown reaction (MT={n})"),
        };
        write!(f, "{name} (MT={n})")
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_mt_to_i32() {
        assert_eq!(i32::from(MtReaction::Mt1Total), 1);
        assert_eq!(i32::from(MtReaction::Mt2Elastic), 2);
        assert_eq!(i32::from(MtReaction::Mt18Fission), 18);
        assert_eq!(i32::from(MtReaction::Mt102Capture), 102);
        assert_eq!(i32::from(MtReaction::Unknown(999)), 999);
    }

    #[test]
    fn try_from_known_mt() {
        assert_eq!(MtReaction::try_from(1).unwrap(), MtReaction::Mt1Total);
        assert_eq!(MtReaction::try_from(2).unwrap(), MtReaction::Mt2Elastic);
        assert_eq!(MtReaction::try_from(18).unwrap(), MtReaction::Mt18Fission);
        assert_eq!(MtReaction::try_from(102).unwrap(), MtReaction::Mt102Capture);
        assert_eq!(
            MtReaction::try_from(451).unwrap(),
            MtReaction::Mt451GeneralInfo
        );
    }

    #[test]
    fn try_from_unknown_returns_err() {
        assert_eq!(MtReaction::try_from(999), Err(999));
        assert_eq!(MtReaction::try_from(7), Err(7));
        assert_eq!(MtReaction::try_from(0), Err(0));
    }

    #[test]
    fn from_any_known() {
        assert_eq!(MtReaction::from_any(18), MtReaction::Mt18Fission);
    }

    #[test]
    fn from_any_unknown_gives_unknown_variant() {
        assert_eq!(MtReaction::from_any(999), MtReaction::Unknown(999));
    }

    #[test]
    fn number_method() {
        assert_eq!(MtReaction::Mt18Fission.number(), 18);
        assert_eq!(MtReaction::Unknown(42).number(), 42);
    }

    #[test]
    fn display_known() {
        let s = MtReaction::Mt102Capture.to_string();
        assert!(s.contains("102"), "missing MT number in '{s}'");
        assert!(
            s.contains("capture") || s.contains("γ"),
            "missing physics name in '{s}'"
        );
    }

    #[test]
    fn display_unknown() {
        let s = MtReaction::Unknown(999).to_string();
        assert!(s.contains("999"), "missing number in unknown display '{s}'");
    }

    #[test]
    fn roundtrip_all_known() {
        let known = [
            1, 2, 3, 4, 5, 11, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 27, 28, 29, 30, 32, 33, 34,
            35, 36, 37, 38, 41, 42, 44, 45, 51, 52, 53, 54, 55, 91, 101, 102, 103, 104, 105, 106,
            107, 108, 109, 111, 112, 113, 114, 115, 116, 117, 151, 301, 444, 451, 452, 455, 456,
            458, 460_i32,
        ];
        for n in known {
            let mt = MtReaction::try_from(n).unwrap_or_else(|_| panic!("MT={n} not in enum"));
            assert_eq!(i32::from(mt), n, "roundtrip MT={n}");
        }
    }
}
