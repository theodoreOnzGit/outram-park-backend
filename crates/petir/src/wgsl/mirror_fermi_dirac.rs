//! `f32` mirrors of `shaders/fermi_dirac.wgsl` — GSL's complete Fermi-Dirac
//! integrals `F_j(x)` at the seven fixed indices.
//!
//! Generated from the same parse of [`crate::specfunc::fermi_dirac`] that
//! produced the shader, so the two cannot drift in their 396 coefficients —
//! GSL's single-precision order, where the `f64` order would need 483 —
//! the largest table set in this module. See [`crate::wgsl::mirror`] for why
//! a mirror exists at all.
//!
//! # The blocker that turned out not to exist
//!
//! GSL's `fd_asymp` calls `fd_neg`, which can run a Levin *u*-transform over
//! two 101-element work arrays. That was recorded as the obstacle to this
//! shader. It is **never reached**: `fd_neg` is called only from `fd_asymp`,
//! which runs only at `x >= 30`, so it always sees `-x <= -30` and always
//! takes its simple alternating series. Established by instrumentation in
//! the `f64` module, which also showed the series converging in two to three
//! terms.
//!
//! Three things follow, and together they left nothing to solve:
//!
//! 1. **No scratch memory** — the acceleration is absent because it is
//!    unreachable.
//! 2. **The reflection term is exactly zero.** `fd_asymp` computes
//!    `cos(j pi) F_j(-x) + 2 seqn exp(...)`, and `cos(j pi)` vanishes
//!    identically for `j = -1/2, 1/2, 3/2` — the only indices that reach it.
//!    Upstream carries it because its `j` is a runtime parameter; ours is
//!    not. Measured contribution before dropping: 1e-31 to 1e-64 relative.
//! 3. **`lnGamma(j+2)` is a compile-time constant** at each of the three, and
//!    `eta(2n)` is a nine-entry table, so neither `gamma` nor `psi_zeta` is a
//!    dependency.
//!
//! # Four machine constants, and one of them corrects upstream
//!
//! | constant | `f64` | `f32` | kind |
//! |---|---|---|---|
//! | `DBL_EPSILON`, series break | 2.22e-16 | 1.1920929e-07 | precision — **retargeted** |
//! | `LOG_DBL_MIN`, underflow | -708.396 | -708.396 | range guard — **kept** |
//! | `1/sqrt(EPSILON)`, `F_1` far cut | 6.711e+07 | 2896.3093 | precision — **retargeted** |
//! | `1/cbrt(EPSILON)`, `F_2` far cut | 1.651e+05 | **2896.3093** | precision **and a corrected formula** |
//!
//! The underflow guard is [`crate::wgsl::mirror_synchrotron`]'s case: a bound
//! derived from the **exponent range** is enforced by the arithmetic anyway,
//! and retargeting it to `f32`'s -87.3365 would discard representable
//! denormals, since `F_j(x) ~ e^x` and `e^{-90}` is a perfectly good `f32`.
//!
//! **The last row is the interesting one.** Past its far cut each of `F_1`
//! and `F_2` drops a Chebyshev fit in `60/x` for the pure degenerate limit,
//! and what the fit carries is the Sommerfeld correction — `1 + (pi^2/3)/x^2`
//! for `F_1`, `1 + pi^2/x^2` for `F_2`. Both have the same `1/x^2` shape, so
//! both want the same **square** root. Upstream uses a **cube** root for
//! `F_2`, apparently matching its `x^3` factor rather than the accuracy
//! requirement, and pays 3.62e-10 for it. Carrying that form into `f32`
//! would cost **2.4e-04**, two thousand `f32` ulp. So the constant is
//! retargeted *and* the formula is corrected — [`crate::wgsl::mirror_lambert`]'s
//! case exactly.
//!
//! The `f64` module reproduces upstream's mistake faithfully and asserts it,
//! because its bar is agreement with GSL. This module's bar is what `f32` can
//! do. `the_f_2_far_cut_corrects_upstream_and_it_is_worth_2400_ulp` measures
//! the difference.
//!
//! # A fifth kind of constant decision: one that cannot be written down
//!
//! The two **overflow** bounds, `GSL_SQRT_DBL_MAX = 1.34e154` and
//! `GSL_ROOT3_DBL_MAX = 5.64e102`, are **deleted** rather than kept or
//! retargeted. Neither is an `f32` — the type stops at 3.4e38 — so both
//! options are unavailable and the branch simply goes. The arithmetic
//! overflows on its own at about 2.6e19 for `x*x` and 1.4e13 for `x*x*x`,
//! returning the same infinity upstream's `OVERFLOW_ERROR` would have.
//!
//! That is a third outcome beside *keep* and *retarget*, and unlike the
//! other two it is **forced, not chosen**: a constant one cannot write down
//! cannot be a guard. `the_overflow_guards_could_not_be_kept_at_all`
//! measures that deleting them changes no answer.

// Under a std-linked build (`cargo test`) f32's inherent powf/exp/sqrt shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// Relative tolerance for the alternating series, retargeted to `f32`.
/// Mirrors `PETIR_FD_EPS`.
const EPS: f32 = f32::EPSILON;
/// GSL's `GSL_LOG_DBL_MIN`, **kept at its `f64` value on purpose** — see the
/// module documentation. Mirrors `PETIR_FD_LOG_MIN`.
const LOG_MIN: f32 = -708.396_42;
/// `1/sqrt(f32::EPSILON)`, the far-field cut for **both** `F_1` and `F_2`.
/// Mirrors `PETIR_FD_FAR_CUT`.
const FAR_CUT: f32 = 2896.3093;
/// The `f32` analogue of upstream's `1/GSL_ROOT3_DBL_EPSILON`, which this
/// module deliberately does **not** use. Kept so the comparison can be
/// measured.
#[cfg(test)]
const F2_CUBE_ROOT_CUT: f32 = 203.187_33;
/// `ln Gamma(3/2)`. Mirrors `PETIR_FD_LG_MHALF`.
const LG_MHALF: f32 = -0.120_782_24;
/// `ln Gamma(5/2)`. Mirrors `PETIR_FD_LG_HALF`.
const LG_HALF: f32 = 0.284_682_87;
/// `ln Gamma(7/2)`. Mirrors `PETIR_FD_LG_3HALF`.
const LG_3HALF: f32 = 1.200_973_6;

/// GSL's `fd_1_a_data` at its SINGLE-PRECISION order: 13 of the 22 stored,
/// where `f64` evaluates 22.
#[rustfmt::skip]
const FD_1_A: [f32; 13] = [
    1.894934058189392, 0.7237719297409058, 0.125, 0.010106519795954227, 0.0,
    -6.0061523981858045e-05, 0.0, 6.816528639319586e-07, 0.0, -9.58957802055238e-09, 0.0,
    1.5151041532490694e-10, 0.0,
];

/// GSL's `fd_1_b_data` at its SINGLE-PRECISION order: 12 of the 22 stored,
/// where `f64` evaluates 22.
#[rustfmt::skip]
const FD_1_B: [f32; 12] = [
    10.409136772155762, 3.899445056915283, 0.5135109424591064, 0.01061873696744442,
    -0.0015844680601730943, 0.0001461392967030406, -1.4080957271289662e-06,
    -2.1779937924293336e-06, 3.9142366858868627e-07, -2.3860263098640644e-08,
    -4.138309694923237e-09, 1.2839652674401236e-09,
];

/// GSL's `fd_1_c_data` at its SINGLE-PRECISION order: 14 of the 23 stored,
/// where `f64` evaluates 23.
#[rustfmt::skip]
const FD_1_C: [f32; 14] = [
    56.7809944152832, 21.007184982299805, 2.245924472808838, 0.0017379363998770714,
    -0.000587164715398103, 0.00016306959150824696, -3.817425749730319e-05,
    7.645272489753552e-06, -1.3134849723428488e-06, 1.9000646034328383e-07,
    -2.1413281814375296e-08, 1.2390637404990912e-09, 2.1848048370465278e-10,
    -1.0134282302232123e-10,
];

/// GSL's `fd_1_d_data` at its SINGLE-PRECISION order: 15 of the 30 stored,
/// where `f64` evaluates 30.
#[rustfmt::skip]
const FD_1_D: [f32; 15] = [
    1.012662649154663, -0.00633125239983201, 0.002483731834217906, -0.0008764333906583488,
    0.0002913344360422343, -9.31877875700593e-05, 2.9015134714427404e-05,
    -8.854870429786388e-06, 2.6603474907460622e-06, -7.89141552104411e-07,
    2.315730256441384e-07, -6.731794854886175e-08, 1.9404803097700096e-08,
    -5.5507127783016585e-09, 1.576609065523371e-09,
];

/// GSL's `fd_1_e_data` at its SINGLE-PRECISION order: 5 of the 10 stored,
/// where `f64` evaluates 10.
#[rustfmt::skip]
const FD_1_E: [f32; 5] = [
    1.0013707876205444, 0.0009138522436842322, 0.00022846306092105806,
    -1.5700000522819772e-17, -1.2700000075185928e-17,
];

/// GSL's `fd_2_a_data` at its SINGLE-PRECISION order: 13 of the 21 stored,
/// where `f64` evaluates 21.
#[rustfmt::skip]
const FD_2_A: [f32; 13] = [
    2.1573662757873535, 0.884967029094696, 0.17841634154319763, 0.02083333395421505,
    0.0012708225985988975, 0.0, -5.0619314606592525e-06, 0.0, 4.320265389878841e-08, 0.0,
    -4.870543968138463e-10, 0.0, 6.420374214916036e-12,
];

/// GSL's `fd_2_b_data` at its SINGLE-PRECISION order: 13 of the 22 stored,
/// where `f64` evaluates 22.
#[rustfmt::skip]
const FD_2_B: [f32; 13] = [
    16.508258819580078, 7.421719551086426, 1.4583098888397217, 0.12877385318279266,
    0.001963611925020814, -0.0002374589821556583, 1.8539662050898187e-05,
    -1.9280564345081075e-07, -2.0195003003209422e-07, 3.296349859738257e-08,
    -1.8858170580671185e-09, -2.7263274970934503e-10, 8.055456302002995e-11,
];

/// GSL's `fd_2_c_data` at its SINGLE-PRECISION order: 13 of the 20 stored,
/// where `f64` evaluates 20.
#[rustfmt::skip]
const FD_2_C: [f32; 13] = [
    168.8712921142578, 81.80260467529297, 15.754084587097168, 1.1232558488845825,
    0.0005905750440433621, -0.00016469713591504842, 3.8856076571391895e-05,
    -7.898736839706544e-06, 1.3978624338051304e-06, -2.1534528116262663e-07,
    2.83151102564716e-08, -2.949785748995737e-09, 1.6755082044017655e-10,
];

/// GSL's `fd_2_d_data` at its SINGLE-PRECISION order: 15 of the 30 stored,
/// where `f64` evaluates 30.
#[rustfmt::skip]
const FD_2_D: [f32; 15] = [
    0.34599605202674866, -0.006331364158540964, 0.0024838296230882406,
    -0.0008765119127929211, 0.0002913925563916564, -9.322746336692944e-05,
    2.90402185783023e-05, -8.869622433849145e-06, 2.6684497242968064e-06,
    -7.933156780381978e-07, 2.3359868350780744e-07, -6.824790688142457e-08,
    1.9810364904060407e-08, -5.7194040614660935e-09, 1.6437942118585624e-09,
];

/// GSL's `fd_2_e_data`, 4 coefficients.
#[rustfmt::skip]
const FD_2_E: [f32; 4] = [
    0.33470410108566284, 0.0009138522436842322, 0.00022846306092105806,
    5.200000093474659e-19,
];

/// GSL's `fd_mhalf_a_data` at its SINGLE-PRECISION order: 13 of the 20 stored,
/// where `f64` evaluates 20.
#[rustfmt::skip]
const FD_MHALF_A: [f32; 20] = [
    1.266329050064087, 0.36978763341903687, 0.027813101187348366, -0.003333284752443433,
    -0.00044381082989275455, 6.16495162830688e-05, 8.758961485000327e-06,
    -1.2622937219930463e-06, -1.8374640831098077e-07, 2.6949509290830065e-08,
    3.976086571100268e-09, -5.894468801947994e-10, -8.773216181312549e-11,
    1.310165691215115e-11, 1.9621619867099538e-12, -2.9458870781130797e-13,
    -4.432340115051968e-14, 6.681600022882209e-15, 1.0084000293654146e-15,
    -1.5610000625196043e-16,
];

/// GSL's `fd_mhalf_b_data` at its SINGLE-PRECISION order: 13 of the 20 stored,
/// where `f64` evaluates 20.
#[rustfmt::skip]
const FD_MHALF_B: [f32; 20] = [
    3.270796060562134, 0.5809004902839661, -0.029931344091892242, -0.0013287935871630907,
    0.000991022097878158, -0.000169095495948568, 6.595585091417888e-06,
    3.59539649252838e-06, -9.430672207599855e-07, 8.757739777820461e-08,
    1.0624765067746011e-08, -4.958700561275009e-09, 7.160432802244543e-10,
    4.507221852689813e-12, -2.3695425829806105e-11, 4.9122208037322146e-12,
    -2.9052769304899195e-13, -9.592909691429063e-14, 3.000280145853962e-14,
    -3.4970000866948493e-15,
];

/// GSL's `fd_mhalf_c_data` at its SINGLE-PRECISION order: 14 of the 25 stored,
/// where `f64` evaluates 25.
#[rustfmt::skip]
const FD_MHALF_C: [f32; 25] = [
    5.828283309936523, 0.6775211095809937, -0.043946247547864914, 0.005825595930218697,
    -0.0008648589137010276, 0.00011001789243891835, -6.9733050622744486e-06,
    -1.7162674339488149e-06, 8.598115641689219e-07, -2.3306678542667214e-07,
    4.850319257343472e-08, -8.130620621216167e-09, 1.0210682299671703e-09,
    -5.318842241641697e-11, -1.9430559591859797e-11, 8.750506395871493e-12,
    -2.324897002345394e-12, 4.831020024124999e-13, -8.120700021718025e-14,
    1.0131999744950916e-14, -4.639999898246958e-16, -2.2400000695354746e-16,
    9.700000289296573e-17, -2.600000057077087e-17, 4.999999918875795e-18,
];

/// GSL's `fd_mhalf_d_data` at its SINGLE-PRECISION order: 16 of the 30 stored,
/// where `f64` evaluates 30.
#[rustfmt::skip]
const FD_MHALF_D: [f32; 30] = [
    2.2530744075775146, 0.0018745153211057186, -0.0007550198351964355,
    0.0002759818744380027, -9.594063158147037e-05, 3.240568548790179e-05,
    -1.0746239240688737e-05, 3.512686589601799e-06, -1.1313072718621697e-06,
    3.577454208425479e-07, -1.1049266390728008e-07, 3.313041574415365e-08,
    -9.583738247442852e-09, 2.6575790457172843e-09, -7.015201197724252e-10,
    1.7471113444855746e-10, -4.0490961278338844e-11, 8.51049959671446e-12,
    -1.5261884638018142e-12, 1.8768509966668456e-13, 1.0057399859220335e-14,
    -1.820020032578494e-14, 8.663399621441307e-15, -3.2057999003077143e-15,
    1.0572000326764582e-15, -3.259000028021507e-16, 9.600000108939323e-17,
    -2.740000044879442e-17, 7.59999981051676e-18, -1.89999995262919e-18,
];

/// GSL's `fd_half_a_data` at its SINGLE-PRECISION order: 12 of the 23 stored,
/// where `f64` evaluates 23.
#[rustfmt::skip]
const FD_HALF_A: [f32; 23] = [
    1.7177138328552246, 0.619257926940918, 0.09328022599220276, 0.004709485452622175,
    -0.0004243667935952544, -4.525698022916913e-05, 5.242650786385639e-06,
    6.387648454619921e-07, -8.057769917968471e-08, -1.0429027419434078e-08,
    1.37694777802011e-09, 1.8471903173722382e-10, -2.5106189696644243e-11,
    -3.449781692949072e-12, 4.784372767754896e-13, 6.688279894110846e-14,
    -9.414700057963979e-15, -1.333299950954654e-15, 1.8980000615186082e-16,
    2.7199999757207672e-17, -3.899999837461447e-18, -6.000000068087077e-19,
    9.999999682655225e-20,
];

/// GSL's `fd_half_b_data` at its SINGLE-PRECISION order: 13 of the 20 stored,
/// where `f64` evaluates 20.
#[rustfmt::skip]
const FD_HALF_B: [f32; 20] = [
    7.6510138511657715, 2.475545644760132, 0.21833598613739014, -0.007730591576546431,
    -0.0002174433902837336, 0.00014766397362109274, -2.1586361981462687e-05,
    8.077127517935878e-07, 3.2885805012483615e-07, -7.947433289245964e-08,
    6.940207075700755e-09, 6.755946913017397e-10, -3.102004764166111e-10,
    4.2677233275112414e-11, -2.169600016679353e-14, -1.1702449989950403e-12,
    2.347570109226954e-13, -1.4139000063253474e-14, -3.863999980982351e-15,
    1.2019999973722256e-15,
];

/// GSL's `fd_half_c_data` at its SINGLE-PRECISION order: 14 of the 23 stored,
/// where `f64` evaluates 23.
#[rustfmt::skip]
const FD_HALF_C: [f32; 23] = [
    29.584339141845703, 8.808343887329102, 0.5037716627120972, -0.021540695801377296,
    0.00214334181509912, -0.0002573656674940139, 2.7933539968216792e-05,
    -1.6785249954409664e-06, -2.7810011715700966e-07, 1.3521805897198647e-07,
    -3.3740423788231055e-08, 6.474834890468628e-09, -1.0096790070690531e-09,
    1.2005756111488353e-10, -6.636313894248236e-12, -1.7105660421110058e-12,
    7.750689738454664e-13, -1.9797299921370942e-13, 3.941400157636554e-14,
    -6.37400000808681e-15, 7.769999984775188e-16, -3.999999935100636e-17,
    -1.4000000434596716e-17,
];

/// GSL's `fd_half_d_data` at its SINGLE-PRECISION order: 16 of the 30 stored,
/// where `f64` evaluates 30.
#[rustfmt::skip]
const FD_HALF_D: [f32; 30] = [
    1.5116909742355347, -0.00360434059984982, 0.0014207743806764483, -0.0005045399302616715,
    0.000169075807207264, -5.463058550958522e-05, 1.7222322640009224e-05,
    -5.335260539141018e-06, 1.6315287894030917e-06, -4.939021209793282e-07,
    1.4825154437403398e-07, -4.4155228806630475e-08, 1.3050316383100835e-08,
    -3.826260197570264e-09, 1.1123226784093276e-09, -3.2047656195466345e-10,
    9.148704710471023e-11, -2.5877895312720334e-11, 7.255073278256141e-12,
    -2.0172225435183266e-12, 5.566890806836533e-13, -1.526246983614074e-13,
    4.16120995083355e-14, -1.1293299703328454e-14, 3.053700099793391e-15,
    -8.233999868720765e-16, 2.215000024446162e-16, -5.950000143344574e-17,
    1.5899999560045294e-17, -4.00000018325482e-18,
];

/// GSL's `fd_3half_a_data` at its SINGLE-PRECISION order: 12 of the 20 stored,
/// where `f64` evaluates 20.
#[rustfmt::skip]
const FD_3HALF_A: [f32; 20] = [
    2.0404775142669678, 0.8122168183326721, 0.1536371111869812, 0.015617432072758675,
    0.00059434276772663, -4.2960946302628145e-05, -3.824645318672992e-06,
    3.8023063098080456e-07, 4.0574615667310354e-08, -4.553036170307223e-09,
    -5.306873274157908e-10, 6.372972982671143e-11, 7.84036724432724e-12,
    -9.840240601521888e-13, -1.2559519938816488e-13, 1.6261699357423588e-14,
    2.131800027913651e-15, -2.824999933485309e-16, -3.7800000015358277e-17,
    5.099999851078862e-18,
];

/// GSL's `fd_3half_b_data` at its SINGLE-PRECISION order: 13 of the 22 stored,
/// where `f64` evaluates 22.
#[rustfmt::skip]
const FD_3HALF_B: [f32; 22] = [
    13.403206825256348, 5.574508190155029, 0.9312285780906677, 0.05463835597038269,
    -0.0014771729474887252, -2.9378554245340638e-05, 1.835703369579278e-05,
    -2.3480592972191516e-06, 8.317378785704932e-08, 2.68264876979174e-08,
    -6.01124439114642e-09, 4.94346008572677e-10, 3.955734004246203e-11,
    -1.7894930329220848e-11, 2.3489719284258692e-12, -1.2822999701455305e-14,
    -5.4191999059862925e-14, 1.0527000007375803e-14, -6.390000142823089e-16,
    -1.4700000042736246e-16, 4.4999998442701544e-17, -4.999999918875795e-18,
];

/// GSL's `fd_3half_c_data` at its SINGLE-PRECISION order: 13 of the 21 stored,
/// where `f64` evaluates 21.
#[rustfmt::skip]
const FD_3HALF_C: [f32; 21] = [
    101.03684997558594, 43.620853424072266, 6.622413635253906, 0.25081413984298706,
    -0.007981248199939728, 0.0006346224690787494, -6.392179057002068e-05,
    6.045351256034337e-06, -3.400768378014618e-07, -4.0726614969344155e-08,
    1.9311483967499044e-08, -4.463283520550476e-09, 7.943471436178129e-10,
    -1.1573569186351662e-10, 1.3046580309150624e-11, -7.411400052698136e-13,
    -1.41809998531299e-13, 6.4909998220581e-14, -1.5969999845114047e-14,
    3.0500000481215473e-15, -4.800000186818559e-16,
];

/// GSL's `fd_3half_d_data` at its SINGLE-PRECISION order: 17 of the 25 stored,
/// where `f64` evaluates 25.
#[rustfmt::skip]
const FD_3HALF_D: [f32; 25] = [
    0.6160645484924316, -0.007123948074877262, 0.002790686674416065, -0.000982952187769115,
    0.0003260229714214802, -0.00010401609324617311, 3.229312278563157e-05,
    -9.82435085461475e-06, 2.942013225037954e-06, -8.699154818714305e-07,
    2.5454599494878494e-07, -7.383050615317188e-08, 2.1254567883488562e-08,
    -6.079653225299353e-09, 1.7294556897695657e-09, -4.896540950483086e-10,
    1.380786041060844e-10, -3.8805729463131655e-11, 1.0875321476699895e-11,
    -3.040730861547658e-12, 8.485625938893515e-13, -2.3642749541995245e-13,
    6.576359736724241e-14, -1.8180699933273252e-14, 4.688400207886016e-15,
];

/// Dirichlet eta at even integers, `eta(2n)` for `n = 1..9`. Nine is
/// generous: `f32` needs three at `x = 30` and two past `x = 100`, where
/// `f64` needs seventeen. Mirrors `petir_fd_eta`.
#[rustfmt::skip]
const ETA: [f32; 9] = [
    0.82246703, 0.94703283, 0.98555109, 0.99623300, 0.99903951,
    0.99975769, 0.99993917, 0.99998476, 0.99999619,
];

/// Clenshaw in GSL's convention, on one of the 22 tables.
fn cheb(c: &[f32], x: f32) -> f32 {
    let Some((&c0, rest)) = c.split_first() else {
        return 0.0;
    };
    let (mut d, mut dd) = (0.0_f32, 0.0_f32);
    let y2 = 2.0 * x;
    for &ci in rest.iter().rev() {
        let temp = d;
        d = y2 * d - dd + ci;
        dd = temp;
    }
    x * d - dd + 0.5 * c0
}

/// Goano's alternating series for an integer index. Mirrors
/// `petir_fd_series`.
fn series(x: f32, p: u32) -> f32 {
    let ex = x.exp();
    let mut term = ex;
    let mut sum = term;
    for n in 2..100 {
        let rat = (n as f32 - 1.0) / n as f32;
        let mut r = 1.0_f32;
        for _ in 0..p {
            r *= rat;
        }
        term *= -ex * r;
        sum += term;
        if (term / sum).abs() < EPS {
            break;
        }
    }
    sum
}

/// The same series for a half-integer index. Mirrors
/// `petir_fd_series_half`.
fn series_half(x: f32, whole: u32) -> f32 {
    let ex = x.exp();
    let mut term = ex;
    let mut sum = term;
    for n in 2..200 {
        let rat = (n as f32 - 1.0) / n as f32;
        let mut r = rat.sqrt();
        for _ in 0..whole {
            r *= rat;
        }
        term *= -ex * r;
        sum += term;
        if (term / sum).abs() < EPS {
            break;
        }
    }
    sum
}

/// GSL's `fd_asymp` **without the reflection term**, which is identically
/// zero at the three half-integer indices. Mirrors `petir_fd_asymp`.
fn asymp(j: f32, lg: f32, x: f32) -> f32 {
    let mut seqn = 0.5_f32;
    let xm2 = (1.0 / x) / x;
    let mut xgam = 1.0_f32;
    let mut add = f32::MAX;
    for (n, &eta) in ETA.iter().enumerate() {
        let n = n as f32 + 1.0;
        let add_previous = add;
        xgam = xgam * xm2 * (j + 1.0 - (2.0 * n - 2.0)) * (j + 1.0 - (2.0 * n - 1.0));
        add = eta * xgam;
        // j is never an integer here, so the series is asymptotic and must be
        // truncated at its smallest term.
        if add.abs() > add_previous.abs() {
            break;
        }
        if (add / seqn).abs() < EPS {
            break;
        }
        seqn += add;
    }
    2.0 * seqn * ((j + 1.0) * x.ln() - lg).exp()
}

/// `F_{-1}(x) = e^x/(1+e^x)`, the Fermi occupation function. Mirrors
/// `petir_fd_m1`.
pub fn fermi_dirac_m1(x: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    if x < LOG_MIN {
        return 0.0;
    }
    if x < 0.0 {
        let ex = x.exp();
        ex / (1.0 + ex)
    } else {
        1.0 / (1.0 + (-x).exp())
    }
}

/// `F_0(x) = ln(1 + e^x)`. Mirrors `petir_fd_0`.
pub fn fermi_dirac_0(x: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    if x < LOG_MIN {
        return 0.0;
    }
    if x < -5.0 {
        let ex = x.exp();
        ex * (1.0 - ex * (0.5 - ex * (1.0 / 3.0 - ex * (0.25 - ex * (0.2 - ex / 6.0)))))
    } else if x < 10.0 {
        (1.0 + x.exp()).ln()
    } else {
        let ex = (-x).exp();
        x + ex * (1.0 - 0.5 * ex + ex * ex / 3.0 - ex * ex * ex / 4.0)
    }
}

/// `F_1(x)`. Mirrors `petir_fd_1`.
pub fn fermi_dirac_1(x: f32) -> f32 {
    f1_with(x, FAR_CUT)
}

/// [`fermi_dirac_1`] with the far-field cut supplied, so its placement can be
/// **measured** rather than argued. [`FAR_CUT`] ships.
fn f1_with(x: f32, cut: f32) -> f32 {
    if x.is_nan() {
        f32::NAN
    } else if x < LOG_MIN {
        0.0
    } else if x < -1.0 {
        series(x, 2)
    } else if x < 1.0 {
        cheb(&FD_1_A, x)
    } else if x < 4.0 {
        cheb(&FD_1_B, 2.0 / 3.0 * (x - 1.0) - 1.0)
    } else if x < 10.0 {
        cheb(&FD_1_C, 1.0 / 3.0 * (x - 4.0) - 1.0)
    } else if x < 30.0 {
        cheb(&FD_1_D, 0.1 * x - 2.0) * x * x
    } else if x < cut {
        cheb(&FD_1_E, 60.0 / x - 1.0) * x * x
    } else {
        // Upstream's GSL_SQRT_DBL_MAX guard is not representable as an f32,
        // so it is gone; `x * x` overflows to infinity on its own.
        0.5 * x * x
    }
}

/// `F_2(x)`. Mirrors `petir_fd_2`.
pub fn fermi_dirac_2(x: f32) -> f32 {
    f2_with(x, FAR_CUT)
}

/// [`fermi_dirac_2`] with the far-field cut supplied. **This is the one
/// whose upstream formula is corrected** — see the module documentation.
fn f2_with(x: f32, cut: f32) -> f32 {
    if x.is_nan() {
        f32::NAN
    } else if x < LOG_MIN {
        0.0
    } else if x < -1.0 {
        series(x, 3)
    } else if x < 1.0 {
        cheb(&FD_2_A, x)
    } else if x < 4.0 {
        cheb(&FD_2_B, 2.0 / 3.0 * (x - 1.0) - 1.0)
    } else if x < 10.0 {
        cheb(&FD_2_C, 1.0 / 3.0 * (x - 4.0) - 1.0)
    } else if x < 30.0 {
        cheb(&FD_2_D, 0.1 * x - 2.0) * x * x * x
    } else if x < cut {
        cheb(&FD_2_E, 60.0 / x - 1.0) * x * x * x
    } else {
        // Likewise GSL_ROOT3_DBL_MAX.
        x * x * x / 6.0
    }
}

/// `F_{-1/2}(x)`. Mirrors `petir_fd_mhalf`.
pub fn fermi_dirac_mhalf(x: f32) -> f32 {
    if x.is_nan() {
        f32::NAN
    } else if x < LOG_MIN {
        0.0
    } else if x < -1.0 {
        series_half(x, 0)
    } else if x < 1.0 {
        cheb(&FD_MHALF_A, x)
    } else if x < 4.0 {
        cheb(&FD_MHALF_B, 2.0 / 3.0 * (x - 1.0) - 1.0)
    } else if x < 10.0 {
        cheb(&FD_MHALF_C, 1.0 / 3.0 * (x - 4.0) - 1.0)
    } else if x < 30.0 {
        cheb(&FD_MHALF_D, 0.1 * x - 2.0) * x.sqrt()
    } else {
        asymp(-0.5, LG_MHALF, x)
    }
}

/// `F_{1/2}(x)`, the one the semiconductor literature means by "the
/// Fermi-Dirac integral". Mirrors `petir_fd_half`.
pub fn fermi_dirac_half(x: f32) -> f32 {
    if x.is_nan() {
        f32::NAN
    } else if x < LOG_MIN {
        0.0
    } else if x < -1.0 {
        series_half(x, 1)
    } else if x < 1.0 {
        cheb(&FD_HALF_A, x)
    } else if x < 4.0 {
        cheb(&FD_HALF_B, 2.0 / 3.0 * (x - 1.0) - 1.0)
    } else if x < 10.0 {
        cheb(&FD_HALF_C, 1.0 / 3.0 * (x - 4.0) - 1.0)
    } else if x < 30.0 {
        cheb(&FD_HALF_D, 0.1 * x - 2.0) * (x * x.sqrt())
    } else {
        asymp(0.5, LG_HALF, x)
    }
}

/// `F_{3/2}(x)`. Mirrors `petir_fd_3half`.
pub fn fermi_dirac_3half(x: f32) -> f32 {
    if x.is_nan() {
        f32::NAN
    } else if x < LOG_MIN {
        0.0
    } else if x < -1.0 {
        series_half(x, 2)
    } else if x < 1.0 {
        cheb(&FD_3HALF_A, x)
    } else if x < 4.0 {
        cheb(&FD_3HALF_B, 2.0 / 3.0 * (x - 1.0) - 1.0)
    } else if x < 10.0 {
        cheb(&FD_3HALF_C, 1.0 / 3.0 * (x - 4.0) - 1.0)
    } else if x < 30.0 {
        cheb(&FD_3HALF_D, 0.1 * x - 2.0) * (x * x * x.sqrt())
    } else {
        asymp(1.5, LG_3HALF, x)
    }
}

/// One dispatcher over the seven indices, in ascending order of `j`.
/// Mirrors `petir_fermi_dirac`.
pub fn fermi_dirac(which: u32, x: f32) -> f32 {
    match which {
        0 => fermi_dirac_m1(x),
        1 => fermi_dirac_mhalf(x),
        2 => fermi_dirac_0(x),
        3 => fermi_dirac_half(x),
        4 => fermi_dirac_1(x),
        5 => fermi_dirac_3half(x),
        6 => fermi_dirac_2(x),
        _ => f32::NAN,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::fermi_dirac as f64fd;

    /// The seven mirrors beside their `f64` originals.
    const PAIRS: [(&str, fn(f32) -> f32, fn(f64) -> f64); 7] = [
        ("F_-1", fermi_dirac_m1, f64fd::fermi_dirac_m1),
        ("F_-1/2", fermi_dirac_mhalf, f64fd::fermi_dirac_mhalf),
        ("F_0", fermi_dirac_0, f64fd::fermi_dirac_0),
        ("F_1/2", fermi_dirac_half, f64fd::fermi_dirac_half),
        ("F_1", fermi_dirac_1, f64fd::fermi_dirac_1),
        ("F_3/2", fermi_dirac_3half, f64fd::fermi_dirac_3half),
        ("F_2", fermi_dirac_2, f64fd::fermi_dirac_2),
    ];

    /// What `f32` costs, against the `f64` module, over 200 001 linear probes
    /// on `[-100, 100]` — which crosses the underflow bound, the series cut,
    /// all four Chebyshev boundaries and the asymptotic branch (2026-09-19):
    ///
    /// | | worst relative | at |
    /// |---|---|---|
    /// | `F_{-1}` | 1.701e-07 | -12.46 |
    /// | `F_{-1/2}` | 3.399e-07 | -1.938 |
    /// | **`F_0`** | **8.514e-06** | **-4.999** |
    /// | `F_{1/2}` | 8.085e-07 | 89.97 |
    /// | `F_1` | 3.592e-07 | -1.310 |
    /// | `F_{3/2}` | 1.380e-06 | 63.49 |
    /// | `F_2` | 6.081e-07 | 4.043 |
    ///
    /// Six of the seven sit at one to twelve `f32` ulp, which is ordinary.
    /// `F_0` is the exception and it is upstream's branch cut, not this
    /// transcription — see the next test.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        for (name, f, g) in PAIRS {
            let (mut worst, mut at) = (0.0_f64, 0.0_f32);
            for k in 0..=200_000 {
                let x = -100.0 + 200.0 * k as f32 / 200_000.0;
                let r = g(x as f64);
                if r > 1e-30 {
                    let e = (((f(x) as f64) - r) / r).abs();
                    if e > worst {
                        worst = e;
                        at = x;
                    }
                }
            }
            let bound = if name == "F_0" { 2e-5 } else { 2e-6 };
            assert!(
                worst < bound,
                "{name} f32 vs f64: {worst:e} at x = {at}. The documented \
                 figures are 1.7e-07 to 1.4e-06, with F_0 alone at 8.5e-06"
            );
        }
    }

    /// **`F_0`'s outlier is upstream's branch cut, and it costs the same
    /// number of ULP at both widths** — which is what shows it is the
    /// formula and not the transcription.
    ///
    /// Just above `x = -5` GSL stops using its small-argument series and
    /// evaluates `ln(1 + e^x)` directly. Adding `6.7e-3` to `1.0` throws away
    /// the low bits before the logarithm runs, and the loss is proportional
    /// to the width's epsilon:
    ///
    /// | | error there | in ulp of its own type |
    /// |---|---|---|
    /// | `f64` module vs `ln_1p` | 1.5e-14 | ~68 |
    /// | this mirror vs `f64` | 8.5e-06 | ~71 |
    ///
    /// A transcription defect would not scale that way. The test asserts the
    /// **ratio**, not either number.
    #[test]
    fn f_0s_outlier_is_upstreams_branch_cut_and_scales_with_epsilon() {
        // Just above the cut, where the naive form runs.
        let x32 = -4.999_f32;
        let x64 = -4.999_f64;

        let f32_ulp = {
            let r = f64fd::fermi_dirac_0(x64);
            (((fermi_dirac_0(x32) as f64) - r) / r).abs() / f64::from(f32::EPSILON)
        };
        let f64_ulp = {
            let r = x64.exp().ln_1p();
            ((f64fd::fermi_dirac_0(x64) - r) / r).abs() / f64::EPSILON
        };
        assert!(
            (30.0..200.0).contains(&f32_ulp) && (30.0..200.0).contains(&f64_ulp),
            "the branch cut is documented as costing about 70 ulp at BOTH \
             widths; measured {f32_ulp:.1} at f32 and {f64_ulp:.1} at f64"
        );
        assert!(
            (f32_ulp / f64_ulp - 1.0).abs() < 0.5,
            "the cost is documented as the same in ulp at both widths -- that \
             is what identifies it as upstream's formula rather than this \
             transcription. f32 {f32_ulp:.1} ulp against f64 {f64_ulp:.1}"
        );

        // And below the cut, where the series runs, both are ordinary.
        let x = -5.5_f32;
        let r = f64fd::fermi_dirac_0(x as f64);
        let e = (((fermi_dirac_0(x) as f64) - r) / r).abs() / f64::from(f32::EPSILON);
        assert!(
            e < 5.0,
            "below the cut F_0 should be a few ulp; it is {e:.1}"
        );
    }

    /// **Correcting upstream's `F_2` far-field cut is worth a factor of
    /// 189**, and that is measured rather than argued.
    ///
    /// Past the cut, `F_2` drops its `60/x` Chebyshev fit for the pure
    /// `x^3/6`. The fit carries `1 + pi^2/x^2`, so the cut belongs where that
    /// correction falls below epsilon — which wants a **square** root of
    /// epsilon, exactly as `F_1`'s does. Upstream uses a **cube** root.
    ///
    /// Measured against the exact far-field form
    /// `x^3/6 + pi^2 x/6`, over `x` in `[1e2, 1e6]` (2026-09-19):
    ///
    /// | cut | worst relative | in `f32` ulp | at |
    /// |---|---|---|---|
    /// | `1/sqrt(eps)` = 2896.31 (**shipped**) | 1.265e-06 | 10.6 | 2920.9 |
    /// | `1/cbrt(eps)` = 203.19 (upstream's form) | 2.390e-04 | **2005** | 203.2 |
    ///
    /// Two thousand ulp is not a rounding difference, and it is why this is
    /// filed as a corrected **formula** rather than a retargeted value.
    #[test]
    fn the_f_2_far_cut_corrects_upstream_and_it_is_worth_189x() {
        const PI2: f64 = core::f64::consts::PI * core::f64::consts::PI;
        let exact = |x: f32| {
            let x = x as f64;
            x * x * x / 6.0 + PI2 * x / 6.0
        };
        let worst = |cut: f32| {
            let (mut w, mut at) = (0.0_f64, 0.0_f32);
            for k in 0..=50_000 {
                let x = 100.0_f32 * (1e4_f32).powf(k as f32 / 50_000.0);
                let e = (((f2_with(x, cut) as f64) - exact(x)) / exact(x)).abs();
                if e > w {
                    w = e;
                    at = x;
                }
            }
            (w, at)
        };
        let (ours, at_ours) = worst(FAR_CUT);
        let (theirs, at_theirs) = worst(F2_CUBE_ROOT_CUT);

        assert!(
            ours < 2e-6,
            "the shipped cut is documented at 1.265e-06 (10.6 f32 ulp); it is \
             {ours:e} at x = {at_ours}"
        );
        assert!(
            theirs > 1e-4,
            "upstream's cube-root form is documented at 2.390e-04 (2005 f32 \
             ulp); it is {theirs:e} at x = {at_theirs}. If it has improved, \
             the correction is no longer justified and this module should go \
             back to the analogue"
        );
        let ratio = theirs / ours;
        assert!(
            (150.0..250.0).contains(&ratio),
            "the correction is documented as worth a factor of 189; measured \
             {ratio:e}"
        );
        // And the two cuts are the roots they claim to be.
        assert_eq!(FAR_CUT, 1.0 / f32::EPSILON.sqrt());
        assert!((F2_CUBE_ROOT_CUT / (1.0 / f32::EPSILON.cbrt()) - 1.0).abs() < 1e-6);
    }

    /// **The underflow guard is kept**, for
    /// [`crate::wgsl::mirror_synchrotron`]'s reason: it is derived from the
    /// exponent range, so the arithmetic enforces it anyway and retargeting
    /// can only discard answers.
    ///
    /// Measured 2026-09-19: `f32`'s analogue of `GSL_LOG_DBL_MIN` is
    /// `ln(f32::MIN_POSITIVE) = -87.3366`, but `F_j(x) ~ e^x` keeps returning
    /// representable denormals until `x = -104`, the smallest being 1e-45.
    /// Retargeting would zero **2218 of 4001** answers over `[-110, -80]`.
    #[test]
    fn the_underflow_guard_is_kept_because_retargeting_it_discards_answers() {
        let f32_log_min = f32::MIN_POSITIVE.ln();
        assert!(
            (-87.34..-87.33).contains(&f32_log_min),
            "f32's analogue of LOG_DBL_MIN is documented as -87.3366; it is \
             {f32_log_min}"
        );
        assert!(
            LOG_MIN < 8.0 * f32_log_min,
            "the kept f64 bound is far out of f32's way"
        );

        // Denormals survive well past where the retargeted guard would fire.
        for x in [-88.0_f32, -95.0, -100.0, -103.0] {
            let v = fermi_dirac_0(x);
            assert!(
                v > 0.0 && v < f32::MIN_POSITIVE,
                "F_0({x}) is documented as a representable denormal; it is {v:e}"
            );
        }
        assert_eq!(
            fermi_dirac_0(-104.0),
            0.0,
            "the arithmetic reaches zero by -104"
        );

        let mut changed = 0_u32;
        let mut smallest_discarded = f32::INFINITY;
        let total = 4001;
        for k in 0..total {
            let x = -110.0 + 30.0 * k as f32 / (total - 1) as f32;
            let kept = fermi_dirac_0(x);
            let retargeted = if x < f32_log_min { 0.0 } else { kept };
            if kept.to_bits() != retargeted.to_bits() {
                changed += 1;
                assert!(
                    retargeted == 0.0 && kept > 0.0,
                    "at x = {x} retargeting gave {retargeted:e} against the kept \
                     {kept:e}; it can only ever replace a real value with zero"
                );
                if kept < smallest_discarded {
                    smallest_discarded = kept;
                }
            }
        }
        assert_eq!(
            changed, 2218,
            "retargeting the underflow guard is documented as zeroing 2218 of \
             {total} answers over [-110, -80]; it zeroed {changed}"
        );
        assert!(
            smallest_discarded <= 1e-44,
            "the smallest discarded answer is documented as 1e-45, a \
             representable f32 denormal; it is {smallest_discarded:e}"
        );
    }

    /// **The two overflow guards could not be kept OR retargeted — they are
    /// not `f32` numbers at all**, and this is the third outcome in the
    /// constant taxonomy.
    ///
    /// `GSL_SQRT_DBL_MAX` is 1.34e154 and `GSL_ROOT3_DBL_MAX` is 5.64e102;
    /// `f32::MAX` is 3.40e38. Writing either as an `f32` literal is a compile
    /// error, and a first draft of this module did exactly that. The branches
    /// are therefore removed and the arithmetic overflows on its own.
    ///
    /// Measured overflow points, by bisection (2026-09-19):
    ///
    /// | | overflows above |
    /// |---|---|
    /// | `F_1` (`x^2/2`) | 10^19.416 |
    /// | `F_2` (`x^3/6`) | 10^12.844 |
    /// | `F_{1/2}` | 10^25.770 |
    /// | `F_{3/2}` | 10^15.621 |
    ///
    /// All four return `+inf`, which is what upstream's `OVERFLOW_ERROR`
    /// returns too — so deleting the guards changes no answer, only where
    /// the infinity comes from.
    #[test]
    fn the_overflow_guards_could_not_be_kept_at_all() {
        // Not representable, which is the whole point.
        assert!(1.340_780_8e154_f64 > f64::from(f32::MAX));
        assert!(5.643_803e102_f64 > f64::from(f32::MAX));
        assert_eq!(1.340_780_8e154_f64 as f32, f32::INFINITY);
        assert_eq!(5.643_803e102_f64 as f32, f32::INFINITY);

        let overflow_decade = |f: fn(f32) -> f32| {
            let (mut lo, mut hi) = (1.0_f32, 38.0_f32);
            for _ in 0..100 {
                let m = 0.5 * (lo + hi);
                if f(10.0_f32.powf(m)).is_finite() {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            lo
        };
        for (name, f, want) in [
            ("F_1", fermi_dirac_1 as fn(f32) -> f32, 19.416_f32),
            ("F_2", fermi_dirac_2, 12.844),
            ("F_1/2", fermi_dirac_half, 25.770),
            ("F_3/2", fermi_dirac_3half, 15.621),
        ] {
            let got = overflow_decade(f);
            assert!(
                (got - want).abs() < 0.01,
                "{name} is documented as overflowing above 10^{want}; measured \
                 10^{got}"
            );
            // And it is an infinity, not a wrong finite number or a NaN.
            assert_eq!(
                f(10.0_f32.powf(want + 1.0)),
                f32::INFINITY,
                "{name} past its point"
            );
        }

        // The three that cannot overflow: bounded, linear, sqrt.
        for (name, f) in [
            ("F_-1", fermi_dirac_m1 as fn(f32) -> f32),
            ("F_0", fermi_dirac_0),
            ("F_-1/2", fermi_dirac_mhalf),
        ] {
            assert!(f(1e38).is_finite(), "{name}(1e38) overflowed");
        }
    }

    /// Every member is positive and non-decreasing, `NaN` propagates, and the
    /// dispatcher agrees with the seven named entry points.
    #[test]
    fn the_shape_survives_f32() {
        for (which, (name, f, _)) in PAIRS.iter().enumerate() {
            // The dispatcher's order is ascending in j, same as PAIRS.
            for k in 0..=400 {
                let x = -20.0 + 0.1 * k as f32;
                assert_eq!(
                    fermi_dirac(which as u32, x).to_bits(),
                    f(x).to_bits(),
                    "the dispatcher disagrees with {name} at x = {x}"
                );
            }
            let mut prev = 0.0_f32;
            for k in 0..=5000 {
                let x = -30.0 + 0.01 * k as f32;
                let v = f(x);
                assert!(v > 0.0 && v.is_finite(), "{name}({x}) = {v:e}");
                // NON-decreasing, not strictly: F_{-1} saturates at 1 in f32
                // around x = 16, far earlier than f64's 33.9.
                assert!(v >= prev, "{name} decreased at {x}: {v:e} vs {prev:e}");
                prev = v;
            }
            assert!(f(f32::NAN).is_nan(), "{name}(NaN)");
            assert_eq!(f(-800.0), 0.0, "{name} below the kept guard");
        }
        assert!(
            fermi_dirac(7, 1.0).is_nan(),
            "an out-of-range index gives NaN"
        );
        assert!(fermi_dirac_m1(20.0) <= 1.0, "F_-1 is bounded by 1");
    }

    /// The shader and this mirror agree on their constants, checked by
    /// parsing the WGSL source — including that neither overflow guard is
    /// declared at all.
    #[test]
    fn the_shader_and_this_mirror_agree_on_their_constants() {
        let src = crate::wgsl::FERMI_DIRAC;
        let read = |name: &str| -> f32 {
            let at = src
                .find(name)
                .unwrap_or_else(|| panic!("{name} not declared in fermi_dirac.wgsl"));
            let rest = &src[at..];
            let eq = rest.find('=').expect("no = after the name");
            let end = rest[eq..].find(';').expect("no ; after the value");
            rest[eq + 1..eq + end]
                .trim()
                .parse::<f32>()
                .unwrap_or_else(|_| panic!("{name} is not an f32 literal"))
        };
        assert_eq!(read("PETIR_FD_EPS: f32"), EPS);
        assert_eq!(read("PETIR_FD_LOG_MIN: f32"), LOG_MIN);
        assert_eq!(read("PETIR_FD_FAR_CUT: f32"), FAR_CUT);
        assert_eq!(read("PETIR_FD_LG_MHALF: f32"), LG_MHALF);
        assert_eq!(read("PETIR_FD_LG_HALF: f32"), LG_HALF);
        assert_eq!(read("PETIR_FD_LG_3HALF: f32"), LG_3HALF);

        // The checks below are about CODE, so the comments come off first:
        // the header discusses GSL_ROOT3_DBL_MAX and GSL_SQRT_DBL_MAX at
        // length, and a first draft of this test failed on its own prose.
        let code: std::string::String = src
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<std::vec::Vec<_>>()
            .join("\n");

        // The overflow guards must NOT be there -- they are not f32 numbers.
        for gone in ["PETIR_FD_SQRT_MAX", "PETIR_FD_ROOT3_MAX", "ROOT3"] {
            assert!(
                !code.contains(gone),
                "{gone} appears in the shader's code. GSL's overflow bounds \
                 are not representable as f32 and must stay deleted, and F_2's \
                 cube-root cut is deliberately corrected to the square root"
            );
        }
        // And both F_1 and F_2 reach for the one shared square-root cut.
        assert!(
            code.matches("PETIR_FD_FAR_CUT").count() >= 3,
            "F_2 is documented as using the same PETIR_FD_FAR_CUT as F_1; the \
             shader's code names it {} times, expected at least 3 (the \
             declaration plus one use in each)",
            code.matches("PETIR_FD_FAR_CUT").count()
        );

        // The eta table reaches the shader intact -- compared as PARSED f32,
        // not as text: `0.98555109` in the source and `0.9855511` from
        // `{:?}` are the same f32 and a first draft of this test compared the
        // strings and failed on that.
        let at = code
            .find("array<f32, 9>(")
            .expect("the eta table is declared as array<f32, 9> in the shader");
        let rest = &code[at..];
        let open = rest.find('(').expect("no ( after the array type");
        let close = rest[open..].find(')').expect("no ) closing the eta table");
        let shader_eta: std::vec::Vec<f32> = rest[open + 1..open + close]
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| t.parse::<f32>().expect("eta literal"))
            .collect();
        assert_eq!(shader_eta.len(), ETA.len(), "the eta table changed length");
        for (k, (a, b)) in shader_eta.iter().zip(ETA.iter()).enumerate() {
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "eta({}) is {a:e} in the shader and {b:e} here",
                2 * (k + 1)
            );
        }
    }
}
