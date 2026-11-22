// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.
//
// PORTED (by script, reference-data/gsl/gen_bicubic_rs.py) from the GNU
// Scientific Library (GSL) 2.8, commit cf180cd7fbd06039a577f9c9ff0b428784765ac1,
// interpolation/bicubic.c. Copyright 2012 David Zaslavsky.
// GPL-3.0-or-later (verified from the file header; see NOTICE).
//
// GENERATED: do not edit by hand; rerun the script. Only integer literals
// became floats and `*z += e` became `z += e`; every coefficient, operation
// order and grouping is the C file's.

//! The bicubic polynomial sums of GSL's `gsl_interp2d_bicubic`, generated from
//! `bicubic.c`. See [`crate::interp2d`] for the type that uses them.

#![allow(
    clippy::all,
    clippy::pedantic,
    unused_variables,
    non_snake_case,
    clippy::too_many_lines
)]

use crate::interp2d::BicubicCell;

/// `bicubic_eval` (bicubic.c lines 251-284): the polynomial sum, copied by script.
pub(crate) fn eval(c: &BicubicCell) -> f64 {
    let BicubicCell { t0, t1, t2, t3, u0, u1, u2, u3, dt, du,
        zminmin, zminmax, zmaxmin, zmaxmax, zxminmin, zxminmax, zxmaxmin, zxmaxmax,
        zyminmin, zyminmax, zymaxmin, zymaxmax, zxyminmin, zxyminmax, zxymaxmin, zxymaxmax } = *c;
    let mut v;
    let mut z = 0.0_f64;
    v = zminmin;
    z += v*t0*u0;
    v = zyminmin;
    z += v*t0*u1;
    v = -3.0*zminmin + 3.0*zminmax - 2.0*zyminmin - zyminmax;
    z += v*t0*u2;
    v = 2.0*zminmin - 2.0*zminmax + zyminmin + zyminmax;
    z += v*t0*u3;
    v = zxminmin;
    z += v*t1*u0;
    v = zxyminmin;
    z += v*t1*u1;
    v = -3.0*zxminmin + 3.0*zxminmax - 2.0*zxyminmin - zxyminmax;
    z += v*t1*u2;
    v = 2.0*zxminmin - 2.0*zxminmax + zxyminmin + zxyminmax;
    z += v*t1*u3;
    v = -3.0*zminmin + 3.0*zmaxmin - 2.0*zxminmin - zxmaxmin;
    z += v*t2*u0;
    v = -3.0*zyminmin + 3.0*zymaxmin - 2.0*zxyminmin - zxymaxmin;
    z += v*t2*u1;
    v = 9.0*zminmin - 9.0*zmaxmin + 9.0*zmaxmax - 9.0*zminmax + 6.0*zxminmin + 3.0*zxmaxmin - 3.0*zxmaxmax - 6.0*zxminmax + 6.0*zyminmin - 6.0*zymaxmin - 3.0*zymaxmax + 3.0*zyminmax + 4.0*zxyminmin + 2.0*zxymaxmin + zxymaxmax + 2.0*zxyminmax;
    z += v*t2*u2;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 4.0*zxminmin - 2.0*zxmaxmin + 2.0*zxmaxmax + 4.0*zxminmax - 3.0*zyminmin + 3.0*zymaxmin + 3.0*zymaxmax - 3.0*zyminmax - 2.0*zxyminmin - zxymaxmin - zxymaxmax - 2.0*zxyminmax;
    z += v*t2*u3;
    v = 2.0*zminmin - 2.0*zmaxmin + zxminmin + zxmaxmin;
    z += v*t3*u0;
    v = 2.0*zyminmin - 2.0*zymaxmin + zxyminmin + zxymaxmin;
    z += v*t3*u1;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 3.0*zxminmin - 3.0*zxmaxmin + 3.0*zxmaxmax + 3.0*zxminmax - 4.0*zyminmin + 4.0*zymaxmin + 2.0*zymaxmax - 2.0*zyminmax - 2.0*zxyminmin - 2.0*zxymaxmin - zxymaxmax - zxyminmax;
    z += v*t3*u2;
    v = 4.0*zminmin - 4.0*zmaxmin + 4.0*zmaxmax - 4.0*zminmax + 2.0*zxminmin + 2.0*zxmaxmin - 2.0*zxmaxmax - 2.0*zxminmax + 2.0*zyminmin - 2.0*zymaxmin - 2.0*zymaxmax + 2.0*zyminmax + zxyminmin + zxymaxmin + zxymaxmax + zxyminmax;
    z += v*t3*u3;
    z
}

/// `bicubic_deriv_x` (bicubic.c lines 363-389): the polynomial sum, copied by script.
pub(crate) fn deriv_x(c: &BicubicCell) -> f64 {
    let BicubicCell { t0, t1, t2, t3, u0, u1, u2, u3, dt, du,
        zminmin, zminmax, zmaxmin, zmaxmax, zxminmin, zxminmax, zxmaxmin, zxmaxmax,
        zyminmin, zyminmax, zymaxmin, zymaxmax, zxyminmin, zxyminmax, zxymaxmin, zxymaxmax } = *c;
    let mut v;
    let mut z = 0.0_f64;
    v = zxminmin;
    z += v*t0*u0;
    v = zxyminmin;
    z += v*t0*u1;
    v = -3.0*zxminmin + 3.0*zxminmax - 2.0*zxyminmin - zxyminmax;
    z += v*t0*u2;
    v = 2.0*zxminmin - 2.0*zxminmax + zxyminmin + zxyminmax;
    z += v*t0*u3;
    v = -3.0*zminmin + 3.0*zmaxmin - 2.0*zxminmin - zxmaxmin;
    z += 2.0*v*t1*u0;
    v = -3.0*zyminmin + 3.0*zymaxmin - 2.0*zxyminmin - zxymaxmin;
    z += 2.0*v*t1*u1;
    v = 9.0*zminmin - 9.0*zmaxmin + 9.0*zmaxmax - 9.0*zminmax + 6.0*zxminmin + 3.0*zxmaxmin - 3.0*zxmaxmax - 6.0*zxminmax + 6.0*zyminmin - 6.0*zymaxmin - 3.0*zymaxmax + 3.0*zyminmax + 4.0*zxyminmin + 2.0*zxymaxmin + zxymaxmax + 2.0*zxyminmax;
    z += 2.0*v*t1*u2;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 4.0*zxminmin - 2.0*zxmaxmin + 2.0*zxmaxmax + 4.0*zxminmax - 3.0*zyminmin + 3.0*zymaxmin + 3.0*zymaxmax - 3.0*zyminmax - 2.0*zxyminmin - zxymaxmin - zxymaxmax - 2.0*zxyminmax;
    z += 2.0*v*t1*u3;
    v = 2.0*zminmin - 2.0*zmaxmin + zxminmin + zxmaxmin;
    z += 3.0*v*t2*u0;
    v = 2.0*zyminmin - 2.0*zymaxmin + zxyminmin + zxymaxmin;
    z += 3.0*v*t2*u1;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 3.0*zxminmin - 3.0*zxmaxmin + 3.0*zxmaxmax + 3.0*zxminmax - 4.0*zyminmin + 4.0*zymaxmin + 2.0*zymaxmax - 2.0*zyminmax - 2.0*zxyminmin - 2.0*zxymaxmin - zxymaxmax - zxyminmax;
    z += 3.0*v*t2*u2;
    v = 4.0*zminmin - 4.0*zmaxmin + 4.0*zmaxmax - 4.0*zminmax + 2.0*zxminmin + 2.0*zxmaxmin - 2.0*zxmaxmax - 2.0*zxminmax + 2.0*zyminmin - 2.0*zymaxmin - 2.0*zymaxmax + 2.0*zyminmax + zxyminmin + zxymaxmin + zxymaxmax + zxyminmax;
    z += 3.0*v*t2*u3;
    z *= dt;
    z
}

/// `bicubic_deriv_y` (bicubic.c lines 466-492): the polynomial sum, copied by script.
pub(crate) fn deriv_y(c: &BicubicCell) -> f64 {
    let BicubicCell { t0, t1, t2, t3, u0, u1, u2, u3, dt, du,
        zminmin, zminmax, zmaxmin, zmaxmax, zxminmin, zxminmax, zxmaxmin, zxmaxmax,
        zyminmin, zyminmax, zymaxmin, zymaxmax, zxyminmin, zxyminmax, zxymaxmin, zxymaxmax } = *c;
    let mut v;
    let mut z = 0.0_f64;
    v = zyminmin;
    z += v*t0*u0;
    v = -3.0*zminmin + 3.0*zminmax - 2.0*zyminmin - zyminmax;
    z += 2.0*v*t0*u1;
    v = 2.0*zminmin-2.0*zminmax + zyminmin + zyminmax;
    z += 3.0*v*t0*u2;
    v = zxyminmin;
    z += v*t1*u0;
    v = -3.0*zxminmin + 3.0*zxminmax - 2.0*zxyminmin - zxyminmax;
    z += 2.0*v*t1*u1;
    v = 2.0*zxminmin - 2.0*zxminmax + zxyminmin + zxyminmax;
    z += 3.0*v*t1*u2;
    v = -3.0*zyminmin + 3.0*zymaxmin - 2.0*zxyminmin - zxymaxmin;
    z += v*t2*u0;
    v = 9.0*zminmin - 9.0*zmaxmin + 9.0*zmaxmax - 9.0*zminmax + 6.0*zxminmin + 3.0*zxmaxmin - 3.0*zxmaxmax - 6.0*zxminmax + 6.0*zyminmin - 6.0*zymaxmin - 3.0*zymaxmax + 3.0*zyminmax + 4.0*zxyminmin + 2.0*zxymaxmin + zxymaxmax + 2.0*zxyminmax;
    z += 2.0*v*t2*u1;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 4.0*zxminmin - 2.0*zxmaxmin + 2.0*zxmaxmax + 4.0*zxminmax - 3.0*zyminmin + 3.0*zymaxmin + 3.0*zymaxmax - 3.0*zyminmax - 2.0*zxyminmin - zxymaxmin - zxymaxmax - 2.0*zxyminmax;
    z += 3.0*v*t2*u2;
    v = 2.0*zyminmin - 2.0*zymaxmin + zxyminmin + zxymaxmin;
    z += v*t3*u0;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 3.0*zxminmin - 3.0*zxmaxmin + 3.0*zxmaxmax + 3.0*zxminmax - 4.0*zyminmin + 4.0*zymaxmin + 2.0*zymaxmax - 2.0*zyminmax - 2.0*zxyminmin - 2.0*zxymaxmin - zxymaxmax - zxyminmax;
    z += 2.0*v*t3*u1;
    v = 4.0*zminmin - 4.0*zmaxmin + 4.0*zmaxmax - 4.0*zminmax + 2.0*zxminmin + 2.0*zxmaxmin - 2.0*zxmaxmax - 2.0*zxminmax + 2.0*zyminmin - 2.0*zymaxmin - 2.0*zymaxmax + 2.0*zyminmax + zxyminmin + zxymaxmin + zxymaxmax + zxyminmax;
    z += 3.0*v*t3*u2;
    z *= du;
    z
}

/// `bicubic_deriv_xx` (bicubic.c lines 571-589): the polynomial sum, copied by script.
pub(crate) fn deriv_xx(c: &BicubicCell) -> f64 {
    let BicubicCell { t0, t1, t2, t3, u0, u1, u2, u3, dt, du,
        zminmin, zminmax, zmaxmin, zmaxmax, zxminmin, zxminmax, zxmaxmin, zxmaxmax,
        zyminmin, zyminmax, zymaxmin, zymaxmax, zxyminmin, zxyminmax, zxymaxmin, zxymaxmax } = *c;
    let mut v;
    let mut z = 0.0_f64;
    v = -3.0*zminmin + 3.0*zmaxmin - 2.0*zxminmin - zxmaxmin;
    z += 2.0*v*t0*u0;
    v = -3.0*zyminmin + 3.0*zymaxmin - 2.0*zxyminmin - zxymaxmin;
    z += 2.0*v*t0*u1;
    v = 9.0*zminmin - 9.0*zmaxmin + 9.0*zmaxmax - 9.0*zminmax + 6.0*zxminmin + 3.0*zxmaxmin - 3.0*zxmaxmax - 6.0*zxminmax + 6.0*zyminmin - 6.0*zymaxmin - 3.0*zymaxmax + 3.0*zyminmax + 4.0*zxyminmin + 2.0*zxymaxmin + zxymaxmax + 2.0*zxyminmax;
    z += 2.0*v*t0*u2;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 4.0*zxminmin - 2.0*zxmaxmin + 2.0*zxmaxmax + 4.0*zxminmax - 3.0*zyminmin + 3.0*zymaxmin + 3.0*zymaxmax - 3.0*zyminmax - 2.0*zxyminmin - zxymaxmin - zxymaxmax - 2.0*zxyminmax;
    z += 2.0*v*t0*u3;
    v = 2.0*zminmin - 2.0*zmaxmin + zxminmin + zxmaxmin;
    z += 6.0*v*t1*u0;
    v = 2.0*zyminmin - 2.0*zymaxmin + zxyminmin + zxymaxmin;
    z += 6.0*v*t1*u1;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 3.0*zxminmin - 3.0*zxmaxmin + 3.0*zxmaxmax + 3.0*zxminmax - 4.0*zyminmin + 4.0*zymaxmin + 2.0*zymaxmax - 2.0*zyminmax - 2.0*zxyminmin - 2.0*zxymaxmin - zxymaxmax - zxyminmax;
    z += 6.0*v*t1*u2;
    v = 4.0*zminmin - 4.0*zmaxmin + 4.0*zmaxmax - 4.0*zminmax + 2.0*zxminmin + 2.0*zxmaxmin - 2.0*zxmaxmax - 2.0*zxminmax + 2.0*zyminmin - 2.0*zymaxmin - 2.0*zymaxmax + 2.0*zyminmax + zxyminmin + zxymaxmin + zxymaxmax + zxyminmax;
    z += 6.0*v*t1*u3;
    z *= dt*dt;
    z
}

/// `bicubic_deriv_xy` (bicubic.c lines 668-688): the polynomial sum, copied by script.
pub(crate) fn deriv_xy(c: &BicubicCell) -> f64 {
    let BicubicCell { t0, t1, t2, t3, u0, u1, u2, u3, dt, du,
        zminmin, zminmax, zmaxmin, zmaxmax, zxminmin, zxminmax, zxmaxmin, zxmaxmax,
        zyminmin, zyminmax, zymaxmin, zymaxmax, zxyminmin, zxyminmax, zxymaxmin, zxymaxmax } = *c;
    let mut v;
    let mut z = 0.0_f64;
    v = zxyminmin;
    z += v*t0*u0;
    v = -3.0*zxminmin + 3.0*zxminmax - 2.0*zxyminmin - zxyminmax;
    z += 2.0*v*t0*u1;
    v = 2.0*zxminmin - 2.0*zxminmax + zxyminmin + zxyminmax;
    z += 3.0*v*t0*u2;
    v = -3.0*zyminmin + 3.0*zymaxmin - 2.0*zxyminmin - zxymaxmin;
    z += 2.0*v*t1*u0;
    v = 9.0*zminmin - 9.0*zmaxmin + 9.0*zmaxmax - 9.0*zminmax + 6.0*zxminmin + 3.0*zxmaxmin - 3.0*zxmaxmax - 6.0*zxminmax + 6.0*zyminmin - 6.0*zymaxmin - 3.0*zymaxmax + 3.0*zyminmax + 4.0*zxyminmin + 2.0*zxymaxmin + zxymaxmax + 2.0*zxyminmax;
    z += 4.0*v*t1*u1;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 4.0*zxminmin - 2.0*zxmaxmin + 2.0*zxmaxmax + 4.0*zxminmax - 3.0*zyminmin + 3.0*zymaxmin + 3.0*zymaxmax - 3.0*zyminmax - 2.0*zxyminmin - zxymaxmin - zxymaxmax - 2.0*zxyminmax;
    z += 6.0*v*t1*u2;
    v = 2.0*zyminmin - 2.0*zymaxmin + zxyminmin + zxymaxmin;
    z += 3.0*v*t2*u0;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 3.0*zxminmin - 3.0*zxmaxmin + 3.0*zxmaxmax + 3.0*zxminmax - 4.0*zyminmin + 4.0*zymaxmin + 2.0*zymaxmax - 2.0*zyminmax - 2.0*zxyminmin - 2.0*zxymaxmin - zxymaxmax - zxyminmax;
    z += 6.0*v*t2*u1;
    v = 4.0*zminmin - 4.0*zmaxmin + 4.0*zmaxmax - 4.0*zminmax + 2.0*zxminmin + 2.0*zxmaxmin - 2.0*zxmaxmax - 2.0*zxminmax + 2.0*zyminmin - 2.0*zymaxmin - 2.0*zymaxmax + 2.0*zyminmax + zxyminmin + zxymaxmin + zxymaxmax + zxyminmax;
    z += 9.0*v*t2*u2;
    z *= dt*du;
    z
}

/// `bicubic_deriv_yy` (bicubic.c lines 767-785): the polynomial sum, copied by script.
pub(crate) fn deriv_yy(c: &BicubicCell) -> f64 {
    let BicubicCell { t0, t1, t2, t3, u0, u1, u2, u3, dt, du,
        zminmin, zminmax, zmaxmin, zmaxmax, zxminmin, zxminmax, zxmaxmin, zxmaxmax,
        zyminmin, zyminmax, zymaxmin, zymaxmax, zxyminmin, zxyminmax, zxymaxmin, zxymaxmax } = *c;
    let mut v;
    let mut z = 0.0_f64;
    v = -3.0*zminmin + 3.0*zminmax - 2.0*zyminmin - zyminmax;
    z += 2.0*v*t0*u0;
    v = 2.0*zminmin-2.0*zminmax + zyminmin + zyminmax;
    z += 6.0*v*t0*u1;
    v = -3.0*zxminmin + 3.0*zxminmax - 2.0*zxyminmin - zxyminmax;
    z += 2.0*v*t1*u0;
    v = 2.0*zxminmin - 2.0*zxminmax + zxyminmin + zxyminmax;
    z += 6.0*v*t1*u1;
    v = 9.0*zminmin - 9.0*zmaxmin + 9.0*zmaxmax - 9.0*zminmax + 6.0*zxminmin + 3.0*zxmaxmin - 3.0*zxmaxmax - 6.0*zxminmax + 6.0*zyminmin - 6.0*zymaxmin - 3.0*zymaxmax + 3.0*zyminmax + 4.0*zxyminmin + 2.0*zxymaxmin + zxymaxmax + 2.0*zxyminmax;
    z += 2.0*v*t2*u0;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 4.0*zxminmin - 2.0*zxmaxmin + 2.0*zxmaxmax + 4.0*zxminmax - 3.0*zyminmin + 3.0*zymaxmin + 3.0*zymaxmax - 3.0*zyminmax - 2.0*zxyminmin - zxymaxmin - zxymaxmax - 2.0*zxyminmax;
    z += 6.0*v*t2*u1;
    v = -6.0*zminmin + 6.0*zmaxmin - 6.0*zmaxmax + 6.0*zminmax - 3.0*zxminmin - 3.0*zxmaxmin + 3.0*zxmaxmax + 3.0*zxminmax - 4.0*zyminmin + 4.0*zymaxmin + 2.0*zymaxmax - 2.0*zyminmax - 2.0*zxyminmin - 2.0*zxymaxmin - zxymaxmax - zxyminmax;
    z += 2.0*v*t3*u0;
    v = 4.0*zminmin - 4.0*zmaxmin + 4.0*zmaxmax - 4.0*zminmax + 2.0*zxminmin + 2.0*zxmaxmin - 2.0*zxmaxmax - 2.0*zxminmax + 2.0*zyminmin - 2.0*zymaxmin - 2.0*zymaxmax + 2.0*zyminmax + zxyminmin + zxymaxmin + zxymaxmax + zxyminmax;
    z += 6.0*v*t3*u1;
    z *= du*du;
    z
}
