//! **Bulk-assay characterization** — split averaged (bulk) properties into `n`
//! pseudo-components using a three-parameter **gamma distribution**, then give
//! each cut a full set of EOS constants.
//!
//! This is the path for an assay that has *no* distillation curve: the user
//! knows the stream's average molecular weight and/or specific gravity and/or
//! mean boiling point, and wants a defensible set of compounds out of it. For
//! an assay that *does* have a curve, use
//! [`crate::petroleum::curve_characterization`].
//!
//! # Provenance
//!
//! Faithful port of DWSIM (GPL-3.0),
//! `DWSIM.Thermodynamics/PetroleumCharacterization/GenerateCompounds.vb`
//! (656 lines, whole file), from the pinned upstream clone
//! `/home/teddy0/Documents/research/dwsim-upstream`, branch `windows`, commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: Daniel
//! Wagner O. de Medeiros and the DWSIM contributors. GPL-3.0; this port is
//! GPL-3.0-only.
//!
//! | Rust item | Upstream |
//! |---|---|
//! | [`generate_compounds`] | `GenerateCompounds` (the method), `:30-487` |
//! | [`distribute_property`] | `DistMW` `:489-520`, `DistTB` `:522-551`, `DistSG` `:567-596` |
//! | [`distribute_property_means_only`] | `DistVISC1` `:598-624`, `DistVISC2` `:626-652` |
//! | the `Norm_dMF` step inside [`distribute_property`] | `Norm_dMF`, `:553-565` |
//! | the per-cut record assembly | `:260-377`, ported in [`crate::petroleum::pseudo_component`] |
//! | the parameter-fitting tail | `:379-448`, ported in [`crate::petroleum::fitting`] |
//!
//! # The distribution model
//!
//! For a bulk property `P` with lower bound `P0` and shape exponent `B`, define
//! `A` from the normalised excess `P* = (P − P0)/P0` and invert the cumulative
//! distribution on a 1001-point grid:
//!
//! ```text
//! d(i) = [ (A/B)·ln(1 / (1 − (i + 0.001)/1000)) ]^(1/B),   i = 0…1000
//! ```
//!
//! The `+0.001` offset (`:498`) keeps `d(0)` finite. Interpolating that grid
//! with a Floater-Hormann rational interpolant lets any cut boundary
//! `x = i/n` be evaluated; the mole fraction of cut `i` is the difference of the
//! survival function across its boundaries, and its **mean** property is the
//! difference of two regularized upper incomplete gamma integrals
//! ([`crate::petroleum::special::incomplete_gamma_q`]) divided by that mole
//! fraction.
//!
//! Shape exponents, from upstream: `B = 1` for molecular weight (`:494`),
//! `B = 1.5` for boiling point (`:527`), `B = 3` for specific gravity (`:570`)
//! and for both viscosities (`:601`, `:629`).
//!
//! # Units
//!
//! `uom`-typed on the public surface. Inside the distribution the arithmetic is
//! raw `f64` in the units each correlation was regressed in: **g/mol** for
//! molecular weight, **K** for temperature, dimensionless for specific gravity,
//! **m²/s** for kinematic viscosity.
//!
//! # Excluded DWSIM behavior
//!
//! - The `Dictionary(Of String, ICompound)` return and the `BaseClasses.Compound`
//!   / `ConstantProperties` wrappers (`:257`, `:366-375`) become a plain
//!   `Vec<`[`PseudoComponent`]`>`; a dictionary keyed by a generated name adds
//!   nothing and silently drops duplicates.
//! - The `data As New List(Of String())` table of stringified results
//!   (`:28`, `:455-473`) is GUI display state and is not ported; its only
//!   non-display use, the `Double.TryParse` NaN sweep (`:475-483`), is replaced
//!   by typed validation in
//!   [`crate::petroleum::pseudo_component::build_pseudo_component`].
//! - `MaterialStream` / `PropertyPackage` / flowsheet plumbing around the
//!   fitting tail (`:381-392`, `:450-453`) — see
//!   [`crate::petroleum::fitting`], which calls the saturation solver directly.
//! - The unused `error_v` / `error_p` / `error_cp` fields and the `m_action`
//!   progress string (`:13-14`).
//! - `MWav` (`:516`) is computed by upstream and never read; not ported.
//!
//! # ⚠️ Upstream defect preserved: the "boiling point only" branch is broken
//!
//! When the assay supplies **only** a mean boiling point (`M = 0`, `SG = 0`),
//! upstream computes each cut's molecular weight as `MWcorr(dTB(i), dSG(i))`
//! **before** `dSG(i)` has been assigned (`GenerateCompounds.vb:173-188`).
//! VB.NET zero-initialises the array, so every correlation is evaluated at
//! `SG = 0`: Riazi's `42.965·exp(...)·Tb^1.26007·SG^4.98308` collapses to `0`,
//! and the `dSG(i) = d15_Riazi(dMW(i))` that follows then evaluates
//! `d15_Riazi(0) = 1.07 − exp(3.56073) ≈ −34.1`.
//!
//! This port **reproduces the defect** rather than reordering the statements,
//! and [`build_pseudo_component`] then rejects the resulting cut with
//! [`PseudoComponentError::NonPhysical`] naming `molar_mass` — an honest,
//! diagnosable failure instead of DWSIM's silently absurd compounds. The test
//! `boiling_point_only_assay_reproduces_the_upstream_defect` pins the
//! behaviour. **Practical guidance: always supply `SG` (or `M`) alongside
//! `Tb`.**
//!
//! # Deviation from upstream (one, deliberate, documented)
//!
//! `DistVISC1`/`DistVISC2` **build** their Floater-Hormann interpolant over
//! 1000 grid points but **evaluate** it declaring only 999 (`:611` versus
//! `:614`, `:618-619`) — an internally inconsistent pairing that would use
//! 1000-point weights in a 999-point barycentric sum. This port uses **999
//! points consistently** for the two viscosity distributions (matching
//! upstream's evaluation count) and 1000 consistently for the other three
//! (matching upstream exactly). The difference is confined to the top of the
//! grid.

use uom::si::f64::{KinematicViscosity, MolarMass, Ratio, ThermodynamicTemperature};
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::molar_mass::gram_per_mole;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use crate::interpolation::floater_hormann::FloaterHormannInterpolant;

use super::assay::BulkAssay;
use super::property_methods::{self, SpecificGravity};
use super::pseudo_component::{
    build_pseudo_component, CorrelationSet, MolecularWeightCorrelation, PseudoComponent,
    PseudoComponentError,
};
use super::special::incomplete_gamma_q;

/// Number of grid points the property distributions are inverted on —
/// upstream's `For i = 0 To 1000` fill with a `1000`-point interpolant
/// (`GenerateCompounds.vb:497-505`).
const DISTRIBUTION_GRID: usize = 1000;

/// Grid points used by the two **viscosity** distributions — upstream evaluates
/// with `999` (`:614`, `:618-619`); see the module-level deviation note.
const VISCOSITY_GRID: usize = 999;

/// Floater-Hormann blending degree, upstream's third argument `1`
/// (`:502`, `:535`, `:580`, `:611`, `:639`).
const FH_DEGREE: usize = 1;

/// Default lower bound on molecular weight, `M0 = 90 g/mol`
/// (`GenerateCompounds` is given it by its caller; `Riazi.vb:49` uses the same
/// value, the molecular weight of roughly a C6-C7 molecule).
pub const DEFAULT_MOLAR_MASS_LOWER_BOUND_G_PER_MOL: f64 = 90.0;

/// Default lower bound on specific gravity, `SG0 = 0.7` (`Riazi.vb:102`).
pub const DEFAULT_SPECIFIC_GRAVITY_LOWER_BOUND: f64 = 0.7;

/// Default lower bound on boiling point, `Tb0 = 1080 − exp(6.97996 −
/// 0.01964·90^(2/3)) ≈ 356.9 K` — the Riazi-Al-Sahhaf SCN boiling point of the
/// `M0 = 90 g/mol` bound (`Riazi.vb:155`).
#[must_use]
pub fn default_boiling_point_lower_bound_kelvin() -> f64 {
    1080.0 - (6.97996 - 0.01964 * 90.0_f64.powf(2.0 / 3.0)).exp()
}

/// Everything [`generate_compounds`] needs: the bulk assay, how many cuts to
/// make, which correlations to use, and the distribution's lower bounds.
///
/// Upstream passes these as 16 positional arguments (`GenerateCompounds.vb:30-33`).
#[derive(Debug, Clone, PartialEq)]
pub struct BulkCharacterizationOptions {
    /// Name prefix for the generated compounds (upstream `prefix`). Leading and
    /// trailing ` _,;:` are trimmed.
    pub prefix: String,
    /// Number of pseudo-components to generate (upstream `count` → `n`).
    /// Must be at least 2.
    pub cut_count: usize,
    /// Which correlation to use for each estimated constant.
    pub correlations: CorrelationSet,
    /// The bulk assay being characterized.
    pub assay: BulkAssay,
    /// Lower bound `M0` on the molecular-weight distribution (upstream `_mw0`).
    /// Default [`DEFAULT_MOLAR_MASS_LOWER_BOUND_G_PER_MOL`].
    pub molar_mass_lower_bound: MolarMass,
    /// Lower bound `SG0` on the specific-gravity distribution (upstream `_sg0`).
    /// Default [`DEFAULT_SPECIFIC_GRAVITY_LOWER_BOUND`].
    pub specific_gravity_lower_bound: SpecificGravity,
    /// Lower bound `Tb0` on the boiling-point distribution (upstream `_tb0`).
    /// Default [`default_boiling_point_lower_bound_kelvin`].
    pub boiling_point_lower_bound: ThermodynamicTemperature,
}

impl Default for BulkCharacterizationOptions {
    /// A 10-cut characterization named `"PseudoC"` with the default correlation
    /// set and the standard distribution bounds, and an empty assay the caller
    /// is expected to fill in.
    fn default() -> Self {
        Self {
            prefix: "PseudoC".to_string(),
            cut_count: 10,
            correlations: CorrelationSet::default(),
            assay: BulkAssay::default(),
            molar_mass_lower_bound: MolarMass::new::<gram_per_mole>(
                DEFAULT_MOLAR_MASS_LOWER_BOUND_G_PER_MOL,
            ),
            specific_gravity_lower_bound: Ratio::new::<ratio>(DEFAULT_SPECIFIC_GRAVITY_LOWER_BOUND),
            boiling_point_lower_bound: ThermodynamicTemperature::new::<kelvin>(
                default_boiling_point_lower_bound_kelvin(),
            ),
        }
    }
}

/// Errors from bulk characterization.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CharacterizationError {
    /// No bulk property at all was supplied. Upstream throws "Invalid input
    /// data. You must provide at least one assay property."
    /// (`GenerateCompounds.vb:35-37`).
    #[error(
        "invalid assay: supply at least one of molar mass, specific gravity, or boiling point"
    )]
    NoAssayProperty,
    /// Fewer than two cuts were requested; the distribution is degenerate.
    #[error("characterization needs at least 2 cuts, got {requested}")]
    TooFewCuts {
        /// The number requested.
        requested: usize,
    },
    /// A cut's correlated constants came out non-physical.
    #[error(transparent)]
    PseudoComponent(#[from] PseudoComponentError),
}

/// The distributed cut fractions and mean property values from one property's
/// gamma distribution — upstream's `dMF` and `dMW`/`dSG`/`dTB`/`dV1`/`dV2`
/// arrays.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDistribution {
    /// Normalised cut fractions [-], one per cut, summing to 1 (upstream `dMF`
    /// after `Norm_dMF`).
    pub fractions: Vec<f64>,
    /// Mean property value in each cut, in the property's own unit (upstream
    /// `dMW` \[g/mol\], `dSG` \[-\], `dTB` \[K\], `dV1`/`dV2` \[m²/s\]).
    pub means: Vec<f64>,
}

/// Invert and integrate a three-parameter gamma distribution over `cut_count`
/// equal-probability cuts, returning both the cut fractions and the mean
/// property in each cut.
///
/// This is the shared body of `DistMW` (`GenerateCompounds.vb:489-520`),
/// `DistTB` (`:522-551`) and `DistSG` (`:567-596`), which differ only in their
/// `(P0, A, B)` parameters. The `Norm_dMF` normalisation (`:553-565`) is
/// applied before returning.
///
/// # Inputs
///
/// - `bulk_value` — the bulk (averaged) property `P`, in its own unit.
/// - `lower_bound` — `P0`, the distribution's lower bound, same unit.
/// - `shape_exponent` — `B` (1 for molecular weight, 1.5 for boiling point,
///   3 for specific gravity and viscosity).
/// - `scale_from_excess` — how `A` is formed from the normalised excess
///   `P* = (P − P0)/P0`. Upstream: identity for `M` (`:495`),
///   `(P*/0.689)^1.5` for `Tb` (`:528`), `(P*/0.619)³` for `SG` (`:571`).
/// - `cut_count` — number of cuts `n`.
/// - `grid_points` — how many of the 1001 grid points to interpolate over
///   (1000 for `M`/`SG`/`Tb`, 999 for the viscosities — see the module note).
///
/// # Valid range
///
/// Requires `bulk_value > lower_bound`; otherwise `A <= 0` and the whole grid
/// is `NaN`. Upstream has no guard; this port returns a distribution of `NaN`s
/// in that case rather than pretending to succeed, so the caller's
/// [`build_pseudo_component`] validation catches it.
#[must_use]
pub fn distribute_property(
    bulk_value: f64,
    lower_bound: f64,
    shape_exponent: f64,
    scale_from_excess: impl Fn(f64) -> f64,
    cut_count: usize,
    grid_points: usize,
) -> PropertyDistribution {
    let b = shape_exponent;
    let a = scale_from_excess((bulk_value - lower_bound) / lower_bound);
    let n = cut_count;

    let interpolant = build_inverse_grid(a, b, grid_points);

    // q(i) = B/A * d(i/n)^B, i = 0..=n  (`:504-506`)
    let q: Vec<f64> = (0..=n)
        .map(|i| b / a * interpolant.evaluate(i as f64 / n as f64).powf(b))
        .collect();

    let mut fractions = vec![0.0_f64; n];
    let mut means = vec![0.0_f64; n];
    for i in 1..=n {
        let val1 = interpolant.evaluate((i as f64 - 1.0) / n as f64);
        let val2 = interpolant.evaluate(i as f64 / n as f64);
        let fraction = (-b / a * val1.powf(b)).exp() - (-b / a * val2.powf(b)).exp();
        fractions[i - 1] = fraction;
        let excess = 1.0 / fraction
            * (a / b).powf(1.0 / b)
            * (incomplete_gamma_q(1.0 + 1.0 / b, q[i - 1])
                - incomplete_gamma_q(1.0 + 1.0 / b, q[i]));
        means[i - 1] = lower_bound * (1.0 + excess);
    }

    normalise_fractions(&mut fractions);
    PropertyDistribution { fractions, means }
}

/// The mean-property half of the distribution only, reusing cut fractions
/// computed by a previous [`distribute_property`] call.
///
/// This is `DistVISC1` (`GenerateCompounds.vb:598-624`) and `DistVISC2`
/// (`:626-652`), which deliberately do **not** recompute `dMF` — the viscosity
/// distribution rides on whatever cut split the molecular-weight /
/// specific-gravity / boiling-point distribution already established.
///
/// `fractions` must have `cut_count` entries.
#[must_use]
pub fn distribute_property_means_only(
    bulk_value: f64,
    lower_bound: f64,
    shape_exponent: f64,
    scale_from_excess: impl Fn(f64) -> f64,
    fractions: &[f64],
    grid_points: usize,
) -> Vec<f64> {
    let b = shape_exponent;
    let a = scale_from_excess((bulk_value - lower_bound) / lower_bound);
    let n = fractions.len();
    let interpolant = build_inverse_grid(a, b, grid_points);

    let q: Vec<f64> = (0..=n)
        .map(|i| b / a * interpolant.evaluate(i as f64 / n as f64).powf(b))
        .collect();

    (1..=n)
        .map(|i| {
            let excess = 1.0 / fractions[i - 1]
                * (a / b).powf(1.0 / b)
                * (incomplete_gamma_q(1.0 + 1.0 / b, q[i - 1])
                    - incomplete_gamma_q(1.0 + 1.0 / b, q[i]));
            lower_bound * (1.0 + excess)
        })
        .collect()
}

/// Build the Floater-Hormann interpolant of the inverted cumulative
/// distribution, `x = i/1000 ↦ d(i)` — `GenerateCompounds.vb:497-502`.
///
/// The `+0.001` offset inside the logarithm is upstream's (`:498`); it keeps
/// `d(0)` finite instead of `0^(1/B)`-degenerate.
fn build_inverse_grid(a: f64, b: f64, grid_points: usize) -> FloaterHormannInterpolant {
    let points: Vec<(f64, f64)> = (0..grid_points)
        .map(|i| {
            let x = i as f64 / DISTRIBUTION_GRID as f64;
            let d = (a / b * (1.0 / (1.0 - (i as f64 + 0.001) / DISTRIBUTION_GRID as f64)).ln())
                .powf(1.0 / b);
            (x, d)
        })
        .collect();
    FloaterHormannInterpolant::new(&points, FH_DEGREE)
        .expect("the fixed 999/1000-point grid always satisfies the interpolant's preconditions")
}

/// `Norm_dMF` (`GenerateCompounds.vb:553-565`): divide every cut fraction by
/// the sum of the **finite** fractions, so they sum to 1.
///
/// Upstream sums only finite, non-infinite entries but then divides *every*
/// entry (including any `NaN`) by that sum — reproduced.
fn normalise_fractions(fractions: &mut [f64]) {
    let sum: f64 = fractions.iter().filter(|v| v.is_finite()).sum();
    for f in fractions.iter_mut() {
        *f /= sum;
    }
}

/// Characterize a **bulk** assay into `cut_count` pseudo-components.
///
/// Ported from `GenerateCompounds.vb:30-487`. The upstream control flow, which
/// this reproduces exactly, is:
///
/// 1. Reject an assay with none of `M`, `SG`, `Tb` (`:35-37`).
/// 2. Distribute whichever properties were supplied, in the priority order
///    `M` → `SG` → `Tb` (`:70-200`), back-filling the rest from the
///    Riazi-Al-Sahhaf SCN relations
///    ([`property_methods::d15_riazi`], [`property_methods::tb_from_mw_riazi`])
///    or from the chosen [`MolecularWeightCorrelation`].
/// 3. Seed the viscosity distribution's lower bounds `V10`/`V20` by running
///    Abbott + Twu on the **first** cut (`:202-208`).
/// 4. Distribute or estimate the two viscosity curves (`:210-246`).
/// 5. Assemble each cut's full constant-property record
///    ([`build_pseudo_component`], `:260-377`).
///
/// The optional parameter-fitting tail (`:379-448`) is **not** run here; call
/// [`crate::petroleum::fitting::apply_parameter_fits`] afterwards if you want
/// it. Splitting it out keeps the expensive bubble-point solves opt-in.
///
/// # Valid range
///
/// The bulk property must exceed its distribution lower bound
/// (`M > M0`, `SG > SG0`, `Tb > Tb0`), and the resulting cuts must fall inside
/// the correlation ranges documented on [`build_pseudo_component`]
/// (`Tb` ≈ 300-850 K, `SG` ≈ 0.6-1.05, `M` ≈ 70-500 g/mol).
///
/// # Errors
///
/// [`CharacterizationError::NoAssayProperty`],
/// [`CharacterizationError::TooFewCuts`], or
/// [`CharacterizationError::PseudoComponent`] if a cut's constants come out
/// non-physical.
pub fn generate_compounds(
    options: &BulkCharacterizationOptions,
) -> Result<Vec<PseudoComponent>, CharacterizationError> {
    let n = options.cut_count;
    if n < 2 {
        return Err(CharacterizationError::TooFewCuts { requested: n });
    }
    let assay = &options.assay;
    let mw_bulk = assay.molar_mass.map_or(0.0, |m| m.get::<gram_per_mole>());
    let sg_bulk = assay.specific_gravity_60f.map_or(0.0, |s| s.get::<ratio>());
    let tb_bulk = assay
        .average_boiling_point
        .map_or(0.0, |t| t.get::<kelvin>());
    if mw_bulk == 0.0 && sg_bulk == 0.0 && tb_bulk == 0.0 {
        return Err(CharacterizationError::NoAssayProperty);
    }

    let mw0 = options.molar_mass_lower_bound.get::<gram_per_mole>();
    let sg0 = options.specific_gravity_lower_bound.get::<ratio>();
    let tb0 = options.boiling_point_lower_bound.get::<kelvin>();

    let mut fractions = vec![0.0_f64; n];
    let mut d_mw = vec![0.0_f64; n];
    let mut d_sg = vec![0.0_f64; n];
    let mut d_tb = vec![0.0_f64; n];

    // Upstream's three distribution closures (`:494-495`, `:527-528`, `:570-571`).
    let dist_mw = |cuts: usize| {
        distribute_property(mw_bulk, mw0, 1.0, |excess| excess, cuts, DISTRIBUTION_GRID)
    };
    let dist_sg = |cuts: usize| {
        distribute_property(
            sg_bulk,
            sg0,
            3.0,
            |excess| (excess / 0.619).powi(3),
            cuts,
            DISTRIBUTION_GRID,
        )
    };
    let dist_tb = |cuts: usize| {
        distribute_property(
            tb_bulk,
            tb0,
            1.5,
            |excess| (excess / 0.689).powf(1.5),
            cuts,
            DISTRIBUTION_GRID,
        )
    };

    let mw_correlation = |tb: f64, sg: f64| -> f64 {
        let tb_q = ThermodynamicTemperature::new::<kelvin>(tb);
        let sg_q = Ratio::new::<ratio>(sg);
        match options.correlations.molecular_weight {
            MolecularWeightCorrelation::Riazi1986 => property_methods::mw_riazi(tb_q, sg_q),
            MolecularWeightCorrelation::Winn1956 => property_methods::mw_winn(tb_q, sg_q),
            MolecularWeightCorrelation::LeeKesler1974 => {
                property_methods::mw_lee_kesler(tb_q, sg_q)
            }
        }
        .get::<gram_per_mole>()
    };
    let sg_from_mw =
        |m: f64| property_methods::d15_riazi(MolarMass::new::<gram_per_mole>(m)).get::<ratio>();
    let tb_from_mw = |m: f64| {
        property_methods::tb_from_mw_riazi(MolarMass::new::<gram_per_mole>(m)).get::<kelvin>()
    };
    let mw_from_sg =
        |s: f64| property_methods::mw_from_d15_riazi(Ratio::new::<ratio>(s)).get::<gram_per_mole>();

    // --- property distribution priority ladder (`:70-200`) -----------------
    if mw_bulk != 0.0 {
        let d = dist_mw(n);
        fractions.copy_from_slice(&d.fractions);
        d_mw.copy_from_slice(&d.means);

        if sg_bulk != 0.0 && tb_bulk != 0.0 {
            let s = dist_sg(n);
            fractions.copy_from_slice(&s.fractions);
            d_sg.copy_from_slice(&s.means);
            let t = dist_tb(n);
            fractions.copy_from_slice(&t.fractions);
            d_tb.copy_from_slice(&t.means);
        } else if sg_bulk == 0.0 && tb_bulk != 0.0 {
            for i in 0..n {
                d_sg[i] = sg_from_mw(d_mw[i]);
            }
            let t = dist_tb(n);
            fractions.copy_from_slice(&t.fractions);
            d_tb.copy_from_slice(&t.means);
        } else if sg_bulk == 0.0 && tb_bulk == 0.0 {
            for i in 0..n {
                d_sg[i] = sg_from_mw(d_mw[i]);
                d_tb[i] = tb_from_mw(d_mw[i]);
            }
        } else {
            // SG present, Tb absent (`:94-101`).
            let s = dist_sg(n);
            fractions.copy_from_slice(&s.fractions);
            d_sg.copy_from_slice(&s.means);
            for i in 0..n {
                d_tb[i] = tb_from_mw(d_mw[i]);
            }
        }
    } else if sg_bulk != 0.0 {
        let s = dist_sg(n);
        fractions.copy_from_slice(&s.fractions);
        d_sg.copy_from_slice(&s.means);

        if tb_bulk != 0.0 {
            // `:113-128` — Tb distributed, MW from the chosen correlation.
            let t = dist_tb(n);
            fractions.copy_from_slice(&t.fractions);
            d_tb.copy_from_slice(&t.means);
            for i in 0..n {
                d_mw[i] = mw_correlation(d_tb[i], d_sg[i]);
            }
        } else {
            // `:130-144` — invert the SCN relations, then re-apply the chosen
            // MW correlation on the resulting (Tb, SG) pair.
            for i in 0..n {
                d_mw[i] = mw_from_sg(d_sg[i]);
                d_tb[i] = tb_from_mw(d_mw[i]);
                d_mw[i] = mw_correlation(d_tb[i], d_sg[i]);
            }
        }
    } else {
        // Only Tb supplied (`:147-198`). NOTE: `d_sg` is still all zeros here,
        // exactly as upstream's zero-initialised `dSG` array is at `:179`. The
        // molecular-weight correlation is therefore evaluated at `SG = 0` and
        // collapses to zero. This is an upstream defect, reproduced verbatim;
        // see the module-level warning. The resulting cut is rejected downstream
        // by `build_pseudo_component` rather than silently emitted.
        let t = dist_tb(n);
        fractions.copy_from_slice(&t.fractions);
        d_tb.copy_from_slice(&t.means);
        for i in 0..n {
            d_mw[i] = mw_correlation(d_tb[i], d_sg[i]);
            d_sg[i] = sg_from_mw(d_mw[i]);
        }
    }

    // --- viscosity (`:202-251`) --------------------------------------------
    let mut t1 = assay
        .viscosity_temperature_1
        .map_or(0.0, |t| t.get::<kelvin>());
    let mut t2 = assay
        .viscosity_temperature_2
        .map_or(0.0, |t| t.get::<kelvin>());
    let v1_bulk = assay
        .kinematic_viscosity_1
        .map_or(0.0, |v| v.get::<square_meter_per_second>());
    let v2_bulk = assay
        .kinematic_viscosity_2
        .map_or(0.0, |v| v.get::<square_meter_per_second>());

    // Lower bounds V10/V20, seeded from the FIRST cut (`:204-208`).
    let first_tb = ThermodynamicTemperature::new::<kelvin>(d_tb[0]);
    let first_sg = Ratio::new::<ratio>(d_sg[0]);
    let v37 = property_methods::visc37_abbott(first_tb, first_sg);
    let v98 = property_methods::visc98_abbott(first_tb, first_sg);
    let abbott_t1 = ThermodynamicTemperature::new::<kelvin>(37.8 + 273.15);
    let abbott_t2 = ThermodynamicTemperature::new::<kelvin>(98.9 + 273.15);
    let v10 = property_methods::visc_twu(
        ThermodynamicTemperature::new::<kelvin>(t1),
        abbott_t1,
        abbott_t2,
        v37,
        v98,
    )
    .get::<square_meter_per_second>();
    let v20 = property_methods::visc_twu(
        ThermodynamicTemperature::new::<kelvin>(t2),
        abbott_t1,
        abbott_t2,
        v37,
        v98,
    )
    .get::<square_meter_per_second>();

    let visc_scale = |excess: f64| (excess / 0.619).powi(3);
    let mut d_v1 = vec![0.0_f64; n];
    let mut d_v2 = vec![0.0_f64; n];

    if v1_bulk != 0.0 && v2_bulk != 0.0 {
        d_v1 = distribute_property_means_only(
            v1_bulk,
            v10,
            3.0,
            visc_scale,
            &fractions,
            VISCOSITY_GRID,
        );
        d_v2 = distribute_property_means_only(
            v2_bulk,
            v20,
            3.0,
            visc_scale,
            &fractions,
            VISCOSITY_GRID,
        );
    } else if v1_bulk == 0.0 && v2_bulk != 0.0 {
        t1 = 37.8 + 273.15;
        for i in 0..n {
            d_v1[i] = property_methods::visc37_abbott(
                ThermodynamicTemperature::new::<kelvin>(d_tb[i]),
                Ratio::new::<ratio>(d_sg[i]),
            )
            .get::<square_meter_per_second>();
        }
        d_v2 = distribute_property_means_only(
            v2_bulk,
            v20,
            3.0,
            visc_scale,
            &fractions,
            VISCOSITY_GRID,
        );
    } else if v1_bulk != 0.0 && v2_bulk == 0.0 {
        t2 = 98.9 + 273.15;
        for i in 0..n {
            d_v2[i] = property_methods::visc98_abbott(
                ThermodynamicTemperature::new::<kelvin>(d_tb[i]),
                Ratio::new::<ratio>(d_sg[i]),
            )
            .get::<square_meter_per_second>();
        }
        d_v1 = distribute_property_means_only(
            v1_bulk,
            v10,
            3.0,
            visc_scale,
            &fractions,
            VISCOSITY_GRID,
        );
    } else {
        t1 = 37.8 + 273.15;
        t2 = 98.9 + 273.15;
        for i in 0..n {
            let tb_q = ThermodynamicTemperature::new::<kelvin>(d_tb[i]);
            let sg_q = Ratio::new::<ratio>(d_sg[i]);
            d_v1[i] = property_methods::visc37_abbott(tb_q, sg_q).get::<square_meter_per_second>();
            d_v2[i] = property_methods::visc98_abbott(tb_q, sg_q).get::<square_meter_per_second>();
        }
    }

    // --- assemble the compounds (`:260-377`) -------------------------------
    let t1_q = ThermodynamicTemperature::new::<kelvin>(t1);
    let t2_q = ThermodynamicTemperature::new::<kelvin>(t2);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut pc = build_pseudo_component(
            &options.prefix,
            i + 1,
            ThermodynamicTemperature::new::<kelvin>(d_tb[i]),
            Ratio::new::<ratio>(d_sg[i]),
            MolarMass::new::<gram_per_mole>(d_mw[i]),
            t1_q,
            t2_q,
            KinematicViscosity::new::<square_meter_per_second>(d_v1[i]),
            KinematicViscosity::new::<square_meter_per_second>(d_v2[i]),
            options.correlations,
        )?;
        pc.mole_fraction = Ratio::new::<ratio>(fractions[i]);
        out.push(pc);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light_crude_assay() -> BulkAssay {
        BulkAssay {
            molar_mass: Some(MolarMass::new::<gram_per_mole>(180.0)),
            specific_gravity_60f: Some(Ratio::new::<ratio>(0.82)),
            average_boiling_point: None,
            viscosity_temperature_1: None,
            viscosity_temperature_2: None,
            kinematic_viscosity_1: None,
            kinematic_viscosity_2: None,
        }
    }

    /// **Methodology.** Characterize a light-crude-style bulk assay
    /// (`M = 180 g/mol`, `SG = 0.82` — representative of a kerosene/gas-oil
    /// blend, inside every correlation's validity window) into 8 cuts. Check
    /// the three closure/ordering properties any physically meaningful
    /// pseudo-component set must have:
    ///
    /// 1. **Molar balance closes** — the cut mole fractions sum to exactly 1.
    /// 2. **Mass balance closes** — the mass fractions derived from
    ///    `w_i ∝ x_i M_i` also sum to 1.
    /// 3. **Monotonicity** — molecular weight, boiling point and critical
    ///    temperature all rise across the cuts (a distillation cut set is
    ///    ordered light-to-heavy by construction).
    ///
    /// Plus: every constant finite and positive, and every cut inside the
    /// Riazi-Daubert validity band (`M` 70-500 g/mol, `Tb` 300-850 K).
    ///
    /// **Results (2026-08-11, this port).** Reported by the assertion messages
    /// on failure. On the checked run all eight cuts satisfy every criterion:
    /// mole fractions sum to 1 within 1e-12, mass fractions likewise, and
    /// `M`, `Tb`, `Tc` are strictly increasing. Test passes.
    #[test]
    fn light_crude_bulk_assay_closes_and_is_monotone() {
        use super::super::pseudo_component::normalise_and_mass_fractions;
        use uom::si::pressure::pascal;

        let options = BulkCharacterizationOptions {
            prefix: "LightCrude".to_string(),
            cut_count: 8,
            assay: light_crude_assay(),
            ..Default::default()
        };
        let mut cuts = generate_compounds(&options).expect("a light crude is characterizable");
        assert_eq!(cuts.len(), 8);

        let mole_sum: f64 = cuts.iter().map(|c| c.mole_fraction.get::<ratio>()).sum();
        assert!(
            (mole_sum - 1.0).abs() < 1.0e-12,
            "mole fractions sum to {mole_sum}"
        );

        for w in cuts.windows(2) {
            assert!(
                w[1].component.molar_mass > w[0].component.molar_mass,
                "M not increasing: {} then {}",
                w[0].component.molar_mass,
                w[1].component.molar_mass
            );
            assert!(
                w[1].component.normal_boiling_point > w[0].component.normal_boiling_point,
                "Tb not increasing"
            );
            assert!(
                w[1].component.critical_temperature > w[0].component.critical_temperature,
                "Tc not increasing"
            );
        }

        for c in &cuts {
            let m_g = c.component.molar_mass * 1000.0;
            assert!(
                (70.0..500.0).contains(&m_g),
                "cut {} molecular weight {m_g} g/mol is outside the correlation range",
                c.component.name
            );
            assert!(
                (300.0..850.0).contains(&c.component.normal_boiling_point),
                "cut {} Tb {} K is outside the correlation range",
                c.component.name,
                c.component.normal_boiling_point
            );
            assert!(c.component.critical_pressure > 0.0);
            assert!(c.component.critical_volume > 0.0);
            assert!(c.component.acentric_factor.is_finite());
            assert!(c.specific_gravity.get::<ratio>() > 0.0);
            assert!(
                c.component.critical_pressure < 1.0e8,
                "Pc = {} Pa is implausible",
                uom::si::f64::Pressure::new::<pascal>(c.component.critical_pressure)
                    .get::<pascal>()
            );
        }

        let mass = normalise_and_mass_fractions(&mut cuts);
        let mass_sum: f64 = mass.iter().map(|w| w.get::<ratio>()).sum();
        assert!(
            (mass_sum - 1.0).abs() < 1.0e-12,
            "mass fractions sum to {mass_sum}"
        );
    }

    /// **Methodology.** The mole-weighted average molecular weight of the
    /// generated cuts must recover the bulk molecular weight that drove the
    /// distribution — the gamma distribution's own closure condition, and the
    /// sharpest single check that the incomplete-gamma cut means are right.
    /// Gate: within 10 %.
    ///
    /// **Results (2026-08-11, this port).** Reported in the assertion message
    /// on failure; on the checked run the recovered average is inside the 10 %
    /// gate. Note this is a genuinely tighter closure than the surrogate-based
    /// [`crate::petroleum::riazi`] path achieves, as expected: this path
    /// integrates the distribution exactly. Test passes.
    #[test]
    fn mole_weighted_molar_mass_recovers_the_bulk_value() {
        let options = BulkCharacterizationOptions {
            prefix: "LightCrude".to_string(),
            cut_count: 12,
            assay: light_crude_assay(),
            ..Default::default()
        };
        let cuts = generate_compounds(&options).expect("characterizable");
        let average: f64 = cuts
            .iter()
            .map(|c| c.mole_fraction.get::<ratio>() * c.component.molar_mass * 1000.0)
            .sum();
        assert!(
            ((average - 180.0) / 180.0).abs() < 0.10,
            "mole-weighted M = {average} g/mol versus bulk 180"
        );
    }

    /// **Methodology.** The usable branches of the priority ladder
    /// (`GenerateCompounds.vb:70-200`) must run and produce finite cuts:
    /// `M` only, `SG` only, `M + SG`, and `SG + Tb`. (The `Tb`-only branch is
    /// covered separately by
    /// `boiling_point_only_assay_reproduces_the_upstream_defect`, because
    /// upstream's version of it is broken — see the module warning.)
    ///
    /// **Results (2026-08-11, this port).** All four branches produce 6 finite,
    /// positive-constant cuts. Test passes.
    #[test]
    fn every_assay_property_combination_produces_cuts() {
        let base = BulkCharacterizationOptions {
            prefix: "Combo".to_string(),
            cut_count: 6,
            ..Default::default()
        };
        let cases: [(Option<f64>, Option<f64>, Option<f64>); 4] = [
            (Some(180.0), None, None),
            (None, Some(0.82), None),
            (Some(180.0), Some(0.82), None),
            (None, Some(0.82), Some(500.0)),
        ];
        for (mw, sg, tb) in cases {
            let options = BulkCharacterizationOptions {
                assay: BulkAssay {
                    molar_mass: mw.map(MolarMass::new::<gram_per_mole>),
                    specific_gravity_60f: sg.map(Ratio::new::<ratio>),
                    average_boiling_point: tb.map(ThermodynamicTemperature::new::<kelvin>),
                    ..Default::default()
                },
                ..base.clone()
            };
            let cuts = generate_compounds(&options)
                .unwrap_or_else(|e| panic!("case ({mw:?}, {sg:?}, {tb:?}) failed: {e}"));
            assert_eq!(cuts.len(), 6);
            for c in &cuts {
                assert!(
                    c.component.molar_mass.is_finite() && c.component.molar_mass > 0.0,
                    "case ({mw:?}, {sg:?}, {tb:?}) produced {c:?}"
                );
                assert!(c.component.critical_temperature.is_finite());
            }
        }
    }

    /// **Methodology.** Pin the upstream defect in the **boiling-point-only**
    /// branch (documented in the module warning): because `dSG` is read before
    /// it is written (`GenerateCompounds.vb:179`), the molecular-weight
    /// correlation is evaluated at `SG = 0` and collapses to zero. This port
    /// reproduces that and then rejects the cut instead of emitting it.
    ///
    /// **Results (2026-08-11, this port).** An assay with only
    /// `Tb = 500 K` returns
    /// `NonPhysical { property: "molar_mass", value: 0.0 }` for the first cut
    /// (`Combo_NBP_116`). Reproduced exactly, and reported rather than hidden.
    /// Test passes.
    #[test]
    fn boiling_point_only_assay_reproduces_the_upstream_defect() {
        let options = BulkCharacterizationOptions {
            prefix: "Combo".to_string(),
            cut_count: 6,
            assay: BulkAssay {
                average_boiling_point: Some(ThermodynamicTemperature::new::<kelvin>(500.0)),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = generate_compounds(&options)
            .expect_err("upstream's boiling-point-only branch cannot produce a valid compound");
        match err {
            CharacterizationError::PseudoComponent(PseudoComponentError::NonPhysical {
                property,
                value,
                ..
            }) => {
                assert_eq!(property, "molar_mass");
                assert_eq!(value, 0.0);
            }
            other => panic!("expected a non-physical molar mass, got {other:?}"),
        }
    }

    /// **Methodology.** Bad requests must be rejected: an empty assay, and
    /// fewer than two cuts.
    ///
    /// **Results (2026-08-11, this port).** An assay with no properties returns
    /// `NoAssayProperty`; `cut_count = 1` returns `TooFewCuts`. Test passes.
    #[test]
    fn bad_requests_are_rejected() {
        let empty = BulkCharacterizationOptions::default();
        assert!(matches!(
            generate_compounds(&empty),
            Err(CharacterizationError::NoAssayProperty)
        ));
        let one_cut = BulkCharacterizationOptions {
            cut_count: 1,
            assay: light_crude_assay(),
            ..Default::default()
        };
        assert!(matches!(
            generate_compounds(&one_cut),
            Err(CharacterizationError::TooFewCuts { requested: 1 })
        ));
    }
}
