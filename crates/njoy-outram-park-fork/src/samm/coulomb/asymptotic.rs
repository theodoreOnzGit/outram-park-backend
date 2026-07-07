//! Moderate-`rho` asymptotic-expansion family — `xsigll`, `asymp1`,
//! `asymp2`, `taylor`, `end1`, `getfg`, ported from `samm.f90:4401-4850`
//! (plus `xsigll` at `samm.f90:4429-4517`). Used by [`super::dispatch::coulx`]
//! when `eta` isn't large enough for [`super::dispatch::bigeta`]'s Bessel-
//! function approach.

use crate::common::phys::{EULER, PI};

/// Coulomb phase shift `sigma_L = arg(Gamma(L+1+i*eta))` for `L=0..=lmax`
/// — ported from `xsigll` (`samm.f90:4429-4517`), needed by [`asymp2`]'s
/// asymptotic expansion of `G_0`. Returns `sigma[0..=lmax]` (`sigma[L]` =
/// upstream's `sigma(L+1)`).
pub fn xsigll(eta: f64, lmax: i32) -> Vec<f64> {
    const SMALL: f64 = 0.000_001;
    const BER: [f64; 5] = [
        0.166_666_666_666_666_666_666_666_666_666_666_667,
        -0.033_333_333_333_333_333_333_333_333_333_333_333,
        0.023_809_523_809_523_809_523_809_523_809_523_810,
        -0.033_333_333_333_333_333_333_333_333_333_333_333,
        0.075_757_575_757_575_757_575_757_575_757_575_758,
    ];
    const MMMXXX: i64 = 100_000;

    let peta = eta.abs();
    let mut sigma0;
    if peta >= 3.0 {
        let mut sum = 0.0_f64;
        for i in 1..=5 {
            let xi = i as f64;
            let m = 2 * i - 1;
            let xm = m as f64;
            sum += BER[i - 1] / (2.0 * xi * xm * peta.powi(m as i32));
        }
        sigma0 = PI / 4.0 + peta * (peta.ln() - 1.0) - sum;
    } else {
        let mut sumas = 0.0_f64;
        for is in 1..=MMMXXX {
            let s = is as f64;
            let temp1 = peta / s;
            let as_;
            if s <= 2.0 * peta {
                as_ = temp1 - temp1.atan();
            } else {
                let mut acc = 0.0_f64;
                let mut k = 0;
                for j in 1..=MMMXXX {
                    let m = j + j + 1;
                    let xm = m as f64;
                    let add = temp1.powi(m as i32) / xm;
                    if k == 0 {
                        acc += add;
                        k = 1;
                    } else {
                        acc -= add;
                        k = 0;
                    }
                    if (add / acc).abs() <= SMALL {
                        break;
                    }
                }
                as_ = acc;
            }
            sumas += as_;
            if (as_ / sumas).abs() <= SMALL {
                break;
            }
        }
        sigma0 = -EULER * peta + sumas;
    }
    if eta < 0.0 {
        sigma0 = -sigma0;
    }

    let mut sigma = vec![0.0_f64; (lmax.max(0) as usize) + 1];
    sigma[0] = sigma0;
    for ll in 1..=lmax {
        let xl = ll as f64;
        sigma[ll as usize] = sigma[(ll - 1) as usize] + (eta / xl).atan();
    }
    sigma
}

/// Asymptotic expansion of `G_0`, `G_0'` at `rhoi=2*eta`, valid for large
/// `eta` — ported from `asymp1` (`samm.f90:4401-4427`, Abramowitz & Stegun
/// Eqs. 14.5.12b/14.5.13b).
pub fn asymp1(eta: f64) -> (f64, f64, f64) {
    let ceta = eta.powf(1.0 / 3.0);
    let seta = ceta.sqrt();
    let temp = 1.0 / (ceta * ceta);
    let u = 1.223_404_016 * seta
        * (1.0
            + temp.powi(2)
                * (0.049_595_701_65
                    + temp
                        * (-0.008_888_888_889
                            + temp.powi(2) * (0.002_455_199_181 + temp * (-0.000_910_895_806_1 + temp.powi(2) * 0.000_253_468_411_5)))));
    let upr = -0.707_881_773_4
        * (1.0
            + temp
                * (-0.172_826_036_9
                    + temp.powi(2)
                        * (0.000_317_460_317_4 + temp * (-0.003_581_214_850 + temp.powi(2) * (0.000_311_782_468_0 - temp * 0.000_907_396_642_7)))))
        / seta;
    let rhoi = 2.0 * eta;
    (u, upr, rhoi)
}

/// Asymptotic expansion of `G_0`, `G_0'` at large `rhoi` for finite `eta`
/// — ported from `asymp2` (`samm.f90:4519-4601`, Abramowitz & Stegun Eqs.
/// 14.5.1-8), doubling `rhoi` and retrying until the continued expansion
/// converges.
pub fn asymp2(eta: f64, rho: f64, sigma0: f64) -> (f64, f64, f64) {
    const EPSLON: f64 = 0.000_001;
    const DEL: f64 = 100.0;

    let mut rhoi = (rho * 2.0).max(10.0).max(10.0 * eta);

    'retry: loop {
        let mut xn = 0.0_f64;
        let mut zold = [1.0_f64, 0.0, 0.0, 1.0 - eta / rhoi];
        let mut z = zold;
        let mut bigz = [z[0].abs(), z[1].abs(), z[2].abs(), z[3].abs()];

        let mut jcheck = 0;
        for _n in 1..=100 {
            let temp = 2.0 * (xn + 1.0) * rhoi;
            let an = (2.0 * xn + 1.0) * eta / temp;
            let bn = (eta * eta - xn * (xn + 1.0)) / temp;
            xn += 1.0;
            let znew = [
                an * zold[0] - bn * zold[1],
                an * zold[1] + bn * zold[0],
                0.0, // filled below (depends on znew[0])
                0.0, // filled below (depends on znew[1])
            ];
            let znew2 = an * zold[2] - bn * zold[3] - znew[0] / rhoi;
            let znew3 = an * zold[3] + bn * zold[2] - znew[1] / rhoi;
            let znew = [znew[0], znew[1], znew2, znew3];

            let mut icheck = 0;
            for i in 0..4 {
                z[i] += znew[i];
                zold[i] = znew[i];
                let temp2 = z[i].abs();
                bigz[i] = bigz[i].max(z[i].abs());
                if bigz[i] / temp2 > DEL {
                    rhoi *= 2.0;
                    continue 'retry;
                }
                if (znew[i] / z[i]).abs() <= EPSLON {
                    icheck += 1;
                }
            }
            let w = z[0] * z[3] - z[1] * z[2];
            if w.abs() > 10.0 {
                rhoi *= 2.0;
                continue 'retry;
            }
            if icheck == 4 {
                jcheck += 1;
                if jcheck >= 4 {
                    let phi = rhoi - eta * (2.0 * rhoi).ln() + sigma0;
                    let cosphi = phi.cos();
                    let sinphi = phi.sin();
                    let g0 = z[0] * cosphi - z[1] * sinphi;
                    let g0pr = z[2] * cosphi - z[3] * sinphi;
                    return (g0, g0pr, rhoi);
                }
            } else {
                jcheck = 0;
            }
        }
        // Fell through the `n=1..100` loop without converging: same
        // "double rhoi and retry" fallback as the `bigz`/`w` early exits.
        rhoi *= 2.0;
    }
}

/// Taylor-series integration of `u=G_0`, `u'=G_0'` from `arg=rhoi` (where
/// they're already known) to `arg=rho` — ported from `taylor`
/// (`samm.f90:4603-4735`), solving `u'' + (1-2*eta/rho)*u = 0` via a
/// power-series expansion in `delta=rho-rhoi`, halving `delta` and
/// stepping partway when the full-`delta` series doesn't converge.
pub fn taylor(eta: f64, rho: f64, u_in: f64, upr_in: f64, rhoi_in: f64) -> (f64, f64, f64) {
    const EPSLON: f64 = 1.0e-6;
    const BIGGER: f64 = 1.0e10;
    const BIGGST: f64 = 1.0e30;
    const DEL: f64 = 100.0;

    let mut u = u_in;
    let mut upr = upr_in;
    let mut rhoi = rhoi_in;
    let mut delta = rho - rhoi;
    if delta == 0.0 {
        return (u, upr, rhoi);
    }

    let mut a = vec![0.0_f64; 101]; // a[1..=100] used (Fortran 1-indexed); a[0] unused.

    'outer: loop {
        a[1] = u;
        a[2] = delta * upr;
        a[3] = -delta * delta / 2.0 * (1.0 - 2.0 * eta / rhoi) * a[1];
        let mut nstart = 4_i64;

        loop {
            let mut jcheck = 0;
            let mut sum = 0.0_f64;
            let mut sumpr = 0.0_f64;
            let mut big = 0.0_f64;
            let mut bigpr = 0.0_f64;
            let mut n_final = 100_i64;
            let mut converged = false;
            let mut failed_at = 0_i64;

            for n in 1..=100_i64 {
                let xn = (n - 1) as f64;
                if n >= nstart {
                    let a_n = -(delta * (xn - 1.0) * (xn - 2.0) * a[(n - 1) as usize]
                        + (rhoi - 2.0 * eta) * delta.powi(2) * a[(n - 2) as usize]
                        + delta.powi(3) * a[(n - 3) as usize])
                        / (rhoi * (xn - 1.0) * xn);
                    a[n as usize] = a_n;
                    if a_n > BIGGER {
                        failed_at = n;
                        break;
                    }
                }
                sum += a[n as usize];
                sumpr += xn * a[n as usize];
                if sum >= BIGGST || sumpr >= BIGGST {
                    failed_at = n;
                    break;
                }
                big = big.max(sum.abs());
                bigpr = bigpr.max(sumpr.abs());
                if sum == 0.0 || sumpr == 0.0 {
                    jcheck = 0;
                } else {
                    if (big / sum).abs() >= DEL || (bigpr / sumpr).abs() >= DEL {
                        failed_at = n;
                        break;
                    }
                    if (a[n as usize] / sum).abs() >= EPSLON || (xn * a[n as usize] / sumpr).abs() >= EPSLON {
                        jcheck = 0;
                    } else {
                        jcheck += 1;
                        if jcheck >= 4 {
                            converged = true;
                            n_final = n;
                            break;
                        }
                    }
                }
                n_final = n;
            }

            if converged {
                // samm.f90:4725-4732 ("60 continue")
                u = sum;
                upr = sumpr / delta;
                rhoi += delta;
                delta = rho - rhoi;
                if delta.abs() >= EPSLON {
                    continue 'outer;
                }
                return (u, upr, rhoi);
            }

            // samm.f90:4710-4723 ("40 continue") -- series didn't converge;
            // halve delta (find u(rhoi+delta/2) instead) and retry from the
            // current `nstart`/`a` state.
            let n = if failed_at != 0 { failed_at } else { n_final };
            nstart = nstart.max(n + 1);
            let m = nstart - 1;
            delta /= 2.0;
            let mut temp = 2.0_f64;
            for k in 1..=m {
                temp /= 2.0;
                a[k as usize] *= temp;
            }
        }
    }
}

/// Degenerate `L=0`-only result (`llmax` forced to `0`) — ported from
/// `end1` (`samm.f90:4737-4751`), the `abs(g0)>1e25` fallback in
/// [`super::dispatch::coulx`] where `getfg`'s recursion would overflow.
pub fn end1(g0: f64, g0pr: f64) -> (i32, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    (0, vec![0.0], vec![0.0], vec![g0], vec![g0pr])
}

/// `F_L`, `G_L` (and derivatives) for `L=0..=llmax`, given `G_0`, `G_0'`
/// — ported from `getfg` (`samm.f90:4753-4850`). Returns the (possibly
/// reduced, see below) `llmax` actually filled, plus `f,fpr,g,gpr` sized to
/// it.
///
/// The forward recursion for `G_L` (Abramowitz & Stegun Eq. 14.2.3) can
/// overflow for large `L`; if `|G_L|` exceeds `1e12` past `L=lll`, upstream
/// stops early and reports the reduced `llmax` back to the caller
/// (`samm.f90:4780-4783`, `if (limit.le.lmax) llmax=limit-1`) — this is why
/// the return type carries `llmax` rather than assuming the input value.
pub fn getfg(eta: f64, rho: f64, lmax: i32, lll: i32, g0: f64, g0pr: f64) -> (i32, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    const BIG: f64 = 1.0e12;

    let limit0 = 3.max(lmax + 1);
    let mut g = vec![0.0_f64; (limit0 as usize) + 1];
    let mut gpr = vec![0.0_f64; (limit0 as usize) + 1];
    g[0] = g0;
    gpr[0] = g0pr;
    g[1] = ((eta + 1.0 / rho) * g[0] - gpr[0]) / (eta * eta + 1.0).sqrt();

    let mut limit = limit0;
    for l in 3..=limit0 {
        let xl = (l - 1) as f64;
        let temp1 = (xl * xl + eta * eta).sqrt();
        let gl = (2.0 * xl - 1.0) / temp1 * (eta / (xl - 1.0) + xl / rho) * g[(l - 2) as usize]
            - xl / temp1 * (1.0 + (eta / (xl - 1.0)).powi(2)).sqrt() * g[(l - 3) as usize];
        g[(l - 1) as usize] = gl;
        if gl.abs() > BIG && l > lll {
            limit = l;
            break;
        }
    }

    // samm.f90:4786-4803 -- find J such that G(J) is well-converged
    // relative to G(limit) three checks in a row.
    let mut gm2 = g[(limit - 2) as usize];
    let mut gm1 = g[(limit - 1) as usize];
    let mut il: i32 = -1;
    let mut j: i64 = limit as i64;
    let mut gm = gm1;
    for jj in (limit as i64)..=10_000 {
        j = jj;
        let xl = jj as f64;
        let temp1 = (xl * xl + eta * eta).sqrt();
        gm = (2.0 * xl - 1.0) / temp1 * (eta / (xl - 1.0) + xl / rho) * gm1
            - xl / temp1 * (1.0 + (eta / (xl - 1.0)).powi(2)).sqrt() * gm2;
        if (g[(limit - 1) as usize] / gm).abs() > 1.0e-4 {
            il = -2;
        }
        if il > 0 {
            break;
        }
        il += 1;
        gm2 = gm1;
        gm1 = gm;
    }

    // samm.f90:4805-4822 -- approximate F(limit+3..j-1) in reverse, not stored.
    let xl_j = j as f64;
    let mut fp1 = xl_j / gm / (xl_j * xl_j + eta * eta).sqrt();
    let mut fp2 = 0.0_f64;
    let mut l = j - 1;
    let n1 = (j - 3 - limit as i64).max(0);
    for _ in 0..n1 {
        l -= 1;
        let xl = l as f64;
        let temp2 = ((xl + 1.0).powi(2) + eta * eta).sqrt();
        let fp = ((2.0 * xl + 3.0) * (eta / (xl + 2.0) + (xl + 1.0) / rho) * fp1
            - (xl + 1.0) * (1.0 + (eta / (xl + 2.0)).powi(2)).sqrt() * fp2)
            / temp2;
        fp2 = fp1;
        fp1 = fp;
    }

    // samm.f90:4824-4835 -- F(1..limit+1), reverse recursion, stored this time.
    let f_size = (limit + 2) as usize;
    let mut f = vec![0.0_f64; f_size];
    for _ in 0..(limit + 2) {
        l -= 1;
        let xl = l as f64;
        let temp2 = ((xl + 1.0).powi(2) + eta * eta).sqrt();
        let fp = ((2.0 * xl + 3.0) * (eta / (xl + 2.0) + (xl + 1.0) / rho) * fp1
            - (xl + 1.0) * (1.0 + (eta / (xl + 2.0)).powi(2)).sqrt() * fp2)
            / temp2;
        f[l as usize] = fp;
        fp2 = fp1;
        fp1 = fp;
    }

    // samm.f90:4837-4845 -- Fpr, Gpr for L=1..limit-1 (Fpr(1) handled separately).
    let mut fpr = vec![0.0_f64; f_size];
    fpr[0] = (1.0 / rho + eta) * f[0] - (1.0 + eta * eta).sqrt() * f[1];
    for l in 2..=limit {
        let xl = l as f64;
        fpr[(l - 1) as usize] = (xl / rho + eta / xl) * f[(l - 1) as usize] - (1.0 + (eta / xl).powi(2)).sqrt() * f[l as usize];
        let temp1 = eta / (xl - 1.0);
        gpr[(l - 1) as usize] = (1.0 + temp1 * temp1).sqrt() * g[(l - 2) as usize] - ((xl - 1.0) / rho + temp1) * g[(l - 1) as usize];
    }

    let llmax_out = if limit <= lmax { limit - 1 } else { lmax };
    (llmax_out, f, fpr, g, gpr)
}
