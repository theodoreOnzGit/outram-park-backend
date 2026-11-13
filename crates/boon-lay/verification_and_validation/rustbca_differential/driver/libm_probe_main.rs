// Attribution probe (gh:#1426): per-operation mismatch rate of the two libm routes
// the RustBCA copy could use against this platform's libm (glibc, via std), over
// the argument ranges RustBCA's kernel uses. "libm" = the `libm` crate (musl
// port), which is what `petir::real::Real::exp(x)` etc. call. "arm" = PETIR's
// free functions `petir::real::{exp, ln, powf}` (ARM optimized-routines, the
// algorithm glibc >= 2.28 uses for exp/log/pow). Standalone scratch crate, see
// probe_Cargo.toml. Prints one line per operation.
use std::f64::consts::PI;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

fn ulps(a: f64, b: f64) -> u64 {
    if a == b {
        return 0;
    }
    (a.to_bits() as i64).abs_diff(b.to_bits() as i64)
}

fn report(name: &str, n: usize, xs: &[(f64, f64)], f_std: &dyn Fn(f64, f64) -> f64, f_lib: &dyn Fn(f64, f64) -> f64, f_arm: Option<&dyn Fn(f64, f64) -> f64>) {
    let mut m_lib = 0usize;
    let mut max_lib = 0u64;
    let mut m_arm = 0usize;
    let mut max_arm = 0u64;
    for &(x, y) in xs {
        let s = f_std(x, y);
        let u = ulps(s, f_lib(x, y));
        if u > 0 {
            m_lib += 1;
            max_lib = max_lib.max(u);
        }
        if let Some(fa) = f_arm {
            let u = ulps(s, fa(x, y));
            if u > 0 {
                m_arm += 1;
                max_arm = max_arm.max(u);
            }
        }
    }
    let arm = if f_arm.is_some() { format!("arm mismatch {:.4}% max {} ulp", 100.0 * m_arm as f64 / n as f64, max_arm) } else { "arm n/a".to_string() };
    println!("{name:10} n {n}: libm mismatch {:.4}% max {} ulp; {arm}", 100.0 * m_lib as f64 / n as f64, max_lib);
}

fn main() {
    let n = 2_000_000usize;
    let mut r = Lcg(12345);
    let uni: Vec<(f64, f64)> = (0..n).map(|_| (r.next(), r.next())).collect();
    let neg_exp: Vec<(f64, f64)> = uni.iter().map(|&(a, _)| (-40.0 * a, 0.0)).collect();
    let pos_exp: Vec<(f64, f64)> = uni.iter().map(|&(a, _)| (-(a.max(1e-300)).ln(), 0.0)).collect();
    let lnin: Vec<(f64, f64)> = uni.iter().map(|&(a, _)| (a.max(1e-300), 0.0)).collect();
    let wide_ln: Vec<(f64, f64)> = uni.iter().map(|&(a, _)| (1.0e-6 + 1.0e6 * a, 0.0)).collect();
    let ang: Vec<(f64, f64)> = uni.iter().map(|&(a, _)| (2.0 * PI * a, 0.0)).collect();
    let small_ang: Vec<(f64, f64)> = uni.iter().map(|&(a, _)| (PI * a, 0.0)).collect();
    let atan: Vec<(f64, f64)> = uni.iter().map(|&(a, b)| (a * 2.0 - 1.0, b * 2.0 - 1.0 + 1.5)).collect();
    let acosin: Vec<(f64, f64)> = uni.iter().map(|&(a, _)| (2.0 * a - 1.0, 0.0)).collect();
    let cbrtin: Vec<(f64, f64)> = uni.iter().map(|&(a, _)| (1.0e30 * a + 1.0e20, 0.0)).collect();
    let powf_third: Vec<(f64, f64)> = uni.iter().map(|&(a, _)| (1.0e30 * a + 1.0e20, -1.0 / 3.0)).collect();
    let powf_gen: Vec<(f64, f64)> = uni.iter().map(|&(a, b)| (10.0 * a + 1e-3, 4.0 * b - 2.0)).collect();
    report("exp(-x)", n, &neg_exp, &|x, _| x.exp(), &|x, _| libm::exp(x), Some(&|x, _| petir::real::exp(x)));
    report("exp(-ln u)", n, &pos_exp, &|x, _| x.exp(), &|x, _| libm::exp(x), Some(&|x, _| petir::real::exp(x)));
    report("ln u", n, &lnin, &|x, _| x.ln(), &|x, _| libm::log(x), Some(&|x, _| petir::real::ln(x)));
    report("ln wide", n, &wide_ln, &|x, _| x.ln(), &|x, _| libm::log(x), Some(&|x, _| petir::real::ln(x)));
    report("sin", n, &ang, &|x, _| x.sin(), &|x, _| libm::sin(x), None);
    report("cos", n, &ang, &|x, _| x.cos(), &|x, _| libm::cos(x), None);
    report("sin_cos.s", n, &small_ang, &|x, _| x.sin_cos().0, &|x, _| libm::sincos(x).0, None);
    report("sin_cos.c", n, &small_ang, &|x, _| x.sin_cos().1, &|x, _| libm::sincos(x).1, None);
    report("atan2", n, &atan, &|y, x| y.atan2(x), &|y, x| libm::atan2(y, x), None);
    report("acos", n, &acosin, &|x, _| x.acos(), &|x, _| libm::acos(x), None);
    report("cbrt", n, &cbrtin, &|x, _| x.cbrt(), &|x, _| libm::cbrt(x), None);
    report("powf -1/3", n, &powf_third, &|x, y| x.powf(y), &|x, y| libm::pow(x, y), Some(&|x, y| petir::real::powf(x, y)));
    report("powf gen", n, &powf_gen, &|x, y| x.powf(y), &|x, y| libm::pow(x, y), Some(&|x, y| petir::real::powf(x, y)));
}
