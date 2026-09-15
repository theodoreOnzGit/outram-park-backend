// Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.

//! `f64` transcendentals as methods, so a crate can adopt the deterministic
//! route by renaming rather than rewriting.
//!
//! # Why a trait and not just [`crate::real`]
//!
//! [`crate::real::exp`] and friends are free functions, and calling them
//! directly is perfectly correct. The problem is *migration cost*: a crate
//! that already writes `x.exp()` in 300 places has to have each expression
//! turned inside out, `(a * b + c).exp()` becoming `exp(a * b + c)`. Every one
//! of those is a chance to move a paren and change the maths silently.
//!
//! [`RealMath`] makes the same migration a mechanical rename — `.exp()` to
//! `.r_exp()` — which a `sed` can do and a reviewer can check by eye. The
//! `r_` prefix is deliberate: it does not shadow the inherent `f64` methods,
//! so a site that was missed still compiles and still calls the platform, and
//! a site that was converted reads as a deliberate choice rather than looking
//! like ordinary `std`.
//!
//! This shape is `outram-mc-libs`' (`bn:op-chyp.5`), lifted here so the other
//! consumers share one trait instead of copying it four times (`bn:op-j57z`).
//!
//! # What it covers, and what it does not
//!
//! **Exactly the three functions PETIR has a fast, deterministic port of** —
//! [`RealMath::r_exp`], [`RealMath::r_ln`], [`RealMath::r_powf`] — which route
//! to [`crate::fast_exp`], [`crate::fast_log`] and [`crate::fast_pow`], ports
//! of ARM optimized-routines verified bit-identical to upstream's own compiled
//! C. This *is* the default route, and it is at worst comparable to the
//! platform and at best faster than it.
//!
//! # The `platform-libm` escape hatch -- read this before using it
//!
//! The `platform-libm` feature re-points all three methods at the platform's
//! own `f64::exp` / `f64::ln` / `f64::powf`, i.e. back at whatever libm the
//! host supplies. **It gives up the cross-platform determinism this module
//! exists to provide**, so it is neither a performance knob nor a portability
//! knob. Earlier revisions of this file stated flatly that no feature gate
//! existed, and the bar for adding one was correspondingly high.
//!
//! It exists for exactly one job: **rebuilding a golden reference that was
//! recorded before the ARM ports were adopted.** A fixture captured on the
//! platform route cannot otherwise be regenerated without checking out the
//! pre-adoption commit, which makes it a dead artefact the moment the
//! surrounding code moves on.
//!
//! Three rules come with it.
//!
//! - **It is not additive**, which Cargo features are supposed to be. Enabling
//!   it changes results in the last ulp for *every* crate in the resolve,
//!   because Cargo unifies features. Enable it only in a single-crate
//!   invocation (`cargo test -p <crate> --features ...`), **never
//!   workspace-wide**, and never in a default feature set.
//! - **It requires `std`** (hence `platform-libm = ["std"]`): the inherent
//!   `f64` transcendental methods are defined in `std`, not `core`. The crate
//!   stays `no_std` in every other configuration.
//! - **Results taken under it are platform-dependent by construction** and must
//!   be labelled with the machine that produced them, not quoted as though
//!   they were reproducible anywhere.
//!
//! # A 1 ulp change does not stay 1 ulp in a chained transient
//!
//! Adopting the ports in `tampines-steam-tables` was verified by its 1009
//! library tests being unchanged and its five generated V&V reports
//! regenerating character-identical. That evidence is sound for *single-state
//! property evaluation*, where a last-ulp difference is invisible against a
//! 1e-8 verification tolerance, and it was blind to chained integration. The
//! Edwards-O'Brien blowdown case -- roughly 20 000 timesteps x 24 cells x
//! several flashes per cell per corrector, order 1e9 chained evaluations
//! through a stiff pressure-velocity coupling -- passed before the sweep and
//! panicked after it. Bisected to a single commit; see `bn:op-ppmk` and
//! `bn:op-s2dc`.
//!
//! That is **not** evidence against the ports, which sit within about 1 ulp of
//! glibc and are identical on every platform, where the platform route is
//! neither. It is evidence that "unit tests unchanged" is the wrong acceptance
//! check for a long chained solve, and it is why this escape hatch earns its
//! keep: it lets the pre-adoption trajectory be re-derived and diffed rather
//! than merely remembered.
//!
//! It deliberately does **not** cover `log10`, `cbrt`, `cos`, `sin`, `tanh` or
//! `atan2`. Those have no ARM scalar-double implementation, so PETIR can only
//! offer the `libm` (musl-derived) versions, which are deterministic but
//! measurably slower than the platform. That is a trade-off a crate should opt
//! into knowingly, so reach for [`crate::real`]'s free functions and mean it,
//! rather than getting it by importing a trait.
//!
//! Nor does it cover `sqrt`, `abs`, `floor`, `ceil`, `round` or `powi`.
//! IEEE-754 requires `sqrt` to be correctly rounded and the others are exact,
//! so they are **already** bit-identical on every platform. Routing them would
//! buy nothing and cost real time — `libm::sqrt` benchmarks 3.73x slower than
//! the hardware instruction.
//!
//! # The bits move once
//!
//! Adopting this changes results in the last ulp: ARM's `exp` and the platform's
//! are different implementations. Nothing becomes less accurate — all three are
//! within about 1 ulp of glibc, and identical on *every* platform, which they
//! were not before — but a fixture pinned to full `f64` precision needs
//! re-baselining once. Do that deliberately, and record the move.
//!
//! ```
//! use petir::mathf::RealMath;
//!
//! let x = 2.0_f64;
//! // Deterministic on every platform, and the same value glibc gives to ~1 ulp.
//! assert!((x.r_ln().r_exp() - x).abs() < 1e-15);
//! ```

/// `f64` transcendentals routed through PETIR's ARM optimized-routines ports.
///
/// Method names carry an `r_` prefix so adoption is a rename, not an
/// expression rewrite, and so a converted call site reads as deliberate. See
/// the module docs for what is deliberately *not* here.
pub trait RealMath {
    /// Natural logarithm, `ln x`.
    ///
    /// Returns `-inf` at `x == ±0` and a NaN for `x < 0`, as the platform
    /// does. Routed to [`crate::fast_log`].
    fn r_ln(self) -> f64;

    /// `e^x`.
    ///
    /// Returns `+inf` on overflow and `+0.0` on underflow, as the platform
    /// does. Routed to [`crate::fast_exp`].
    fn r_exp(self) -> f64;

    /// `x^y` for real `y`.
    ///
    /// Full IEEE-754 semantics, including the sign of a result from a negative
    /// base with an odd-integer exponent. Routed to [`crate::fast_pow`].
    fn r_powf(self, y: f64) -> f64;
}

/// The default, deterministic route: PETIR's ARM optimized-routines ports.
#[cfg(not(feature = "platform-libm"))]
impl RealMath for f64 {
    #[inline]
    fn r_ln(self) -> f64 {
        crate::real::ln(self)
    }

    #[inline]
    fn r_exp(self) -> f64 {
        crate::real::exp(self)
    }

    #[inline]
    fn r_powf(self, y: f64) -> f64 {
        crate::real::powf(self, y)
    }
}

/// The `platform-libm` route: the host's own libm, through the inherent `f64`
/// methods. Platform-dependent in the last ulp. See the module docs for the one
/// job this is for (rebuilding a pre-adoption golden reference) and the three
/// rules that come with it.
#[cfg(feature = "platform-libm")]
impl RealMath for f64 {
    #[inline]
    fn r_ln(self) -> f64 {
        f64::ln(self)
    }

    #[inline]
    fn r_exp(self) -> f64 {
        f64::exp(self)
    }

    #[inline]
    fn r_powf(self, y: f64) -> f64 {
        f64::powf(self, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trait must be a pure re-spelling of [`crate::real`] — bit for bit,
    /// not approximately. If these ever diverge, a call site's meaning depends
    /// on which spelling it used, which is the whole thing this trait exists to
    /// avoid.
    #[cfg(not(feature = "platform-libm"))]
    #[test]
    fn the_methods_are_bit_identical_to_the_free_functions() {
        for k in -400..=400 {
            let x = f64::from(k) * 0.37;
            assert_eq!(x.r_exp().to_bits(), crate::real::exp(x).to_bits(), "exp at {x}");
            if x > 0.0 {
                assert_eq!(x.r_ln().to_bits(), crate::real::ln(x).to_bits(), "ln at {x}");
                assert_eq!(
                    x.r_powf(2.5).to_bits(),
                    crate::real::powf(x, 2.5).to_bits(),
                    "powf at {x}"
                );
            }
        }
    }

    /// ...and that the route really is the ARM port, not `libm`. This is the
    /// property adopters are buying; pin it here rather than trusting the
    /// module docs to stay true.
    #[cfg(not(feature = "platform-libm"))]
    #[test]
    fn the_route_is_the_arm_port() {
        for k in 1..=300 {
            let x = f64::from(k) * 0.29;
            assert_eq!(x.r_exp().to_bits(), crate::fast_exp::exp_ieee(x).to_bits());
            assert_eq!(x.r_ln().to_bits(), crate::fast_log::ln_ieee(x).to_bits());
            assert_eq!(x.r_powf(1.7).to_bits(), crate::fast_pow::powf_ieee(x, 1.7).to_bits());
        }
    }

    /// Under `platform-libm` the route must be the PLATFORM's libm, not the
    /// ARM port. This is the mirror of `the_route_is_the_arm_port`: whichever
    /// configuration is built, exactly one of the two pins the route, so the
    /// feature can never silently fail to take effect.
    #[cfg(feature = "platform-libm")]
    #[test]
    fn the_route_is_the_platform_libm() {
        for k in 1..=300 {
            let x = f64::from(k) * 0.29;
            assert_eq!(x.r_exp().to_bits(), f64::exp(x).to_bits(), "exp at {x}");
            assert_eq!(x.r_ln().to_bits(), f64::ln(x).to_bits(), "ln at {x}");
            assert_eq!(x.r_powf(1.7).to_bits(), f64::powf(x, 1.7).to_bits(), "powf at {x}");
        }
    }

    /// The two routes must actually differ somewhere, or the escape hatch is a
    /// no-op and a golden reference "rebuilt" under it would silently be the
    /// port's trajectory wearing the platform's label.
    ///
    /// Asserted as "at least one disagreement across the sweep", not as a
    /// per-point inequality: the two agree exactly at most arguments, which is
    /// the point of the ports being within ~1 ulp of glibc. On a host whose
    /// libm happens to be bit-identical to ARM's everywhere this would fail
    /// loudly, which is the correct outcome -- there would then be no
    /// pre-adoption trajectory to recover.
    #[cfg(feature = "platform-libm")]
    #[test]
    fn the_two_routes_are_not_the_same_function() {
        let differs = (1..=20_000).any(|k| {
            let x = f64::from(k) * 0.0013;
            x.r_exp().to_bits() != crate::real::exp(x).to_bits()
                || x.r_ln().to_bits() != crate::real::ln(x).to_bits()
                || x.r_powf(1.7).to_bits() != crate::real::powf(x, 1.7).to_bits()
        });
        assert!(differs, "platform libm is bit-identical to the ARM port on this host");
    }

    /// The IEEE special values must survive the extra hop.
    #[test]
    fn the_special_values_pass_through() {
        assert_eq!(0.0_f64.r_ln(), f64::NEG_INFINITY);
        assert!((-1.0_f64).r_ln().is_nan());
        assert_eq!(1000.0_f64.r_exp(), f64::INFINITY);
        assert_eq!((-1000.0_f64).r_exp(), 0.0);
        assert_eq!(2.0_f64.r_powf(0.0), 1.0);
        assert_eq!((-2.0_f64).r_powf(3.0), -8.0);
        assert!((-2.0_f64).r_powf(0.5).is_nan());
    }
}
