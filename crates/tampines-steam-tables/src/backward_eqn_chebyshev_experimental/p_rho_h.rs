//! Experimental explicit pressure-from-density-and-enthalpy correlations,
//! `p(rho,h)`, spanning IAPWS-IF97 Regions 1 to 5.
//!
//! # Status — NOT an IAPWS release
//!
//! IAPWS-IF97 publishes no `(rho,h)` backward equations. These are in-house
//! fits against this crate's own forward equations, useful as an inverse-flash
//! building block for solvers that carry density and enthalpy as their state
//! variables (mass- and energy-conserving finite-volume schemes, for example).
//! Do not cite them as IAPWS values.
//!
//! # Structure: two independent partitions
//!
//! Getting a pressure out of `(rho, h)` needs two separate decisions, and they
//! must not be confused:
//!
//! 1. **Which thermodynamic region** the state is in (IF97 Region 1 to 5).
//!    This is a physical question, answered by [`RhoHRegion`].
//! 2. **Which fitted polynomial surface** to evaluate ([`RhoHFitPiece`]).
//!    This is a purely numerical question: each region's fit is split into
//!    several local surfaces so that no single polynomial has to cover too
//!    wide a range. **These fit-piece boundaries are not thermodynamic
//!    boundaries** — they carry no physical meaning and must never be quoted
//!    as saturation, quality or region boundaries.
//!
//! # Choosing a region: prefer the caller's own knowledge
//!
//! If the calling code already knows the region — which it usually does, since
//! it generally knows pressure or temperature from the surrounding solve —
//! pass it in via [`p_rho_h_in_region`] and the classifier is bypassed
//! entirely. That is the accurate path, and it is the recommended one.
//!
//! [`rho_h_region_candidate`] exists for the genuinely `(rho,h)`-only case,
//! where no pressure or temperature is available yet. **It is a statistical
//! surrogate, not a boundary equation**, with roughly 97.3% overall accuracy on
//! its derivation sample (Region 1 ~98.5%, Region 2 ~98.7%, Region 3 ~99.8%,
//! **Region 4 ~91.6%**, Region 5 ~96.3%). A misclassification is not a small
//! numerical error — it selects a polynomial fitted for different physics and
//! returns a badly wrong pressure. Region 4 is both the weakest case and the
//! one a two-phase solver spends most of its time in.
//!
//! [`rho_h_region_scores`] returns the raw discriminant scores, whose top-two
//! margin is a cheap ambiguity signal: a narrow margin is the natural trigger
//! for verifying the choice against reconstructed pressure and this crate's
//! IAPWS saturation machinery.
//!
//! # Fit domains are hard edges
//!
//! As elsewhere in this module tree, each surface is a Chebyshev polynomial
//! that diverges outside the interval it was fitted on, and nothing here
//! clamps or validates its input.
//!
//! # Verification results, and two real limitations
//!
//! Measured 2026-08-31 over 3600 single-phase states generated from the
//! crate's own forward equations, with the region supplied (classifier not in
//! the loop). Relative error in the recovered pressure:
//!
//! | Region | n | median | 90th pct | max |
//! |---|---|---|---|---|
//! | 1 | 299 | 5.62e-4 | 7.58e-2 | **3.19** |
//! | 2 | 1185 | 9.79e-6 | 2.26e-5 | 1.55e-4 |
//! | 3 | 16 | 1.08e-4 | 3.25e-4 | 3.69e-4 |
//! | 5 | 2100 | 2.15e-5 | 5.25e-5 | 1.31e-3 |
//!
//! **Limitation 1 — do not use this in subcooled liquid.** Region 1 is not
//! merely less accurate, it is *intrinsically ill-conditioned*: liquid water
//! is very nearly incompressible, so along an isotherm density barely moves
//! while pressure changes by orders of magnitude. Inverting that mapping
//! amplifies any error enormously — the worst state measured recovers a
//! pressure a factor of ~4 out (1.0e-3 MPa read back as 4.2e-3 MPa at
//! T = 280 K). This is a property of the state variables, not a defect in the
//! fit, and no better fit can remove it. In subcooled liquid, carry pressure
//! as a state variable or use an equation of state parameterised the other
//! way.
//!
//! **Limitation 2 — Region 4 is untested.** The Region 4 surfaces are present
//! but no verification test covers them, because two-phase states are not
//! reachable through the single-phase `(T,p)` flash used to generate the test
//! set. Region 4 is also the classifier's weakest case. Treat the two-phase
//! path here as unverified.
//!
//! The classifier itself was measured on the same sweep at **97.5% agreement**
//! with the crate's forward dispatcher over single-phase states (Region 1
//! 98.3%, Region 2 99.3%, Region 3 100%, Region 5 96.4%), consistent with its
//! self-reported ~97.3%. Region 4 is absent from that measurement.

use uom::si::{
    available_energy::kilojoule_per_kilogram, f64::*, mass_density::kilogram_per_cubic_meter,
    pressure::megapascal,
};

use super::chebyshev::{cheb2_sparse, cheb_basis, scale};

/// IAPWS-IF97 thermodynamic region of a `(rho,h)` state.
///
/// This is a physical classification. It is distinct from [`RhoHFitPiece`],
/// which only selects a fitted polynomial surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RhoHRegion {
    /// Region 1 — subcooled liquid.
    Region1,
    /// Region 2 — vapour / superheated steam.
    Region2,
    /// Region 3 — near-critical single phase and supercritical fluid.
    Region3,
    /// Region 4 — vapour-liquid equilibrium (the saturation line).
    Region4,
    /// Region 5 — ultra-high-temperature steam.
    Region5,
}

/// Identifies one fitted local polynomial surface.
///
/// **These are numerical fit partitions, not thermodynamic boundaries.** The
/// suffix indexes the local surface within a region's fit; it carries no
/// physical meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum RhoHFitPiece {
    R1_0,
    R1_1,
    R1_2,
    R2_0,
    R2_1,
    R2_2,
    R2_3,
    R2_4,
    R2_5,
    R3_0,
    R3_1,
    R3_2,
    R4_0,
    R4_1,
    R4_2,
    R4_3,
    R4_4,
    R4_5,
    R4_6,
    R4_7,
    R4_8,
    R4_9,
    R4_10,
    R5,
}

// ---------------------------------------------------------------------------
// Fit-piece partition boundaries.
//
// These select which fitted polynomial is numerically appropriate. They are
// NOT IAPWS thermodynamic region boundaries.
// ---------------------------------------------------------------------------

/// Region 1 fit-piece enthalpy split, in kJ/kg. Numerical, not thermodynamic.
const R1_H_01: f64 = 269.4;
/// Region 1 fit-piece enthalpy split, in kJ/kg. Numerical, not thermodynamic.
const R1_H_12: f64 = 672.0;
/// Region 2 fit-piece density splits, in kg/m3. Numerical, not thermodynamic.
const R2_RHO: [f64; 5] = [0.05, 1.5, 10.0, 40.0, 100.0];
/// Region 3 fit-piece density split, in kg/m3. Numerical, not thermodynamic.
const R3_RHO_01: f64 = 355.563_378_43;
/// Region 3 fit-piece density split, in kg/m3. Numerical, not thermodynamic.
const R3_RHO_12: f64 = 479.770_530_49;
/// Region 4 fit-piece density splits, in kg/m3. Numerical, not thermodynamic.
const R4_RHO: [f64; 10] = [0.1, 1.0, 5.0, 20.0, 60.0, 120.0, 220.0, 280.0, 340.0, 400.0];

/// Selects the Region 1 fit surface from specific enthalpy in kJ/kg.
#[inline]
pub fn region_1_piece_from_h(h_kj_kg: f64) -> RhoHFitPiece {
    if h_kj_kg < R1_H_01 {
        RhoHFitPiece::R1_0
    } else if h_kj_kg < R1_H_12 {
        RhoHFitPiece::R1_1
    } else {
        RhoHFitPiece::R1_2
    }
}

/// Selects the Region 2 fit surface from density in kg/m3.
#[inline]
pub fn region_2_piece_from_rho(rho_kg_m3: f64) -> RhoHFitPiece {
    match rho_kg_m3 {
        r if r < R2_RHO[0] => RhoHFitPiece::R2_0,
        r if r < R2_RHO[1] => RhoHFitPiece::R2_1,
        r if r < R2_RHO[2] => RhoHFitPiece::R2_2,
        r if r < R2_RHO[3] => RhoHFitPiece::R2_3,
        r if r < R2_RHO[4] => RhoHFitPiece::R2_4,
        _ => RhoHFitPiece::R2_5,
    }
}

/// Selects the Region 3 fit surface from density in kg/m3.
#[inline]
pub fn region_3_piece_from_rho(rho_kg_m3: f64) -> RhoHFitPiece {
    if rho_kg_m3 < R3_RHO_01 {
        RhoHFitPiece::R3_0
    } else if rho_kg_m3 < R3_RHO_12 {
        RhoHFitPiece::R3_1
    } else {
        RhoHFitPiece::R3_2
    }
}

/// Selects the Region 4 fit surface from density in kg/m3.
#[inline]
pub fn region_4_piece_from_rho(rho_kg_m3: f64) -> RhoHFitPiece {
    match rho_kg_m3 {
        r if r < R4_RHO[0] => RhoHFitPiece::R4_0,
        r if r < R4_RHO[1] => RhoHFitPiece::R4_1,
        r if r < R4_RHO[2] => RhoHFitPiece::R4_2,
        r if r < R4_RHO[3] => RhoHFitPiece::R4_3,
        r if r < R4_RHO[4] => RhoHFitPiece::R4_4,
        r if r < R4_RHO[5] => RhoHFitPiece::R4_5,
        r if r < R4_RHO[6] => RhoHFitPiece::R4_6,
        r if r < R4_RHO[7] => RhoHFitPiece::R4_7,
        r if r < R4_RHO[8] => RhoHFitPiece::R4_8,
        r if r < R4_RHO[9] => RhoHFitPiece::R4_9,
        _ => RhoHFitPiece::R4_10,
    }
}

/// Selects the fit surface for a state whose thermodynamic region is known.
///
/// This is the accurate path: it uses the caller's own region knowledge rather
/// than the statistical classifier.
pub fn fit_piece_for_region(region: RhoHRegion, rho_kg_m3: f64, h_kj_kg: f64) -> RhoHFitPiece {
    match region {
        RhoHRegion::Region1 => region_1_piece_from_h(h_kj_kg),
        RhoHRegion::Region2 => region_2_piece_from_rho(rho_kg_m3),
        RhoHRegion::Region3 => region_3_piece_from_rho(rho_kg_m3),
        RhoHRegion::Region4 => region_4_piece_from_rho(rho_kg_m3),
        RhoHRegion::Region5 => RhoHFitPiece::R5,
    }
}

// ---------------------------------------------------------------------------
// Statistical (rho,h) region classifier.
// ---------------------------------------------------------------------------

const LOG_RHO_MIN: f64 = -3.16645808175136967e+00;
const LOG_RHO_MAX: f64 = 3.01895419062749415e+00;
const H_MIN: f64 = 7.73368540962804524e-02;
const H_MAX: f64 = 7.37697216509577356e+03;
const TERMS: [(usize, usize); 45] = [
    (0, 0),
    (0, 1),
    (0, 2),
    (0, 3),
    (0, 4),
    (0, 5),
    (0, 6),
    (0, 7),
    (0, 8),
    (1, 0),
    (1, 1),
    (1, 2),
    (1, 3),
    (1, 4),
    (1, 5),
    (1, 6),
    (1, 7),
    (2, 0),
    (2, 1),
    (2, 2),
    (2, 3),
    (2, 4),
    (2, 5),
    (2, 6),
    (3, 0),
    (3, 1),
    (3, 2),
    (3, 3),
    (3, 4),
    (3, 5),
    (4, 0),
    (4, 1),
    (4, 2),
    (4, 3),
    (4, 4),
    (5, 0),
    (5, 1),
    (5, 2),
    (5, 3),
    (6, 0),
    (6, 1),
    (6, 2),
    (7, 0),
    (7, 1),
    (8, 0),
];
const SCORE: [[f64; 45]; 5] = [
    [
        -1.89854695810920560e+00,
        -1.50464405603243057e+00,
        -4.30662916280487307e-01,
        -2.63180397461878990e-01,
        6.22060706750176343e-02,
        -5.97140742665595373e-02,
        1.09301808997743638e-01,
        -6.03409607778944493e-02,
        4.80167183847403437e-02,
        -1.33596541484711073e+00,
        -3.35396842859674083e+00,
        -9.11534675581281983e-01,
        -1.35015015191718130e-01,
        -1.96725309562283762e-01,
        9.48926038751191775e-02,
        7.27366316425824971e-02,
        -1.13185863279330418e-01,
        -1.63263574000725087e+00,
        -2.22155572965708448e+00,
        -9.18745054629138580e-01,
        -1.88985519244930428e-01,
        -1.02647213422116260e-01,
        5.42677184710498237e-02,
        3.75444881053384680e-02,
        -9.36914950006273228e-01,
        -2.18463449020448630e+00,
        -6.33604277643688873e-01,
        8.40071472017424187e-02,
        -2.12293042047274838e-01,
        1.49677661926459482e-01,
        -9.51811970420764597e-01,
        -1.14769602145588268e+00,
        -4.97102180056374632e-01,
        -1.01798414839489731e-02,
        -1.11103417830905979e-01,
        -3.27481936919757022e-01,
        -1.07766622138585944e+00,
        -9.22030924931186019e-02,
        4.65420167263172133e-03,
        -3.04219096462485350e-01,
        -3.76151559514608824e-01,
        -7.59305802463222240e-02,
        -5.26382755959930379e-02,
        -2.68105084813112071e-01,
        -5.17685955448073387e-02,
    ],
    [
        -1.21232869854114189e+00,
        2.07416184959155703e-01,
        -1.03921456589460925e+00,
        -8.53509372077943668e-02,
        6.51456961031836812e-01,
        -3.85697288609830991e-01,
        -9.12107504784237799e-02,
        1.62245182309449981e-01,
        3.14441158060278669e-01,
        4.15259775669048370e-01,
        -2.11837696354569704e+00,
        4.05078153047090328e-01,
        -6.12144345655032396e-01,
        -4.93628190688230550e-01,
        3.11008075081590063e-01,
        -2.30983360198396243e-01,
        -8.89712377616840543e-03,
        -1.06247269114930187e+00,
        7.46937353109116353e-01,
        -1.08788750856101046e+00,
        -1.47171250023890615e-01,
        -5.52743464932810846e-02,
        -1.13560001882946113e-01,
        1.78814414523915277e-01,
        3.35979583830225303e-01,
        -1.38850918167226456e+00,
        2.18183732838122812e-01,
        -3.05767266179706509e-01,
        -2.15882309519219578e-01,
        2.58114933972412169e-02,
        -6.13108374689262936e-01,
        5.67395331010999926e-01,
        -7.44670146821405088e-01,
        1.10215573710405093e-01,
        -1.37091352218377344e-01,
        1.23974034421248769e-01,
        -4.73971953086223696e-01,
        1.08204238111013207e-01,
        -1.06391441670672091e-01,
        -1.02231126331373781e-01,
        8.85632505730940961e-02,
        -1.47634938574163321e-01,
        2.04922995856926700e-02,
        -4.41048050254172902e-02,
        2.49877292302877405e-02,
    ],
    [
        -1.21497728585515175e+00,
        -5.87792891526424555e+00,
        2.01872130051578313e+00,
        6.23807091262321256e-01,
        4.56874705754056032e-01,
        -1.87658700609365625e-01,
        -1.12206755705644715e-01,
        1.25321114564305780e-01,
        -1.04933285866551998e-01,
        -7.66128275703697348e+00,
        1.18633401430568952e+00,
        -3.59816989269345822e+00,
        4.01592081504205822e+00,
        5.67909768484041955e-01,
        -5.76344826435133850e-01,
        -3.66647947392630957e-02,
        2.26283501927759673e-01,
        -2.90508971864021970e-01,
        -9.78766509282215580e+00,
        3.54701537391903621e+00,
        7.64348855094987623e-01,
        8.79963609242749634e-01,
        -4.40729932976088090e-01,
        5.04058002461813884e-02,
        -5.06120327085422428e+00,
        5.55175403515888455e-01,
        -2.12634937669293178e+00,
        2.32610877789719739e+00,
        3.32767132057903370e-01,
        -3.60072055443423411e-01,
        -3.32461966465485204e-01,
        -4.92381885446420053e+00,
        1.77768362777514399e+00,
        4.19115849342272972e-01,
        2.20359263941911715e-01,
        -2.06619448250692495e+00,
        1.10795299144601900e-01,
        -6.93150869783602408e-01,
        7.55154597367694858e-01,
        -1.74262518705313257e-01,
        -1.38878664978905508e+00,
        5.52552348603488230e-01,
        -3.52859683099084664e-01,
        -1.25989156984407047e-01,
        -6.23914643255994161e-02,
    ],
    [
        7.10306539402623915e+00,
        5.96050520819727314e+00,
        6.05469658792053522e+00,
        5.47608655406601863e-01,
        -8.32576663022651875e-01,
        -1.31175069513780884e-01,
        3.27124992256248581e-01,
        -1.12719184024454078e-01,
        -1.53364024958764916e-01,
        8.22311422715595164e+00,
        2.28022626947368678e+01,
        6.60493802538330321e+00,
        1.17669934830195144e+00,
        -5.27571833580399052e-01,
        -2.29521444088413989e-01,
        1.84678442553975142e-01,
        1.44890703800874515e-01,
        1.17903230002992885e+01,
        1.16385420654157361e+01,
        9.12751503027803857e+00,
        5.54325606224443823e-01,
        -9.66347016587449814e-01,
        4.93344926385301044e-02,
        1.42455663129096993e-01,
        4.96776097322326038e+00,
        1.52825895646216647e+01,
        3.88023951389370403e+00,
        7.64923051387153663e-01,
        -4.38583095693256431e-01,
        4.20695438410293929e-02,
        6.16265193368690323e+00,
        5.58212182268148371e+00,
        4.50456942612732369e+00,
        2.42309478076708246e-01,
        -4.57158499484702163e-01,
        1.71062884751811639e+00,
        6.19908571595942171e+00,
        1.19536807245663335e+00,
        2.42453984172899345e-01,
        1.80555958362931812e+00,
        1.24214506620369192e+00,
        1.15279501352303959e+00,
        1.55888408564812037e-01,
        1.21577359442156463e+00,
        1.15787349722841035e-01,
    ],
    [
        -5.74812132046250146e-01,
        6.50499626704199363e-01,
        -8.89368614316344563e-02,
        -4.64660755251877444e-01,
        -8.53968198747233516e-03,
        8.39580666213992771e-02,
        1.79779453334554418e-01,
        -2.45680573765918703e-01,
        -1.38618045389089201e-01,
        -6.54113550429606505e-01,
        -1.46954593183611926e+00,
        -3.73265825458822142e-01,
        1.36133346049945952e-01,
        5.23710040002022051e-02,
        1.06076912961294401e-01,
        -1.35025487529199961e-01,
        7.01419713248250809e-02,
        -9.29601620498440306e-01,
        -7.16161768409349531e-01,
        -5.19834366522850089e-01,
        1.02092710729843439e-01,
        8.19776213696763734e-02,
        2.82242337658528616e-02,
        -5.22729206374909286e-02,
        -3.71080627223881410e-01,
        -1.08182700293276723e+00,
        -1.49894227745379888e-01,
        4.46226107512724471e-02,
        6.03471556489763306e-02,
        -1.70186874167139596e-02,
        -5.11038509279509223e-01,
        -3.43466462453783428e-01,
        -2.41462118767558714e-01,
        3.68966982150657624e-02,
        3.37392883432318172e-02,
        -1.32824564597115197e-01,
        -4.60207216108921735e-01,
        -2.84325747208923410e-02,
        1.34341971190196398e-02,
        -1.58436973076537119e-01,
        -8.49505275932356529e-02,
        -6.00919124819758108e-02,
        -1.71231222362362677e-02,
        -9.06327554501952792e-02,
        -1.66583832941136385e-02,
    ],
];

/// Candidate IF97 region for a `(rho,h)` state, from the statistical
/// classifier.
///
/// `rho_kg_m3` is density in kg/m3 and `h_kj_kg` specific enthalpy in kJ/kg.
///
/// **This is a numerical surrogate, not a boundary equation.** Accuracy on the
/// derivation sample is roughly 97.3% overall, and only ~91.6% for Region 4.
/// Prefer [`fit_piece_for_region`] with a region the caller already knows;
/// reach for this only when nothing but `(rho,h)` is available. Check
/// [`rho_h_region_scores`] for the top-two margin when the answer matters.
///
/// # Panics
///
/// Panics if `rho_kg_m3` is not strictly positive, since the classifier works
/// in `log10(rho)`.
pub fn rho_h_region_candidate(rho_kg_m3: f64, h_kj_kg: f64) -> RhoHRegion {
    let scores = rho_h_region_scores(rho_kg_m3, h_kj_kg);
    let mut best = 0;
    for r in 1..5 {
        if scores[r] > scores[best] {
            best = r;
        }
    }
    [
        RhoHRegion::Region1,
        RhoHRegion::Region2,
        RhoHRegion::Region3,
        RhoHRegion::Region4,
        RhoHRegion::Region5,
    ][best]
}

/// Raw discriminant scores for each region, ordered Region 1 to Region 5.
///
/// The largest score is the classifier's choice. The margin between the two
/// largest is an ambiguity signal: when it is small, verify the region against
/// reconstructed pressure and this crate's IAPWS saturation machinery rather
/// than trusting the classification.
///
/// # Panics
///
/// Panics if `rho_kg_m3` is not strictly positive.
pub fn rho_h_region_scores(rho_kg_m3: f64, h_kj_kg: f64) -> [f64; 5] {
    assert!(
        rho_kg_m3 > 0.0,
        "the (rho,h) region classifier needs a strictly positive density, got {rho_kg_m3} kg/m3"
    );
    let x = scale(rho_kg_m3.log10(), LOG_RHO_MIN, LOG_RHO_MAX);
    let y = scale(h_kj_kg, H_MIN, H_MAX);
    let tx = cheb_basis::<9>(x);
    let ty = cheb_basis::<9>(y);
    let mut scores = [0.0; 5];
    for (r, score) in scores.iter_mut().enumerate() {
        for (k, &(i, j)) in TERMS.iter().enumerate() {
            *score += SCORE[r][k] * tx[i] * ty[j];
        }
    }
    scores
}

// ---------------------------------------------------------------------------
// Public evaluation entry points.
// ---------------------------------------------------------------------------

/// Evaluates the fitted `p(rho,h)` surface identified by `piece`.
///
/// `rho_kg_m3` is density in kg/m3 and `h_kj_kg` specific enthalpy in kJ/kg;
/// the return value is pressure in MPa. The caller is responsible for having
/// selected an appropriate `piece` — see [`fit_piece_for_region`].
pub fn p_rho_h_piece_explicit(piece: RhoHFitPiece, rho_kg_m3: f64, h_kj_kg: f64) -> f64 {
    let (rho, h) = (rho_kg_m3, h_kj_kg);
    match piece {
        RhoHFitPiece::R1_0 => piece_r1_0(rho, h),
        RhoHFitPiece::R1_1 => piece_r1_1(rho, h),
        RhoHFitPiece::R1_2 => piece_r1_2(rho, h),
        RhoHFitPiece::R2_0 => piece_r2_0(rho, h),
        RhoHFitPiece::R2_1 => piece_r2_1(rho, h),
        RhoHFitPiece::R2_2 => piece_r2_2(rho, h),
        RhoHFitPiece::R2_3 => piece_r2_3(rho, h),
        RhoHFitPiece::R2_4 => piece_r2_4(rho, h),
        RhoHFitPiece::R2_5 => piece_r2_5(rho, h),
        RhoHFitPiece::R3_0 => piece_r3_0(rho, h),
        RhoHFitPiece::R3_1 => piece_r3_1(rho, h),
        RhoHFitPiece::R3_2 => piece_r3_2(rho, h),
        RhoHFitPiece::R4_0 => piece_r4_0(rho, h),
        RhoHFitPiece::R4_1 => piece_r4_1(rho, h),
        RhoHFitPiece::R4_2 => piece_r4_2(rho, h),
        RhoHFitPiece::R4_3 => piece_r4_3(rho, h),
        RhoHFitPiece::R4_4 => piece_r4_4(rho, h),
        RhoHFitPiece::R4_5 => piece_r4_5(rho, h),
        RhoHFitPiece::R4_6 => piece_r4_6(rho, h),
        RhoHFitPiece::R4_7 => piece_r4_7(rho, h),
        RhoHFitPiece::R4_8 => piece_r4_8(rho, h),
        RhoHFitPiece::R4_9 => piece_r4_9(rho, h),
        RhoHFitPiece::R4_10 => piece_r4_10(rho, h),
        RhoHFitPiece::R5 => piece_r5(rho, h),
    }
}

/// Pressure in MPa from density in kg/m3 and specific enthalpy in kJ/kg, for a
/// state whose thermodynamic region the caller already knows.
///
/// This is the accurate path — it does not use the statistical classifier.
pub fn p_rho_h_in_region_explicit(region: RhoHRegion, rho_kg_m3: f64, h_kj_kg: f64) -> f64 {
    let piece = fit_piece_for_region(region, rho_kg_m3, h_kj_kg);
    p_rho_h_piece_explicit(piece, rho_kg_m3, h_kj_kg)
}

/// Pressure from density and specific enthalpy, for a state whose
/// thermodynamic region the caller already knows.
///
/// This is the dimensioned entry point and the recommended one: supplying the
/// region bypasses the statistical classifier entirely.
pub fn p_rho_h_in_region(region: RhoHRegion, rho: MassDensity, h: AvailableEnergy) -> Pressure {
    let rho_kg_m3 = rho.get::<kilogram_per_cubic_meter>();
    let h_kj_kg = h.get::<kilojoule_per_kilogram>();
    Pressure::new::<megapascal>(p_rho_h_in_region_explicit(region, rho_kg_m3, h_kj_kg))
}

/// Pressure from density and specific enthalpy, classifying the region with
/// the statistical surrogate.
///
/// Use this only when nothing but `(rho,h)` is available. Where the region is
/// known, [`p_rho_h_in_region`] is both faster and accurate. See the module
/// documentation for the classifier's error rates — a misclassification
/// produces a badly wrong pressure, not a slightly wrong one.
pub fn p_rho_h_classified(rho: MassDensity, h: AvailableEnergy) -> Pressure {
    let rho_kg_m3 = rho.get::<kilogram_per_cubic_meter>();
    let h_kj_kg = h.get::<kilojoule_per_kilogram>();
    let region = rho_h_region_candidate(rho_kg_m3, h_kj_kg);
    Pressure::new::<megapascal>(p_rho_h_in_region_explicit(region, rho_kg_m3, h_kj_kg))
}

// ---------------------------------------------------------------------------
// Fitted coefficient tables and local surfaces.
//
// Private: callers go through the dispatch functions above. The suffixes index
// numerical fit partitions and carry no physical meaning.
// ---------------------------------------------------------------------------

const R1_0_X0: f64 = 9.80903518920910869e+02;
const R1_0_X1: f64 = 1.04410717400600674e+03;
const R1_0_H0: f64 = -2.26870883194624819e-02;
const R1_0_H1: f64 = 2.69396478385778096e+02;
const R1_0: [(usize, usize, f64); 45] = [
    (0, 0, 4.08874957461074402e+01),
    (0, 1, 1.91219651790009095e+01),
    (0, 2, 4.25635531917665588e+00),
    (0, 3, -4.96821436767312774e-01),
    (0, 4, 7.77226374109968449e-02),
    (0, 5, -1.50700631158301856e-02),
    (0, 6, 3.09574738764891409e-03),
    (0, 7, -5.73327963383231474e-04),
    (0, 8, 9.84947476576346652e-05),
    (1, 0, 6.51035750381658147e+01),
    (1, 1, 3.89713316847691726e-01),
    (1, 2, 1.61550284917412607e-01),
    (1, 3, -2.28709080576715608e-02),
    (1, 4, 1.81960865892564216e-02),
    (1, 5, -7.85116657651758684e-03),
    (1, 6, 2.09577513304500754e-03),
    (1, 7, -3.56222137747291498e-04),
    (2, 0, 1.31712848488417400e+00),
    (2, 1, 6.55254958521531594e-01),
    (2, 2, -1.25774323581861747e-01),
    (2, 3, 1.45543869118854134e-02),
    (2, 4, 9.45064950485507564e-05),
    (2, 5, -8.75447362327837989e-04),
    (2, 6, 2.02525372273081370e-04),
    (3, 0, -7.09123841981286612e-02),
    (3, 1, 6.06285319761629624e-02),
    (3, 2, -1.82462890985578829e-02),
    (3, 3, 4.03593251678821541e-03),
    (3, 4, -1.03563287401813140e-03),
    (3, 5, 1.37025199965895035e-04),
    (4, 0, -8.16400197255900094e-03),
    (4, 1, 6.48367569714698461e-03),
    (4, 2, -4.56653278402222376e-03),
    (4, 3, 1.46562266640507736e-03),
    (4, 4, -2.73980206307678409e-04),
    (5, 0, -1.99210269445930510e-03),
    (5, 1, 2.17559683925073197e-03),
    (5, 2, -9.66405571946503980e-04),
    (5, 3, 2.59794030763851305e-04),
    (6, 0, -2.87395964736880628e-04),
    (6, 1, 4.35280617762009699e-04),
    (6, 2, -9.38588344457021737e-05),
    (7, 0, -3.16560893647547216e-05),
    (7, 1, 3.27605822783073693e-05),
    (8, 0, -9.17997637078771780e-07),
];
fn piece_r1_0(rho: f64, h: f64) -> f64 {
    let x = scale(rho, R1_0_X0, R1_0_X1);
    let y = scale(h, R1_0_H0, R1_0_H1);
    cheb2_sparse(x, y, &R1_0)
}
const R1_1_X0: f64 = 9.08431906770890691e+02;
const R1_1_X1: f64 = 1.02921487939477174e+03;
const R1_1_H0: f64 = 2.69400185184668828e+02;
const R1_1_H1: f64 = 6.71994680086711583e+02;
const R1_1: [(usize, usize, f64); 45] = [
    (0, 0, 4.32669868057555220e+01),
    (0, 1, 6.00248688633604175e+01),
    (0, 2, 2.37219639827642625e+00),
    (0, 3, -2.79894393250971096e-01),
    (0, 4, 1.25155095800943712e-02),
    (0, 5, -7.49565215206315742e-03),
    (0, 6, 6.63932223870918005e-04),
    (0, 7, -1.01488888610434409e-04),
    (0, 8, 6.10887832304100688e-06),
    (1, 0, 1.07738475558230661e+02),
    (1, 1, 3.69001456008342688e+00),
    (1, 2, 3.31121036816517111e-01),
    (1, 3, -1.63535963232754683e-01),
    (1, 4, -3.18473974053739423e-02),
    (1, 5, 1.42307861074853349e-03),
    (1, 6, -1.04811754536601374e-03),
    (1, 7, 6.78363845956789587e-05),
    (2, 0, 7.37499820775552539e+00),
    (2, 1, -2.76585783799216001e-01),
    (2, 2, -1.96069552853439161e-01),
    (2, 3, -7.93093975037742760e-02),
    (2, 4, -1.99234696731812651e-03),
    (2, 5, -3.30275947046091692e-03),
    (2, 6, 6.55259215739190343e-04),
    (3, 0, -1.38241054916025202e-01),
    (3, 1, -2.27298488816111699e-02),
    (3, 2, -1.43491036423592899e-01),
    (3, 3, -1.55024177251283229e-03),
    (3, 4, -5.77965049524841162e-03),
    (3, 5, 2.84265031739295865e-03),
    (4, 0, -1.70758022815168733e-02),
    (4, 1, -9.24650970767735003e-02),
    (4, 2, -1.07572596351391343e-03),
    (4, 3, -1.56045897847523706e-03),
    (4, 4, 6.01611126536238765e-03),
    (5, 0, -1.35214718282046387e-02),
    (5, 1, 9.58309299987434964e-03),
    (5, 2, 5.49688025194339149e-03),
    (5, 3, 8.04402021013608977e-03),
    (6, 0, 4.09381041505914051e-03),
    (6, 1, 8.04757796785319536e-03),
    (6, 2, 6.53098407622382509e-03),
    (7, 0, 1.80674944830017527e-03),
    (7, 1, 2.86532223158029054e-03),
    (8, 0, 2.93807472402273753e-04),
];
fn piece_r1_1(rho: f64, h: f64) -> f64 {
    let x = scale(rho, R1_1_X0, R1_1_X1);
    let y = scale(h, R1_1_H0, R1_1_H1);
    cheb2_sparse(x, y, &R1_1)
}
const R1_2_X0: f64 = 5.78990759910600673e+02;
const R1_2_X1: f64 = 9.67973119037807237e+02;
const R1_2_H0: f64 = 6.72102294292316856e+02;
const R1_2_H1: f64 = 1.66385288321702660e+03;
const R1_2: [(usize, usize, f64); 45] = [
    (0, 0, -1.72902146989465359e+03),
    (0, 1, 2.18321065725945846e+03),
    (0, 2, -1.87151227785705282e+03),
    (0, 3, 4.84902588530653702e+02),
    (0, 4, -2.61869067141350854e+02),
    (0, 5, 2.23065471185291244e+01),
    (0, 6, -8.98090004505870887e+00),
    (0, 7, 9.01188805212342625e-02),
    (0, 8, -4.82300931220407125e-02),
    (1, 0, 2.38457536219896610e+03),
    (1, 1, -5.48945116177524869e+03),
    (1, 2, 2.16580619373441641e+03),
    (1, 3, -1.49091269195567520e+03),
    (1, 4, 2.27718475882356927e+02),
    (1, 5, -1.01643394353065133e+02),
    (1, 6, 3.40627521129162414e+00),
    (1, 7, -1.38763551797660578e+00),
    (2, 0, -2.41699983417633121e+03),
    (2, 1, 2.73975965333724025e+03),
    (2, 2, -2.52022205945034193e+03),
    (2, 3, 6.00490651178531493e+02),
    (2, 4, -3.21336862496181254e+02),
    (2, 5, 2.15256970018532385e+01),
    (2, 6, -8.54533991828790640e+00),
    (3, 0, 9.90340765301540387e+02),
    (3, 1, -2.56972672366749111e+03),
    (3, 2, 8.90664474085254028e+02),
    (3, 3, -6.11347345965341106e+02),
    (3, 4, 6.73584022739041757e+01),
    (3, 5, -2.92755313828791124e+01),
    (4, 0, -7.66176468400673230e+02),
    (4, 1, 7.46228551856551690e+02),
    (4, 2, -7.08573270798330896e+02),
    (4, 3, 1.16884909052140685e+02),
    (4, 4, -6.09194557435026098e+01),
    (5, 0, 1.63955616038727754e+02),
    (5, 1, -4.84927161232655919e+02),
    (5, 2, 1.14033633751750116e+02),
    (5, 3, -7.87350795007427422e+01),
    (6, 0, -8.90887805608065406e+01),
    (6, 1, 5.83812398643904373e+01),
    (6, 2, -6.16545143926874175e+01),
    (7, 0, 6.08544754082280104e+00),
    (7, 1, -2.67539422892046410e+01),
    (8, 0, -2.46459047014300747e+00),
];
fn piece_r1_2(rho: f64, h: f64) -> f64 {
    let x = scale(rho, R1_2_X0, R1_2_X1);
    let y = scale(h, R1_2_H0, R1_2_H1);
    cheb2_sparse(x, y, &R1_2)
}
const R2_0_X0: f64 = -2.84891636630184308e+00;
const R2_0_X1: f64 = -1.30111384706234756e+00;
const R2_0_H0: f64 = 2.50483024047674235e+03;
const R2_0_H1: f64 = 4.16059507522786862e+03;
const R2_0: [(usize, usize, f64); 36] = [
    (0, 0, -6.02789460489490825e+00),
    (0, 1, 6.48871848456119227e-01),
    (0, 2, -1.24983685062140429e-01),
    (0, 3, 2.93652221805261668e-02),
    (0, 4, -7.43288321789379539e-03),
    (0, 5, 1.94331431136758365e-03),
    (0, 6, -4.84686524592774508e-04),
    (0, 7, 1.09533459162610097e-04),
    (1, 0, 1.78213097805969611e+00),
    (1, 1, -3.01724090370146004e-04),
    (1, 2, 2.69690740957679762e-04),
    (1, 3, -2.00114531564826948e-04),
    (1, 4, 1.40206002069516656e-04),
    (1, 5, -7.60885494808277078e-05),
    (1, 6, 4.25780193066714599e-05),
    (2, 0, 3.29858350389692154e-05),
    (2, 1, -6.89617081125513487e-05),
    (2, 2, 6.19083977802662090e-05),
    (2, 3, -4.85710140250582750e-05),
    (2, 4, 2.79150663411378033e-05),
    (2, 5, -1.55317388296622527e-05),
    (3, 0, 1.38119190718950378e-05),
    (3, 1, -2.49036929176676870e-05),
    (3, 2, 1.94268269003714619e-05),
    (3, 3, -1.07887881298268790e-05),
    (3, 4, 5.75011119871459297e-06),
    (4, 0, -1.64745971328233956e-06),
    (4, 1, 2.26171400848613368e-06),
    (4, 2, -2.48033718939750622e-07),
    (4, 3, -4.06524175715581395e-07),
    (5, 0, 1.13562687461610217e-06),
    (5, 1, -1.63263510425190094e-06),
    (5, 2, 7.56691182426383661e-07),
    (6, 0, -1.57530152010591648e-07),
    (6, 1, 4.60153709967921657e-07),
    (7, 0, 2.90793571381896127e-09),
];
fn piece_r2_0(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R2_0_X0, R2_0_X1);
    let y = scale(h, R2_0_H0, R2_0_H1);
    let z = cheb2_sparse(x, y, &R2_0);
    z.exp()
}
const R2_1_X0: f64 = -1.30102244828751612e+00;
const R2_1_X1: f64 = 1.76082140305519774e-01;
const R2_1_H0: f64 = 2.57379424581118519e+03;
const R2_1_H1: f64 = 4.16040432725245228e+03;
const R2_1: [(usize, usize, f64); 36] = [
    (0, 0, -2.50336003236042259e+00),
    (0, 1, 5.91463705127741268e-01),
    (0, 2, -1.03848918435220192e-01),
    (0, 3, 2.17431553940175132e-02),
    (0, 4, -4.67235326991657148e-03),
    (0, 5, 9.50120988604615966e-04),
    (0, 6, -1.63549030170097705e-04),
    (0, 7, 1.53801972721261198e-05),
    (1, 0, 1.70146049421574053e+00),
    (1, 1, -1.90631553855532881e-03),
    (1, 2, 1.84228005796077112e-03),
    (1, 3, -1.31367215523600883e-03),
    (1, 4, 7.81352399155680753e-04),
    (1, 5, -3.52165392705402634e-04),
    (1, 6, 1.34582621822391355e-04),
    (2, 0, 2.15702839028452661e-04),
    (2, 1, -5.04811496141410482e-04),
    (2, 2, 4.95263925914246552e-04),
    (2, 3, -3.42020340329271175e-04),
    (2, 4, 1.71593186821736217e-04),
    (2, 5, -6.52242014319647201e-05),
    (3, 0, 2.43892594802573591e-05),
    (3, 1, -6.75607185928922728e-05),
    (3, 2, 7.77265852087392363e-05),
    (3, 3, -4.69622240235231973e-05),
    (3, 4, 2.09404936972386549e-05),
    (4, 0, -1.26781560506563629e-06),
    (4, 1, -2.38992504898320281e-06),
    (4, 2, 5.97355711542610103e-06),
    (4, 3, -2.90916689943413437e-06),
    (5, 0, -1.09219735862501550e-06),
    (5, 1, 1.16504010599314885e-06),
    (5, 2, -4.38033690626034802e-07),
    (6, 0, -1.53658716467335938e-07),
    (6, 1, 8.86301569645674411e-08),
    (7, 0, -3.74677063191756934e-08),
];
fn piece_r2_1(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R2_1_X0, R2_1_X1);
    let y = scale(h, R2_1_H0, R2_1_H1);
    let z = cheb2_sparse(x, y, &R2_1);
    z.exp()
}
const R2_2_X0: f64 = 1.76107724827200274e-01;
const R2_2_X1: f64 = 9.99954289700779042e-01;
const R2_2_H0: f64 = 2.72278727382004945e+03;
const R2_2_H1: f64 = 4.15678931087180172e+03;
const R2_2: [(usize, usize, f64); 36] = [
    (0, 0, 2.21549898483119345e-01),
    (0, 1, 4.88919849217533675e-01),
    (0, 2, -7.06748644017142907e-02),
    (0, 3, 1.14231199418897370e-02),
    (0, 4, -1.49112409669583594e-03),
    (0, 5, -9.08456835000633833e-06),
    (0, 6, 1.02826231380895314e-04),
    (0, 7, -5.43931903200747354e-05),
    (1, 0, 9.47932671456018672e-01),
    (1, 1, -2.80187192281078367e-04),
    (1, 2, 1.62458826097501194e-03),
    (1, 3, -1.24470471575215667e-03),
    (1, 4, 6.90429651618715149e-04),
    (1, 5, -2.84376768454930090e-04),
    (1, 6, 9.15991529716568006e-05),
    (2, 0, -2.00006970803113702e-04),
    (2, 1, 1.18162556467792226e-04),
    (2, 2, 1.91699821543480091e-04),
    (2, 3, -1.34554667405728881e-04),
    (2, 4, 5.77068396665194057e-05),
    (2, 5, -1.18341136423765494e-05),
    (3, 0, -4.15345831902857489e-05),
    (3, 1, 4.91963291974090392e-05),
    (3, 2, -1.95430640650246018e-06),
    (3, 3, 1.92952789489640658e-06),
    (3, 4, -4.03689623772178902e-06),
    (4, 0, -4.21971885355772473e-06),
    (4, 1, 7.78299728015753981e-06),
    (4, 2, -2.14407856223046866e-06),
    (4, 3, 1.77010151512188390e-06),
    (5, 0, -3.86325386626949949e-07),
    (5, 1, 9.86050437408754452e-07),
    (5, 2, -8.03832859466205995e-07),
    (6, 0, 9.59382024842358055e-08),
    (6, 1, 2.20783772157998300e-08),
    (7, 0, 2.89025660589202227e-07),
];
fn piece_r2_2(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R2_2_X0, R2_2_X1);
    let y = scale(h, R2_2_H0, R2_2_H1);
    let z = cheb2_sparse(x, y, &R2_2);
    z.exp()
}
const R2_3_X0: f64 = 1.00000685159669755e+00;
const R2_3_X1: f64 = 1.60204536340749515e+00;
const R2_3_H0: f64 = 2.76878502326288435e+03;
const R2_3_H1: f64 = 4.13537447516571865e+03;
const R2_3: [(usize, usize, f64); 36] = [
    (0, 0, 1.87484321036251123e+00),
    (0, 1, 4.61749514301402431e-01),
    (0, 2, -6.05096371518767800e-02),
    (0, 3, 8.02583546109715031e-03),
    (0, 4, -4.86672432490764507e-04),
    (0, 5, -2.46813921487020718e-04),
    (0, 6, 1.25338220318050897e-04),
    (0, 7, -5.28040306118297058e-05),
    (1, 0, 6.89957580706417106e-01),
    (1, 1, 3.66537022540833158e-03),
    (1, 2, 8.58559519812249075e-04),
    (1, 3, -6.60536928430262770e-04),
    (1, 4, 1.94792761241564767e-04),
    (1, 5, -2.90611354376760704e-05),
    (1, 6, -2.41935407633880210e-05),
    (2, 0, -3.53134721392044961e-04),
    (2, 1, 7.66906718845610457e-04),
    (2, 2, -9.21757780497056272e-05),
    (2, 3, 7.64406231801289127e-05),
    (2, 4, -6.07745689535254784e-05),
    (2, 5, 2.90606033016263852e-05),
    (3, 0, 3.00175833885559330e-05),
    (3, 1, 6.57960899112709655e-05),
    (3, 2, -1.54822320075324876e-05),
    (3, 3, 1.04758041443564166e-05),
    (3, 4, -5.49784094421883984e-06),
    (4, 0, 1.67308481923055666e-05),
    (4, 1, -2.84264446773624458e-06),
    (4, 2, 8.48792875633698295e-07),
    (4, 3, -1.66603185186438869e-06),
    (5, 0, 2.50129620181641797e-06),
    (5, 1, -6.35688727357310563e-07),
    (5, 2, -6.55268522256586135e-07),
    (6, 0, 1.68525641341250494e-07),
    (6, 1, 4.96239109544082613e-07),
    (7, 0, 9.69962023098182836e-08),
];
fn piece_r2_3(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R2_3_X0, R2_3_X1);
    let y = scale(h, R2_3_H0, R2_3_H1);
    let z = cheb2_sparse(x, y, &R2_3);
    z.exp()
}
const R2_4_X0: f64 = 1.60206279303379606e+00;
const R2_4_X1: f64 = 1.99996946118700403e+00;
const R2_4_H0: f64 = 2.60907732217926377e+03;
const R2_4_H1: f64 = 4.06907116647421799e+03;
const R2_4: [(usize, usize, f64); 45] = [
    (0, 0, 2.92766686388611275e+00),
    (0, 1, 5.52523324535840743e-01),
    (0, 2, -7.61531646791393652e-02),
    (0, 3, 8.98651227405103219e-03),
    (0, 4, 5.25930286695390289e-05),
    (0, 5, -4.81300326341742627e-04),
    (0, 6, 1.96994150715806919e-04),
    (0, 7, -5.89946810167048662e-05),
    (0, 8, 7.22967880137309437e-06),
    (1, 0, 4.57089222584205090e-01),
    (1, 1, 7.69346580561295398e-03),
    (1, 2, -8.87853712748833232e-04),
    (1, 3, 7.19968244709866278e-04),
    (1, 4, -6.03988639139006549e-04),
    (1, 5, 2.97642096465358956e-04),
    (1, 6, -8.43237867856862157e-05),
    (1, 7, 1.20677890617435212e-05),
    (2, 0, 1.20905765999169902e-03),
    (2, 1, 3.79337743006334237e-04),
    (2, 2, -1.39448457439417451e-04),
    (2, 3, 7.47267265309939789e-05),
    (2, 4, -1.99480685703206754e-05),
    (2, 5, 9.72870554557736965e-06),
    (2, 6, 3.55089768482004121e-06),
    (3, 0, 3.07625746449122195e-04),
    (3, 1, -9.22484928002055516e-05),
    (3, 2, 2.45028780628407089e-05),
    (3, 3, -1.24428068016995443e-05),
    (3, 4, 1.20546129536571066e-05),
    (3, 5, -7.86842996465827820e-07),
    (4, 0, 3.78077849932838484e-05),
    (4, 1, -1.49297322692146140e-05),
    (4, 2, 4.94506252403078234e-06),
    (4, 3, -6.86323738035044322e-07),
    (4, 4, 1.75815562122556976e-06),
    (5, 0, 2.59536230040602927e-06),
    (5, 1, 2.99460473127493313e-07),
    (5, 2, -1.04546634671816194e-08),
    (5, 3, 6.31855413037249841e-07),
    (6, 0, 2.39489210276380474e-07),
    (6, 1, 1.97341057839182727e-08),
    (6, 2, 2.00972405191764389e-07),
    (7, 0, -2.49550526374707481e-07),
    (7, 1, 7.35223579738500235e-07),
    (8, 0, -1.68763597982966233e-07),
];
fn piece_r2_4(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R2_4_X0, R2_4_X1);
    let y = scale(h, R2_4_H0, R2_4_H1);
    let z = cheb2_sparse(x, y, &R2_4);
    z.exp()
}
const R2_5_X0: f64 = 2.00002189372911188e+00;
const R2_5_X1: f64 = 2.58220329552946737e+00;
const R2_5_H0: f64 = 2.56603301436908805e+03;
const R2_5_H1: f64 = 3.94018645230022457e+03;
const R2_5: [(usize, usize, f64); 45] = [
    (0, 0, 4.06308069979476638e+00),
    (0, 1, 5.41899657771169108e-01),
    (0, 2, -8.84935838813819819e-02),
    (0, 3, 2.07311899743695789e-04),
    (0, 4, -6.76261564624390347e-03),
    (0, 5, -2.16112917719813889e-03),
    (0, 6, -5.01755405454097593e-04),
    (0, 7, -1.03637725259294878e-04),
    (0, 8, -9.56968410840904703e-06),
    (1, 0, 7.65157152413112773e-01),
    (1, 1, -3.82917606085206794e-02),
    (1, 2, -2.28286452574723020e-02),
    (1, 3, -1.54511414464828991e-02),
    (1, 4, -1.03458318330738302e-02),
    (1, 5, -3.33336038368594158e-03),
    (1, 6, -8.54864652450691774e-04),
    (1, 7, -4.46165545369165296e-05),
    (2, 0, 3.07423115560562267e-02),
    (2, 1, -2.85265411949133206e-02),
    (2, 2, -1.30543198934845277e-02),
    (2, 3, -1.07650564069703795e-02),
    (2, 4, -5.63675612198752223e-03),
    (2, 5, -1.97463149255533823e-03),
    (2, 6, -1.93630245733300083e-04),
    (3, 0, 4.19961363712644236e-03),
    (3, 1, -1.12201246756244953e-02),
    (3, 2, -6.08763368485519887e-03),
    (3, 3, -4.59672725106045291e-03),
    (3, 4, -2.39304162685286589e-03),
    (3, 5, -5.59548174724758306e-04),
    (4, 0, -4.63686367781048013e-05),
    (4, 1, -2.96592207267021507e-03),
    (4, 2, -1.71284572965193604e-03),
    (4, 3, -1.15917220839844778e-03),
    (4, 4, -6.16949262778083898e-04),
    (5, 0, -3.38021037882948861e-05),
    (5, 1, -4.09315309497604676e-04),
    (5, 2, -4.76291865540901156e-05),
    (5, 3, -1.52851108793445163e-04),
    (6, 0, -1.47382143945079793e-05),
    (6, 1, 4.33324409024877079e-05),
    (6, 2, 6.04647599061755104e-05),
    (7, 0, -2.21612561790256180e-05),
    (7, 1, 1.74243337163408411e-05),
    (8, 0, -4.34170224915328785e-06),
];
fn piece_r2_5(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R2_5_X0, R2_5_X1);
    let y = scale(h, R2_5_H0, R2_5_H1);
    let z = cheb2_sparse(x, y, &R2_5);
    z.exp()
}
const R3_0_X0: f64 = 2.05720184187474775e+00;
const R3_0_X1: f64 = 2.55091699210255918e+00;
const R3_0_H0: f64 = 1.93199421203845577e+03;
const R3_0_H1: f64 = 2.72940650113708762e+03;
const R3_0: [(usize, usize, f64); 36] = [
    (0, 0, 2.95701248238224368e+00),
    (0, 1, 8.86502379650635053e-01),
    (0, 2, -2.30983221333827732e-01),
    (0, 3, 1.18637049737081890e-01),
    (0, 4, -5.98633401959528452e-02),
    (0, 5, 2.11923462396602924e-02),
    (0, 6, -4.85143023873447609e-03),
    (0, 7, 4.16521286122459701e-04),
    (1, 0, 7.87910784981004086e-01),
    (1, 1, -2.84480008463631973e-01),
    (1, 2, 2.69852278387556876e-01),
    (1, 3, -1.89736507066622484e-01),
    (1, 4, 9.31121796329542062e-02),
    (1, 5, -3.01429942246089905e-02),
    (1, 6, 5.37808826778139997e-03),
    (2, 0, -4.29132313498960319e-02),
    (2, 1, 1.55249352115553074e-01),
    (2, 2, -1.16642459315605893e-01),
    (2, 3, 6.49155212792042607e-02),
    (2, 4, -2.31497281631329321e-02),
    (2, 5, 5.62472117879077044e-03),
    (3, 0, 2.60784109318745782e-03),
    (3, 1, 9.80367042776460022e-03),
    (3, 2, -1.02880644372797467e-02),
    (3, 3, 5.74826898959112948e-03),
    (3, 4, -1.96193668629841790e-03),
    (4, 0, 1.24727726919854620e-02),
    (4, 1, -1.97346120412310033e-02),
    (4, 2, 1.01981235845030223e-02),
    (4, 3, -3.63703263517364643e-03),
    (5, 0, -1.18491778857693743e-03),
    (5, 1, 1.94181753169647307e-03),
    (5, 2, -7.65955955978176633e-04),
    (6, 0, 4.66752616895021359e-05),
    (6, 1, 2.12027376639960602e-05),
    (7, 0, 1.37663196301112274e-05),
];
fn piece_r3_0(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R3_0_X0, R3_0_X1);
    let y = scale(h, R3_0_H0, R3_0_H1);
    let z = cheb2_sparse(x, y, &R3_0);
    z.exp_m1()
}
const R3_1_X0: f64 = 2.55091716957062697e+00;
const R3_1_X1: f64 = 2.68102847161999769e+00;
const R3_1_H0: f64 = 1.81994006717181901e+03;
const R3_1_H1: f64 = 2.81065686173760332e+03;
const R3_1: [(usize, usize, f64); 36] = [
    (0, 0, 3.91665681108572628e+00),
    (0, 1, 9.87596180801568746e-01),
    (0, 2, -1.26066153841336570e-01),
    (0, 3, 2.28334176287083891e-02),
    (0, 4, 1.88318909245698309e-02),
    (0, 5, -3.21199523272456015e-03),
    (0, 6, 4.26825011317294185e-03),
    (0, 7, -6.12455163577092657e-04),
    (1, 0, 2.74520695327533037e-01),
    (1, 1, 8.99664587225885298e-02),
    (1, 2, -3.27155587616985849e-02),
    (1, 3, 6.91913111440049799e-02),
    (1, 4, -8.88014417797088756e-03),
    (1, 5, 1.44090288806374457e-02),
    (1, 6, 2.81580902940720861e-04),
    (2, 0, 3.84881311948391119e-02),
    (2, 1, 1.47182670314353382e-04),
    (2, 2, 3.13983353508257013e-02),
    (2, 3, 1.00622860361050739e-02),
    (2, 4, 6.99469510858840779e-03),
    (2, 5, 4.59721743433681111e-03),
    (3, 0, 7.82938686041810376e-03),
    (3, 1, 4.19552646529223073e-03),
    (3, 2, 1.07921420824102464e-02),
    (3, 3, 1.68177363262752015e-03),
    (3, 4, 3.70320899508611934e-03),
    (4, 0, 4.19593628863539211e-04),
    (4, 1, 2.42693262627847800e-03),
    (4, 2, 5.34010224384799930e-04),
    (4, 3, 9.19285185045903451e-04),
    (5, 0, -1.52266910437611947e-05),
    (5, 1, 2.25595260736498460e-06),
    (5, 2, -3.66642332127535724e-05),
    (6, 0, -9.28263827323949528e-06),
    (6, 1, -2.01379453512277737e-05),
    (7, 0, 6.65679723379014957e-06),
];
fn piece_r3_1(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R3_1_X0, R3_1_X1);
    let y = scale(h, R3_1_H0, R3_1_H1);
    let z = cheb2_sparse(x, y, &R3_1);
    z.exp_m1()
}
const R3_2_X0: f64 = 2.68104262897156076e+00;
const R3_2_X1: f64 = 2.88197168896622236e+00;
const R3_2_H0: f64 = 1.55396148076069471e+03;
const R3_2_H1: f64 = 2.46867359785514282e+03;
const R3_2: [(usize, usize, f64); 36] = [
    (0, 0, 4.89774758630787321e+00),
    (0, 1, 2.13818775340694422e+00),
    (0, 2, -2.18601712920703212e-01),
    (0, 3, -3.03299944758241735e-01),
    (0, 4, -3.80543426101895677e-01),
    (0, 5, -1.40250503309192448e-01),
    (0, 6, -2.87733365667999057e-02),
    (0, 7, -5.52660400050702699e-04),
    (1, 0, 2.40525448134686659e+00),
    (1, 1, 1.35885532370100881e+00),
    (1, 2, 5.75960993482293016e-01),
    (1, 3, -5.79684521652773954e-01),
    (1, 4, -5.45798023298798340e-01),
    (1, 5, -2.11626383988316541e-01),
    (1, 6, -3.79257752316365104e-02),
    (2, 0, 1.33305966656671093e+00),
    (2, 1, 1.68350784427268563e+00),
    (2, 2, 7.79237058915984382e-01),
    (2, 3, -2.06393522162163029e-01),
    (2, 4, -1.85524421150191882e-01),
    (2, 5, -1.04472825658269972e-01),
    (3, 0, 7.09637817719459663e-01),
    (3, 1, 1.40686522915984646e+00),
    (3, 2, 4.30383587327149986e-01),
    (3, 3, 1.25322501742832687e-01),
    (3, 4, -8.29386373904990593e-02),
    (4, 0, 3.63829051894934641e-01),
    (4, 1, 4.83044216871438836e-01),
    (4, 2, 2.65934175617511592e-01),
    (4, 3, 1.74287180372057551e-02),
    (5, 0, 8.29038358099554035e-02),
    (5, 1, 1.24344588109011570e-01),
    (5, 2, 4.86484588238391066e-02),
    (6, 0, 7.47777397853527110e-03),
    (6, 1, 1.69188279906836159e-02),
    (7, 0, 4.33332835127489438e-04),
];
fn piece_r3_2(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R3_2_X0, R3_2_X1);
    let y = scale(h, R3_2_H0, R3_2_H1);
    let z = cheb2_sparse(x, y, &R3_2);
    z.exp_m1()
}
const R4_0_X0: f64 = -2.30477002266859987e+00;
const R4_0_X1: f64 = -1.00010016499256116e+00;
const R4_0_H0: f64 = 1.40543339363148391e+02;
const R4_0_H1: f64 = 2.58964274011508132e+03;
const R4_0: [(usize, usize, f64); 45] = [
    (0, 0, -6.75189988294282273e+00),
    (0, 1, 1.22133375023907265e+00),
    (0, 2, -3.23466617748371543e-01),
    (0, 3, 1.10381129083309770e-01),
    (0, 4, -4.23258774264398188e-02),
    (0, 5, 1.64375369269803250e-02),
    (0, 6, -7.07711943809840108e-03),
    (0, 7, 2.41321467319900122e-03),
    (0, 8, -1.07168995950866612e-03),
    (1, 0, 1.47376474782708167e+00),
    (1, 1, 1.42478246494166405e-01),
    (1, 2, -6.68506452580402144e-02),
    (1, 3, 3.25115719694159741e-02),
    (1, 4, -1.41587589972276759e-02),
    (1, 5, 6.65289937487776629e-03),
    (1, 6, -2.12669128465750937e-03),
    (1, 7, 1.14937547155465948e-03),
    (2, 0, -1.03705288421131366e-03),
    (2, 1, 1.67853294689076040e-03),
    (2, 2, -4.00455613130365300e-04),
    (2, 3, 1.50962636172145598e-04),
    (2, 4, -6.35973549501279729e-04),
    (2, 5, 3.32082257322731194e-04),
    (2, 6, -4.29833316651108454e-04),
    (3, 0, -3.72944578038457604e-03),
    (3, 1, 6.14275264297517182e-03),
    (3, 2, -3.34061224505558278e-03),
    (3, 3, 1.65770577312714020e-03),
    (3, 4, -6.17005290020944657e-04),
    (3, 5, 2.87389150213688529e-04),
    (4, 0, 1.18742519396903244e-03),
    (4, 1, -1.81523515789973550e-03),
    (4, 2, 6.86785777991425817e-04),
    (4, 3, -1.15632863172964318e-04),
    (4, 4, -3.59290490239692995e-05),
    (5, 0, -5.91848887405111270e-04),
    (5, 1, 8.62451915182662699e-04),
    (5, 2, -3.12194295418088930e-04),
    (5, 3, 6.40432249864600686e-05),
    (6, 0, 2.30085731935199343e-04),
    (6, 1, -3.11814615915435671e-04),
    (6, 2, 7.92399237269858709e-05),
    (7, 0, -5.50884257652722663e-05),
    (7, 1, 6.40008560910471026e-05),
    (8, 0, 8.54786207844429823e-06),
];
fn piece_r4_0(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R4_0_X0, R4_0_X1);
    let y = scale(h, R4_0_H0, R4_0_H1);
    let z = cheb2_sparse(x, y, &R4_0);
    z.exp()
}
const R4_1_X0: f64 = -9.99823568875638835e-01;
const R4_1_X1: f64 = -3.75068176872341366e-04;
const R4_1_H0: f64 = 2.25345013311768554e+01;
const R4_1_H1: f64 = 2.69637184072302807e+03;
const R4_1: [(usize, usize, f64); 45] = [
    (0, 0, -4.37105993136680482e+00),
    (0, 1, 1.91025197922552037e+00),
    (0, 2, -7.10132642097121680e-01),
    (0, 3, 3.21411184676221040e-01),
    (0, 4, -1.57076499125258173e-01),
    (0, 5, 7.35500323809806511e-02),
    (0, 6, -3.63626221883810927e-02),
    (0, 7, 1.41653348310244558e-02),
    (0, 8, -6.55534781018001639e-03),
    (1, 0, 1.03211892808947447e+00),
    (1, 1, 2.77887646072637495e-01),
    (1, 2, -1.55203021684495235e-01),
    (1, 3, 9.01682606334413966e-02),
    (1, 4, -4.56872549018536300e-02),
    (1, 5, 2.54011640719164974e-02),
    (1, 6, -9.78156655280752914e-03),
    (1, 7, 5.22389677130975956e-03),
    (2, 0, -1.36641721251672711e-02),
    (2, 1, 2.07344491000101133e-02),
    (2, 2, -1.21924011180095694e-02),
    (2, 3, 6.06491939151442179e-03),
    (2, 4, -2.75370713864972081e-03),
    (2, 5, 8.06410479893105532e-04),
    (2, 6, -1.21574519919214342e-04),
    (3, 0, -6.66850227395598310e-04),
    (3, 1, 3.31248729456955964e-04),
    (3, 2, -1.07174332844573600e-04),
    (3, 3, -5.05006631256184595e-04),
    (3, 4, 2.47333752221534978e-04),
    (3, 5, -3.75451437756521577e-04),
    (4, 0, 2.48397425841908096e-04),
    (4, 1, -2.15939308584114242e-04),
    (4, 2, 5.24545195246319180e-04),
    (4, 3, -1.51384148451776004e-04),
    (4, 4, 3.30455602128342256e-04),
    (5, 0, 9.17765302127794807e-05),
    (5, 1, -2.50166019862982369e-04),
    (5, 2, 1.29454148189057933e-04),
    (5, 3, -1.96027281833675930e-04),
    (6, 0, 2.74012938284712084e-05),
    (6, 1, 2.46622415448474394e-05),
    (6, 2, 8.32950467924933635e-05),
    (7, 0, -9.07115257332560992e-06),
    (7, 1, 7.60383219358127041e-05),
    (8, 0, -1.63798123601183731e-05),
];
fn piece_r4_1(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R4_1_X0, R4_1_X1);
    let y = scale(h, R4_1_H0, R4_1_H1);
    let z = cheb2_sparse(x, y, &R4_1);
    z.exp()
}
const R4_2_X0: f64 = 8.20185796793902635e-05;
const R4_2_X1: f64 = 6.98815032155890914e-01;
const R4_2_H0: f64 = 1.90221433895477823e+01;
const R4_2_H1: f64 = 2.77102916044776157e+03;
const R4_2: [(usize, usize, f64); 45] = [
    (0, 0, -2.67738913718814064e+00),
    (0, 1, 2.52256044849299332e+00),
    (0, 2, -1.05705914142719992e+00),
    (0, 3, 5.12226841861503579e-01),
    (0, 4, -2.51259069747760633e-01),
    (0, 5, 1.17478538316026337e-01),
    (0, 6, -5.17931576335913663e-02),
    (0, 7, 1.92764271718495507e-02),
    (0, 8, -6.17794423943711542e-03),
    (1, 0, 6.42028774259348900e-01),
    (1, 1, 3.03609609187871776e-01),
    (1, 2, -1.65389740014023345e-01),
    (1, 3, 8.25308859351752966e-02),
    (1, 4, -3.55058835189803468e-02),
    (1, 5, 1.18160186831460581e-02),
    (1, 6, -2.13431003895120939e-03),
    (1, 7, -8.67130403046180876e-04),
    (2, 0, -1.11567151307567797e-02),
    (2, 1, 1.42763716762583457e-02),
    (2, 2, -6.33717325650487179e-03),
    (2, 3, 2.16628590851643569e-03),
    (2, 4, 1.24846055224887939e-04),
    (2, 5, -4.44937017650076996e-04),
    (2, 6, 4.96791661280330957e-04),
    (3, 0, -5.91239080945011241e-04),
    (3, 1, 5.88553200175760747e-04),
    (3, 2, -2.11353627126489066e-04),
    (3, 3, 3.75944664428748502e-05),
    (3, 4, 8.68313339489389867e-05),
    (3, 5, -1.65305739138832100e-05),
    (4, 0, -3.98608315993910836e-04),
    (4, 1, 6.35497989784568926e-04),
    (4, 2, -5.76725081866878556e-04),
    (4, 3, 3.49835872569703179e-04),
    (4, 4, -2.75529951740660834e-04),
    (5, 0, -1.67734610521967593e-04),
    (5, 1, 4.56901348055389640e-04),
    (5, 2, -3.12900830962828727e-04),
    (5, 3, 2.97673747076141544e-04),
    (6, 0, 6.13057530396685281e-05),
    (6, 1, -6.76376470101014638e-05),
    (6, 2, 1.34379616926047203e-04),
    (7, 0, 5.41071796480829177e-05),
    (7, 1, -8.39838882657807526e-05),
    (8, 0, -1.90478922456154202e-05),
];
fn piece_r4_2(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R4_2_X0, R4_2_X1);
    let y = scale(h, R4_2_H0, R4_2_H1);
    let z = cheb2_sparse(x, y, &R4_2);
    z.exp()
}
const R4_3_X0: f64 = 6.99036230764635724e-01;
const R4_3_X1: f64 = 1.30102678391389270e+00;
const R4_3_H0: f64 = 5.10995378370226394e+00;
const R4_3_H1: f64 = 2.80291801688862597e+03;
const R4_3: [(usize, usize, f64); 45] = [
    (0, 0, -1.57723164276332906e+00),
    (0, 1, 3.22596409488446412e+00),
    (0, 2, -1.42192205188715826e+00),
    (0, 3, 6.75465649796187728e-01),
    (0, 4, -3.04527064909748790e-01),
    (0, 5, 1.23631134490616287e-01),
    (0, 6, -4.17947060360757891e-02),
    (0, 7, 1.05679618584414887e-02),
    (0, 8, -7.80011667874727578e-04),
    (1, 0, 4.85607922121305824e-01),
    (1, 1, 3.18571926592906007e-01),
    (1, 2, -1.45287359762550278e-01),
    (1, 3, 4.72125351198399504e-02),
    (1, 4, -3.95470123120107275e-03),
    (1, 5, -1.02287374579114586e-02),
    (1, 6, 8.52410673432108836e-03),
    (1, 7, -4.68101557746502129e-03),
    (2, 0, -8.48796668806728327e-03),
    (2, 1, 4.64762352425077824e-03),
    (2, 2, 2.23316737798759553e-03),
    (2, 3, -4.25171955058751844e-03),
    (2, 4, 3.71881653077297831e-03),
    (2, 5, -2.09306391433865258e-03),
    (2, 6, 8.14319445942972850e-04),
    (3, 0, -7.39709059269339045e-05),
    (3, 1, -6.44258255975368815e-04),
    (3, 2, 7.16623895637011061e-04),
    (3, 3, -7.96404002736878988e-04),
    (3, 4, 6.34263535910447891e-04),
    (3, 5, -3.41817117221622285e-04),
    (4, 0, -1.23353354950152976e-03),
    (4, 1, 2.20015213913687339e-03),
    (4, 2, -1.84785680074514591e-03),
    (4, 3, 1.16588418242183594e-03),
    (4, 4, -6.75561561768632675e-04),
    (5, 0, -9.49316943647847866e-04),
    (5, 1, 1.84714304524397836e-03),
    (5, 2, -1.27033913327766874e-03),
    (5, 3, 7.54798802693097963e-04),
    (6, 0, 1.87787006380773327e-05),
    (6, 1, -7.10711921694658932e-06),
    (6, 2, 2.70829142360319949e-05),
    (7, 0, 7.44849172126500004e-06),
    (7, 1, -2.87521632381494264e-05),
    (8, 0, 3.16849223656397617e-05),
];
fn piece_r4_3(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R4_3_X0, R4_3_X1);
    let y = scale(h, R4_3_H0, R4_3_H1);
    let z = cheb2_sparse(x, y, &R4_3);
    z.exp()
}
const R4_4_X0: f64 = 1.30104810914432556e+00;
const R4_4_X1: f64 = 1.77814740795995352e+00;
const R4_4_H0: f64 = 1.48232987926005222e+02;
const R4_4_H1: f64 = 2.79949185408670564e+03;
const R4_4: [(usize, usize, f64); 45] = [
    (0, 0, -2.72225756597988366e-01),
    (0, 1, 3.00696095475757286e+00),
    (0, 2, -1.17536249471404730e+00),
    (0, 3, 4.86080739256432803e-01),
    (0, 4, -1.88873601247716266e-01),
    (0, 5, 6.46532697361050512e-02),
    (0, 6, -1.75478176233236395e-02),
    (0, 7, 2.93452651262622122e-03),
    (0, 8, 3.30777921341277449e-04),
    (1, 0, 3.47789393201288599e-01),
    (1, 1, 2.44590264677239849e-01),
    (1, 2, -9.31578404652647074e-02),
    (1, 3, 2.10003129746744559e-02),
    (1, 4, 2.80090450150879402e-03),
    (1, 5, -6.66292898101666399e-03),
    (1, 6, 4.15519575868560261e-03),
    (1, 7, -1.51660973541824467e-03),
    (2, 0, -8.03164522370745405e-03),
    (2, 1, 2.10627726763004371e-03),
    (2, 2, 1.50728126806065404e-03),
    (2, 3, -2.42804226443898768e-03),
    (2, 4, 1.22150076828425212e-03),
    (2, 5, -3.63126810958193250e-04),
    (2, 6, -1.26284882754033499e-04),
    (3, 0, -1.19622724743285541e-04),
    (3, 1, -3.96586186175497423e-04),
    (3, 2, 2.27161683205484187e-04),
    (3, 3, -1.43425422229273114e-04),
    (3, 4, -4.18487840362155995e-05),
    (3, 5, 5.32976426594967739e-05),
    (4, 0, 2.44694773706386772e-04),
    (4, 1, -4.43055951416191957e-04),
    (4, 2, 3.72632662079093284e-04),
    (4, 3, -2.40405772940874025e-04),
    (4, 4, 1.35681873057797380e-04),
    (5, 0, 5.00092130089723399e-05),
    (5, 1, -1.00697267839481064e-04),
    (5, 2, 1.16590248879970066e-04),
    (5, 3, -6.73757668138241734e-05),
    (6, 0, 3.10973220746666147e-06),
    (6, 1, 1.99270133279952058e-05),
    (6, 2, -7.10930390828899329e-06),
    (7, 0, -5.28684853731877709e-05),
    (7, 1, 7.74996780904084199e-05),
    (8, 0, 1.85418275115180060e-05),
];
fn piece_r4_4(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R4_4_X0, R4_4_X1);
    let y = scale(h, R4_4_H0, R4_4_H1);
    let z = cheb2_sparse(x, y, &R4_4);
    z.exp()
}
const R4_5_X0: f64 = 1.77817073518483504e+00;
const R4_5_X1: f64 = 2.07905531441405289e+00;
const R4_5_H0: f64 = 2.44228305095014164e+01;
const R4_5_H1: f64 = 2.70195508001442477e+03;
const R4_5: [(usize, usize, f64); 45] = [
    (0, 0, -2.53824119714123830e-01),
    (0, 1, 4.10906634497212231e+00),
    (0, 2, -1.68523601023593517e+00),
    (0, 3, 6.61603734193969895e-01),
    (0, 4, -2.25584817518931918e-01),
    (0, 5, 5.96652887068506513e-02),
    (0, 6, -8.70862610824241039e-03),
    (0, 7, -1.02900247953582099e-03),
    (0, 8, 1.09930867861157710e-03),
    (1, 0, 1.75784651520352997e-01),
    (1, 1, 1.58187911207219006e-01),
    (1, 2, -3.84944514670596816e-02),
    (1, 3, -1.00998071258683458e-02),
    (1, 4, 1.39547138149600392e-02),
    (1, 5, -7.44071955966651381e-03),
    (1, 6, 2.52313869595290497e-03),
    (1, 7, -7.65263479282044141e-05),
    (2, 0, -5.32814220843476696e-04),
    (2, 1, -6.92633489355664548e-03),
    (2, 2, 6.22560468748273310e-03),
    (2, 3, -4.57851463208168094e-03),
    (2, 4, 2.23652773713162601e-03),
    (2, 5, -7.53289638667811756e-04),
    (2, 6, 2.85255488138721602e-04),
    (3, 0, -9.88700957905657915e-04),
    (3, 1, 1.70775602526131750e-03),
    (3, 2, -1.37677568688332583e-03),
    (3, 3, 1.00504952796348287e-03),
    (3, 4, -5.00179381180012477e-04),
    (3, 5, 2.42672589179701077e-04),
    (4, 0, 6.48802295798380554e-04),
    (4, 1, -1.10228688265249911e-03),
    (4, 2, 9.08332209190815362e-04),
    (4, 3, -4.88463876364379139e-04),
    (4, 4, 2.45980923821081455e-04),
    (5, 0, -2.04397421293814547e-04),
    (5, 1, 3.99128101030319640e-04),
    (5, 2, -2.26415984793280119e-04),
    (5, 3, 1.28368444952694334e-04),
    (6, 0, -1.43906569422174266e-05),
    (6, 1, 3.84270470593226285e-05),
    (6, 2, -1.73817171322319022e-05),
    (7, 0, -7.06440653644824101e-06),
    (7, 1, 1.35062699556728746e-05),
    (8, 0, -3.97776241433969947e-07),
];
fn piece_r4_5(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R4_5_X0, R4_5_X1);
    let y = scale(h, R4_5_H0, R4_5_H1);
    let z = cheb2_sparse(x, y, &R4_5);
    z.exp()
}
const R4_6_X0: f64 = 2.07937404643202672e+00;
const R4_6_X1: f64 = 2.34240935511491388e+00;
const R4_6_H0: f64 = 2.33924569910018562e+02;
const R4_6_H1: f64 = 2.54419530682595450e+03;
const R4_6: [(usize, usize, f64); 45] = [
    (0, 0, 6.60895025622602272e-01),
    (0, 1, 3.21647760989580433e+00),
    (0, 2, -1.09414063063426825e+00),
    (0, 3, 3.44219945332672084e-01),
    (0, 4, -1.02408077097477179e-01),
    (0, 5, 2.13491135386357361e-02),
    (0, 6, -3.53891594934254480e-03),
    (0, 7, -3.02129102895231933e-04),
    (0, 8, 2.03512501241630598e-04),
    (1, 0, 1.31373846583682313e-01),
    (1, 1, 1.08901967218603693e-01),
    (1, 2, -3.18345152493545447e-02),
    (1, 3, -1.21924601570405494e-02),
    (1, 4, 2.81725079135542638e-03),
    (1, 5, -4.18433261239431302e-03),
    (1, 6, -5.44382936576344738e-05),
    (1, 7, -2.35387133249245149e-04),
    (2, 0, -4.71324395527049095e-03),
    (2, 1, -5.11357870116426871e-03),
    (2, 2, -1.40692475963112291e-03),
    (2, 3, -2.97644856094165947e-03),
    (2, 4, -1.23516128558777329e-03),
    (2, 5, -5.63061302997823021e-04),
    (2, 6, -3.74250047249012987e-04),
    (3, 0, -4.13429064030781479e-04),
    (3, 1, -1.20616118345010914e-03),
    (3, 2, -6.07062938814970977e-04),
    (3, 3, -6.82965093931371258e-04),
    (3, 4, -2.53953606087190973e-04),
    (3, 5, -2.23506965598100065e-04),
    (4, 0, -1.75205946380610109e-04),
    (4, 1, -2.97913027966280488e-05),
    (4, 2, -2.78547965964239127e-04),
    (4, 3, -6.04312067747265936e-07),
    (4, 4, -1.09155442815962129e-04),
    (5, 0, 2.76503803452653243e-05),
    (5, 1, -1.03172652282089067e-04),
    (5, 2, 3.72865368258469538e-05),
    (5, 3, -4.78498869167481557e-05),
    (6, 0, -2.92924369049486516e-06),
    (6, 1, 5.00604965963451053e-06),
    (6, 2, -7.11663819980732568e-06),
    (7, 0, -1.02803456323096922e-06),
    (7, 1, 9.63167723971399007e-06),
    (8, 0, 4.21576151366299215e-06),
];
fn piece_r4_6(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R4_6_X0, R4_6_X1);
    let y = scale(h, R4_6_H0, R4_6_H1);
    let z = cheb2_sparse(x, y, &R4_6);
    z.exp()
}
const R4_7_X0: f64 = 2.34242272187318035e+00;
const R4_7_X1: f64 = 2.44707381359249121e+00;
const R4_7_H0: f64 = 1.98113892836137353e+02;
const R4_7_H1: f64 = 2.28568223347998719e+03;
const R4_7: [(usize, usize, f64); 45] = [
    (0, 0, 4.97102377074291724e-01),
    (0, 1, 3.50276759917927727e+00),
    (0, 2, -1.11667182280588628e+00),
    (0, 3, 3.10898463444857776e-01),
    (0, 4, -8.45842350192798076e-02),
    (0, 5, 1.37057584341909091e-02),
    (0, 6, -4.71071617606253976e-03),
    (0, 7, 1.59075296695795893e-04),
    (0, 8, -9.16345253926961864e-04),
    (1, 0, 4.55314196230903467e-02),
    (1, 1, 2.65254884704787045e-02),
    (1, 2, -5.10964380807645436e-04),
    (1, 3, -1.64462572634144558e-02),
    (1, 4, 4.11611030260669759e-03),
    (1, 5, -6.52037690343588387e-03),
    (1, 6, 9.54675363313521835e-04),
    (1, 7, -1.71975443614029487e-03),
    (2, 0, -6.27639036421161916e-04),
    (2, 1, -2.47800404058747545e-03),
    (2, 2, -5.35797777505747901e-04),
    (2, 3, -9.41011349276593819e-04),
    (2, 4, -1.03184912525250156e-03),
    (2, 5, -4.85199518970093571e-05),
    (2, 6, -4.76526430040297762e-04),
    (3, 0, 1.51796338603879468e-03),
    (3, 1, -3.14351722012440240e-03),
    (3, 2, 2.04917961149322881e-03),
    (3, 3, -1.47626064756380170e-03),
    (3, 4, 5.31865721498712193e-04),
    (3, 5, -2.49160473628031712e-04),
    (4, 0, 4.17909062505244036e-04),
    (4, 1, -8.09454224369626535e-04),
    (4, 2, 4.82160507317217998e-04),
    (4, 3, -2.77161284127034290e-04),
    (4, 4, 4.24816797113955889e-05),
    (5, 0, -5.09236248410408605e-05),
    (5, 1, 8.16205235964646012e-05),
    (5, 2, -4.14746441352027126e-05),
    (5, 3, -8.77241047991263149e-07),
    (6, 0, 2.48623110650808863e-05),
    (6, 1, -2.12589239996419148e-05),
    (6, 2, 1.88189456805578678e-05),
    (7, 0, -5.75218932960625229e-06),
    (7, 1, 1.84050185911749430e-05),
    (8, 0, 4.45170495784547289e-06),
];
fn piece_r4_7(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R4_7_X0, R4_7_X1);
    let y = scale(h, R4_7_H0, R4_7_H1);
    let z = cheb2_sparse(x, y, &R4_7);
    z.exp()
}
const R4_8_X0: f64 = 2.44726544610969654e+00;
const R4_8_X1: f64 = 2.53141373995940677e+00;
const R4_8_H0: f64 = 4.98789780561862301e+02;
const R4_8_H1: f64 = 2.15531082290974427e+03;
const R4_8: [(usize, usize, f64); 45] = [
    (0, 0, 1.30332362392781720e+00),
    (0, 1, 2.29537233390849638e+00),
    (0, 2, -5.54055614191510504e-01),
    (0, 3, 1.11044683953067630e-01),
    (0, 4, -2.85189562388344989e-02),
    (0, 5, 6.46062241388652213e-04),
    (0, 6, -2.27944340209216504e-03),
    (0, 7, -7.25826723282906386e-04),
    (0, 8, -8.26554843743935574e-04),
    (1, 0, 3.29796468670067386e-02),
    (1, 1, 2.14792356361699494e-02),
    (1, 2, -7.21077125782895127e-03),
    (1, 3, -6.27126577606806554e-03),
    (1, 4, -1.95507749938378644e-03),
    (1, 5, -2.33971746508201267e-03),
    (1, 6, -9.04370433764009455e-04),
    (1, 7, -7.04877536204311659e-04),
    (2, 0, -1.36280133627123467e-03),
    (2, 1, 4.28119492214044559e-04),
    (2, 2, -1.71245437916393330e-03),
    (2, 3, 4.74226413580922286e-04),
    (2, 4, -8.90133851788348605e-04),
    (2, 5, 5.21726065545170906e-05),
    (2, 6, -4.77585063458675911e-05),
    (3, 0, -5.50446453954048405e-04),
    (3, 1, 8.84974655693622730e-04),
    (3, 2, -7.34067052539376093e-04),
    (3, 3, 3.77280862483254052e-04),
    (3, 4, -2.34899759433842122e-04),
    (3, 5, 5.57426813477652385e-05),
    (4, 0, -2.08317679378550207e-04),
    (4, 1, 3.34856792982746491e-04),
    (4, 2, -4.48813963721847605e-04),
    (4, 3, 2.96403202256726491e-04),
    (4, 4, -2.18372767514189887e-04),
    (5, 0, 7.66002722530032145e-05),
    (5, 1, -1.45677325223270685e-04),
    (5, 2, 9.22384720145212394e-05),
    (5, 3, -7.08285641617703940e-05),
    (6, 0, 9.15694429225549870e-05),
    (6, 1, -1.06214998981210806e-04),
    (6, 2, 7.97118587492636578e-05),
    (7, 0, -6.70630925665289233e-06),
    (7, 1, 9.80877433514980982e-07),
    (8, 0, 1.58743606403062951e-05),
];
fn piece_r4_8(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R4_8_X0, R4_8_X1);
    let y = scale(h, R4_8_H0, R4_8_H1);
    let z = cheb2_sparse(x, y, &R4_8);
    z.exp()
}
const R4_9_X0: f64 = 2.53153548424576647e+00;
const R4_9_X1: f64 = 2.60198763490987339e+00;
const R4_9_H0: f64 = 4.74736906300579676e+02;
const R4_9_H1: f64 = 2.05356843751865745e+03;
const R4_9: [(usize, usize, f64); 45] = [
    (0, 0, 1.21106799795944764e+00),
    (0, 1, 2.38798154593615575e+00),
    (0, 2, -5.53079313047015519e-01),
    (0, 3, 1.07748136790766488e-01),
    (0, 4, -2.61003390193732879e-02),
    (0, 5, 1.61939728928066828e-03),
    (0, 6, -2.56332491159307237e-03),
    (0, 7, -6.85831284948510344e-05),
    (0, 8, -8.48898726334451951e-04),
    (1, 0, 2.45240954092336948e-02),
    (1, 1, 1.99447795761901123e-02),
    (1, 2, -4.06182301334530229e-03),
    (1, 3, -3.38299334811246414e-03),
    (1, 4, -8.58337699687604762e-04),
    (1, 5, -1.37412685305278340e-03),
    (1, 6, -1.31861912000875986e-04),
    (1, 7, -3.69773508255649046e-04),
    (2, 0, 1.07440900113356191e-03),
    (2, 1, -2.75312909634474511e-03),
    (2, 2, 2.15351432556601689e-03),
    (2, 3, -1.69344707362899228e-03),
    (2, 4, 1.11083796746375986e-03),
    (2, 5, -6.38212930150267011e-04),
    (2, 6, 2.91316678225038157e-04),
    (3, 0, -2.46684620816733197e-04),
    (3, 1, 5.14882047963874227e-04),
    (3, 2, -4.54125581955375647e-04),
    (3, 3, 3.95655172452529110e-04),
    (3, 4, -1.93991735724795912e-04),
    (3, 5, 1.00416171569340598e-04),
    (4, 0, 8.42541352896197137e-05),
    (4, 1, -6.16726929420690289e-05),
    (4, 2, 1.79816842802201259e-05),
    (4, 3, 5.01055304426786412e-05),
    (4, 4, -2.85837524748610471e-05),
    (5, 0, 6.99335230509755609e-05),
    (5, 1, -1.64488823802128823e-04),
    (5, 2, 7.75619938184682328e-05),
    (5, 3, -3.41208401582024744e-05),
    (6, 0, 1.22238353867973706e-04),
    (6, 1, -1.40542475653797312e-04),
    (6, 2, 9.39115321046720672e-05),
    (7, 0, -2.99353014854616233e-05),
    (7, 1, 1.26243090853939146e-05),
    (8, 0, 2.84062007651024446e-05),
];
fn piece_r4_9(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R4_9_X0, R4_9_X1);
    let y = scale(h, R4_9_H0, R4_9_H1);
    let z = cheb2_sparse(x, y, &R4_9);
    z.exp()
}
const R4_10_X0: f64 = 2.60215903294189799e+00;
const R4_10_X1: f64 = 2.95095418163826739e+00;
const R4_10_H0: f64 = 2.76411856425111353e+02;
const R4_10_H1: f64 = 1.96066520786037700e+03;
const R4_10: [(usize, usize, f64); 45] = [
    (0, 0, 6.44998706135134214e-01),
    (0, 1, 3.25001120220684481e+00),
    (0, 2, -8.38721420892318270e-01),
    (0, 3, 1.57044506453244137e-01),
    (0, 4, -5.40262398204180289e-02),
    (0, 5, -7.09826407545064418e-04),
    (0, 6, -5.81323117470868705e-03),
    (0, 7, -8.81662211612988488e-04),
    (0, 8, -5.29718343999182207e-04),
    (1, 0, 6.53471845188762918e-02),
    (1, 1, 5.57541319603996541e-02),
    (1, 2, -3.24144504954667345e-02),
    (1, 3, -3.58202970040674279e-02),
    (1, 4, -1.86536017212587495e-02),
    (1, 5, -9.69216331220255391e-03),
    (1, 6, -4.17407129416308940e-03),
    (1, 7, -1.38900404849983490e-03),
    (2, 0, -1.54500673213530289e-02),
    (2, 1, -1.94610016376115628e-02),
    (2, 2, -1.69399465314701049e-02),
    (2, 3, -9.03194560395416055e-03),
    (2, 4, -8.56169173760669595e-03),
    (2, 5, -2.23804477994006719e-03),
    (2, 6, -1.56549431945190651e-03),
    (3, 0, -1.03137728995355746e-03),
    (3, 1, -6.08009084039669214e-03),
    (3, 2, -2.36207952454589977e-03),
    (3, 3, -3.98862718778543804e-03),
    (3, 4, -9.16254220518822630e-04),
    (3, 5, -9.32174886991099625e-04),
    (4, 0, 2.77911351628354190e-04),
    (4, 1, 4.99759836129363212e-05),
    (4, 2, -3.96232490689319116e-05),
    (4, 3, -3.38023335125683944e-04),
    (4, 4, -1.34740352252393677e-04),
    (5, 0, 2.83005829314303121e-04),
    (5, 1, 5.40277955224186131e-04),
    (5, 2, 1.79554574034249858e-04),
    (5, 3, 4.10746407214945507e-05),
    (6, 0, 8.53435869570538422e-05),
    (6, 1, 2.06204102861212881e-04),
    (6, 2, 2.87827630378353392e-05),
    (7, 0, 1.72288274201421661e-05),
    (7, 1, 5.34085939515475461e-05),
    (8, 0, 9.04342970753289550e-06),
];
fn piece_r4_10(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R4_10_X0, R4_10_X1);
    let y = scale(h, R4_10_H0, R4_10_H1);
    let z = cheb2_sparse(x, y, &R4_10);
    z.exp()
}
const R5_X0: f64 = -3.16832143336669692e+00;
const R5_X1: f64 = 2.02758741479910887e+00;
const R5_H0: f64 = 3.94191892089459998e+03;
const R5_H1: f64 = 7.37684463881971988e+03;
const R5: [(usize, usize, f64); 45] = [
    (0, 0, -1.62349593561094729e+00),
    (0, 1, 4.14869106405301891e-01),
    (0, 2, -5.60429676498306328e-02),
    (0, 3, 1.04703436877614468e-02),
    (0, 4, -2.10852138932301125e-03),
    (0, 5, 4.09757556428296356e-04),
    (0, 6, -7.11228416356982286e-05),
    (0, 7, 9.64419440302794621e-06),
    (0, 8, 1.00559487122920381e-06),
    (1, 0, 5.99329812777029147e+00),
    (1, 1, 7.46644585391774245e-03),
    (1, 2, -1.92226749125752453e-03),
    (1, 3, 4.27878174888308939e-04),
    (1, 4, -9.20939574576208240e-05),
    (1, 5, 1.44975846250267734e-05),
    (1, 6, -2.20273380652153398e-06),
    (1, 7, -2.51380542189050654e-06),
    (2, 0, 8.92156625794022010e-03),
    (2, 1, 5.42806478553330835e-03),
    (2, 2, -1.36179969087050785e-03),
    (2, 3, 2.95719373698135701e-04),
    (2, 4, -5.25288426307148805e-05),
    (2, 5, 9.29556543141935082e-06),
    (2, 6, 1.70758536068269035e-06),
    (3, 0, 5.96027696019938488e-03),
    (3, 1, 3.27885592516314033e-03),
    (3, 2, -7.79660047530295617e-04),
    (3, 3, 1.43418463242064447e-04),
    (3, 4, -2.20606364400852092e-05),
    (3, 5, -2.16537920499818429e-06),
    (4, 0, 3.60385168223094354e-03),
    (4, 1, 1.53623148091128095e-03),
    (4, 2, -3.23308604371701626e-04),
    (4, 3, 4.96267318688072321e-05),
    (4, 4, 2.48291535893299662e-06),
    (5, 0, 1.85845957300847728e-03),
    (5, 1, 5.76732626909082311e-04),
    (5, 2, -9.63478576215286213e-05),
    (5, 3, -1.97243037434940852e-06),
    (6, 0, 9.36761762741899302e-04),
    (6, 1, 1.18770412322662344e-04),
    (6, 2, 4.40467833303857262e-06),
    (7, 0, 3.57773381163052325e-04),
    (7, 1, -7.05407530233795886e-06),
    (8, 0, 1.57169922051158054e-04),
];
fn piece_r5(rho: f64, h: f64) -> f64 {
    let x = scale(rho.log10(), R5_X0, R5_X1);
    let y = scale(h, R5_H0, R5_H1);
    let z = cheb2_sparse(x, y, &R5);
    z.exp()
}
