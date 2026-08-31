//! Experimental explicit near-critical Region 4 `(h,s)` flash: saturation
//! pressure `p(h,s)` and vapour quality `x(h,s)` for states close to the
//! critical point.
//!
//! # Status — NOT an IAPWS release
//!
//! IAPWS-IF97 supplementary release S04 covers `(h,s)` backward equations, but
//! the near-critical band handled here is where this crate previously fell
//! back on an iterative pressure solve. These correlations are an in-house fit
//! intended to replace that fallback with a direct evaluation; they are not a
//! normative IAPWS release and must not be cited as IAPWS values.
//!
//! # Methodology
//!
//! `p(h,s)` is a tensor-product Chebyshev polynomial in scaled `h` and scaled
//! `s`, fitted in `log(p)` and exponentiated on evaluation — so the returned
//! pressure is positive by construction.
//!
//! Quality is then recovered without iteration: the fitted pressure is fed to
//! this crate's existing IAPWS Region 4 saturation-temperature backward
//! equation ([`sat_temp_4`]), and the saturated-liquid and saturated-vapour
//! enthalpies at that temperature come from two one-dimensional Chebyshev
//! series in scaled temperature. Quality is the usual lever rule
//! `x = (h - h_f) / (h_g - h_f)`.
//!
//! The saturation-temperature step reuses the crate's IAPWS equation rather
//! than the duplicate copy carried in the original prototype, so there is one
//! `T_sat(p)` implementation in this crate, not two.
//!
//! # Fit domain
//!
//! - Saturation temperature: `623.15 K <= T_sat <= 647.04 K` (the near-critical
//!   band; the critical point is 647.096 K)
//! - Enthalpy: `1672.2 <= h <= 2560.9 kJ/kg`
//! - Entropy: `3.780 <= s <= 5.206 kJ/(kg K)`
//!
//! # The `(h,s)` box is a bounding box, NOT the valid domain
//!
//! This is the sharpest edge on these correlations, and it is easy to get
//! wrong. The enthalpy and entropy ranges above are the bounding box of the
//! fitted data. The states actually fitted occupy a **curved two-phase wedge
//! inside that box**, because on the saturation line `h` and `s` are strongly
//! coupled — most `(h,s)` pairs drawn from the box are not near-critical
//! two-phase states at all.
//!
//! `p(h,s)` is fitted in `log(p)` with coefficients of order `1e4`, which
//! cancel to give a `log(p)` of order 3 on the wedge. Off the wedge that
//! cancellation does not happen and the exponential runs away: sampling the
//! bounding box uniformly on 2026-08-31 produced pressures as large as
//! `1e71 MPa`, while the same correlation evaluated on genuine two-phase
//! states reproduced IAPWS saturation pressure to a maximum relative error of
//! `1.14e-4`.
//!
//! So a large or absurd result here does not mean the fit is broken — it
//! almost certainly means the input `(h,s)` pair is not a near-critical
//! two-phase state. **Callers must establish that before calling.** Neither
//! function clamps or validates its input.
//!
//! # Known accuracy limitation
//!
//! Accuracy degrades approaching the critical point, where `h_g - h_f` tends
//! to zero and the quality lever rule is ill-conditioned: a small absolute
//! error in the fitted `h_f`/`h_g` becomes a large relative error in `x`. This
//! is inherent to the formulation, not to the fit. See the crate's "Known
//! accuracy pitfalls" note on Region 3/4 behaviour within ~0.5 K of `T_c`.
//!
//! # Verification results
//!
//! Measured 2026-08-31 against the crate's IAPWS `sat_pressure_4`, over 795
//! two-phase states generated forwards across the fitted band (saturation
//! temperature 623.15–647.04 K, vapour quality 0.05–0.95):
//!
//! | Statistic | Relative error in `p` |
//! |---|---|
//! | median | 1.34e-6 |
//! | 90th percentile | 7.96e-6 |
//! | 99th percentile | 5.90e-5 |
//! | maximum | 1.14e-4 |
//!
//! The reference is IAPWS throughout, so this is a genuine comparison against
//! IAPWS rather than a self-consistency check. It is **not** a validation
//! sign-off: no human has reviewed these results, and the quality correlation
//! `x(h,s)` is not covered by the above — only the pressure is. See
//! [`super::tests`] for the methodology and to reproduce the numbers.

use uom::si::{
    available_energy::kilojoule_per_kilogram, f64::*, pressure::megapascal,
    specific_heat_capacity::kilojoule_per_kilogram_kelvin, thermodynamic_temperature::kelvin,
};

use crate::region_4_vap_liq_equilibrium::sat_temp_4;

use super::chebyshev::{cheb1, cheb2_dense, scale};

const HMIN: f64 = 1.67224591548638750e+03;
const HMAX: f64 = 2.56094341917250858e+03;
const SMIN: f64 = 3.78047092212877844e+00;
const SMAX: f64 = 5.20618677696282361e+00;
const TLO: f64 = 623.15;
const THI: f64 = 647.04;
const P_HS_LOG_COEFFS: [[f64; 11]; 11] = [
    [
        2.32309865097255461e+04,
        -1.92612020813200179e+04,
        -1.15115573382592884e+04,
        4.89146531267570845e+03,
        4.05314657848180650e+03,
        3.40652887416941812e+04,
        -2.73995198636909954e+04,
        -4.31232923670718992e+03,
        -1.82191924024316759e+04,
        -2.39661449061447865e+03,
        -1.51559785494604239e+03,
    ],
    [
        -9.09378956190811732e+03,
        -1.04170459091191933e+04,
        2.26549361871354922e+04,
        9.38593392496360684e+03,
        -3.18508577995605519e+04,
        -6.18435202287371430e+03,
        1.00471964076455151e+01,
        3.42044705994441392e+04,
        1.03151670856396286e+04,
        5.65853617898009543e+03,
        7.71349135475452385e+02,
    ],
    [
        -1.28381814073311762e+04,
        3.38899085025583336e+04,
        -1.90079379346556670e+04,
        -1.79323510140184662e+04,
        3.02751825418872504e+04,
        -3.40439210502773931e+04,
        1.34545243113087054e+04,
        -9.18442287631244108e+03,
        1.17243904110697458e+04,
        1.93942867181978227e+03,
        2.17371162708070733e+03,
    ],
    [
        6.22360284566483278e+03,
        8.89327280985249672e+03,
        -1.38251059864502204e+04,
        -1.32823767780140133e+04,
        3.78233507067037935e+04,
        3.04773807554728819e+03,
        1.99269442038519924e+03,
        -3.06985663775594548e+04,
        -2.15240841761104894e+04,
        -1.08046959570473773e+04,
        -2.42184451949804134e+03,
    ],
    [
        4.00361765183352873e+02,
        -3.56395832858428548e+04,
        3.19833206902794300e+04,
        2.95111678261880297e+04,
        -4.70796489528619713e+04,
        3.49051869971128644e+03,
        1.43088828338235589e+03,
        4.46861653067642837e+04,
        2.27517489001295289e+04,
        1.08156599918612064e+04,
        8.66634891411693388e+02,
    ],
    [
        3.53686467931208754e+04,
        -7.08455302923267845e+03,
        -4.07726031191324146e+04,
        8.29727834299210372e+03,
        -7.55389913418229844e+03,
        3.23824655918734388e+04,
        -2.36388847938129256e+04,
        -2.89041345579297049e+04,
        -1.54355299352850307e+04,
        -5.27551598926028237e+03,
        5.34202853096227045e+02,
    ],
    [
        -2.24374119676815353e+04,
        -4.86380809016843159e+02,
        1.47131684922351051e+04,
        1.04340556247915902e+04,
        -5.79112733116828349e+03,
        -3.17878243172292678e+04,
        2.93930074380027945e+04,
        -3.70712965281392826e+02,
        1.33890441196088432e+04,
        -2.58923351240997044e+03,
        -5.38277150724425155e+01,
    ],
    [
        7.04732471961997362e+01,
        2.72041506710441790e+04,
        -9.99585227510957338e+03,
        -2.29529901961295764e+04,
        4.97556465893475397e+04,
        -2.71266681051165287e+04,
        2.42873931226690875e+04,
        -1.81327718782052907e+04,
        5.01590394498213027e+03,
        2.11681680610126932e+02,
        1.68801184522635594e+00,
    ],
    [
        -1.68578847021039182e+04,
        7.80638432145761089e+03,
        7.40152668173249185e+03,
        -2.25105753063885495e+04,
        1.88217835492138292e+04,
        -2.76588015346784268e+04,
        1.38197001053813638e+04,
        -4.85436317989615054e+03,
        -3.12095066418916758e+02,
        -5.01079015064715350e+00,
        -2.20405758618653635e-02,
    ],
    [
        -2.19879414584935148e+03,
        7.20274359745677066e+03,
        1.19395661875165706e+03,
        -7.58223415905545517e+03,
        1.31748767862571058e+04,
        -5.61949036022803739e+03,
        2.34693424002257098e+03,
        2.04456209985888563e+02,
        4.95757815060642315e+00,
        4.38148720204480924e-02,
        1.19225096568698063e-04,
    ],
    [
        -1.66333556522845583e+03,
        1.11482227143501223e+03,
        1.27748126491282164e+03,
        -2.39104094593010450e+03,
        9.52399111430651374e+02,
        -4.53444345711646747e+02,
        -5.02151091226742210e+01,
        -1.63480029488800938e+00,
        -2.17741632168326760e-02,
        -1.18947358714649454e-04,
        -2.35937477555125952e-07,
    ],
];

const HF_COEFFS: [f64; 19] = [
    1.80825398923045304e+03,
    1.60086291751135349e+02,
    3.29796694038905116e+01,
    1.61377380873189011e+01,
    9.66662493733826089e+00,
    6.36006374111844419e+00,
    4.43947932182388083e+00,
    3.21089148762468568e+00,
    2.39830127778323998e+00,
    1.81570857731877489e+00,
    1.41043346799058189e+00,
    1.09086187929476286e+00,
    8.69528867940877026e-01,
    6.72148342381608366e-01,
    5.45645092653918029e-01,
    4.06329115329288115e-01,
    3.34325448451074814e-01,
    2.13617117569744858e-01,
    1.77546365341095275e-01,
];

const HG_COEFFS: [f64; 19] = [
    2.42030857715089951e+03,
    -1.77823189442253124e+02,
    -4.90582249275414881e+01,
    -2.25951231273347233e+01,
    -1.30815718028869785e+01,
    -8.41169403350882661e+00,
    -5.77059020014656454e+00,
    -4.11328167727976179e+00,
    -3.02957447734499663e+00,
    -2.26466911704268092e+00,
    -1.73608052852095285e+00,
    -1.32737139664079895e+00,
    -1.04484734512887623e+00,
    -7.99671214150348808e-01,
    -6.41589138543912196e-01,
    -4.73947457268855010e-01,
    -3.85751679676533010e-01,
    -2.45022914453735641e-01,
    -2.01624984454697731e-01,
];

/// Experimental near-critical Region 4 saturation pressure from enthalpy and
/// entropy, on bare floats.
///
/// `h_kj_kg` is specific enthalpy in kJ/kg and `s_kj_kg_k` specific entropy in
/// kJ/(kg K); the return value is pressure in MPa. See the module
/// documentation for the fit domain.
#[inline]
pub fn p_hs_4_near_critical_explicit(h_kj_kg: f64, s_kj_kg_k: f64) -> f64 {
    let x = scale(h_kj_kg, HMIN, HMAX);
    let y = scale(s_kj_kg_k, SMIN, SMAX);
    cheb2_dense(x, y, &P_HS_LOG_COEFFS).exp()
}

/// Saturated-liquid enthalpy `h_f` on the near-critical branch, in kJ/kg, for
/// a saturation temperature `t_kelvin` in K.
///
/// Fitted over `623.15 K <= T_sat <= 647.04 K`.
#[inline]
pub fn h_f_near_critical_explicit(t_kelvin: f64) -> f64 {
    cheb1(scale(t_kelvin, TLO, THI), &HF_COEFFS)
}

/// Saturated-vapour enthalpy `h_g` on the near-critical branch, in kJ/kg, for
/// a saturation temperature `t_kelvin` in K.
///
/// Fitted over `623.15 K <= T_sat <= 647.04 K`.
#[inline]
pub fn h_g_near_critical_explicit(t_kelvin: f64) -> f64 {
    cheb1(scale(t_kelvin, TLO, THI), &HG_COEFFS)
}

/// Experimental near-critical Region 4 vapour quality from enthalpy and
/// entropy, on bare floats.
///
/// `h_kj_kg` is specific enthalpy in kJ/kg and `s_kj_kg_k` specific entropy in
/// kJ/(kg K); the return value is the dimensionless vapour mass fraction.
///
/// Accuracy degrades near the critical point, where `h_g - h_f` tends to zero
/// — see the module documentation.
pub fn x_hs_4_near_critical_explicit(h_kj_kg: f64, s_kj_kg_k: f64) -> f64 {
    let p_mpa = p_hs_4_near_critical_explicit(h_kj_kg, s_kj_kg_k);
    let t_sat = sat_temp_4(Pressure::new::<megapascal>(p_mpa)).get::<kelvin>();
    let h_f = h_f_near_critical_explicit(t_sat);
    let h_g = h_g_near_critical_explicit(t_sat);
    (h_kj_kg - h_f) / (h_g - h_f)
}

/// Experimental near-critical Region 4 saturation pressure from specific
/// enthalpy and specific entropy, `p(h,s)`.
///
/// This is the dimensioned entry point; it wraps
/// [`p_hs_4_near_critical_explicit`]. Valid for the near-critical band
/// `623.15 K <= T_sat <= 647.04 K` — see the module documentation.
pub fn p_hs_4_near_critical(h: AvailableEnergy, s: SpecificHeatCapacity) -> Pressure {
    let h_kj_kg = h.get::<kilojoule_per_kilogram>();
    let s_kj_kg_k = s.get::<kilojoule_per_kilogram_kelvin>();
    Pressure::new::<megapascal>(p_hs_4_near_critical_explicit(h_kj_kg, s_kj_kg_k))
}

/// Experimental near-critical Region 4 vapour quality from specific enthalpy
/// and specific entropy, `x(h,s)`.
///
/// This is the dimensioned entry point; it wraps
/// [`x_hs_4_near_critical_explicit`]. Returns the dimensionless vapour mass
/// fraction. Accuracy degrades near the critical point — see the module
/// documentation.
pub fn x_hs_4_near_critical(h: AvailableEnergy, s: SpecificHeatCapacity) -> Ratio {
    let h_kj_kg = h.get::<kilojoule_per_kilogram>();
    let s_kj_kg_k = s.get::<kilojoule_per_kilogram_kelvin>();
    Ratio::new::<uom::si::ratio::ratio>(x_hs_4_near_critical_explicit(h_kj_kg, s_kj_kg_k))
}
