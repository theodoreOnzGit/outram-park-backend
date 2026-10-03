// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/cmapf_mod.f90 (module cmapf_mod: cc2gll, ccrvll,
//                    ccrvxy, cg2cll, cg2cxy, cgszll, cgszxy, cll2xy, cnllxy,
//                    cnxyll, cpolll, cpolxy, cspanf, cxy2ll, eqvlat, stcm1p,
//                    stcm2p, stlmbr)
// Original author  : Dr. Albion Taylor, NOAA / OAR / Air Resources Laboratory
//                    ("General conformal map routines for meteorological
//                    modelers", written 3/31/94 to 11/26/94), with changes by
//                    A. Stohl (xi, xi0, eta, eta0 in double precision "to
//                    avoid problems at poles"; pole guard removed in cnxyll).
// Original licence : distributed inside FLEXPART under FLEXPART's
//                    GPL-3.0-or-later headers —
//                    SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! General conformal map projections: Albion Taylor's (NOAA/ARL) `cmapf`
//! library, as FLEXPART ships it in `cmapf_mod.f90`.
//!
//! One family of maps covers polar stereographic (`tnglat = ±90`), Lambert
//! conformal (`0 < |tnglat| < 90`) and Mercator (`tnglat = 0`): the cone
//! constant is `gamma = sin(tnglat)`. FLEXPART uses it for exactly two maps,
//! `northpolemap` and `southpolemap`, built in `gridcheck_ecmwf.f90` as
//!
//! ```text
//! sizenorth = 6*(90 - switchnorth)/dy
//! call stlmbr(northpolemap, 90., 0.)
//! call stcm2p(northpolemap, 0.,0., switchnorth,0., sizenorth,sizenorth, switchnorth,180.)
//! ```
//!
//! (south: `-90`, `6*(switchsouth + 90)/dy`), and `advance.f90` moves
//! particles poleward of `switchnorth`/`switchsouth` on those maps with
//! [`cll2xy`], [`cgszll`] and [`cxy2ll`]; `verttransform_ecmwf.f90` rotates
//! winds onto them with [`cc2gll`].
//!
//! # The map descriptor `strcmp(9)`
//!
//! [`Strcmp`] holds upstream's nine-element array; `Strcmp.0[i]` is
//! `strcmp(i+1)`:
//!
//! | index | upstream | meaning |
//! |---|---|---|
//! | 0 | `strcmp(1)` | `gamma`, sine of the tangent latitude |
//! | 1 | `strcmp(2)` | `lambda_0`, reference longitude (deg, in (-180, 180]) |
//! | 2 | `strcmp(3)` | `x_0`, grid x of the canonical origin |
//! | 3 | `strcmp(4)` | `y_0`, grid y of the canonical origin |
//! | 4 | `strcmp(5)` | cosine of the rotation from (xi, eta) to (x, y) |
//! | 5 | `strcmp(6)` | sine of that rotation |
//! | 6 | `strcmp(7)` | grid size at the equator (km per grid unit) |
//! | 7 | `strcmp(8)` | radial coordinate 1 degree from the north pole |
//! | 8 | `strcmp(9)` | radial coordinate 1 degree from the south pole |
//!
//! # Translation notes
//!
//! * **Line for line, f64 throughout**, in Fortran's left-to-right operation
//!   order, so the port agrees with a `-fdefault-real-8` build bit for bit.
//!   Upstream mixes default `real` (the descriptor, positions, winds) with
//!   `real(kind=dp)` (A. Stohl's `xi`, `eta`, `xi0`, `eta0` and most
//!   intermediates). It is *not* consistent about it: `cnllxy` returns `xi`,
//!   `eta` as default `real`, `cll2xy` stores them as `real`, and `cg2cxy`
//!   holds `radial` as `real`. In the shipped (real(4)) build those values
//!   are rounded to `f32`; the port does not reproduce that rounding — it is
//!   what the code-to-code test's real(4) comparison measures.
//! * **Constants are upstream's literals**: `pi = 3.14159265358979` (which is
//!   *not* `std::f64::consts::PI`; it is 3.2e-15 short), `rearth = 6371.2` km,
//!   `almst1 = .9999999`, and `radpdg = pi/180`, `dgprad = 180/pi` derived from
//!   that `pi`.
//! * Fortran `mod` on reals is `fmod`, which is Rust's `%`. Fortran
//!   `sign(a, b)` is `|a|` with the sign of `b` (`-0.0` counts as negative,
//!   gfortran's default), i.e. `a.abs().copysign(b)`.
//!
//! # Upstream quirks, reproduced and documented
//!
//! * `cc2gxy` is documented in upstream's header comment but **does not
//!   exist** in `cmapf_mod.f90`; it is not ported.
//! * Only six routines are public upstream (`cc2gll`, `cll2xy`, `cgszll`,
//!   `cxy2ll`, `stlmbr`, `stcm2p`); the rest are module-private and unused by
//!   FLEXPART. They are ported because they are part of the library and
//!   verified like the others.
//! * **The ll and xy routines use different polar thresholds.** `cc2gll` /
//!   `cg2cll` switch to the polar wind orientation at `|lat| > 89.985`;
//!   `cg2cxy` switches when its radial coordinate passes `strcmp(8)` /
//!   `strcmp(9)`, i.e. at `|lat| > 89` (as `stlmbr` sets them). Between 89
//!   and 89.985 degrees the two conventions disagree on what "north" means.
//! * **Mercator maps return non-finite values at the poles.** `cnllxy` sets
//!   `eta = 1/gamma` when `|sin(lat)| >= almst1` (`|lat| >~ 89.974`); with
//!   `gamma = 0` that is `+inf`, and `cll2xy` then yields `inf` or `NaN`.
//! * **No pole guard in `cnxyll`.** A. Stohl commented out the
//!   `arg1 >= almst1` guard "to avoid problems close to the poles". At the
//!   image of the pole `arg1` is 1 up to rounding, so `log(1 - arg1)` can be
//!   `log` of a tiny negative number: `NaN` latitude, reproduced. Measured
//!   (2026-10-02, real(8) build): the exact canonical pole `(0, 1/gamma)` of
//!   a southern Lambert map (tangent -35) returns `NaN`.
//! * **Stohl's double precision does not reach `xi0`, `eta0` in the shipped
//!   build.** `xi0 = (x - strcmp(3))*strcmp(7)/rearth` has only default-real
//!   operands, so in real(4) it is evaluated in single precision and only
//!   then stored to `real(kind=dp)`. Near the pole image the shipped
//!   `cxy2ll` therefore resolves direction no better than an all-`f32`
//!   code would; `cnxyll`, whose `xi`, `eta` arrive in `dp`, agrees with the
//!   port to 1.3e-6 everywhere in the shipped build.
//! * **`eqvlat(±90, ±90)` is `NaN`**, not ±90: with `sinl1 == sinl2 == ±1`
//!   the near-equal branch forms `tau = 0/0`.
//! * `cgszll` returns `2*strcmp(7)` at `|lat| > 89.985` only for
//!   `|gamma| > 0.9999`; for other maps it evaluates
//!   `cos(lat)*exp(gamma*ymerc)` there, and returns 0 when `cos(lat) <= 0`
//!   (latitudes beyond the pole, or rounding at exactly ±90: with upstream's
//!   `pi`, `cos(90 deg)` is +1.6e-15 in `f64` but negative in `f32`, so the
//!   shipped build returns 0 at exactly ±90 where the port and real(8) do not).
//! * `ccrvxy`'s `temp == 0` branch needs `xpolg` and `ypolg` to be exactly
//!   zero. For `|gamma| == 1` it is not reached on the `gridcheck` polar maps
//!   (no `x` within 2e5 ulps of the pole zeroes `xpolg`, searched with this
//!   port); it is reached on a polar map straight from [`stlmbr`].
//! * In the `-fdefault-real-8` build, `cnxyll`'s `sngl(...)` returns an
//!   8-byte real (the port's `f64` longitudes are bit-exact against it).
//!
//! # Units
//!
//! Bare `f64` in upstream's units: latitudes and longitudes in degrees, grid
//! coordinates in grid units, grid sizes in km per grid unit, curvature in
//! radians per km, wind components in any one consistent unit (upstream's
//! comment says km/h; the rotation is unit-free).

/// Upstream's `pi` literal (`cmapf_mod.f90`), 3.2e-15 short of `PI`.
#[allow(clippy::approx_constant, clippy::excessive_precision)]
const PI_CMAPF: f64 = 3.14159265358979;
/// Earth radius, km.
const REARTH: f64 = 6371.2;
/// "Almost one".
const ALMST1: f64 = 0.9999999;
/// Radians per degree, from upstream's `pi`.
const RADPDG: f64 = PI_CMAPF / 180.0;
/// Degrees per radian, from upstream's `pi`.
const DGPRAD: f64 = 180.0 / PI_CMAPF;

/// Upstream's 9-element map descriptor `strcmp(9)`; element `i` is
/// `strcmp(i+1)`. See the module documentation for the index mapping.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Strcmp(pub [f64; 9]);

impl Strcmp {
    /// `strcmp(1)`: `gamma`, the sine of the tangent latitude.
    pub fn gamma(&self) -> f64 {
        self.0[0]
    }
    /// `strcmp(2)`: reference longitude, degrees.
    pub fn reference_longitude(&self) -> f64 {
        self.0[1]
    }
    /// `strcmp(3)`, `strcmp(4)`: grid coordinates of the canonical origin.
    pub fn origin(&self) -> (f64, f64) {
        (self.0[2], self.0[3])
    }
    /// `strcmp(5)`, `strcmp(6)`: cosine and sine of the grid rotation.
    pub fn rotation(&self) -> (f64, f64) {
        (self.0[4], self.0[5])
    }
    /// `strcmp(7)`: grid size at the equator, km per grid unit.
    pub fn equator_grid_size(&self) -> f64 {
        self.0[6]
    }
    /// `strcmp(8)`, `strcmp(9)`: radial coordinates 1 degree from the north
    /// and south poles (the polar thresholds of [`cg2cxy`]).
    pub fn polar_radials(&self) -> (f64, f64) {
        (self.0[7], self.0[8])
    }
}

/// Fortran `sign(a, b)`.
fn fsign(a: f64, b: f64) -> f64 {
    a.abs().copysign(b)
}

/// `cspanf(value, begin, end)`: `value` reduced modulo `end - begin` into the
/// half-open interval `(min, max]` of the two bounds (either order).
/// `cspanf(-180, -180, 180)` is therefore `180`.
pub fn cspanf(value: f64, begin: f64, end: f64) -> f64 {
    let first = begin.min(end);
    let last = begin.max(end);
    let val = (value - first) % (last - first);
    if val <= 0.0 {
        val + last
    } else {
        val + first
    }
}

/// `eqvlat(xlat1, xlat2)`: the tangent latitude (deg) equivalent to a Lambert
/// map "true at `xlat1` and `xlat2`". Returns `NaN` for `(±90, ±90)` (see the
/// module quirks).
pub fn eqvlat(xlat1: f64, xlat2: f64) -> f64 {
    let ssind = |x: f64| (RADPDG * x).sin();
    let sinl1 = ssind(xlat1);
    let sinl2 = ssind(xlat2);
    let (al1, al2);
    if (sinl1 - sinl2).abs() > 0.001 {
        al1 = ((1.0 - sinl1) / (1.0 - sinl2)).ln();
        al2 = ((1.0 + sinl1) / (1.0 + sinl2)).ln();
    } else {
        // Case lat1 near or equal to lat2
        let mut tau = -((sinl1 - sinl2) / (2.0 - sinl1 - sinl2));
        tau = tau * tau;
        al1 = 2.0 / (2.0 - sinl1 - sinl2)
            * (1.0 + tau * (1.0 / 3.0 + tau * (1.0 / 5.0 + tau * (1.0 / 7.0))));
        tau = (sinl1 - sinl2) / (2.0 + sinl1 + sinl2);
        tau = tau * tau;
        al2 = -(2.0 / (2.0 + sinl1 + sinl2)
            * (1.0 + tau * (1.0 / 3.0 + tau * (1.0 / 5.0 + tau * (1.0 / 7.0)))));
    }
    ((al1 + al2) / (al1 - al2)).asin() / RADPDG
}

/// `stlmbr(strcmp, tnglat, xlong)`: a map of tangent latitude `tnglat` (deg;
/// +90 north polar stereographic, -90 south, 0 Mercator, else Lambert) whose
/// region is connected for longitudes `xlong ± 180`. The grid placement
/// (`strcmp(3..7)`) is the canonical one; complete it with [`stcm2p`] or
/// [`stcm1p`].
pub fn stlmbr(tnglat: f64, xlong: f64) -> Strcmp {
    let mut s = Strcmp::default();
    s.0[0] = (RADPDG * tnglat).sin();
    s.0[1] = cspanf(xlong, -180.0, 180.0);
    s.0[2] = 0.0;
    s.0[3] = 0.0;
    s.0[4] = 1.0;
    s.0[5] = 0.0;
    s.0[6] = REARTH;
    let (_xi, eta) = cnllxy(&s, 89.0, xlong);
    s.0[7] = 2.0 * eta - s.0[0] * eta * eta;
    let (_xi, eta) = cnllxy(&s, -89.0, xlong);
    s.0[8] = 2.0 * eta - s.0[0] * eta * eta;
    s
}

/// `stcm1p`: place the grid so that `(x1, y1)` is at `(xlat1, xlong1)`, the
/// grid size at `(xlatg, xlongg)` is `gridsz` km, and a y grid line there
/// points `orient` degrees from north.
#[allow(clippy::too_many_arguments)]
pub fn stcm1p(
    s: &mut Strcmp,
    x1: f64,
    y1: f64,
    xlat1: f64,
    xlong1: f64,
    xlatg: f64,
    xlongg: f64,
    gridsz: f64,
    orient: f64,
) {
    s.0[2] = 0.0;
    s.0[3] = 0.0;
    let turn = RADPDG * (orient - s.0[0] * cspanf(xlongg - s.0[1], -180.0, 180.0));
    s.0[4] = turn.cos();
    s.0[5] = -turn.sin();
    s.0[6] = 1.0;
    s.0[6] = gridsz * s.0[6] / cgszll(s, xlatg, s.0[1]);
    let (x1a, y1a) = cll2xy(s, xlat1, xlong1);
    s.0[2] = s.0[2] + x1 - x1a;
    s.0[3] = s.0[3] + y1 - y1a;
}

/// `stcm2p`: place the grid so that `(x1, y1)` is at `(xlat1, xlong1)` and
/// `(x2, y2)` at `(xlat2, xlong2)`.
#[allow(clippy::too_many_arguments)]
pub fn stcm2p(
    s: &mut Strcmp,
    x1: f64,
    y1: f64,
    xlat1: f64,
    xlong1: f64,
    x2: f64,
    y2: f64,
    xlat2: f64,
    xlong2: f64,
) {
    for k in 2..6 {
        s.0[k] = 0.0;
    }
    s.0[4] = 1.0;
    s.0[6] = 1.0;
    let (x1a, y1a) = cll2xy(s, xlat1, xlong1);
    let (x2a, y2a) = cll2xy(s, xlat2, xlong2);
    let den = ((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2)).sqrt();
    let dena = ((x1a - x2a) * (x1a - x2a) + (y1a - y2a) * (y1a - y2a)).sqrt();
    s.0[4] = ((x1a - x2a) * (x1 - x2) + (y1a - y2a) * (y1 - y2)) / den / dena;
    s.0[5] = ((y1a - y2a) * (x1 - x2) - (x1a - x2a) * (y1 - y2)) / den / dena;
    s.0[6] = s.0[6] * dena / den;
    let (x1a, y1a) = cll2xy(s, xlat1, xlong1);
    s.0[2] = s.0[2] + x1 - x1a;
    s.0[3] = s.0[3] + y1 - y1a;
}

/// `cnllxy`: geographic `(xlat, xlong)` (deg) to canonical `(xi, eta)`
/// (equator-centred, radian units). Within `|sin(lat)| >= almst1` of a pole
/// it returns `(0, 1/gamma)` — `+inf` for a Mercator map.
pub fn cnllxy(s: &Strcmp, xlat: f64, xlong: f64) -> (f64, f64) {
    let gamma = s.0[0];
    let dlat = xlat;
    let mut dlong = cspanf(xlong - s.0[1], -180.0, 180.0);
    dlong *= RADPDG;
    let mut gdlong = gamma * dlong;
    let (sndgam, csdgam);
    if gdlong.abs() < 0.01 {
        // gamma small or zero: series, avoids round-off / divide by zero.
        gdlong = gdlong * gdlong;
        sndgam = dlong
            * (1.0
                - 1.0 / 6.0 * gdlong * (1.0 - 1.0 / 20.0 * gdlong * (1.0 - 1.0 / 42.0 * gdlong)));
        csdgam = dlong
            * dlong
            * 0.5
            * (1.0
                - 1.0 / 12.0 * gdlong * (1.0 - 1.0 / 30.0 * gdlong * (1.0 - 1.0 / 56.0 * gdlong)));
    } else {
        sndgam = gdlong.sin() / gamma;
        csdgam = (1.0 - gdlong.cos()) / gamma / gamma;
    }
    let slat = (RADPDG * dlat).sin();
    if slat >= ALMST1 || slat <= -ALMST1 {
        return (0.0, 1.0 / s.0[0]);
    }
    let mercy = 0.5 * ((1.0 + slat) / (1.0 - slat)).ln();
    let gmercy = gamma * mercy;
    let rhog1 = if gmercy.abs() < 0.001 {
        mercy * (1.0 - 0.5 * gmercy * (1.0 - 1.0 / 3.0 * gmercy * (1.0 - 1.0 / 4.0 * gmercy)))
    } else {
        (1.0 - (-gmercy).exp()) / gamma
    };
    let eta = rhog1 + (1.0 - gamma * rhog1) * gamma * csdgam;
    let xi = (1.0 - gamma * rhog1) * sndgam;
    (xi, eta)
}

/// `cnxyll`: canonical `(xi, eta)` to geographic `(xlat, xlong)` (deg). The
/// longitude is NOT wrapped (see [`cxy2ll`]). No pole guard (module quirks).
pub fn cnxyll(s: &Strcmp, xi: f64, eta: f64) -> (f64, f64) {
    let gamma = s.0[0];
    // Equivalent Mercator coordinate. (Upstream also computes an unused
    // `odist = xi*xi + eta*eta`.)
    let arg2 = 2.0 * eta - gamma * (xi * xi + eta * eta);
    let arg1 = gamma * arg2;
    let ymerc = if arg1.abs() < 0.01 {
        let t = arg1 / (2.0 - arg1);
        let temp = t * t;
        arg2 / (2.0 - arg1) * (1.0 + temp * (1.0 / 3.0 + temp * (1.0 / 5.0 + temp * (1.0 / 7.0))))
    } else {
        -(1.0 - arg1).ln() / 2.0 / gamma
    };
    // Convert ymerc to latitude
    let temp = (-ymerc.abs()).exp();
    let mut xlat = fsign(((1.0 - temp) * (1.0 + temp)).atan2(2.0 * temp), ymerc);
    // Longitudes
    let gxi = gamma * xi;
    let cgeta = 1.0 - gamma * eta;
    let along = if gxi.abs() < 0.01 * cgeta {
        let t = gxi / cgeta;
        let temp = t * t;
        xi / cgeta * (1.0 - temp * (1.0 / 3.0 - temp * (1.0 / 5.0 - temp * (1.0 / 7.0))))
    } else {
        gxi.atan2(cgeta) / gamma
    };
    // Upstream: sngl(strcmp(2) + dgprad*along); the port keeps f64.
    let xlong = s.0[1] + DGPRAD * along;
    xlat *= DGPRAD;
    (xlat, xlong)
}

/// `cll2xy`: geographic `(xlat, xlong)` (deg) to grid `(x, y)`.
pub fn cll2xy(s: &Strcmp, xlat: f64, xlong: f64) -> (f64, f64) {
    let (xi, eta) = cnllxy(s, xlat, xlong);
    let x = s.0[2] + REARTH / s.0[6] * (xi * s.0[4] + eta * s.0[5]);
    let y = s.0[3] + REARTH / s.0[6] * (eta * s.0[4] - xi * s.0[5]);
    (x, y)
}

/// Grid `(x, y)` to canonical `(xi0, eta0, xi, eta)` — the four lines every
/// xy routine opens with.
fn xy_to_canonical(s: &Strcmp, x: f64, y: f64) -> (f64, f64, f64, f64) {
    let xi0 = (x - s.0[2]) * s.0[6] / REARTH;
    let eta0 = (y - s.0[3]) * s.0[6] / REARTH;
    let xi = xi0 * s.0[4] - eta0 * s.0[5];
    let eta = eta0 * s.0[4] + xi0 * s.0[5];
    (xi0, eta0, xi, eta)
}

/// `cxy2ll`: grid `(x, y)` to geographic `(xlat, xlong)` (deg), longitude
/// wrapped into (-180, 180].
pub fn cxy2ll(s: &Strcmp, x: f64, y: f64) -> (f64, f64) {
    let (_, _, xi, eta) = xy_to_canonical(s, x, y);
    let (xlat, xlong) = cnxyll(s, xi, eta);
    (xlat, cspanf(xlong, -180.0, 180.0))
}

/// `cgszll`: grid size (km per grid unit) at `(xlat, xlong)`.
pub fn cgszll(s: &Strcmp, xlat: f64, _xlong: f64) -> f64 {
    let ymerc = if xlat > 89.985 {
        // Close to north pole
        if s.0[0] > 0.9999 {
            return 2.0 * s.0[6];
        }
        let efact = (RADPDG * xlat).cos();
        if efact <= 0.0 {
            return 0.0;
        }
        -(efact / (1.0 + (RADPDG * xlat).sin())).ln()
    } else if xlat < -89.985 {
        // Close to south pole
        if s.0[0] < -0.9999 {
            return 2.0 * s.0[6];
        }
        let efact = (RADPDG * xlat).cos();
        if efact <= 0.0 {
            return 0.0;
        }
        (efact / (1.0 - (RADPDG * xlat).sin())).ln()
    } else {
        let slat = (RADPDG * xlat).sin();
        ((1.0 + slat) / (1.0 - slat)).ln() / 2.0
    };
    s.0[6] * (RADPDG * xlat).cos() * (s.0[0] * ymerc).exp()
}

/// `cgszxy`: grid size (km per grid unit) at grid `(x, y)`.
pub fn cgszxy(s: &Strcmp, x: f64, y: f64) -> f64 {
    let (_, _, xi, eta) = xy_to_canonical(s, x, y);
    let radial = 2.0 * eta - s.0[0] * (xi * xi + eta * eta);
    let efact = s.0[0] * radial;
    if efact > ALMST1 {
        return if s.0[0] > ALMST1 { 2.0 * s.0[6] } else { 0.0 };
    }
    let ymerc = if efact.abs() < 1.0e-2 {
        let t = efact / (2.0 - efact);
        let temp = t * t;
        radial / (2.0 - efact)
            * (1.0 + temp * (1.0 / 3.0 + temp * (1.0 / 5.0 + temp * (1.0 / 7.0))))
    } else {
        -(1.0 - efact).ln() / 2.0 / s.0[0]
    };
    if ymerc > 6.0 {
        if s.0[0] > ALMST1 {
            2.0 * s.0[6]
        } else {
            0.0
        }
    } else if ymerc < -6.0 {
        if s.0[0] < -ALMST1 {
            2.0 * s.0[6]
        } else {
            0.0
        }
    } else {
        let efact = ymerc.exp();
        2.0 * s.0[6] * (s.0[0] * ymerc).exp() / (efact + 1.0 / efact)
    }
}

/// The `(xpolg, ypolg)` rotation shared by `cc2gll`, `cg2cll`, `cpolll`.
fn ll_rotation(s: &Strcmp, rot: f64) -> (f64, f64) {
    let slong = (RADPDG * rot).sin();
    let clong = (RADPDG * rot).cos();
    let xpolg = slong * s.0[4] + clong * s.0[5];
    let ypolg = clong * s.0[4] - slong * s.0[5];
    (xpolg, ypolg)
}

/// The rotation angle of `cc2gll` / `cg2cll`, with the meteorological polar
/// orientation ("north" along the prime meridian) beyond `|lat| > 89.985`.
fn ll_rot(s: &Strcmp, xlat: f64, xlong: f64) -> f64 {
    let along = cspanf(xlong - s.0[1], -180.0, 180.0);
    if xlat > 89.985 {
        -(s.0[0] * along) + xlong - 180.0
    } else if xlat < -89.985 {
        -(s.0[0] * along) - xlong
    } else {
        -(s.0[0] * along)
    }
}

/// `cc2gll`: geographic wind `(ue, vn)` at `(xlat, xlong)` to grid
/// components `(ug, vg)`.
pub fn cc2gll(s: &Strcmp, xlat: f64, xlong: f64, ue: f64, vn: f64) -> (f64, f64) {
    let (xpolg, ypolg) = ll_rotation(s, ll_rot(s, xlat, xlong));
    let ug = ypolg * ue + xpolg * vn;
    let vg = ypolg * vn - xpolg * ue;
    (ug, vg)
}

/// `cg2cll`: grid wind `(ug, vg)` at `(xlat, xlong)` to geographic
/// components `(ue, vn)`.
pub fn cg2cll(s: &Strcmp, xlat: f64, xlong: f64, ug: f64, vg: f64) -> (f64, f64) {
    let (xpolg, ypolg) = ll_rotation(s, ll_rot(s, xlat, xlong));
    let ue = ypolg * ug - xpolg * vg;
    let vn = ypolg * vg + xpolg * ug;
    (ue, vn)
}

/// `cg2cxy`: grid wind `(ug, vg)` at grid `(x, y)` to geographic components
/// `(ue, vn)`. The polar orientation switches at `strcmp(8)`/`strcmp(9)`
/// (±89 deg), not at ±89.985 as in [`cg2cll`].
pub fn cg2cxy(s: &Strcmp, x: f64, y: f64, ug: f64, vg: f64) -> (f64, f64) {
    let (xi0, eta0, xi, eta) = xy_to_canonical(s, x, y);
    let radial = 2.0 * eta - s.0[0] * (xi * xi + eta * eta);
    let (xpolg, ypolg);
    if radial > s.0[7] {
        // North of 89 degrees: polar meteorological orientation.
        let (_xlat, xlong) = cnxyll(s, xi, eta);
        let rot = s.0[0] * (xlong - s.0[1]) - xlong - 180.0;
        let slong = -(RADPDG * rot).sin();
        let clong = (RADPDG * rot).cos();
        xpolg = slong * s.0[4] + clong * s.0[5];
        ypolg = clong * s.0[4] - slong * s.0[5];
    } else if radial < s.0[8] {
        // South of -89 degrees.
        let (_xlat, xlong) = cnxyll(s, xi, eta);
        let rot = s.0[0] * (xlong - s.0[1]) + xlong;
        let slong = -(RADPDG * rot).sin();
        let clong = (RADPDG * rot).cos();
        xpolg = slong * s.0[4] + clong * s.0[5];
        ypolg = clong * s.0[4] - slong * s.0[5];
    } else {
        // Normal case: direction related to true north.
        let xp = s.0[5] - s.0[0] * xi0;
        let yp = s.0[4] - s.0[0] * eta0;
        let temp = (xp * xp + yp * yp).sqrt();
        xpolg = xp / temp;
        ypolg = yp / temp;
    }
    let ue = ypolg * ug - xpolg * vg;
    let vn = ypolg * vg + xpolg * ug;
    (ue, vn)
}

/// `ccrvll`: map-curvature vector `(gx, gy)` (rad/km) at `(xlat, xlong)`.
pub fn ccrvll(s: &Strcmp, xlat: f64, xlong: f64) -> (f64, f64) {
    let along = cspanf(xlong - s.0[1], -180.0, 180.0);
    let slong = (RADPDG * s.0[0] * along).sin();
    let clong = (RADPDG * s.0[0] * along).cos();
    let xpolg = -slong * s.0[4] + clong * s.0[5];
    let ypolg = clong * s.0[4] + slong * s.0[5];
    let temp = (RADPDG * xlat).sin();
    let ctemp = (RADPDG * xlat).cos();
    let curv = (s.0[0] - temp) / ctemp / REARTH;
    (curv * xpolg, curv * ypolg)
}

/// `ccrvxy`: map-curvature vector `(gx, gy)` (rad/km) at grid `(x, y)`.
pub fn ccrvxy(s: &Strcmp, x: f64, y: f64) -> (f64, f64) {
    let temp = s.0[0] * s.0[6] / REARTH;
    let xpolg = s.0[5] + temp * (s.0[2] - x);
    let ypolg = s.0[4] + temp * (s.0[3] - y);
    let temp = (xpolg * xpolg + ypolg * ypolg).sqrt();
    if temp > 0.0 {
        let ymerc = -temp.ln() / s.0[0];
        let efact = ymerc.exp();
        let curv = ((s.0[0] - 1.0) * efact + (s.0[0] + 1.0) / efact) * 0.5 / REARTH;
        (xpolg * curv / temp, ypolg * curv / temp)
    } else if s.0[0].abs() == 1.0 {
        (0.0, 0.0)
    } else {
        (1.0 / REARTH, 1.0 / REARTH)
    }
}

/// `cpolll`: direction cosines `(enx, eny, enz)` of the Earth's rotation axis
/// in map coordinates at `(xlat, xlong)`.
pub fn cpolll(s: &Strcmp, xlat: f64, xlong: f64) -> (f64, f64, f64) {
    let along = cspanf(xlong - s.0[1], -180.0, 180.0);
    let rot = -(s.0[0] * along);
    let (xpolg, ypolg) = ll_rotation(s, rot);
    let clat = (RADPDG * xlat).cos();
    (clat * xpolg, clat * ypolg, (RADPDG * xlat).sin())
}

/// `cpolxy`: direction cosines `(enx, eny, enz)` of the Earth's rotation axis
/// in map coordinates at grid `(x, y)`.
pub fn cpolxy(s: &Strcmp, x: f64, y: f64) -> (f64, f64, f64) {
    let (_, _, xi, eta) = xy_to_canonical(s, x, y);
    let radial = 2.0 * eta - s.0[0] * (xi * xi + eta * eta);
    let temp = s.0[0] * radial;
    if temp >= 1.0 {
        return (0.0, 0.0, fsign(1.0, s.0[0]));
    }
    let ymerc = if temp.abs() < 1.0e-2 {
        let t = temp / (2.0 - temp);
        let temp2 = t * t;
        radial / (2.0 - temp)
            * (1.0 + temp2 * (1.0 / 3.0 + temp2 * (1.0 / 5.0 + temp2 * (1.0 / 7.0))))
    } else {
        -0.5 * (1.0 - temp).ln() / s.0[0]
    };
    let arg = ymerc.exp();
    let oarg = 1.0 / arg;
    let clat = 2.0 / (arg + oarg);
    let enz = (arg - oarg) * clat / 2.0;
    let temp = clat / (1.0 - temp).sqrt();
    let xpol = -(xi * s.0[0] * temp);
    let ypol = (1.0 - eta * s.0[0]) * temp;
    let enx = xpol * s.0[4] + ypol * s.0[5];
    let eny = ypol * s.0[4] - xpol * s.0[5];
    (enx, eny, enz)
}
