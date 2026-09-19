// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::airy`, which ports GSL
// 2.8's specfunc/airy.c.
//   Copyright (C) 1996-2000 Gerard Jungman. The underlying Chebyshev
//   expansions are SLATEC's (ai, aie, bi, bie), which GSL vendored.
//
// Ai(x) and Bi(x) solve y'' - x y = 0: Ai decays as x -> +inf, Bi grows, and
// both oscillate as x -> -inf with slowly increasing wavelength. They are the
// canonical turning-point functions.
//
// EVERY COEFFICIENT WAS EXTRACTED BY SCRIPT from the f64 module -- 281
// literals across thirteen Chebyshev series -- and the same script emits
// `wgsl::mirror_airy`, so the shader and its CPU mirror cannot disagree about
// a constant. The f64 module's own tables are audited bit-for-bit against
// GSL's source by tests/gsl_tables_audit.rs.
//
// THE ORDER MATTERS AND WAS NOT GUESSED. In airy.c the data arrays are not
// named after the series that use them -- aif_cs points at ai_data_f, bif_cs
// at data_bif, aip_cs at data_aip. The extraction read the cheb_series
// declarations; guessing would have mis-paired four of the thirteen.
//
// WHAT f32 COSTS HERE, AND WHERE. Measured against the f64 module; the full
// per-branch table is in mirror_airy. The short version is that the
// oscillatory branch below x = -1 is the expensive one, and it is the PHASE,
// not the modulus, that costs: theta grows like (2/3)|x|^{3/2}, so by
// x = -30 it is about 110 radians, where one f32 ulp is already 7.6e-06 rad.
// cos and sin then carry that straight into the answer, and WGSL specifies
// its trigonometric builtins to an ABSOLUTE 2^-11 inside [-pi, pi] with the
// behaviour outside left to the implementation -- so a conforming device may
// be much further out than llvmpipe is here.
//
// PREFER THE SCALED ENTRY POINTS ON A GPU. petir_airy_bi overflows f32 near
// x = 26.07 where the f64 module reaches x = 104.1; petir_airy_bi_scaled does
// not overflow at all, and petir_airy_ai_scaled stays representable far past
// where petir_airy_ai has underflowed to zero. This is the same advice
// bessel.wgsl gives for I_0_scaled, for the same reason.
//
// THE OVERFLOW GUARD CARRIES GSL'S f64 CONSTANT AND THAT IS DELIBERATE.
// Retargeting it to the f32 analogue of the same inequality (x = 25.87) was
// predicted to be necessary, by analogy with debye.wgsl. It is not merely
// unnecessary -- it is WRONG: over x in (25.87, 26.07) it would return +inf
// for values between 3.6e37 and 7.8e37, all representable in an f32 whose
// maximum is 3.4e38. Upstream's constant never fires before exp() genuinely
// overflows. Measured table in mirror_airy.
//
// NOTE ON BIT-IDENTITY: these series are held in inline array<f32, N>
// literals, which docs/wgsl-coverage.md records as costing GPU/CPU
// bit-identity even in a branch that calls no transcendental. Do not expect
// exact agreement with the mirror; the budgets in tests/wgsl_gpu.rs are set
// accordingly.

const PETIR_AIRY_PI_4: f32 = 0.7853982;    // pi/4

// GSL's am21 series, 37 coefficients.
fn petir_airy_cheb_am21(x: f32) -> f32 {
    var c = array<f32, 37>(
        0.006580919027328491, 0.002367598470300436, 0.0001324741606367752,
        1.5760089809191413e-05, 2.752970203800942e-06, 6.102678753450164e-07,
        1.5950884346693783e-07, 4.710339496227789e-08, 1.529338788941459e-08,
        5.359072297039802e-09, 2.0000909817241563e-09, 7.872292262511849e-10,
        3.243103008365722e-10, 1.390105947018938e-10, 6.170110256054073e-11,
        2.8249099240373887e-11, 1.329790010745624e-11, 6.418799953361587e-12,
        3.1696999625713262e-12, 1.5981000160356085e-12, 8.213000050015518e-13,
        4.2960001333212927e-13, 2.2840000832476115e-13, 1.2319999350123706e-13,
        6.750000135658657e-14, 3.7400001173283626e-14, 2.099999997029825e-14,
        1.1899999757293556e-14, 6.79999998231531e-15, 3.9000001517900795e-15,
        2.3000000189311474e-15, 1.3000000153036537e-15, 8.000000134899068e-16,
        5.000000018137469e-16, 3.0000001167615996e-16, 1.0000000168623835e-16,
        1.0000000168623835e-16,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 36; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's ath1 series, 36 coefficients.
fn petir_airy_cheb_ath1(x: f32) -> f32 {
    var c = array<f32, 36>(
        -0.07125838100910187, -0.005904719699174166, -0.0001211454436997883,
        -9.886085535981692e-06, -1.3808410130877746e-06, -2.6142640763282543e-07,
        -6.05043268819827e-08, -1.618436229477993e-08, -4.834649125484702e-09,
        -1.5765526661937201e-09, -5.523151935804549e-10, -2.0545440349017952e-10,
        -8.043411769964592e-11, -3.291251993164934e-11, -1.399875007579432e-11,
        -6.161510104213397e-12, -2.796140055605356e-12, -1.304280034669647e-12,
        -6.237300172878824e-13, -3.0511999719352867e-13, -1.523899956961186e-13,
        -7.757999836077376e-14, -4.020000071757249e-14, -2.1169999482812188e-14,
        -1.1319999875891804e-14, -6.139999956627758e-15, -3.369999989990039e-15,
        -1.8800000025845235e-15, -1.0500000408665599e-15, -6.000000233523199e-16,
        -3.400000044097214e-16, -2.000000033724767e-16, -1.0999999986962872e-16,
        -7.000000051862236e-17, -3.999999935100636e-17, -1.999999967550318e-17,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 35; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's am22 series, 33 coefficients.
fn petir_airy_cheb_am22(x: f32) -> f32 {
    var c = array<f32, 33>(
        -0.015628444030880928, 0.007783364504575729, 0.0008670577662996948,
        0.00015696627087891102, 3.563962673069909e-05, 9.245983164873905e-06,
        2.621101657496183e-06, 7.918822007013659e-07, 2.5104154133259726e-07,
        8.265223527814669e-08, 2.8057115741830785e-08, 9.768211128857729e-09,
        3.47407924650156e-09, 1.2582813679884453e-09, 4.629882588425005e-10,
        1.7272824837100131e-10, 6.52319170901805e-11, 2.4904710238526917e-11,
        9.601559998462239e-12, 3.734480173017696e-12, 1.4641700395515156e-12,
        5.782599876366645e-13, 2.29910003795436e-13, 9.196999715618825e-14,
        3.6999998334272255e-14, 1.4959999452873914e-14, 6.079999954292526e-15,
        2.4800000259368434e-15, 1.0099999687236596e-15, 4.0999999831089888e-16,
        1.700000022048607e-16, 7.000000051862236e-17, 1.999999967550318e-17,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 32; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's ath2 series, 32 coefficients.
fn petir_airy_cheb_ath2(x: f32) -> f32 {
    var c = array<f32, 32>(
        0.004405273590236902, -0.030429193750023842, -0.001385653275065124,
        -0.000180444389116019, -3.380847192602232e-05, -7.678183465031907e-06,
        -1.967839352801093e-06, -5.483727250066295e-07, -1.625461578669274e-07,
        -5.053049889625072e-08, -1.631580737182503e-08, -5.434204197740655e-09,
        -1.857398568283486e-09, -6.489512260898778e-10, -2.310594771071095e-10,
        -8.363282288925689e-11, -3.071196075232763e-11, -1.1423670169541378e-11,
        -4.298110044959058e-12, -1.633889963430224e-12, -6.269299857898647e-13,
        -2.42599993913184e-13, -9.461000234113615e-14, -3.7159999469876803e-14,
        -1.4689999230607133e-14, -5.839999944951598e-15, -2.3300000200987634e-15,
        -9.300000361960959e-16, -3.700000055773374e-16, -1.5000000583807998e-16,
        -6.000000233523199e-17, -1.999999967550318e-17,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 31; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's aif series, 9 coefficients.
fn petir_airy_cheb_aif(x: f32) -> f32 {
    var c = array<f32, 9>(
        -0.03797135874629021, 0.05919189006090164, 0.0009862928418442607,
        6.8488438955682795e-06, 2.59420254167253e-08, 6.176612693531425e-11,
        1.0092453809522686e-13, 1.2014000333477736e-16, 9.999999682655225e-20,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 8; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's aig series, 8 coefficients.
fn petir_airy_cheb_aig(x: f32) -> f32 {
    var c = array<f32, 8>(
        0.01815236546099186, 0.02157256379723549, 0.0002567835617810488, 1.4265214076658594e-06,
        4.5721151309408015e-09, 9.525169715474124e-12, 1.3919999694740875e-14,
        9.99999983775159e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 7; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's bif series, 9 coefficients.
fn petir_airy_cheb_bif(x: f32) -> f32 {
    var c = array<f32, 9>(
        -0.01673021726310253, 0.10252335667610168, 0.0017083092825487256,
        1.1862545761687215e-05, 4.493290717277887e-08, 1.0698206903692054e-10,
        1.748064312658698e-13, 2.0809999415861237e-16, 1.7999999428779406e-19,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 8; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's big series, 8 coefficients.
fn petir_airy_cheb_big(x: f32) -> f32 {
    var c = array<f32, 8>(
        0.022466223686933517, 0.03736477717757225, 0.00044476217590272427,
        2.4708076580282068e-06, 7.91913556952295e-09, 1.6498070271042664e-11,
        2.411000002075503e-14, 1.999999967550318e-17,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 7; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's bif2 series, 10 coefficients.
fn petir_airy_cheb_bif2(x: f32) -> f32 {
    var c = array<f32, 10>(
        0.09984572976827621, 0.47862496972084045, 0.025155212730169296, 0.0005820693913847208,
        7.499766070395708e-06, 6.134602870133676e-08, 3.4627539724496614e-10,
        1.4288909682205753e-12, 4.496199961824543e-15, 1.1100000332756245e-17,
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

// GSL's big2 series, 10 coefficients.
fn petir_airy_cheb_big2(x: f32) -> f32 {
    var c = array<f32, 10>(
        0.033305663615465164, 0.16130921244621277, 0.006319007370620966, 0.0001187904563266784,
        1.3045346349827014e-06, 9.374126364036783e-09, 4.745802015260203e-11,
        1.7831069466008737e-13, 5.166999975226942e-16, 1.0999999780167719e-18,
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

// GSL's aip series, 36 coefficients.
fn petir_airy_cheb_aip(x: f32) -> f32 {
    var c = array<f32, 36>(
        -0.018751930445432663, -0.009144384413957596, 0.0009010457433760166,
        -0.000139418407343328, 2.738158218562603e-05, -6.2750423239776865e-06,
        1.6064843748608837e-06, -4.4763922346646723e-07, 1.3346358684884763e-07,
        -4.2073533990105716e-08, 1.3902199391679915e-08, -4.783184959222808e-09,
        1.704789753809166e-09, -6.268389696195698e-10, 2.3698243367675786e-10,
        -9.186411353834245e-11, 3.6427853788989495e-11, -1.474755560726404e-11,
        6.0851007392670464e-12, -2.5552771704129285e-12, 1.0906186977827081e-12,
        -4.725870302729751e-13, 2.0769691034313448e-13, -9.249762414342833e-14,
        4.1709670927595616e-14, -1.9029909755201996e-14, 8.779067899619512e-15,
        -4.092755627948478e-15, 1.927106892913124e-15, -9.16019863280782e-16,
        4.3935670442485483e-16, -2.1255030526348646e-16, 1.0367349755169655e-16,
        -5.096419896105794e-17, 2.523770069674405e-17, -1.2579300361744028e-17,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 35; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's bip series, 24 coefficients.
fn petir_airy_cheb_bip(x: f32) -> f32 {
    var c = array<f32, 24>(
        -0.083220474421978, 0.011461189016699791, 0.000428964413003996, -0.00014906639989931136,
        -1.3076597497274633e-05, 6.327598384814337e-06, -4.2226696450597956e-07,
        -1.9147186947066075e-07, 6.453106493609084e-08, -7.84485454374817e-09,
        -9.607721285220805e-10, 7.000471313745038e-10, -1.773178964770139e-10,
        2.2720889406024902e-11, 1.6540399692260843e-12, -1.8517099745901655e-12,
        5.957599864825358e-13, -1.2194000149901019e-13, 1.3339999813339123e-14,
        1.7200000316502776e-15, -1.4500000211417337e-15, 4.899999837780218e-16,
        -1.0999999986962872e-16, 9.99999983775159e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 23; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's bip2 series, 29 coefficients.
fn petir_airy_cheb_bip2(x: f32) -> f32 {
    var c = array<f32, 29>(
        -0.11359673738479614, 0.004138147458434105, 0.0001353470579488203,
        1.0427316738059744e-05, 1.3474955267156474e-06, 1.696537452744451e-07,
        -1.0096500524525709e-08, -1.6729119423075645e-08, -4.5815364835277705e-09,
        3.7366812422057194e-10, 5.766930266659642e-10, 6.218126707979721e-11,
        -6.329411994521195e-11, -1.491504836304003e-11, 7.88962124798065e-12,
        2.4960513256983008e-12, -1.21300745056091e-12, -3.740492887215757e-13,
        2.237727012152288e-13, 4.749019984228754e-14, -4.5261598559821065e-14,
        -3.0171999677885454e-15, 9.105799847594544e-15, -9.814000071387365e-16,
        -1.6429000222967574e-15, 5.532999735361973e-16, 2.1749999523032617e-16,
        -1.737000009371451e-16, -1.000000045813705e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 28; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// Airy modulus and phase for x <= -1. Returns vec2(modulus, phase).
//
// Below -1 both functions oscillate, and a Chebyshev series in x directly
// would need more terms as the oscillation tightens. M and theta are smooth
// in 16/x^3, which is why this decomposition exists at all.
fn petir_airy_mod_phase(x: f32) -> vec2<f32> {
    var m: f32;
    var p: f32;
    if (x < -2.0) {
        let z = 16.0 / (x * x * x) + 1.0;
        m = petir_airy_cheb_am21(z);
        p = petir_airy_cheb_ath1(z);
    } else if (x <= -1.0) {
        let z = (16.0 / (x * x * x) + 9.0) / 7.0;
        m = petir_airy_cheb_am22(z);
        p = petir_airy_cheb_ath2(z);
    } else {
        return vec2<f32>(bitcast<f32>(0x7fc00000u), bitcast<f32>(0x7fc00000u));
    }
    let mm = 0.3125 + m;
    let pp = -0.625 + p;
    let sqx = sqrt(-x);
    return vec2<f32>(sqrt(mm / sqx), PETIR_AIRY_PI_4 - x * sqx * pp);
}

// exp(+(2/3) x^{3/2}) Ai(x), for x >= 1. Upstream's airy_aie.
fn petir_airy_aie(x: f32) -> f32 {
    let sqx = sqrt(x);
    let z = 2.0 / (x * sqx) - 1.0;
    let y = sqrt(sqx);
    return (0.28125 + petir_airy_cheb_aip(z)) / y;
}

// exp(-(2/3) x^{3/2}) Bi(x), for x >= 2. Upstream's airy_bie, two
// sub-branches at x = 4. ATR and BTR are upstream's own constants.
fn petir_airy_bie(x: f32) -> f32 {
    let sqx = sqrt(x);
    let y = sqrt(sqx);
    var c: f32;
    if (x < 4.0) {
        c = petir_airy_cheb_bip(8.750691 / (x * sqx) - 2.0938363);
    } else {
        c = petir_airy_cheb_bip2(16.0 / (x * sqx) - 1.0);
    }
    return (0.625 + c) / y;
}

// Ai(x) for all real x. NaN propagates; underflows to zero on the positive
// axis and never overflows.
fn petir_airy_ai(x: f32) -> f32 {
    if (!(x == x)) { return bitcast<f32>(0x7fc00000u); }
    if (x < -1.0) {
        let mp = petir_airy_mod_phase(x);
        return mp.x * cos(mp.y);
    }
    if (x <= 1.0) {
        let z = x * x * x;
        return 0.375 + (petir_airy_cheb_aif(z) - x * (0.25 + petir_airy_cheb_aig(z)));
    }
    let x32 = x * sqrt(x);
    return petir_airy_aie(x) * exp(-2.0 * x32 / 3.0);
}

// exp(+(2/3) x^{3/2}) Ai(x) for x > 0, and plain Ai(x) for x <= 0 where the
// function oscillates and needs no scaling. That asymmetry is upstream's.
fn petir_airy_ai_scaled(x: f32) -> f32 {
    if (!(x == x)) { return bitcast<f32>(0x7fc00000u); }
    if (x < -1.0) {
        let mp = petir_airy_mod_phase(x);
        return mp.x * cos(mp.y);
    }
    if (x <= 1.0) {
        let z = x * x * x;
        let v = 0.375 + (petir_airy_cheb_aif(z) - x * (0.25 + petir_airy_cheb_aig(z)));
        if (x > 0.0) { return v * exp(2.0 / 3.0 * sqrt(z)); }
        return v;
    }
    return petir_airy_aie(x);
}

// Bi(x) for all real x. OVERFLOWS TO +inf on the positive axis -- near
// x = 25.9 in f32, against x = 104.1 in f64. Use petir_airy_bi_scaled there.
//
// The guard below carries GSL's f64 threshold unchanged. In f32 it never
// fires before exp() has overflowed on its own, so it is redundant rather
// than wrong -- see mirror_airy, which measures that and keeps it as the more
// literal transcription.
fn petir_airy_bi(x: f32) -> f32 {
    if (!(x == x)) { return bitcast<f32>(0x7fc00000u); }
    if (x < -1.0) {
        let mp = petir_airy_mod_phase(x);
        return mp.x * sin(mp.y);
    }
    if (x < 1.0) {
        let z = x * x * x;
        return 0.625 + petir_airy_cheb_bif(z) + x * (0.4375 + petir_airy_cheb_big(z));
    }
    if (x <= 2.0) {
        let z = (2.0 * x * x * x - 9.0) / 7.0;
        return 1.125 + petir_airy_cheb_bif2(z) + x * (0.625 + petir_airy_cheb_big2(z));
    }
    let y = 2.0 * x * sqrt(x) / 3.0;
    // GSL_LOG_DBL_MAX - 1, upstream's own bound, kept deliberately.
    if (y > 708.3964) {
        return bitcast<f32>(0x7f800000u);
    }
    return petir_airy_bie(x) * exp(y);
}

// exp(-(2/3) x^{3/2}) Bi(x) for x > 0, plain Bi(x) for x <= 0. Note the
// exponent's sign is the OPPOSITE of ai_scaled's, because Bi grows where Ai
// decays. Never overflows.
fn petir_airy_bi_scaled(x: f32) -> f32 {
    if (!(x == x)) { return bitcast<f32>(0x7fc00000u); }
    if (x < -1.0) {
        let mp = petir_airy_mod_phase(x);
        return mp.x * sin(mp.y);
    }
    if (x < 1.0) {
        let z = x * x * x;
        let v = 0.625 + petir_airy_cheb_bif(z) + x * (0.4375 + petir_airy_cheb_big(z));
        if (x > 0.0) { return v * exp(-2.0 / 3.0 * sqrt(z)); }
        return v;
    }
    if (x <= 2.0) {
        let x3 = x * x * x;
        let z = (2.0 * x3 - 9.0) / 7.0;
        let s = exp(-2.0 / 3.0 * sqrt(x3));
        return s * (1.125 + petir_airy_cheb_bif2(z) + x * (0.625 + petir_airy_cheb_big2(z)));
    }
    return petir_airy_bie(x);
}
