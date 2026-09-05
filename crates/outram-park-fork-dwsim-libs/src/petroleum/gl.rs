//! Ideal-gas enthalpy and entropy of formation of a petroleum pseudo-component,
//! estimated from a **molecular-type (PNA) analysis** back-calculated from the
//! cut's bulk specific gravity, molecular weight and boiling point.
//!
//! # Provenance
//!
//! Faithful port of DWSIM (GPL-3.0),
//! `DWSIM.Thermodynamics/PetroleumCharacterization/GL.vb` (128 lines, whole
//! file — the single method `calculate_Hf_Sf`, `:29-122`), from the pinned
//! upstream clone `/home/teddy0/Documents/research/dwsim-upstream`, branch
//! `windows`, commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream
//! copyright: 2008 Daniel Wagner O. de Medeiros and the DWSIM contributors.
//! GPL-3.0; this port is GPL-3.0-only.
//!
//! "GL" is upstream's own abbreviation for the method; the file cites
//! **Riazi, M. R. & Daubert, T. E. (1986), "Prediction of molecular-type
//! analysis of petroleum fraction and coal liquids", *Ind. Eng. Chem. Process
//! Des. Dev.* 25, 1009-1015** (`GL.vb:67-69`) for the carbon-to-hydrogen ratio
//! and paraffin-content steps.
//!
//! # How it works
//!
//! 1. The **refractivity intercept** `I` and refractive index `IR` are
//!    estimated from `Tb` and `SG` (`:33-34`).
//! 2. Two group parameters `G` (gravity-based) and `L` (refractivity-based)
//!    give the **ring content**: total rings `Rt`, aromatic rings `Rar`,
//!    naphthenic rings `Rnf` (`:41-65`).
//! 3. The Riazi-Daubert C/H weight ratio and paraffin content `P` fix the
//!    **carbon distribution** across paraffinic, naphthenic and aromatic
//!    structures (`:72-99`).
//! 4. Group-contribution sums over those carbon counts give the ideal-gas
//!    enthalpy `Hf` and entropy `Sf` of formation, normalised per unit mass
//!    (`:112-115`).
//!
//! # Units — read this before using the results
//!
//! `enthalpy_of_formation` and `entropy_of_formation` are returned as **raw
//! `f64` in whatever units DWSIM's group-contribution constants are expressed
//! in, divided by the molecular weight in g/mol** (`GL.vb:114-115`). DWSIM
//! assigns them straight to its `ConstantProperties.IG_Enthalpy_of_Formation_25C`
//! and `IG_Entropy_of_Formation_25C` fields, whose declared units are
//! **kJ/kg** and **kJ/(kg·K)**.
//!
//! **This port does not wrap them in `uom` types**, deliberately: upstream
//! documents neither the units of the raw group-contribution coefficients
//! (`−0.4354`, `−20.63`, `11.71`, …) nor a literature source for them, so
//! asserting a dimension would be an unverified claim. They are carried through
//! as opaque, documented `f64` values with their DWSIM provenance intact.
//! `carbon_number` and `hydrogen_number` are unambiguous dimensionless atom
//! counts per molecule.
//!
//! # Excluded DWSIM behavior
//!
//! Nothing is excluded; the whole file is ported. The `Return New Double() {Hf,
//! Sf, Nc, Nh}` untyped array (`:120`) becomes the typed
//! [`FormationProperties`].
//!
//! # Upstream quirks preserved
//!
//! - **`Narnf` is never assigned** (`GL.vb:106`, `:112`, `:113` read it; no
//!   line writes it), so VB's implicit zero-initialisation makes it `0.0`. The
//!   term it multiplies (`+27.26·Narnf`, `−16.6·Narnf`) therefore always
//!   vanishes. Reproduced as a named `0.0` constant.
//! - **`m0` is unassigned when `Rt == 1` exactly** (`:78-79` test `Rt < 1` and
//!   `Rt > 1` but not equality), leaving it `0.0`. Reproduced.
//! - **`Nnf1` is computed twice** (`:102` and `:107`) with the same expression.
//!   Reproduced once.
//! - **`Gcor`/`Lcor` are unconditional copies** of `G`/`L` (`:44-45`) — a
//!   correction hook upstream never wired up.

use uom::si::f64::{MolarMass, ThermodynamicTemperature};
use uom::si::molar_mass::gram_per_mole;
use uom::si::thermodynamic_temperature::kelvin;

use super::property_methods::SpecificGravity;
use uom::si::ratio::ratio;

/// Upstream's `Narnf` — the aromatic-naphthenic bridging carbon count. Never
/// assigned in `GL.vb`, hence identically zero. Kept as a named constant so the
/// ported expressions read the same as upstream's.
const NARNF: f64 = 0.0;

/// Ideal-gas formation properties and atom counts of a petroleum
/// pseudo-component, as produced by [`calculate_formation_properties`].
///
/// Corresponds to upstream's `Double() {Hf, Sf, Nc, Nh}` (`GL.vb:120`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormationProperties {
    /// Ideal-gas **enthalpy of formation at 25 °C**, per unit mass.
    ///
    /// DWSIM assigns this to `IG_Enthalpy_of_Formation_25C` [kJ/kg]. See the
    /// module-level "Units" note — the underlying group-contribution
    /// coefficients are undocumented upstream, so this is carried as a raw
    /// `f64` rather than a `uom` quantity.
    pub enthalpy_of_formation: f64,
    /// Ideal-gas **entropy of formation at 25 °C**, per unit mass.
    ///
    /// DWSIM assigns this to `IG_Entropy_of_Formation_25C` [kJ/(kg·K)]. Same
    /// units caveat as [`Self::enthalpy_of_formation`].
    pub entropy_of_formation: f64,
    /// Carbon atoms per molecule `Nc` [-]. Forced to `0.0` if the calculation
    /// produced `NaN` (`GL.vb:117`).
    pub carbon_number: f64,
    /// Hydrogen atoms per molecule `Nh = Nc·(12/CH)` [-], where `CH` is the
    /// carbon-to-hydrogen **weight** ratio. Forced to `0.0` on `NaN`
    /// (`GL.vb:118`).
    pub hydrogen_number: f64,
}

impl FormationProperties {
    /// Ideal-gas **Gibbs energy of formation at 25 °C**, per unit mass:
    /// `Gf = Hf − 298.15·Sf`.
    ///
    /// Inlined upstream at `GenerateCompounds.vb:341` and `DistCurves.cs:745`.
    /// Same units caveat as the two terms it combines.
    #[must_use]
    pub fn gibbs_energy_of_formation(&self) -> f64 {
        self.enthalpy_of_formation - 298.15 * self.entropy_of_formation
    }

    /// The pseudo-component's nominal formula string, `"C<nc>H<nh>"` with two
    /// decimals — upstream's `ConstantProperties.Formula`
    /// (`GenerateCompounds.vb:343`, `DistCurves.cs:747`). Non-integer counts
    /// are expected: a pseudo-component is an average over many real molecules.
    #[must_use]
    pub fn formula(&self) -> String {
        format!("C{:.2}H{:.2}", self.carbon_number, self.hydrogen_number)
    }
}

/// Estimate the ideal-gas formation properties and PNA carbon distribution of a
/// petroleum cut from its specific gravity, molecular weight, and mean boiling
/// point.
///
/// Ported from `GL.vb:29-122` (`calculate_Hf_Sf`).
///
/// # Inputs
///
/// - `specific_gravity` — `SG` at 15.6/15.6 °C [-]. Physical range ≈ 0.6-1.05.
/// - `molar_mass` — the cut's molecular weight `M`; the correlation is
///   regressed in **g/mol** and this function converts internally.
/// - `boiling_point` — the cut's mean boiling point `Tb` [K]. Converted to °R
///   (`×1.8`) internally, matching upstream.
///
/// # Valid range
///
/// The Riazi-Daubert molecular-type analysis it rests on is regressed for
/// petroleum fractions with `M` ≈ 70-300 g/mol and `Tb` ≈ 300-800 K. Outside
/// that band `Rt`, `Rar` or `Mc` can go negative and the results become
/// meaningless (upstream returns them anyway; so does this port, except that
/// `NaN` carbon/hydrogen counts are zeroed exactly as upstream does).
#[must_use]
pub fn calculate_formation_properties(
    specific_gravity: SpecificGravity,
    molar_mass: MolarMass,
    boiling_point: ThermodynamicTemperature,
) -> FormationProperties {
    let sg = specific_gravity.get::<ratio>();
    let m = molar_mass.get::<gram_per_mole>();
    let tb = boiling_point.get::<kelvin>();

    // Refractivity intercept and refractive index (`:33-34`). `1.8*TB` is °R.
    let i = 0.02266
        * (0.0003905 * (1.8 * tb) + 2.468 * sg - 0.0005704 * 1.8 * tb * sg).exp()
        * (1.8 * tb).powf(0.0572)
        * sg.powf(-0.72);
    let ir = ((1.0 + 2.0 * i) / (1.0 - i)).sqrt();

    // Gravity- and refractivity-based group parameters (`:41-42`).
    let g = m * (sg - 0.8513) / sg + 23.6;
    let l = m * (ir - 1.4752) / ir.powi(2) + 4.51;

    // `Gcor`/`Lcor` are unconditional copies upstream (`:44-45`).
    let g_cor = g;
    let l_cor = l;

    // Ring content (`:47-65`).
    let (rt, rar, _rnf, r_nf) = if g < 16.0 {
        let rnf = (g_cor - 4.6154 * l_cor) / 4.45;
        let rar = (l_cor - 2.178 * rnf) / 5.46;
        (rar + rnf, rar, rnf, rnf)
    } else {
        let alpha = if g < 26.0 {
            (3.685 - 0.58 * g_cor / l_cor).clamp(0.0, 1.0)
        } else {
            (2.512 - 0.484 * (g_cor - 13.1) / (l_cor - 1.45)).clamp(0.0, 1.0)
        };
        let rt = 1.0 + (l_cor - (2.18 + 3.28 * alpha.powf(1.5))) / (2.32 + 4.45 * alpha.powf(1.5));
        let rar = rt * alpha;
        let rnf = rt - rar;
        (rt, rar, rnf, rnf)
    };
    let rnf = r_nf;

    // Riazi-Daubert (1986) carbon-to-hydrogen weight ratio and paraffin
    // content (`:72-74`).
    let ch = 17.22
        * (0.00825 * (1.8 * tb) + 16.94 * sg - 0.00694 * (1.8 * tb) * sg).exp()
        * (1.8 * tb).powf(-2.725)
        * sg.powf(-6.798);
    let p = 257.0 - 287.7 * sg + 2.876 * ch;

    let nh_ = (100.0 + p) / 100.0;

    // `m0` is left at 0 when Rt == 1 exactly — upstream's own gap (`:78-79`).
    let m0 = if rt < 1.0 {
        6.0
    } else if rt > 1.0 {
        2.0 - 2.0 * (rt - 1.0) / rt
    } else {
        0.0
    };

    // Molecular-core mass (`:81-85`).
    let mc = if (rt - 1.0) < 0.0 {
        0.85632 * (m - nh_ + m0 * rar + rt)
    } else {
        0.85632 * (m - nh_ + m0 * rar + rt + 2.0 * (rt - 1.0))
    };

    // Carbon distribution over aromatic / naphthenic / paraffinic structures
    // (`:87-94`).
    let car = 100.0 * (12.011 * m0 * rar) / mc;
    let mut cnf = 100.0 * (12.011 * m0 * rnf) / mc;
    let mut cpn = 100.0 - car - cnf;
    if car + cnf > 100.0 {
        cnf = 100.0 - car;
        cpn = 0.0;
    }

    // Atom counts per structural family (`:96-110`).
    let npn = 0.01 * cpn * mc / 12.011;
    let nc2 = npn;
    let nar = 0.01 * car * mc / 12.011;
    let nnf = 0.01 * cnf * mc / 12.011;
    let nar2 = (2.0 * (rar - 1.0)).max(0.0);
    let nnf2 = (2.0 * (rnf - 1.0)).max(0.0);
    let nar1 = nar - nar2 - NARNF;
    let nnf1 = nnf - nnf2;

    let nc = npn + nar + nnf;
    let nh = nc * (12.0 / ch);

    // Group-contribution sums (`:112-115`).
    let hf = (-0.4354 * p - 20.63 * nc2 - 21.94 * nnf1 - 3.96 * nnf2
        + 11.71 * nar1
        + 21.76 * nar2
        + 27.26 * NARNF
        + 6.64 * rt * (1.0 - npn))
        / m;
    let sf = (1.542 * p + 39.0 * nc2 + 46.9 * nar1 - 16.6 * nar2 - 16.6 * NARNF + 50.8 * nnf1
        - 15.0 * nnf2
        + 2.9 * rt * (1.0 - npn))
        / m;

    FormationProperties {
        enthalpy_of_formation: hf,
        entropy_of_formation: sf,
        carbon_number: if nc.is_nan() { 0.0 } else { nc },
        hydrogen_number: if nh.is_nan() { 0.0 } else { nh },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::Ratio;

    fn sg(v: f64) -> SpecificGravity {
        Ratio::new::<ratio>(v)
    }

    /// **Methodology.** For a kerosene-range pseudo-component
    /// (`SG = 0.78`, `M = 160 g/mol`, `Tb = 450 K`) the PNA back-calculation
    /// must yield a chemically sensible molecule: carbon number in the C8-C16
    /// band implied by `M ≈ 160` (roughly `M/14` ≈ 11 CH₂ units) and a
    /// hydrogen-to-carbon **atom** ratio between 1.0 (aromatic) and 2.3
    /// (paraffinic). Enthalpy/entropy must be finite.
    ///
    /// **Results (2026-08-11, this port).** `Nc = 11.346`, `Nh = 22.580`,
    /// `H/C = 1.990`; `Hf = -1.7936`, `Sf = 3.4444` (DWSIM's per-mass units,
    /// see the module note). All inside the stated bands — test passes.
    #[test]
    fn kerosene_cut_gives_a_chemically_sensible_molecule() {
        let r = calculate_formation_properties(
            sg(0.78),
            MolarMass::new::<gram_per_mole>(160.0),
            ThermodynamicTemperature::new::<kelvin>(450.0),
        );
        assert!(
            (8.0..16.0).contains(&r.carbon_number),
            "Nc out of band: {r:?}"
        );
        let h_over_c = r.hydrogen_number / r.carbon_number;
        assert!((1.0..2.3).contains(&h_over_c), "H/C = {h_over_c}: {r:?}");
        assert!(r.enthalpy_of_formation.is_finite(), "{r:?}");
        assert!(r.entropy_of_formation.is_finite(), "{r:?}");
        assert!(r.gibbs_energy_of_formation().is_finite(), "{r:?}");
    }

    /// **Methodology.** The formula string must round the atom counts to two
    /// decimals in the `C…H…` form DWSIM writes into
    /// `ConstantProperties.Formula`.
    ///
    /// **Results (2026-08-11, this port).** For the kerosene cut above the
    /// formula is `"C11.35H22.58"`. Test passes.
    #[test]
    fn formula_string_matches_dwsim_format() {
        let r = calculate_formation_properties(
            sg(0.78),
            MolarMass::new::<gram_per_mole>(160.0),
            ThermodynamicTemperature::new::<kelvin>(450.0),
        );
        let f = r.formula();
        assert!(f.starts_with('C') && f.contains('H'), "bad formula {f}");
        assert_eq!(f.matches('.').count(), 2, "expected two decimals: {f}");
    }

    /// **Methodology.** Heavier, denser cuts are more aromatic, so their
    /// hydrogen-to-carbon atom ratio must fall. Compare a light paraffinic cut
    /// (`SG = 0.72`, `M = 110`, `Tb = 380 K`) with a heavy aromatic one
    /// (`SG = 0.90`, `M = 280`, `Tb = 600 K`).
    ///
    /// **Results (2026-08-11, this port).** Light cut `H/C = 2.165`; heavy
    /// cut `H/C = 1.726`. The ordering holds — test passes.
    #[test]
    fn heavier_cuts_are_more_aromatic() {
        let light = calculate_formation_properties(
            sg(0.72),
            MolarMass::new::<gram_per_mole>(110.0),
            ThermodynamicTemperature::new::<kelvin>(380.0),
        );
        let heavy = calculate_formation_properties(
            sg(0.90),
            MolarMass::new::<gram_per_mole>(280.0),
            ThermodynamicTemperature::new::<kelvin>(600.0),
        );
        let hc_light = light.hydrogen_number / light.carbon_number;
        let hc_heavy = heavy.hydrogen_number / heavy.carbon_number;
        assert!(
            hc_heavy < hc_light,
            "expected heavy cut to be more aromatic: {hc_heavy} vs {hc_light}"
        );
    }
}
