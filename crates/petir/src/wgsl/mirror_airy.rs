//! `f32` mirrors of `shaders/airy.wgsl` — GSL's Airy functions `Ai` and `Bi`
//! and their exponentially scaled forms.
//!
//! Generated from the same parse of [`crate::specfunc::airy`] that produced
//! the shader, so the two cannot drift in their 281 coefficients. See
//! [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # The oscillatory branch is where `f32` is spent, and it is the phase
//!
//! Below `x = -1` both functions are written as `M(x) cos(theta)` and
//! `M(x) sin(theta)`, with `theta` growing like `(2/3)|x|^{3/2}`.
//!
//! **Measured, separating the two.** Running the identical `mod_phase`
//! formula in `f64` on this module's own coefficients isolates `f32`
//! arithmetic from everything else:
//!
//! | `x` | \|theta\| | modulus, relative | phase, absolute | phase / (\|theta\| eps) |
//! |---|---|---|---|---|
//! | -2 | 1.07 | 4.218e-08 | 5.022e-08 | 0.39 |
//! | -5 | 6.66 | 5.999e-08 | 1.697e-07 | 0.21 |
//! | -10 | 20.29 | 1.663e-08 | 1.259e-06 | 0.52 |
//! | -20 | 58.84 | 3.694e-08 | 1.752e-06 | 0.25 |
//! | -30 | 108.76 | 1.735e-09 | 4.263e-06 | 0.33 |
//! | -50 | 234.92 | 2.964e-08 | 1.214e-05 | 0.43 |
//!
//! The modulus is flat at about one `f32` ulp and does not grow. The phase's
//! absolute error grows **linearly with `theta`**, and the last column is why
//! that is not a defect: it stays between 0.2 and 0.5 of one ulp *of theta
//! itself*, so the phase is as accurate as `f32` can represent it. The error
//! is the **representation**, not the transcription, and no `f32`
//! implementation of `Ai` can do better on this branch.
//!
//! `cos` and `sin` then carry that through, and WGSL specifies them to an
//! absolute 2^-11 inside `[-pi, pi]` with the behaviour outside left to the
//! implementation — so a conforming device may be much further out than
//! llvmpipe is here. `the_phase_is_what_costs_the_oscillatory_branch`
//! re-measures the whole table.
//!
//! # GSL's `f64` overflow threshold is KEPT, and retargeting it would be WRONG
//!
//! `Bi` overflows when `(2/3) x^{3/2}` exceeds `ln(MAX) - 1`. In `f64` that
//! is `x = 104.1`; the `f32` analogue of the same inequality gives
//! `x = 25.87`. By analogy with [`crate::wgsl::mirror_debye`], where two
//! `f64` thresholds genuinely had to be retargeted, retargeting looked
//! necessary here too.
//!
//! ~~It is not, because `exp` overflows on its own at the same place, so the
//! guard is merely redundant.~~ **CORRECTED 2026-09-19** — that was written
//! before it was measured, and it is wrong in both halves. The two cuts do
//! **not** coincide, and retargeting is not harmless but actively
//! **destructive**:
//!
//! | `x` | GSL's `f64` cut | retargeted `f32` cut | true `Bi(x)` |
//! |---|---|---|---|
//! | 25.8 | 2.193e+37 | 2.193e+37 | 2.193e+37 |
//! | 25.9 | 3.643e+37 | **`+inf`** | 3.643e+37 |
//! | 26.0 | 6.057e+37 | **`+inf`** | 6.057e+37 |
//! | 26.05 | 7.813e+37 | **`+inf`** | 7.813e+37 |
//! | 26.07 | `+inf` | `+inf` | 8.652e+37 |
//!
//! Over a window of about 0.2 in `x` the retargeted guard returns `+inf` for
//! answers that are comfortably representable — `f32::MAX` is 3.4e+38, four
//! times the largest of them. Upstream's constant simply never fires before
//! `exp` genuinely overflows, which is what makes it correct rather than
//! merely literal. `the_f64_overflow_threshold_is_the_correct_one_in_f32`
//! asserts both columns.
//!
//! **Three cases now, and they do not point the same way.** Debye's `xcut`
//! comes from `f64`'s exponent range and controls a loop length, so
//! retargeting it was worth a factor of fifty; `dilog`'s cut comes from a
//! truncation order and retargeting it is neutral at best; this one is a
//! **range guard on a quantity that is still representable**, and retargeting
//! it loses answers. The rule is *"know what upstream's constant is FOR"*,
//! which is a stronger requirement than knowing what it equals.

// Under a std-linked build (`cargo test`) f32's inherent sqrt/exp/sin/cos
// shadow these trait methods, leaving the import formally unused. See
// crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `pi/4`, the constant term of the Airy phase. Mirrors `PETIR_AIRY_PI_4`.
const PI_4: f32 = 0.7853982;

/// Clenshaw in GSL's convention. Mirrors each `petir_airy_cheb_*`.
fn cheb(x: f32, c: &[f32]) -> f32 {
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

/// GSL's `am21` series, 37 coefficients.
#[rustfmt::skip]
const AM21: [f32; 37] = [
    0.006580919027328491, 0.002367598470300436, 0.0001324741606367752, 1.5760089809191413e-05,
    2.752970203800942e-06, 6.102678753450164e-07, 1.5950884346693783e-07, 4.710339496227789e-08,
    1.529338788941459e-08, 5.359072297039802e-09, 2.0000909817241563e-09, 7.872292262511849e-10,
    3.243103008365722e-10, 1.390105947018938e-10, 6.170110256054073e-11, 2.8249099240373887e-11,
    1.329790010745624e-11, 6.418799953361587e-12, 3.1696999625713262e-12,
    1.5981000160356085e-12, 8.213000050015518e-13, 4.2960001333212927e-13,
    2.2840000832476115e-13, 1.2319999350123706e-13, 6.750000135658657e-14,
    3.7400001173283626e-14, 2.099999997029825e-14, 1.1899999757293556e-14, 6.79999998231531e-15,
    3.9000001517900795e-15, 2.3000000189311474e-15, 1.3000000153036537e-15,
    8.000000134899068e-16, 5.000000018137469e-16, 3.0000001167615996e-16,
    1.0000000168623835e-16, 1.0000000168623835e-16,
];

/// GSL's `ath1` series, 36 coefficients.
#[rustfmt::skip]
const ATH1: [f32; 36] = [
    -0.07125838100910187, -0.005904719699174166, -0.0001211454436997883, -9.886085535981692e-06,
    -1.3808410130877746e-06, -2.6142640763282543e-07, -6.05043268819827e-08,
    -1.618436229477993e-08, -4.834649125484702e-09, -1.5765526661937201e-09,
    -5.523151935804549e-10, -2.0545440349017952e-10, -8.043411769964592e-11,
    -3.291251993164934e-11, -1.399875007579432e-11, -6.161510104213397e-12,
    -2.796140055605356e-12, -1.304280034669647e-12, -6.237300172878824e-13,
    -3.0511999719352867e-13, -1.523899956961186e-13, -7.757999836077376e-14,
    -4.020000071757249e-14, -2.1169999482812188e-14, -1.1319999875891804e-14,
    -6.139999956627758e-15, -3.369999989990039e-15, -1.8800000025845235e-15,
    -1.0500000408665599e-15, -6.000000233523199e-16, -3.400000044097214e-16,
    -2.000000033724767e-16, -1.0999999986962872e-16, -7.000000051862236e-17,
    -3.999999935100636e-17, -1.999999967550318e-17,
];

/// GSL's `am22` series, 33 coefficients.
#[rustfmt::skip]
const AM22: [f32; 33] = [
    -0.015628444030880928, 0.007783364504575729, 0.0008670577662996948, 0.00015696627087891102,
    3.563962673069909e-05, 9.245983164873905e-06, 2.621101657496183e-06, 7.918822007013659e-07,
    2.5104154133259726e-07, 8.265223527814669e-08, 2.8057115741830785e-08,
    9.768211128857729e-09, 3.47407924650156e-09, 1.2582813679884453e-09, 4.629882588425005e-10,
    1.7272824837100131e-10, 6.52319170901805e-11, 2.4904710238526917e-11, 9.601559998462239e-12,
    3.734480173017696e-12, 1.4641700395515156e-12, 5.782599876366645e-13, 2.29910003795436e-13,
    9.196999715618825e-14, 3.6999998334272255e-14, 1.4959999452873914e-14,
    6.079999954292526e-15, 2.4800000259368434e-15, 1.0099999687236596e-15,
    4.0999999831089888e-16, 1.700000022048607e-16, 7.000000051862236e-17, 1.999999967550318e-17,
];

/// GSL's `ath2` series, 32 coefficients.
#[rustfmt::skip]
const ATH2: [f32; 32] = [
    0.004405273590236902, -0.030429193750023842, -0.001385653275065124, -0.000180444389116019,
    -3.380847192602232e-05, -7.678183465031907e-06, -1.967839352801093e-06,
    -5.483727250066295e-07, -1.625461578669274e-07, -5.053049889625072e-08,
    -1.631580737182503e-08, -5.434204197740655e-09, -1.857398568283486e-09,
    -6.489512260898778e-10, -2.310594771071095e-10, -8.363282288925689e-11,
    -3.071196075232763e-11, -1.1423670169541378e-11, -4.298110044959058e-12,
    -1.633889963430224e-12, -6.269299857898647e-13, -2.42599993913184e-13,
    -9.461000234113615e-14, -3.7159999469876803e-14, -1.4689999230607133e-14,
    -5.839999944951598e-15, -2.3300000200987634e-15, -9.300000361960959e-16,
    -3.700000055773374e-16, -1.5000000583807998e-16, -6.000000233523199e-17,
    -1.999999967550318e-17,
];

/// GSL's `aif` series, 9 coefficients.
#[rustfmt::skip]
const AIF: [f32; 9] = [
    -0.03797135874629021, 0.05919189006090164, 0.0009862928418442607, 6.8488438955682795e-06,
    2.59420254167253e-08, 6.176612693531425e-11, 1.0092453809522686e-13, 1.2014000333477736e-16,
    9.999999682655225e-20,
];

/// GSL's `aig` series, 8 coefficients.
#[rustfmt::skip]
const AIG: [f32; 8] = [
    0.01815236546099186, 0.02157256379723549, 0.0002567835617810488, 1.4265214076658594e-06,
    4.5721151309408015e-09, 9.525169715474124e-12, 1.3919999694740875e-14, 9.99999983775159e-18,
];

/// GSL's `bif` series, 9 coefficients.
#[rustfmt::skip]
const BIF: [f32; 9] = [
    -0.01673021726310253, 0.10252335667610168, 0.0017083092825487256, 1.1862545761687215e-05,
    4.493290717277887e-08, 1.0698206903692054e-10, 1.748064312658698e-13,
    2.0809999415861237e-16, 1.7999999428779406e-19,
];

/// GSL's `big` series, 8 coefficients.
#[rustfmt::skip]
const BIG: [f32; 8] = [
    0.022466223686933517, 0.03736477717757225, 0.00044476217590272427, 2.4708076580282068e-06,
    7.91913556952295e-09, 1.6498070271042664e-11, 2.411000002075503e-14, 1.999999967550318e-17,
];

/// GSL's `bif2` series, 10 coefficients.
#[rustfmt::skip]
const BIF2: [f32; 10] = [
    0.09984572976827621, 0.47862496972084045, 0.025155212730169296, 0.0005820693913847208,
    7.499766070395708e-06, 6.134602870133676e-08, 3.4627539724496614e-10,
    1.4288909682205753e-12, 4.496199961824543e-15, 1.1100000332756245e-17,
];

/// GSL's `big2` series, 10 coefficients.
#[rustfmt::skip]
const BIG2: [f32; 10] = [
    0.033305663615465164, 0.16130921244621277, 0.006319007370620966, 0.0001187904563266784,
    1.3045346349827014e-06, 9.374126364036783e-09, 4.745802015260203e-11,
    1.7831069466008737e-13, 5.166999975226942e-16, 1.0999999780167719e-18,
];

/// GSL's `aip` series, 36 coefficients.
#[rustfmt::skip]
const AIP: [f32; 36] = [
    -0.018751930445432663, -0.009144384413957596, 0.0009010457433760166, -0.000139418407343328,
    2.738158218562603e-05, -6.2750423239776865e-06, 1.6064843748608837e-06,
    -4.4763922346646723e-07, 1.3346358684884763e-07, -4.2073533990105716e-08,
    1.3902199391679915e-08, -4.783184959222808e-09, 1.704789753809166e-09,
    -6.268389696195698e-10, 2.3698243367675786e-10, -9.186411353834245e-11,
    3.6427853788989495e-11, -1.474755560726404e-11, 6.0851007392670464e-12,
    -2.5552771704129285e-12, 1.0906186977827081e-12, -4.725870302729751e-13,
    2.0769691034313448e-13, -9.249762414342833e-14, 4.1709670927595616e-14,
    -1.9029909755201996e-14, 8.779067899619512e-15, -4.092755627948478e-15,
    1.927106892913124e-15, -9.16019863280782e-16, 4.3935670442485483e-16,
    -2.1255030526348646e-16, 1.0367349755169655e-16, -5.096419896105794e-17,
    2.523770069674405e-17, -1.2579300361744028e-17,
];

/// GSL's `bip` series, 24 coefficients.
#[rustfmt::skip]
const BIP: [f32; 24] = [
    -0.083220474421978, 0.011461189016699791, 0.000428964413003996, -0.00014906639989931136,
    -1.3076597497274633e-05, 6.327598384814337e-06, -4.2226696450597956e-07,
    -1.9147186947066075e-07, 6.453106493609084e-08, -7.84485454374817e-09,
    -9.607721285220805e-10, 7.000471313745038e-10, -1.773178964770139e-10,
    2.2720889406024902e-11, 1.6540399692260843e-12, -1.8517099745901655e-12,
    5.957599864825358e-13, -1.2194000149901019e-13, 1.3339999813339123e-14,
    1.7200000316502776e-15, -1.4500000211417337e-15, 4.899999837780218e-16,
    -1.0999999986962872e-16, 9.99999983775159e-18,
];

/// GSL's `bip2` series, 29 coefficients.
#[rustfmt::skip]
const BIP2: [f32; 29] = [
    -0.11359673738479614, 0.004138147458434105, 0.0001353470579488203, 1.0427316738059744e-05,
    1.3474955267156474e-06, 1.696537452744451e-07, -1.0096500524525709e-08,
    -1.6729119423075645e-08, -4.5815364835277705e-09, 3.7366812422057194e-10,
    5.766930266659642e-10, 6.218126707979721e-11, -6.329411994521195e-11,
    -1.491504836304003e-11, 7.88962124798065e-12, 2.4960513256983008e-12, -1.21300745056091e-12,
    -3.740492887215757e-13, 2.237727012152288e-13, 4.749019984228754e-14,
    -4.5261598559821065e-14, -3.0171999677885454e-15, 9.105799847594544e-15,
    -9.814000071387365e-16, -1.6429000222967574e-15, 5.532999735361973e-16,
    2.1749999523032617e-16, -1.737000009371451e-16, -1.000000045813705e-18,
];

/// Airy modulus and phase for `x <= -1`. Mirrors `petir_airy_mod_phase`.
fn mod_phase(x: f32) -> (f32, f32) {
    let (m, p) = if x < -2.0 {
        let z = 16.0 / (x * x * x) + 1.0;
        (cheb(z, &AM21), cheb(z, &ATH1))
    } else if x <= -1.0 {
        let z = (16.0 / (x * x * x) + 9.0) / 7.0;
        (cheb(z, &AM22), cheb(z, &ATH2))
    } else {
        return (f32::NAN, f32::NAN);
    };
    let m = 0.3125 + m;
    let p = -0.625 + p;
    let sqx = (-x).sqrt();
    ((m / sqx).sqrt(), PI_4 - x * sqx * p)
}

/// `exp(+(2/3) x^{3/2}) Ai(x)` for `x >= 1`. Mirrors `petir_airy_aie`.
fn aie(x: f32) -> f32 {
    let sqx = x.sqrt();
    let z = 2.0 / (x * sqx) - 1.0;
    (0.28125 + cheb(z, &AIP)) / sqx.sqrt()
}

/// `exp(-(2/3) x^{3/2}) Bi(x)` for `x >= 2`. Mirrors `petir_airy_bie`.
fn bie(x: f32) -> f32 {
    let sqx = x.sqrt();
    let c = if x < 4.0 {
        cheb(8.750691 / (x * sqx) - 2.0938363, &BIP)
    } else {
        cheb(16.0 / (x * sqx) - 1.0, &BIP2)
    };
    (0.625 + c) / sqx.sqrt()
}

/// `Ai(x)` in `f32` for all real `x`. Mirrors `petir_airy_ai`.
pub fn airy_ai(x: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    if x < -1.0 {
        let (m, t) = mod_phase(x);
        m * t.cos()
    } else if x <= 1.0 {
        let z = x * x * x;
        0.375 + (cheb(z, &AIF) - x * (0.25 + cheb(z, &AIG)))
    } else {
        aie(x) * (-2.0 * x * x.sqrt() / 3.0).exp()
    }
}

/// `exp(+(2/3) x^{3/2}) Ai(x)` for `x > 0`, plain `Ai(x)` below. Mirrors
/// `petir_airy_ai_scaled`.
pub fn airy_ai_scaled(x: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    if x < -1.0 {
        let (m, t) = mod_phase(x);
        m * t.cos()
    } else if x <= 1.0 {
        let z = x * x * x;
        let v = 0.375 + (cheb(z, &AIF) - x * (0.25 + cheb(z, &AIG)));
        if x > 0.0 {
            v * (2.0 / 3.0 * z.sqrt()).exp()
        } else {
            v
        }
    } else {
        aie(x)
    }
}

/// `Bi(x)` in `f32` for all real `x`. Mirrors `petir_airy_bi`.
///
/// Overflows to `+inf` near `x = 25.9`, against `x = 104.1` for the `f64`
/// module. See the module documentation on why GSL's `f64` threshold is
/// nonetheless kept verbatim.
pub fn airy_bi(x: f32) -> f32 {
    airy_bi_at_cut(x, 708.3964)
}

/// [`airy_bi`] with the overflow threshold supplied, so that keeping GSL's
/// `f64` value can be *measured* against retargeting it rather than argued.
fn airy_bi_at_cut(x: f32, cut: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    if x < -1.0 {
        let (m, t) = mod_phase(x);
        m * t.sin()
    } else if x < 1.0 {
        let z = x * x * x;
        0.625 + cheb(z, &BIF) + x * (0.4375 + cheb(z, &BIG))
    } else if x <= 2.0 {
        let z = (2.0 * x * x * x - 9.0) / 7.0;
        1.125 + cheb(z, &BIF2) + x * (0.625 + cheb(z, &BIG2))
    } else {
        let y = 2.0 * x * x.sqrt() / 3.0;
        if y > cut {
            f32::INFINITY
        } else {
            bie(x) * y.exp()
        }
    }
}

/// `exp(-(2/3) x^{3/2}) Bi(x)` for `x > 0`, plain `Bi(x)` below. Mirrors
/// `petir_airy_bi_scaled`. Never overflows.
pub fn airy_bi_scaled(x: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    if x < -1.0 {
        let (m, t) = mod_phase(x);
        m * t.sin()
    } else if x < 1.0 {
        let z = x * x * x;
        let v = 0.625 + cheb(z, &BIF) + x * (0.4375 + cheb(z, &BIG));
        if x > 0.0 {
            v * (-2.0 / 3.0 * z.sqrt()).exp()
        } else {
            v
        }
    } else if x <= 2.0 {
        let x3 = x * x * x;
        let z = (2.0 * x3 - 9.0) / 7.0;
        (-2.0 / 3.0 * x3.sqrt()).exp() * (1.125 + cheb(z, &BIF2) + x * (0.625 + cheb(z, &BIG2)))
    } else {
        bie(x)
    }
}

// Everything above this line is generated; the test module below is written
// by hand, so re-running the generator would drop it. Same arrangement as
// `mirror_bessel`, `mirror_psi_zeta` and `mirror_debye`.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::specfunc::airy as f64_airy;

    /// The **same** `mod_phase` formula in `f64`, using this module's own
    /// `f32` coefficients promoted. Any difference from [`mod_phase`] is
    /// therefore `f32` **arithmetic** alone — not the coefficients, not the
    /// branch, not the upstream tables. That is what makes the attribution
    /// below a measurement rather than an assertion.
    fn mod_phase_ref(x: f32) -> (f64, f64) {
        fn cheb64(x: f64, c: &[f32]) -> f64 {
            let Some((&c0, rest)) = c.split_first() else {
                return 0.0;
            };
            let (mut d, mut dd) = (0.0f64, 0.0f64);
            let y2 = 2.0 * x;
            for &ci in rest.iter().rev() {
                let t = d;
                d = y2 * d - dd + ci as f64;
                dd = t;
            }
            x * d - dd + 0.5 * (c0 as f64)
        }
        let x = x as f64;
        let (m, p) = if x < -2.0 {
            let z = 16.0 / (x * x * x) + 1.0;
            (cheb64(z, &AM21), cheb64(z, &ATH1))
        } else {
            let z = (16.0 / (x * x * x) + 9.0) / 7.0;
            (cheb64(z, &AM22), cheb64(z, &ATH2))
        };
        let (m, p) = (0.3125 + m, -0.625 + p);
        let sqx = (-x).sqrt();
        ((m / sqx).sqrt(), (PI_4 as f64) - x * sqx * p)
    }

    /// Worst relative and absolute difference against the `f64` module over a
    /// window, skipping where the function is too small for a relative figure
    /// to mean anything.
    fn worst(
        f: &dyn Fn(f32) -> f32,
        g: &dyn Fn(f64) -> f64,
        lo: f32,
        hi: f32,
        floor: f64,
    ) -> (f64, f32, f64) {
        let (mut wr, mut at, mut wa) = (0.0f64, 0.0f32, 0.0f64);
        for k in 0..=2000 {
            let x = lo + (hi - lo) * (k as f32 / 2000.0);
            let r = g(x as f64);
            let v = f(x) as f64;
            if !r.is_finite() || !v.is_finite() {
                continue;
            }
            wa = wa.max((v - r).abs());
            if r.abs() > floor {
                let e = ((v - r) / r).abs();
                if e > wr {
                    wr = e;
                    at = x;
                }
            }
        }
        (wr, at, wa)
    }

    /// What `f32` costs, branch by branch, against PETIR's own `f64` `airy`.
    ///
    /// # Results, measured 2026-09-19
    ///
    /// Worst relative where `|f| > 1e-3`:
    ///
    /// | window | `Ai` | `Bi` | route |
    /// |---|---|---|---|
    /// | `[-30, -8)` | **1.590e-03** | **1.739e-03** | modulus / phase |
    /// | `[-8, -1)` | 2.082e-04 | 1.568e-04 | modulus / phase |
    /// | `[-1, 1]` | 2.314e-07 | 5.233e-07 | central series |
    /// | `(1, 30]` | 4.248e-07 | — | `aie` |
    /// | `(1, 2]` | — | 1.416e-07 | second central series |
    /// | `(2, 25]` | — | 7.826e-06 | `bie` |
    ///
    /// Everything away from the oscillatory branch is one to two `f32` ulp.
    /// The oscillatory branch is three to four orders worse in *relative*
    /// terms, and `the_phase_is_what_costs_the_oscillatory_branch` shows why
    /// and that it is irreducible. The bounded statement there is the
    /// **absolute** error, 3.558e-06 over `[-30, -8)`, which is asserted
    /// separately.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        let ai = &airy_ai as &dyn Fn(f32) -> f32;
        let bi = &airy_bi as &dyn Fn(f32) -> f32;
        let ai64 = &f64_airy::airy_ai as &dyn Fn(f64) -> f64;
        let bi64 = &f64_airy::airy_bi as &dyn Fn(f64) -> f64;

        // Away from the oscillation, one or two ulp.
        for (name, f, g, lo, hi, budget) in [
            ("Ai central", ai, ai64, -1.0f32, 1.0f32, 3e-6f64),
            ("Bi central", bi, bi64, -1.0, 1.0, 3e-6),
            ("Ai aie", ai, ai64, 1.0001, 30.0, 3e-6),
            ("Bi bie", bi, bi64, 1.0001, 25.0, 5e-5),
        ] {
            let (wr, at, _) = worst(f, g, lo, hi, 1e-3);
            assert!(wr < budget, "{name}: {wr:e} at {at}");
        }

        // The oscillatory branch, bounded ABSOLUTELY because it crosses the
        // zeros of both functions.
        for (name, f, g) in [("Ai", ai, ai64), ("Bi", bi, bi64)] {
            let (_, _, wa) = worst(f, g, -30.0, -8.0, 1e-3);
            assert!(wa < 3e-5, "{name} absolute on [-30, -8): {wa:e}");
        }
    }

    /// **The oscillatory branch's error is the phase, and the phase is as
    /// accurate as `f32` allows** — so the error is irreducible, not a defect
    /// in this transcription.
    ///
    /// Two claims, both asserted:
    ///
    /// 1. The **modulus** error does not grow with `|x|`; it stays at about
    ///    one `f32` ulp from `x = -2` to `x = -50`.
    /// 2. The **phase** error does grow, linearly with `theta`, and stays
    ///    below one ulp *of theta itself* — measured between 0.21 and 0.52 of
    ///    `|theta| * f32::EPSILON`.
    ///
    /// Claim 2 is the one that matters: a phase good to a fraction of its own
    /// ulp cannot be improved in `f32`, and `cos`/`sin` turn an absolute
    /// phase error into a comparable relative error in the answer. The
    /// reference is the identical formula evaluated in `f64` on **this
    /// module's own coefficients**, so nothing but the arithmetic differs.
    ///
    /// | `x` | `|theta|` | modulus rel | phase abs | phase / (`|theta|` eps) |
    /// |---|---|---|---|---|
    /// | -2 | 1.07 | 4.218e-08 | 5.022e-08 | 0.39 |
    /// | -10 | 20.29 | 1.663e-08 | 1.259e-06 | 0.52 |
    /// | -30 | 108.76 | 1.735e-09 | 4.263e-06 | 0.33 |
    /// | -50 | 234.92 | 2.964e-08 | 1.214e-05 | 0.43 |
    #[test]
    fn the_phase_is_what_costs_the_oscillatory_branch() {
        let eps = f32::EPSILON as f64;
        let mut worst_mod = 0.0f64;
        let mut worst_ulp_of_theta = 0.0f64;
        let mut phase_err_at = [0.0f64; 2];
        for (i, x) in [-2.0f32, -5.0, -10.0, -20.0, -30.0, -50.0]
            .iter()
            .enumerate()
        {
            let (m32, t32) = mod_phase(*x);
            let (m64, t64) = mod_phase_ref(*x);
            worst_mod = worst_mod.max(((m32 as f64 - m64) / m64).abs());
            let pe = (t32 as f64 - t64).abs();
            worst_ulp_of_theta = worst_ulp_of_theta.max(pe / (t64.abs() * eps));
            if i == 0 {
                phase_err_at[0] = pe;
            }
            if i == 5 {
                phase_err_at[1] = pe;
            }
        }

        // 1. The modulus does not degrade with |x|.
        assert!(
            worst_mod < 5e-7,
            "the modulus is documented as staying at about one f32 ulp \
             across the whole oscillatory branch, and reached {worst_mod:e}"
        );

        // 2. The phase stays inside one ulp OF ITSELF, which is what makes
        // the error irreducible rather than a transcription defect.
        assert!(
            worst_ulp_of_theta < 1.0,
            "the phase is documented as accurate to a FRACTION of one ulp of \
             theta (measured 0.21 to 0.52), which is why the oscillatory \
             branch's error cannot be improved in f32. It measured \
             {worst_ulp_of_theta:.2} ulp -- if it now exceeds one ulp the \
             phase computation itself has a defect and is worth fixing"
        );

        // And the absolute phase error really does grow with theta -- x = -50
        // against x = -2, whose |theta| differ by a factor of 220.
        assert!(
            phase_err_at[1] > 50.0 * phase_err_at[0],
            "the phase error is documented as growing linearly with theta: \
             {:e} at x = -2 against {:e} at x = -50, whose |theta| differ by \
             220x",
            phase_err_at[0],
            phase_err_at[1]
        );
    }

    /// **GSL's `f64` overflow threshold is the correct one in `f32`, and
    /// retargeting it would discard representable answers.**
    ///
    /// The `f32` analogue of upstream's inequality fires at `x = 25.87`; the
    /// value there is still about 2.2e+37 and grows to 7.8e+37 before `exp`
    /// genuinely overflows at `x = 26.07`. `f32::MAX` is 3.4e+38, so every
    /// one of those is comfortably representable and the retargeted guard
    /// would return `+inf` for all of them.
    ///
    /// This is the opposite conclusion to [`crate::wgsl::mirror_debye`],
    /// where retargeting was worth a factor of fifty, and it is asserted here
    /// so the difference between the two cases cannot quietly be forgotten.
    #[test]
    fn the_f64_overflow_threshold_is_the_correct_one_in_f32() {
        // The window where the two cuts disagree.
        for x in [25.9f32, 26.0, 26.05] {
            let kept = airy_bi_at_cut(x, 708.3964);
            let retargeted = airy_bi_at_cut(x, 87.72);
            let truth = f64_airy::airy_bi(x as f64);
            assert!(
                kept.is_finite(),
                "Bi({x}) with GSL's threshold is documented as finite, and gave {kept}"
            );
            assert!(
                retargeted.is_infinite(),
                "Bi({x}) with the RETARGETED threshold is documented as \
                 wrongly infinite -- the whole point of this test. It gave \
                 {retargeted}, so the two cuts now agree and the module docs \
                 need re-deriving"
            );
            assert!(
                ((kept as f64 - truth) / truth).abs() < 1e-5,
                "Bi({x}) = {kept:e} vs f64 {truth:e}"
            );
            assert!(
                (truth as f32).is_finite(),
                "Bi({x}) = {truth:e} is documented as representable in f32"
            );
        }
        // Below the window both agree; above it both overflow.
        assert!(airy_bi_at_cut(25.8, 708.3964).is_finite());
        assert!(airy_bi_at_cut(25.8, 87.72).is_finite());
        assert!(airy_bi(26.5).is_infinite());
        assert!(airy_bi(1e6).is_infinite());
        // The scaled form never overflows, which is why it is the one to use.
        for x in [26.0f32, 1e6, 1e30] {
            assert!(airy_bi_scaled(x).is_finite(), "Bi_scaled({x:e})");
        }
    }

    /// The scalings are the documented ones, and their asymmetry is
    /// deliberate: `Ai`'s exponent is positive, `Bi`'s negative, and on the
    /// negative axis neither is scaled.
    #[test]
    fn the_scalings_are_the_documented_ones_and_are_not_symmetric() {
        for k in 1..=50 {
            let x = 0.1 * k as f32;
            let e = 2.0 / 3.0 * x * x.sqrt();
            let (ai, bi) = (airy_ai(x), airy_bi(x));
            assert!(
                ((airy_ai_scaled(x) - ai * e.exp()) / (ai * e.exp())).abs() < 1e-5,
                "Ai_scaled at {x}"
            );
            assert!(
                ((airy_bi_scaled(x) - bi * (-e).exp()) / (bi * (-e).exp())).abs() < 1e-5,
                "Bi_scaled at {x}"
            );
        }
        for k in 1..=120 {
            let x = -0.1 * k as f32;
            assert_eq!(airy_ai_scaled(x), airy_ai(x), "Ai_scaled at {x}");
            assert_eq!(airy_bi_scaled(x), airy_bi(x), "Bi_scaled at {x}");
        }
    }

    /// The values at the origin, the shape, the first zeros, and the
    /// refusals — in `f32`.
    #[test]
    fn the_shape_and_the_refusals_survive_f32() {
        assert!((airy_ai(0.0) - 0.355_028_05).abs() < 1e-6, "Ai(0)");
        assert!((airy_bi(0.0) - 0.614_926_6).abs() < 1e-6, "Bi(0)");
        let mut prev = airy_ai(0.0);
        for k in 1..=200 {
            let x = 0.05 * k as f32;
            let v = airy_ai(x);
            assert!(v > 0.0 && v < prev, "Ai not decreasing at {x}");
            prev = v;
        }
        assert!(
            airy_ai(-2.33) > 0.0 && airy_ai(-2.35) < 0.0,
            "Ai's first zero"
        );
        assert!(
            airy_bi(-1.17) > 0.0 && airy_bi(-1.18) < 0.0,
            "Bi's first zero"
        );
        for f in [
            &airy_ai as &dyn Fn(f32) -> f32,
            &airy_bi,
            &airy_ai_scaled,
            &airy_bi_scaled,
        ] {
            assert!(f(f32::NAN).is_nan());
        }
        // Ai underflows rather than overflowing.
        assert_eq!(airy_ai(1e6), 0.0);
        assert!(airy_ai_scaled(1e6).is_finite() && airy_ai_scaled(1e6) > 0.0);
    }
}
