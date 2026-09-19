// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::fermi_dirac`, which ports
// GSL 2.8's specfunc/fermi_dirac.c.
//   Copyright (C) 1996-2000 Gerard Jungman.
//   Branch structure and small-argument series after Goano, ACM TOMS 21
//   (1995) 221-232; the Chebyshev fits are Jungman's.
//
//   F_j(x) = (1/Gamma(j+1)) integral_0^inf t^j / (e^{t-x} + 1) dt
//
// the occupation integrals of a Fermi gas. x is the reduced chemical
// potential mu/kT. Seven fixed indices: j = -1, -1/2, 0, 1/2, 1, 3/2, 2.
//
// EVERY COEFFICIENT WAS EXTRACTED BY SCRIPT from the f64 module -- 483
// literals across 22 Chebyshev series, the largest table set in this module
// -- and the same script emits `wgsl::mirror_fermi_dirac`, so the shader and
// its CPU mirror cannot disagree about a constant.
//
// ============================================================================
// THE SERIES ACCELERATION IS NOT HERE, AND THAT IS A MEASUREMENT
// ============================================================================
//
// GSL's fd_asymp calls fd_neg, which can run a Levin u-transform over two
// 101-element work arrays. That was recorded as the blocker for this shader:
// 202 f32 of function-local storage is real, and psi_zeta.wgsl already
// declined a single 101-element local array for the same reason.
//
// It is never reached. fd_neg is called only from fd_asymp, which runs only
// at x >= 30, so fd_neg always sees -x <= -30 and always takes its simple
// alternating series instead. Proven by instrumentation in the f64 module,
// not by reading branch conditions.
//
// Three consequences, all measured, and together they remove every obstacle:
//
//   1. NO SCRATCH MEMORY. The acceleration is unreachable, so it is absent.
//
//   2. THE REFLECTION TERM IS EXACTLY ZERO. fd_asymp computes
//      cos(j*pi) * F_j(-x) + 2 * seqn * exp(...), and cos(j*pi) vanishes
//      identically for j = -1/2, 1/2, 3/2 -- the only indices that reach it.
//      Upstream carries the term because its j is a runtime parameter; ours
//      is not. Measured contribution before dropping: 1e-31 to 1e-64
//      relative. So fd_neg is not transcribed at all.
//
//   3. lnGamma(j+2) IS A COMPILE-TIME CONSTANT for each of the three, so
//      gamma.wgsl is not a dependency. Likewise eta(2n) is a nine-entry
//      table rather than a call into psi_zeta.wgsl.
//
// ============================================================================
// FOUR MACHINE CONSTANTS, AND ONE OF THEM IS UPSTREAM'S MISTAKE
// ============================================================================
//
//   GSL_DBL_EPSILON, series break    2.22e-16 -> 1.1920929e-7   PRECISION,
//                                                               retargeted
//
//   GSL_LOG_DBL_MIN, underflow       -708.396 -> KEPT           RANGE GUARD
//       Same call as synchrotron.wgsl's: a bound derived from the exponent
//       range is enforced by the arithmetic anyway, and retargeting it to
//       f32's -87.3365 would discard representable denormals, since
//       F_j(x) ~ e^x and e^{-90} = 8.2e-40 is a perfectly good f32.
//
//   1/GSL_SQRT_DBL_EPSILON, F_1      6.711e7  -> 2896.3093      PRECISION,
//                                                               retargeted
//       Past this, F_1 drops its 60/x Chebyshev fit for the pure degenerate
//       limit x^2/2. What the fit carries is the Sommerfeld correction
//       1 + (pi^2/3)/x^2, so the cut belongs where that falls below eps.
//       At upstream's value the correction is 6.7e-16, about 3 f64 eps; at
//       1/sqrt(f32 eps) it is 3.9e-07, about 3 f32 eps. Same relationship,
//       so the retargeting is the straightforward analogue.
//
//   1/GSL_ROOT3_DBL_EPSILON, F_2     1.651e5  -> 2896.3093      PRECISION,
//                                                    AND A CORRECTED FORMULA
//       THIS IS NOT THE ANALOGUE OF UPSTREAM'S EXPRESSION, DELIBERATELY.
//       F_2's fit carries 1 + pi^2/x^2, the same 1/x^2 shape as F_1's, so it
//       wants the same SQUARE root. Upstream uses the CUBE root -- apparently
//       matching the x^3 factor rather than the accuracy requirement -- and
//       pays for it: at 1.6514e5 the correction is still 3.62e-10, so
//       gsl_sf_fermi_dirac_2 steps down by 3.56e-10 at that point. The f64
//       module reproduces that faithfully and asserts it, because its bar is
//       agreement with GSL. Here the bar is what f32 can do, and carrying
//       the cube-root form would cost 2.4e-04 -- two thousand f32 eps. So
//       the CONSTANT is retargeted and the FORMULA is corrected, which is
//       lambert.wgsl's case exactly (see docs/wgsl-coverage.md).
//
// AND A FIFTH KIND, WHICH HAD NOT COME UP BEFORE: A GUARD THAT IS NOT
// REPRESENTABLE AT THE NARROWER WIDTH.
//
//   GSL_SQRT_DBL_MAX, F_1 overflow   1.34e154  -> DELETED
//   GSL_ROOT3_DBL_MAX, F_2 overflow  5.64e102  -> DELETED
//
// Neither is an f32 at all -- f32::MAX is 3.4e38 -- so "keep" and "retarget"
// are both unavailable. The branch is removed and the arithmetic is left to
// overflow on its own, which it does at ~2.6e19 for x*x and ~1.4e13 for
// x*x*x, returning the same infinity upstream's OVERFLOW_ERROR would have.
// That is a THIRD outcome beside keep and retarget, and it is forced rather
// than chosen: a constant one cannot even write down cannot be a guard.

// GSL's fd_1_a_data, 22 coefficients.
fn petir_fd_cheb_1_a(x: f32) -> f32 {
    var c = array<f32, 22>(
        1.894934058189392, 0.7237719297409058, 0.125, 0.010106519795954227, 0.0,
        -6.0061523981858045e-05, 0.0, 6.816528639319586e-07, 0.0, -9.58957802055238e-09,
        0.0, 1.5151041532490694e-10, 0.0, -2.578561496269227e-12, 0.0,
        4.622699927925647e-14, 0.0, -8.611999968136021e-16, 0.0, 1.6499999980444308e-17,
        0.0, -3.0000000340435383e-19,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 21; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_1_b_data, 22 coefficients.
fn petir_fd_cheb_1_b(x: f32) -> f32 {
    var c = array<f32, 22>(
        10.409136772155762, 3.899445056915283, 0.5135109424591064, 0.01061873696744442,
        -0.0015844680601730943, 0.0001461392967030406, -1.4080957271289662e-06,
        -2.1779937924293336e-06, 3.9142366858868627e-07, -2.3860263098640644e-08,
        -4.138309694923237e-09, 1.2839652674401236e-09, -1.3969599088614615e-10,
        -4.9077430487598495e-12, 4.3998780313581065e-12, -7.172910269220845e-13,
        2.4319999817170104e-14, 1.4229999353875845e-14, -3.446000021182431e-15,
        2.929999990511524e-16, 3.700000055773374e-17, -1.600000073301928e-17,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 21; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_1_c_data, 23 coefficients.
fn petir_fd_cheb_1_c(x: f32) -> f32 {
    var c = array<f32, 23>(
        56.7809944152832, 21.007184982299805, 2.245924472808838, 0.0017379363998770714,
        -0.000587164715398103, 0.00016306959150824696, -3.817425749730319e-05,
        7.645272489753552e-06, -1.3134849723428488e-06, 1.9000646034328383e-07,
        -2.1413281814375296e-08, 1.2390637404990912e-09, 2.1848048370465278e-10,
        -1.0134282302232123e-10, 2.484728048313123e-11, -4.73066984890691e-12,
        7.355500214988042e-13, -8.739999690773534e-14, 4.850000118178507e-15,
        1.2300000478722558e-15, -5.600000041489789e-16, 1.4000000103724471e-16,
        -3.0000001167615996e-17,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 22; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_1_d_data, 30 coefficients.
fn petir_fd_cheb_1_d(x: f32) -> f32 {
    var c = array<f32, 30>(
        1.012662649154663, -0.00633125239983201, 0.002483731834217906,
        -0.0008764333906583488, 0.0002913344360422343, -9.31877875700593e-05,
        2.9015134714427404e-05, -8.854870429786388e-06, 2.6603474907460622e-06,
        -7.89141552104411e-07, 2.315730256441384e-07, -6.731794854886175e-08,
        1.9404803097700096e-08, -5.5507127783016585e-09, 1.576609065523371e-09,
        -4.4493109196963587e-10, 1.2482927191914683e-10, -3.483928770475764e-11,
        9.67915504690442e-12, -2.678624032650956e-12, 7.388851900116955e-13,
        -2.0328279803829424e-13, 5.5811498949958835e-14, -1.5298700823426596e-14,
        4.188599947029144e-15, -1.1457999965966133e-15, 3.1319998783771376e-16,
        -8.559999821410692e-17, 2.3300000333336532e-17, -5.90000013588401e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 29; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_1_e_data, 10 coefficients.
fn petir_fd_cheb_1_e(x: f32) -> f32 {
    var c = array<f32, 10>(
        1.0013707876205444, 0.0009138522436842322, 0.00022846306092105806,
        -1.5700000522819772e-17, -1.2700000075185928e-17, -9.699999627552083e-18,
        -6.899999871504985e-18, -4.6000001900635275e-18, -2.9000001018404714e-18,
        -1.6999999848254795e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 9; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_2_a_data, 21 coefficients.
fn petir_fd_cheb_2_a(x: f32) -> f32 {
    var c = array<f32, 21>(
        2.1573662757873535, 0.884967029094696, 0.17841634154319763, 0.02083333395421505,
        0.0012708225985988975, 0.0, -5.0619314606592525e-06, 0.0, 4.320265389878841e-08,
        0.0, -4.870543968138463e-10, 0.0, 6.420374214916036e-12, 0.0,
        -9.374239665765893e-14, 0.0, 1.4715000466836528e-15, 0.0, -2.4400000001160574e-17,
        0.0, 3.99999987306209e-19,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 20; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_2_b_data, 22 coefficients.
fn petir_fd_cheb_2_b(x: f32) -> f32 {
    var c = array<f32, 22>(
        16.508258819580078, 7.421719551086426, 1.4583098888397217, 0.12877385318279266,
        0.001963611925020814, -0.0002374589821556583, 1.8539662050898187e-05,
        -1.9280564345081075e-07, -2.0195003003209422e-07, 3.296349859738257e-08,
        -1.8858170580671185e-09, -2.7263274970934503e-10, 8.055456302002995e-11,
        -8.313223373579426e-12, -2.2448900648553566e-13, 2.1877799864702258e-13,
        -3.4290001122826846e-14, 1.2250000123846137e-15, 5.810000155542219e-16,
        -1.369999956265272e-16, 1.2000000136174153e-17, 1.000000045813705e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 21; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_2_c_data, 20 coefficients.
fn petir_fd_cheb_2_c(x: f32) -> f32 {
    var c = array<f32, 20>(
        168.8712921142578, 81.80260467529297, 15.754084587097168, 1.1232558488845825,
        0.0005905750440433621, -0.00016469713591504842, 3.8856076571391895e-05,
        -7.898736839706544e-06, 1.3978624338051304e-06, -2.1534528116262663e-07,
        2.83151102564716e-08, -2.949785748995737e-09, 1.6755082044017655e-10,
        2.2342289476839916e-11, -1.0351299678523773e-11, 2.4111700049761486e-12,
        -4.353099911785785e-13, 6.447000187393206e-14, -7.389999934692346e-15,
        4.300000079125694e-16,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 19; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_2_d_data, 30 coefficients.
fn petir_fd_cheb_2_d(x: f32) -> f32 {
    var c = array<f32, 30>(
        0.34599605202674866, -0.006331364158540964, 0.0024838296230882406,
        -0.0008765119127929211, 0.0002913925563916564, -9.322746336692944e-05,
        2.90402185783023e-05, -8.869622433849145e-06, 2.6684497242968064e-06,
        -7.933156780381978e-07, 2.3359868350780744e-07, -6.824790688142457e-08,
        1.9810364904060407e-08, -5.7194040614660935e-09, 1.6437942118585624e-09,
        -4.706493528239264e-10, 1.3432614742736604e-10, -3.823400623881312e-11,
        1.0857720104950896e-11, -3.0772745462925855e-12, 8.706484968742934e-13,
        -2.4595429859441964e-13, 6.938530696179446e-14, -1.9549389979555375e-14,
        5.5016200234023985e-15, -1.5465700356999596e-15, 4.34289991562396e-16,
        -1.217800047044495e-16, 3.3940001391548976e-17, -8.809999610559328e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 29; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_2_e_data, 4 coefficients.
fn petir_fd_cheb_2_e(x: f32) -> f32 {
    var c = array<f32, 4>(
        0.33470410108566284, 0.0009138522436842322, 0.00022846306092105806,
        5.200000093474659e-19,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 3; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_mhalf_a_data, 20 coefficients.
fn petir_fd_cheb_mhalf_a(x: f32) -> f32 {
    var c = array<f32, 20>(
        1.266329050064087, 0.36978763341903687, 0.027813101187348366, -0.003333284752443433,
        -0.00044381082989275455, 6.16495162830688e-05, 8.758961485000327e-06,
        -1.2622937219930463e-06, -1.8374640831098077e-07, 2.6949509290830065e-08,
        3.976086571100268e-09, -5.894468801947994e-10, -8.773216181312549e-11,
        1.310165691215115e-11, 1.9621619867099538e-12, -2.9458870781130797e-13,
        -4.432340115051968e-14, 6.681600022882209e-15, 1.0084000293654146e-15,
        -1.5610000625196043e-16,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 19; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_mhalf_b_data, 20 coefficients.
fn petir_fd_cheb_mhalf_b(x: f32) -> f32 {
    var c = array<f32, 20>(
        3.270796060562134, 0.5809004902839661, -0.029931344091892242,
        -0.0013287935871630907, 0.000991022097878158, -0.000169095495948568,
        6.595585091417888e-06, 3.59539649252838e-06, -9.430672207599855e-07,
        8.757739777820461e-08, 1.0624765067746011e-08, -4.958700561275009e-09,
        7.160432802244543e-10, 4.507221852689813e-12, -2.3695425829806105e-11,
        4.9122208037322146e-12, -2.9052769304899195e-13, -9.592909691429063e-14,
        3.000280145853962e-14, -3.4970000866948493e-15,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 19; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_mhalf_c_data, 25 coefficients.
fn petir_fd_cheb_mhalf_c(x: f32) -> f32 {
    var c = array<f32, 25>(
        5.828283309936523, 0.6775211095809937, -0.043946247547864914, 0.005825595930218697,
        -0.0008648589137010276, 0.00011001789243891835, -6.9733050622744486e-06,
        -1.7162674339488149e-06, 8.598115641689219e-07, -2.3306678542667214e-07,
        4.850319257343472e-08, -8.130620621216167e-09, 1.0210682299671703e-09,
        -5.318842241641697e-11, -1.9430559591859797e-11, 8.750506395871493e-12,
        -2.324897002345394e-12, 4.831020024124999e-13, -8.120700021718025e-14,
        1.0131999744950916e-14, -4.639999898246958e-16, -2.2400000695354746e-16,
        9.700000289296573e-17, -2.600000057077087e-17, 4.999999918875795e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 24; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_mhalf_d_data, 30 coefficients.
fn petir_fd_cheb_mhalf_d(x: f32) -> f32 {
    var c = array<f32, 30>(
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
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 29; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_half_a_data, 23 coefficients.
fn petir_fd_cheb_half_a(x: f32) -> f32 {
    var c = array<f32, 23>(
        1.7177138328552246, 0.619257926940918, 0.09328022599220276, 0.004709485452622175,
        -0.0004243667935952544, -4.525698022916913e-05, 5.242650786385639e-06,
        6.387648454619921e-07, -8.057769917968471e-08, -1.0429027419434078e-08,
        1.37694777802011e-09, 1.8471903173722382e-10, -2.5106189696644243e-11,
        -3.449781692949072e-12, 4.784372767754896e-13, 6.688279894110846e-14,
        -9.414700057963979e-15, -1.333299950954654e-15, 1.8980000615186082e-16,
        2.7199999757207672e-17, -3.899999837461447e-18, -6.000000068087077e-19,
        9.999999682655225e-20,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 22; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_half_b_data, 20 coefficients.
fn petir_fd_cheb_half_b(x: f32) -> f32 {
    var c = array<f32, 20>(
        7.6510138511657715, 2.475545644760132, 0.21833598613739014, -0.007730591576546431,
        -0.0002174433902837336, 0.00014766397362109274, -2.1586361981462687e-05,
        8.077127517935878e-07, 3.2885805012483615e-07, -7.947433289245964e-08,
        6.940207075700755e-09, 6.755946913017397e-10, -3.102004764166111e-10,
        4.2677233275112414e-11, -2.169600016679353e-14, -1.1702449989950403e-12,
        2.347570109226954e-13, -1.4139000063253474e-14, -3.863999980982351e-15,
        1.2019999973722256e-15,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 19; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_half_c_data, 23 coefficients.
fn petir_fd_cheb_half_c(x: f32) -> f32 {
    var c = array<f32, 23>(
        29.584339141845703, 8.808343887329102, 0.5037716627120972, -0.021540695801377296,
        0.00214334181509912, -0.0002573656674940139, 2.7933539968216792e-05,
        -1.6785249954409664e-06, -2.7810011715700966e-07, 1.3521805897198647e-07,
        -3.3740423788231055e-08, 6.474834890468628e-09, -1.0096790070690531e-09,
        1.2005756111488353e-10, -6.636313894248236e-12, -1.7105660421110058e-12,
        7.750689738454664e-13, -1.9797299921370942e-13, 3.941400157636554e-14,
        -6.37400000808681e-15, 7.769999984775188e-16, -3.999999935100636e-17,
        -1.4000000434596716e-17,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 22; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_half_d_data, 30 coefficients.
fn petir_fd_cheb_half_d(x: f32) -> f32 {
    var c = array<f32, 30>(
        1.5116909742355347, -0.00360434059984982, 0.0014207743806764483,
        -0.0005045399302616715, 0.000169075807207264, -5.463058550958522e-05,
        1.7222322640009224e-05, -5.335260539141018e-06, 1.6315287894030917e-06,
        -4.939021209793282e-07, 1.4825154437403398e-07, -4.4155228806630475e-08,
        1.3050316383100835e-08, -3.826260197570264e-09, 1.1123226784093276e-09,
        -3.2047656195466345e-10, 9.148704710471023e-11, -2.5877895312720334e-11,
        7.255073278256141e-12, -2.0172225435183266e-12, 5.566890806836533e-13,
        -1.526246983614074e-13, 4.16120995083355e-14, -1.1293299703328454e-14,
        3.053700099793391e-15, -8.233999868720765e-16, 2.215000024446162e-16,
        -5.950000143344574e-17, 1.5899999560045294e-17, -4.00000018325482e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 29; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_3half_a_data, 20 coefficients.
fn petir_fd_cheb_3half_a(x: f32) -> f32 {
    var c = array<f32, 20>(
        2.0404775142669678, 0.8122168183326721, 0.1536371111869812, 0.015617432072758675,
        0.00059434276772663, -4.2960946302628145e-05, -3.824645318672992e-06,
        3.8023063098080456e-07, 4.0574615667310354e-08, -4.553036170307223e-09,
        -5.306873274157908e-10, 6.372972982671143e-11, 7.84036724432724e-12,
        -9.840240601521888e-13, -1.2559519938816488e-13, 1.6261699357423588e-14,
        2.131800027913651e-15, -2.824999933485309e-16, -3.7800000015358277e-17,
        5.099999851078862e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 19; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_3half_b_data, 22 coefficients.
fn petir_fd_cheb_3half_b(x: f32) -> f32 {
    var c = array<f32, 22>(
        13.403206825256348, 5.574508190155029, 0.9312285780906677, 0.05463835597038269,
        -0.0014771729474887252, -2.9378554245340638e-05, 1.835703369579278e-05,
        -2.3480592972191516e-06, 8.317378785704932e-08, 2.68264876979174e-08,
        -6.01124439114642e-09, 4.94346008572677e-10, 3.955734004246203e-11,
        -1.7894930329220848e-11, 2.3489719284258692e-12, -1.2822999701455305e-14,
        -5.4191999059862925e-14, 1.0527000007375803e-14, -6.390000142823089e-16,
        -1.4700000042736246e-16, 4.4999998442701544e-17, -4.999999918875795e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 21; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_3half_c_data, 21 coefficients.
fn petir_fd_cheb_3half_c(x: f32) -> f32 {
    var c = array<f32, 21>(
        101.03684997558594, 43.620853424072266, 6.622413635253906, 0.25081413984298706,
        -0.007981248199939728, 0.0006346224690787494, -6.392179057002068e-05,
        6.045351256034337e-06, -3.400768378014618e-07, -4.0726614969344155e-08,
        1.9311483967499044e-08, -4.463283520550476e-09, 7.943471436178129e-10,
        -1.1573569186351662e-10, 1.3046580309150624e-11, -7.411400052698136e-13,
        -1.41809998531299e-13, 6.4909998220581e-14, -1.5969999845114047e-14,
        3.0500000481215473e-15, -4.800000186818559e-16,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 20; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's fd_3half_d_data, 25 coefficients.
fn petir_fd_cheb_3half_d(x: f32) -> f32 {
    var c = array<f32, 25>(
        0.6160645484924316, -0.007123948074877262, 0.002790686674416065,
        -0.000982952187769115, 0.0003260229714214802, -0.00010401609324617311,
        3.229312278563157e-05, -9.82435085461475e-06, 2.942013225037954e-06,
        -8.699154818714305e-07, 2.5454599494878494e-07, -7.383050615317188e-08,
        2.1254567883488562e-08, -6.079653225299353e-09, 1.7294556897695657e-09,
        -4.896540950483086e-10, 1.380786041060844e-10, -3.8805729463131655e-11,
        1.0875321476699895e-11, -3.040730861547658e-12, 8.485625938893515e-13,
        -2.3642749541995245e-13, 6.576359736724241e-14, -1.8180699933273252e-14,
        4.688400207886016e-15,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 24; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

const PETIR_FD_EPS: f32 = 1.1920929e-7;
// GSL's f64 value, KEPT -- see the header.
const PETIR_FD_LOG_MIN: f32 = -708.39642;
// 1/sqrt(f32::EPSILON). Used by BOTH F_1 and F_2; upstream's F_2 uses a cube
// root, which is corrected here -- see the header.
const PETIR_FD_FAR_CUT: f32 = 2896.3093;
// ln Gamma(j+2) for j = -1/2, 1/2, 3/2 -- constants because j is fixed here.
const PETIR_FD_LG_MHALF: f32 = -0.12078224;
const PETIR_FD_LG_HALF: f32 = 0.28468287;
const PETIR_FD_LG_3HALF: f32 = 1.2009736;

// Dirichlet eta at even integers, eta(2n) for n = 1..9. Nine is generous:
// f32 needs three at x = 30 and two past x = 100, where f64 needs seventeen.
fn petir_fd_eta(n: u32) -> f32 {
    var e = array<f32, 9>(
        0.82246703, 0.94703283, 0.98555109, 0.99623300, 0.99903951,
        0.99975769, 0.99993917, 0.99998476, 0.99999619,
    );
    if (n < 1u || n > 9u) { return 1.0; }
    return e[n - 1u];
}

// Goano's alternating series (6) for an INTEGER index, with the ratio raised
// to the integer power p = j + 1: sum_{n>=1} (-1)^{n-1} e^{nx} / n^{j+1},
// written as a running product so no power is evaluated per term.
fn petir_fd_series(x: f32, p: u32) -> f32 {
    let ex = exp(x);
    var term = ex;
    var sum = term;
    for (var n: u32 = 2u; n < 100u; n = n + 1u) {
        let rat = (f32(n) - 1.0) / f32(n);
        var r = 1.0;
        for (var k: u32 = 0u; k < p; k = k + 1u) { r = r * rat; }
        term = term * (-ex) * r;
        sum = sum + term;
        if (abs(term / sum) < PETIR_FD_EPS) { break; }
    }
    return sum;
}

// The same series for a HALF-INTEGER index: the ratio carries a sqrt, and
// `whole` supplies the remaining integer part -- 0 for -1/2, 1 for 1/2,
// 2 for 3/2, matching upstream's sqrt(rat), rat*sqrt(rat), rat*rat*sqrt(rat).
fn petir_fd_series_half(x: f32, whole: u32) -> f32 {
    let ex = exp(x);
    var term = ex;
    var sum = term;
    for (var n: u32 = 2u; n < 200u; n = n + 1u) {
        let rat = (f32(n) - 1.0) / f32(n);
        var r = sqrt(rat);
        for (var k: u32 = 0u; k < whole; k = k + 1u) { r = r * rat; }
        term = term * (-ex) * r;
        sum = sum + term;
        if (abs(term / sum) < PETIR_FD_EPS) { break; }
    }
    return sum;
}

// GSL's fd_asymp for a half-integer j, WITHOUT the reflection term, which is
// identically zero here because cos(j*pi) vanishes for j = -1/2, 1/2, 3/2.
// `lg` is ln Gamma(j+2), a constant at each of the three.
fn petir_fd_asymp(j: f32, lg: f32, x: f32) -> f32 {
    var seqn = 0.5;
    let xm2 = (1.0 / x) / x;
    var xgam = 1.0;
    var add = 3.4028235e38;
    for (var n: u32 = 1u; n <= 9u; n = n + 1u) {
        let add_previous = add;
        xgam = xgam * xm2 * (j + 1.0 - (2.0 * f32(n) - 2.0)) * (j + 1.0 - (2.0 * f32(n) - 1.0));
        add = petir_fd_eta(n) * xgam;
        // j is never an integer here, so the series is genuinely asymptotic
        // and must be truncated at its smallest term.
        if (abs(add) > abs(add_previous)) { break; }
        if (abs(add / seqn) < PETIR_FD_EPS) { break; }
        seqn = seqn + add;
    }
    return 2.0 * seqn * exp((j + 1.0) * log(x) - lg);
}

// F_{-1}(x) = e^x/(1+e^x), the Fermi occupation function. Bounded by 1.
fn petir_fd_m1(x: f32) -> f32 {
    if (!(x >= PETIR_FD_LOG_MIN)) {
        if (x != x) { return bitcast<f32>(0x7fc00000u); }
        return 0.0;
    }
    if (x < 0.0) {
        // So the exponential evaluated is the small one.
        let ex = exp(x);
        return ex / (1.0 + ex);
    }
    return 1.0 / (1.0 + exp(-x));
}

// F_0(x) = ln(1+e^x). Three pieces so neither limit loses its leading
// behaviour to cancellation.
fn petir_fd_0(x: f32) -> f32 {
    if (!(x >= PETIR_FD_LOG_MIN)) {
        if (x != x) { return bitcast<f32>(0x7fc00000u); }
        return 0.0;
    }
    if (x < -5.0) {
        let ex = exp(x);
        let ser = 1.0 - ex * (0.5 - ex * (1.0 / 3.0 - ex * (0.25 - ex * (0.2 - ex / 6.0))));
        return ex * ser;
    }
    if (x < 10.0) {
        return log(1.0 + exp(x));
    }
    let ex = exp(-x);
    return x + ex * (1.0 - 0.5 * ex + ex * ex / 3.0 - ex * ex * ex / 4.0);
}

fn petir_fd_1(x: f32) -> f32 {
    if (!(x >= PETIR_FD_LOG_MIN)) {
        if (x != x) { return bitcast<f32>(0x7fc00000u); }
        return 0.0;
    }
    if (x < -1.0) { return petir_fd_series(x, 2u); }
    if (x < 1.0) { return petir_fd_cheb_1_a(x); }
    if (x < 4.0) { return petir_fd_cheb_1_b(2.0 / 3.0 * (x - 1.0) - 1.0); }
    if (x < 10.0) { return petir_fd_cheb_1_c(1.0 / 3.0 * (x - 4.0) - 1.0); }
    if (x < 30.0) { return petir_fd_cheb_1_d(0.1 * x - 2.0) * x * x; }
    if (x < PETIR_FD_FAR_CUT) { return petir_fd_cheb_1_e(60.0 / x - 1.0) * x * x; }
    // Upstream's OVERFLOW_ERROR bound, GSL_SQRT_DBL_MAX = 1.34e154, IS NOT
    // REPRESENTABLE as an f32 -- so it is not kept and not retargeted, it
    // ceases to exist. x*x reaches infinity at ~2.6e19 on its own, which is
    // the same answer the guard would have given. See the header.
    return 0.5 * x * x;
}

fn petir_fd_2(x: f32) -> f32 {
    if (!(x >= PETIR_FD_LOG_MIN)) {
        if (x != x) { return bitcast<f32>(0x7fc00000u); }
        return 0.0;
    }
    if (x < -1.0) { return petir_fd_series(x, 3u); }
    if (x < 1.0) { return petir_fd_cheb_2_a(x); }
    if (x < 4.0) { return petir_fd_cheb_2_b(2.0 / 3.0 * (x - 1.0) - 1.0); }
    if (x < 10.0) { return petir_fd_cheb_2_c(1.0 / 3.0 * (x - 4.0) - 1.0); }
    if (x < 30.0) { return petir_fd_cheb_2_d(0.1 * x - 2.0) * x * x * x; }
    // PETIR_FD_FAR_CUT, not the cube-root analogue -- see the header.
    if (x < PETIR_FD_FAR_CUT) { return petir_fd_cheb_2_e(60.0 / x - 1.0) * x * x * x; }
    // GSL_ROOT3_DBL_MAX = 5.64e102 is likewise not an f32; x*x*x reaches
    // infinity at ~1.4e13 by itself.
    return x * x * x / 6.0;
}

fn petir_fd_mhalf(x: f32) -> f32 {
    if (!(x >= PETIR_FD_LOG_MIN)) {
        if (x != x) { return bitcast<f32>(0x7fc00000u); }
        return 0.0;
    }
    if (x < -1.0) { return petir_fd_series_half(x, 0u); }
    if (x < 1.0) { return petir_fd_cheb_mhalf_a(x); }
    if (x < 4.0) { return petir_fd_cheb_mhalf_b(2.0 / 3.0 * (x - 1.0) - 1.0); }
    if (x < 10.0) { return petir_fd_cheb_mhalf_c(1.0 / 3.0 * (x - 4.0) - 1.0); }
    if (x < 30.0) { return petir_fd_cheb_mhalf_d(0.1 * x - 2.0) * sqrt(x); }
    return petir_fd_asymp(-0.5, PETIR_FD_LG_MHALF, x);
}

fn petir_fd_half(x: f32) -> f32 {
    if (!(x >= PETIR_FD_LOG_MIN)) {
        if (x != x) { return bitcast<f32>(0x7fc00000u); }
        return 0.0;
    }
    if (x < -1.0) { return petir_fd_series_half(x, 1u); }
    if (x < 1.0) { return petir_fd_cheb_half_a(x); }
    if (x < 4.0) { return petir_fd_cheb_half_b(2.0 / 3.0 * (x - 1.0) - 1.0); }
    if (x < 10.0) { return petir_fd_cheb_half_c(1.0 / 3.0 * (x - 4.0) - 1.0); }
    if (x < 30.0) { return petir_fd_cheb_half_d(0.1 * x - 2.0) * (x * sqrt(x)); }
    return petir_fd_asymp(0.5, PETIR_FD_LG_HALF, x);
}

fn petir_fd_3half(x: f32) -> f32 {
    if (!(x >= PETIR_FD_LOG_MIN)) {
        if (x != x) { return bitcast<f32>(0x7fc00000u); }
        return 0.0;
    }
    if (x < -1.0) { return petir_fd_series_half(x, 2u); }
    if (x < 1.0) { return petir_fd_cheb_3half_a(x); }
    if (x < 4.0) { return petir_fd_cheb_3half_b(2.0 / 3.0 * (x - 1.0) - 1.0); }
    if (x < 10.0) { return petir_fd_cheb_3half_c(1.0 / 3.0 * (x - 4.0) - 1.0); }
    if (x < 30.0) { return petir_fd_cheb_3half_d(0.1 * x - 2.0) * (x * x * sqrt(x)); }
    return petir_fd_asymp(1.5, PETIR_FD_LG_3HALF, x);
}

// One dispatcher over the seven indices, in ascending order of j:
// 0 -> F_{-1}, 1 -> F_{-1/2}, 2 -> F_0, 3 -> F_{1/2}, 4 -> F_1,
// 5 -> F_{3/2}, 6 -> F_2. Any other `which` gives NaN.
fn petir_fermi_dirac(which: u32, x: f32) -> f32 {
    switch (which) {
        case 0u: { return petir_fd_m1(x); }
        case 1u: { return petir_fd_mhalf(x); }
        case 2u: { return petir_fd_0(x); }
        case 3u: { return petir_fd_half(x); }
        case 4u: { return petir_fd_1(x); }
        case 5u: { return petir_fd_3half(x); }
        case 6u: { return petir_fd_2(x); }
        default: { return bitcast<f32>(0x7fc00000u); }
    }
}
