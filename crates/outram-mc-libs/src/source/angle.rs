/// Angular source distributions.
///
/// C++ source: `src/distribution_angle.cpp`, `include/openmc/distribution_angle.h`.
use crate::geometry::position::Direction;

/// Trait for angular distributions.
pub trait AngleDist: Send + Sync {
    /// Sample a direction. `e` is the particle energy (eV) for energy-dependent
    /// distributions (e.g. Legendre expansion at each energy point).
    fn sample(&self, seed: &mut u64, e: f64) -> Direction;
}

/// Isotropic — uniform on the unit sphere.
/// TODO: port from `distribution_angle.cpp`.
pub struct IsotropicAngle;
impl AngleDist for IsotropicAngle {
    fn sample(&self, seed: &mut u64, _e: f64) -> Direction {
        let (u, v, w) = crate::rng::distributions::isotropic_direction(seed);
        Direction::new(u, v, w)
    }
}

/// Monodirectional — all particles in the same direction.
pub struct MonodirectionalAngle {
    pub d: Direction,
}
impl AngleDist for MonodirectionalAngle {
    fn sample(&self, _seed: &mut u64, _e: f64) -> Direction {
        self.d
    }
}

/// Every source angular distribution, as a closed enum. See
/// [`super::spatial::SpatialKind`] for why this is an enum and not a trait
/// object.
pub enum AngleKind {
    /// [`IsotropicAngle`].
    Isotropic(IsotropicAngle),
    /// [`MonodirectionalAngle`].
    Monodirectional(MonodirectionalAngle),
    /// [`PolarAzimuthalAngle`] — a beam with angular spread, or any source
    /// that is neither isotropic nor a single ray (GitHub #264).
    PolarAzimuthal(PolarAzimuthalAngle),
}

impl AngleDist for AngleKind {
    fn sample(&self, seed: &mut u64, e: f64) -> Direction {
        match self {
            AngleKind::Isotropic(d) => d.sample(seed, e),
            AngleKind::Monodirectional(d) => d.sample(seed, e),
            AngleKind::PolarAzimuthal(d) => d.sample(seed, e),
        }
    }
}

// ── Polar-azimuthal angular distribution (GitHub #264) ──────────────────────

/// A 1-D distribution over a bounded variable, for the `mu` and `phi` factors
/// of [`PolarAzimuthalAngle`].
///
/// Deliberately small: these are the two shapes a beam or cone source actually
/// needs. Upstream composes arbitrary `Distribution` objects here; the full
/// tabulated/Watt/Maxwell family already exists in
/// [`super::energy`] for energies and is not duplicated on the angle side
/// until something needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScalarDist {
    /// Always this value.
    Constant(f64),
    /// Uniform on `[lo, hi]`.
    Uniform { lo: f64, hi: f64 },
}

impl ScalarDist {
    fn sample(&self, seed: &mut u64) -> f64 {
        match *self {
            Self::Constant(v) => v,
            Self::Uniform { lo, hi } => lo + (hi - lo) * crate::rng::lcg::prn(seed),
        }
    }
}

/// **Polar-azimuthal** source direction: sample `mu` about a reference axis and
/// `phi` about it. `openmc::PolarAzimuthal`
/// (`src/distribution_multi.cpp:98`).
///
/// # The asymmetry this closes
///
/// This crate already had a `PolarAzimuthalFilter` for **tallies**
/// (`tally::filter`), but nothing on the **source** side: a beam with angular
/// spread, or any source that is neither isotropic nor a single ray, could not
/// be expressed at all. GitHub #264 names that asymmetry explicitly.
///
/// # Construction
///
/// `u_ref` is the reference axis; `v_ref` is any vector not parallel to it, and
/// the frame is completed with `w_ref = u_ref x v_ref` exactly as upstream does
/// (`:71`). [`PolarAzimuthalAngle::new`] orthonormalises `v_ref` against
/// `u_ref` rather than requiring the caller to supply an orthogonal pair --
/// upstream does **not** do that, and a non-orthogonal `v_ref` there silently
/// skews the azimuth.
///
/// # The `mu = +/-1` special cases are not decoration
///
/// At `mu = +/-1` upstream returns `+/-u_ref` directly (`:111-114`) instead of
/// evaluating the general formula. `f = sqrt(1 - mu^2)` is zero there, so the
/// azimuthal terms vanish mathematically — but only if `v_ref` and `w_ref` are
/// finite. Keeping the branch means a degenerate frame cannot turn a
/// perfectly good `mu = 1` sample into a NaN direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolarAzimuthalAngle {
    u_ref: Direction,
    v_ref: Direction,
    w_ref: Direction,
    mu: ScalarDist,
    phi: ScalarDist,
}

impl PolarAzimuthalAngle {
    /// Build from a reference axis, a (not necessarily orthogonal) second
    /// vector, and the two factor distributions.
    ///
    /// # Errors
    ///
    /// A zero-length axis, or a `v_ref` parallel to it — in both cases the
    /// frame is degenerate and the azimuth is meaningless. Refused rather than
    /// producing NaN directions at run time.
    pub fn new(
        u_ref: Direction,
        v_hint: Direction,
        mu: ScalarDist,
        phi: ScalarDist,
    ) -> Result<Self, String> {
        let norm = |d: Direction| (d.u * d.u + d.v * d.v + d.w * d.w).sqrt();
        let nu = norm(u_ref);
        if nu < 1.0e-12 {
            return Err("the reference axis has zero length".to_string());
        }
        let u_ref = Direction::new(u_ref.u / nu, u_ref.v / nu, u_ref.w / nu);

        // Gram-Schmidt v against u. Upstream takes v_ref as given; doing this
        // here means a caller who passes a convenient-but-skew vector gets the
        // frame they meant rather than a quietly sheared azimuth.
        let dot = v_hint.u * u_ref.u + v_hint.v * u_ref.v + v_hint.w * u_ref.w;
        let v = Direction::new(
            v_hint.u - dot * u_ref.u,
            v_hint.v - dot * u_ref.v,
            v_hint.w - dot * u_ref.w,
        );
        let nv = norm(v);
        if nv < 1.0e-12 {
            return Err(
                "v_ref is parallel to the reference axis, so the azimuthal frame is \
                 degenerate"
                    .to_string(),
            );
        }
        let v_ref = Direction::new(v.u / nv, v.v / nv, v.w / nv);
        let w_ref = Direction::new(
            u_ref.v * v_ref.w - u_ref.w * v_ref.v,
            u_ref.w * v_ref.u - u_ref.u * v_ref.w,
            u_ref.u * v_ref.v - u_ref.v * v_ref.u,
        );
        Ok(Self {
            u_ref,
            v_ref,
            w_ref,
            mu,
            phi,
        })
    }

    /// A **cone** of half-angle `theta` about `axis`, uniform in solid angle
    /// inside it.
    ///
    /// Uniform in solid angle means uniform in `mu`, **not** uniform in the
    /// polar angle. Sampling `theta` uniformly instead would pile particles up
    /// on the axis -- the classic error, and the reason this constructor
    /// exists rather than leaving a caller to write `Uniform { lo: 0, hi: theta }`.
    pub fn cone(axis: Direction, half_angle_rad: f64) -> Result<Self, String> {
        if !(0.0..=std::f64::consts::PI).contains(&half_angle_rad) {
            return Err(format!(
                "cone half-angle {half_angle_rad} is outside [0, pi]"
            ));
        }
        // Any vector not parallel to the axis; `new` orthonormalises it.
        let hint = if axis.u.abs() < 0.9 {
            Direction::new(1.0, 0.0, 0.0)
        } else {
            Direction::new(0.0, 1.0, 0.0)
        };
        Self::new(
            axis,
            hint,
            ScalarDist::Uniform {
                lo: half_angle_rad.cos(),
                hi: 1.0,
            },
            ScalarDist::Uniform {
                lo: 0.0,
                hi: std::f64::consts::TAU,
            },
        )
    }
}

impl AngleDist for PolarAzimuthalAngle {
    fn sample(&self, seed: &mut u64, _e: f64) -> Direction {
        let mu = self.mu.sample(seed).clamp(-1.0, 1.0);
        let phi = self.phi.sample(seed);
        // Upstream `:111-114`: the poles return the axis directly.
        if mu == 1.0 {
            return self.u_ref;
        }
        if mu == -1.0 {
            return Direction::new(-self.u_ref.u, -self.u_ref.v, -self.u_ref.w);
        }
        let f = (1.0 - mu * mu).max(0.0).sqrt();
        let (s, c) = phi.sin_cos();
        Direction::new(
            mu * self.u_ref.u + f * c * self.v_ref.u + f * s * self.w_ref.u,
            mu * self.u_ref.v + f * c * self.v_ref.v + f * s * self.w_ref.v,
            mu * self.u_ref.w + f * c * self.v_ref.w + f * s * self.w_ref.w,
        )
    }
}
