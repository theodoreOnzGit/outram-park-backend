//! TVD flux limiters — field-agnostic `psi(r)` functions on plain `f64`.
//!
//! A **flux limiter** `psi(r)` blends a high-order (linear/central) face flux
//! with first-order upwind to suppress spurious oscillations near sharp
//! gradients, where `r` is the ratio of successive solution gradients. `psi = 0`
//! is first-order upwind; `psi = 1` recovers second-order (linear) differencing.
//!
//! This is a **pure-`f64`, mesh-free** API so any finite-volume code (e.g. the
//! `outram-park-fork-pflotran` solute/energy transport) can build higher-order
//! TVD advection without depending on this crate's field/mesh types. A separate,
//! field-tied limiter for rhoCentralFoam reconstruction lives at
//! [`crate::fv_operators::fvc::Limiter`]; this module is the reusable,
//! general one, and the two should be consolidated eventually.
//!
//! # Provenance (translated from OpenFOAM upstream source)
//!
//! Each limiter here is a Rust translation of the corresponding `limiter()`
//! method in OpenFOAM's
//! `src/finiteVolume/interpolation/surfaceInterpolation/limitedSchemes/<name>/<name>.H`,
//! **Copyright (C) 2011-2022 OpenFOAM Foundation**, GNU General Public License
//! version 3 or later (this crate is GPL-3.0). Source read from
//! `github.com/OpenFOAM/OpenFOAM-dev` (master) on 2026-07-22. OpenFOAM® is a
//! registered trademark of OpenCFD Ltd (ESI Group); this is an independent
//! translation, not an official OpenFOAM product (see the workspace
//! `TRADEMARKS.md`).
//!
//! The exact upstream expression is quoted in each variant's doc comment. Only
//! OpenFOAM's **r-based** limiters are ported: OpenFOAM's NVD-based schemes
//! (`QUICK`, `Gamma`, `SFCD`, `Phi`) use actual cell values rather than a pure
//! `psi(r)` and are **not** representable here, so they are deliberately omitted
//! rather than approximated.

/// A TVD flux limiter `psi(r)`, translated from OpenFOAM's r-based
/// `limitedSchemes`. `psi = 0` is first-order upwind, `psi = 1` is second-order
/// linear; the TVD variants clip extrema (`psi(r) = 0` for `r <= 0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FluxLimiter {
    /// First-order `upwind`: `psi = 0`.
    Upwind,
    /// Unlimited `linear` (central) differencing: `psi = 1` (2nd order, not TVD).
    Linear,
    /// `vanLeer`: upstream `(r + mag(r))/(1 + mag(r))`. Smooth, symmetric.
    VanLeer,
    /// `vanAlbada`: upstream `r*(r + 1)/(sqr(r) + 1)` with `r = max(0, r)`. Symmetric.
    VanAlbada,
    /// `Minmod`: upstream `max(min(r, 1), 0)`. Most diffusive TVD limiter. Symmetric.
    Minmod,
    /// `SuperBee`: upstream `max(max(min(2r, 1), min(r, 2)), 0)`. Most compressive. Symmetric.
    SuperBee,
    /// `MUSCL`: upstream `max(min(min(2r, 0.5r + 0.5), 2), 0)`. Symmetric.
    Muscl,
    /// `UMIST`: upstream `max(min(min(min(2r, 0.75r + 0.25), 0.25r + 0.75), 2), 0)`.
    /// Third-order biased (not symmetric).
    Umist,
    /// `OSPRE`: upstream `1.5 r (r + 1)/(r (r + 1) + 1)` with `r = max(0, r)`. Symmetric.
    Ospre,
    /// `limitedLinear(k)`: upstream `max(min((2/k) r, 1), 0)`, coefficient
    /// `k` in `[0, 1]` (`k -> 0` approaches unlimited linear, `k = 1` most
    /// limited). A k-blended bounded scheme — not strictly within the classic
    /// `psi <= 2r` Sweby envelope for small `k`.
    LimitedLinear(f64),
}

impl FluxLimiter {
    /// The flux-limiter function `psi(r)`. `r` may be any `f64`; a non-finite `r`
    /// returns `0`. For the TVD limiters `r <= 0` (a local extremum) returns `0`.
    /// The result is clamped non-negative.
    pub fn psi(&self, r: f64) -> f64 {
        if !r.is_finite() {
            return 0.0;
        }
        let val = match *self {
            FluxLimiter::Upwind => 0.0,
            FluxLimiter::Linear => 1.0,
            // vanLeer: (r + |r|)/(1 + |r|)
            FluxLimiter::VanLeer => (r + r.abs()) / (1.0 + r.abs()),
            // vanAlbada: r*(r+1)/(r^2+1), r = max(0,r)
            FluxLimiter::VanAlbada => {
                let r = r.max(0.0);
                r * (r + 1.0) / (r * r + 1.0)
            }
            // Minmod: max(min(r,1),0)
            FluxLimiter::Minmod => r.min(1.0).max(0.0),
            // SuperBee: max(max(min(2r,1), min(r,2)), 0)
            FluxLimiter::SuperBee => (2.0 * r).min(1.0).max(r.min(2.0)).max(0.0),
            // MUSCL: max(min(min(2r, 0.5r+0.5), 2), 0)
            FluxLimiter::Muscl => (2.0 * r).min(0.5 * r + 0.5).min(2.0).max(0.0),
            // UMIST: max(min(min(min(2r, 0.75r+0.25), 0.25r+0.75), 2), 0)
            FluxLimiter::Umist => (2.0 * r)
                .min(0.75 * r + 0.25)
                .min(0.25 * r + 0.75)
                .min(2.0)
                .max(0.0),
            // OSPRE: 1.5 r (r+1)/(r(r+1)+1), r = max(0,r)
            FluxLimiter::Ospre => {
                let r = r.max(0.0);
                1.5 * r * (r + 1.0) / (r * (r + 1.0) + 1.0)
            }
            // limitedLinear(k): max(min((2/k) r, 1), 0), twoByk = 2/max(k, small)
            FluxLimiter::LimitedLinear(k) => {
                let two_by_k = 2.0 / k.max(1e-30);
                (two_by_k * r).min(1.0).max(0.0)
            }
        };
        val.max(0.0)
    }

    /// True if this is a second-order **TVD** limiter (everything except
    /// [`Upwind`](Self::Upwind) and [`Linear`](Self::Linear)).
    pub fn is_tvd(&self) -> bool {
        !matches!(self, FluxLimiter::Upwind | FluxLimiter::Linear)
    }

    /// True if the limiter is **symmetric** (`psi(r)/r == psi(1/r)`), the
    /// property of the second-order symmetric TVD limiters. UMIST is
    /// third-order-biased (asymmetric); `limitedLinear` is a k-blend and not in
    /// this set.
    pub fn is_symmetric(&self) -> bool {
        matches!(
            self,
            FluxLimiter::VanLeer
                | FluxLimiter::VanAlbada
                | FluxLimiter::Minmod
                | FluxLimiter::SuperBee
                | FluxLimiter::Muscl
                | FluxLimiter::Ospre
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The classic r-based TVD limiters that must sit inside the Sweby envelope.
    const CLASSIC: &[FluxLimiter] = &[
        FluxLimiter::VanLeer,
        FluxLimiter::VanAlbada,
        FluxLimiter::Minmod,
        FluxLimiter::SuperBee,
        FluxLimiter::Muscl,
        FluxLimiter::Umist,
        FluxLimiter::Ospre,
    ];

    #[test]
    fn consistency_at_r_one() {
        for l in CLASSIC {
            assert!((l.psi(1.0) - 1.0).abs() < 1e-12, "{l:?} psi(1) != 1");
        }
        assert_eq!(FluxLimiter::Linear.psi(1.0), 1.0);
        assert_eq!(FluxLimiter::Upwind.psi(1.0), 0.0);
    }

    #[test]
    fn extrema_are_clipped() {
        for l in CLASSIC {
            for &r in &[-3.0, -1.0, -0.1, 0.0] {
                assert_eq!(l.psi(r), 0.0, "{l:?} psi({r}) != 0");
            }
        }
    }

    #[test]
    fn within_sweby_tvd_envelope() {
        // Sufficient TVD constraints: 0 <= psi <= 2r and psi <= 2 for r > 0.
        for l in CLASSIC {
            let mut r = 0.01;
            while r <= 6.0 {
                let p = l.psi(r);
                assert!(p >= 0.0, "{l:?} psi({r}) < 0");
                assert!(p <= 2.0 * r + 1e-12, "{l:?} psi({r})={p} > 2r");
                assert!(p <= 2.0 + 1e-12, "{l:?} psi({r})={p} > 2");
                r += 0.01;
            }
        }
    }

    #[test]
    fn symmetric_limiters_satisfy_symmetry() {
        for l in CLASSIC.iter().filter(|l| l.is_symmetric()) {
            for &r in &[0.2, 0.5, 0.75, 1.3, 2.0, 4.0] {
                let lhs = l.psi(r) / r;
                let rhs = l.psi(1.0 / r);
                assert!(
                    (lhs - rhs).abs() < 1e-10,
                    "{l:?} asymmetric at r={r}: {lhs} vs {rhs}"
                );
            }
        }
    }

    #[test]
    fn known_spot_values() {
        assert!((FluxLimiter::SuperBee.psi(0.5) - 1.0).abs() < 1e-12);
        assert!((FluxLimiter::SuperBee.psi(2.0) - 2.0).abs() < 1e-12);
        assert!((FluxLimiter::Minmod.psi(0.5) - 0.5).abs() < 1e-12);
        assert!((FluxLimiter::Minmod.psi(3.0) - 1.0).abs() < 1e-12);
        assert!((FluxLimiter::VanLeer.psi(1.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn limited_linear_matches_upstream_form() {
        // psi = max(min((2/k) r, 1), 0); bounded in [0,1]; psi(1)=1 for any k<=1.
        for &k in &[0.25, 0.5, 1.0] {
            let l = FluxLimiter::LimitedLinear(k);
            for &r in &[-1.0, 0.0, 0.1, 0.5, 1.0, 3.0] {
                let p = l.psi(r);
                assert!(
                    (0.0..=1.0).contains(&p),
                    "limitedLinear({k}) psi({r})={p} out of [0,1]"
                );
            }
            assert!((l.psi(1.0) - 1.0).abs() < 1e-12);
            assert_eq!(l.psi(-1.0), 0.0);
        }
    }

    #[test]
    fn non_finite_r_is_safe() {
        for l in CLASSIC {
            assert_eq!(l.psi(f64::NAN), 0.0);
            assert_eq!(l.psi(f64::INFINITY), 0.0);
        }
    }
}
