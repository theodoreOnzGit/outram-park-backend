//! `XQ`/`XXXX` matrix assembly from the inverted level matrix — ported
//! from `setxqx` (`samm.f90:6231-6284`).
//!
//! `XQ = Y^-1 * R` and `XXXX = sqrt(P)/L * XQ * sqrt(P)`, the quantity
//! [`super::sectio::sectio`] actually needs (upstream's own comment:
//! the scattering matrix element is `U = I + 2i*XXXX`).

use crate::samm::linpack::PackedComplexMatrix;

#[inline]
fn packed(i: i64, j: i64) -> i64 {
    if i <= j {
        i + j * (j - 1) / 2
    } else {
        j + i * (i - 1) / 2
    }
}

/// `XXXX`, the packed complex-symmetric matrix [`super::sectio::sectio`]
/// consumes.
pub struct XqxOutput {
    pub xxxxr: Vec<f64>,
    pub xxxxi: Vec<f64>,
}

/// Compute `XQ = Y^-1 * R` and `XXXX = sqrt(P)/L * XQ * sqrt(P)` — ported
/// from `setxqx` (`samm.f90:6231-6284`). `rootp`/`elinvr`/`elinvi` are
/// [`super::setr::setr`]'s per-channel outputs of the same name; `yinv` is
/// [`crate::samm::rmatrix_invert::invert`]'s output for `rmat`.
pub fn setxqx(nchan: usize, rmat: &PackedComplexMatrix, yinv: &PackedComplexMatrix, rootp: &[f64], elinvr: &[f64], elinvi: &[f64]) -> XqxOutput {
    let n = nchan as i64;

    // samm.f90:6258-6270 -- Xq(k,i) = Y^-1 * R (note the asymmetry: Xq is
    // not itself symmetric even though Y^-1 and R both are).
    let mut xqr = vec![0.0_f64; nchan * nchan];
    let mut xqi = vec![0.0_f64; nchan * nchan];
    let idx = |k: usize, i: usize| k * nchan + i; // 0-indexed [k][i] flattened

    for i in 1..=n {
        for j in 1..=n {
            let ij = packed(j, i);
            for k in 1..=n {
                let jk = packed(k, j);
                let yr = yinv.re[(ij - 1) as usize];
                let yi = yinv.im[(ij - 1) as usize];
                let rr = rmat.re[(jk - 1) as usize];
                let ri = rmat.im[(jk - 1) as usize];
                let e = idx((k - 1) as usize, (i - 1) as usize);
                xqr[e] += yr * rr - yi * ri;
                xqi[e] += yr * ri + yi * rr;
            }
        }
    }

    // samm.f90:6272-6282 -- Xxxx = sqrt(P)/L * Xq * sqrt(P) (symmetric).
    let mut xxxxr = vec![0.0_f64; (nchan * (nchan + 1)) / 2];
    let mut xxxxi = vec![0.0_f64; (nchan * (nchan + 1)) / 2];
    let mut ij = 0_usize;
    for i in 1..=n {
        let plr = rootp[(i - 1) as usize] * elinvr[(i - 1) as usize];
        let pli = rootp[(i - 1) as usize] * elinvi[(i - 1) as usize];
        for j in 1..=i {
            let xqr_ji = xqr[idx((j - 1) as usize, (i - 1) as usize)];
            let xqi_ji = xqi[idx((j - 1) as usize, (i - 1) as usize)];
            xxxxr[ij] = rootp[(j - 1) as usize] * (xqr_ji * plr - xqi_ji * pli);
            xxxxi[ij] = rootp[(j - 1) as usize] * (xqi_ji * plr + xqr_ji * pli);
            ij += 1;
        }
    }

    XqxOutput { xxxxr, xxxxi }
}
