// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//   Source: bibfor/fracture/chauxi.F90 (near-tip auxiliary fields),
//           bibfor/fracture/cakg2d.F90 (2-D K/G post-processing, symmetry and
//           axisymmetric corrections, `G_IRWIN`),
//           bibfor/fracture/calcG_type.F90 (`G_IRWIN` from the per-mode roots),
//           bibfor/fracture/gkmet1.F90, gkmet3.F90 (the crack-kink angle
//           `beta`, present upstream only as commented-out code),
//           bibfor/fracture/plegen.F90, dplegen.F90 (crack-front Legendre
//           basis and its derivative),
//           bibfor/fracture/hatSmooth.F90 (crack-front hat smoothing)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Linear-elastic fracture mechanics: the parts of code_aster's `CALC_G` that
//! are *not* finite-element work.
//!
//! # Read this first: most of `bibfor/fracture` is **not** ported, and cannot be
//!
//! code_aster computes the energy release rate `G` and the stress intensity
//! factors `K_I, K_II, K_III` by the **G-theta method**: a domain integral of a
//! bilinear form over a ring of finite elements surrounding the crack front,
//! driven by a *virtual crack extension field* `theta`. Evaluating that integral
//! needs, at minimum:
//!
//! - element shape functions and their derivatives on the reference element,
//! - a Gauss quadrature rule per element type,
//! - a mesh carrying a named crack-front node group and its curvilinear
//!   abscissae,
//! - assembled nodal displacement, stress and internal-variable fields,
//! - a solver-side "compute this option over this element group and sum the
//!   elementary results" driver (upstream's `calcul` / `mesomm`).
//!
//! This crate has **none** of that for solid mechanics at the required
//! generality, so the 72-file `bibfor/fracture` directory is, today, mostly
//! unportable. Rather than produce a module that looks like G-theta and computes
//! nothing, this file ports only the subset that is genuinely closed-form
//! algebra, and the module-level report below states precisely what is blocked
//! and on what.
//!
//! # What *is* here (portable now, and verified)
//!
//! | Item | Upstream | What it is |
//! |---|---|---|
//! | [`CrackPlaneState`], [`LinearElasticConstants`] | `chauxi.F90` (`ka`, `mu`) | Kolosov `kappa` and the effective modulus `E'`, per stress state |
//! | [`irwin_energy_release_rate`], [`ModeEnergyRelease`] | `calcG_type.F90::addValues`, `cakg2d.F90` | Irwin's `G <-> K` relation and the mixed-mode sum |
//! | [`westergaard_unit_field`], [`NearTipField`] | `chauxi.F90` | Williams/Westergaard near-tip displacement fields and their gradients |
//! | [`near_tip_stress`] | (Hooke applied to the above) | the singular stress field, used as the verification oracle |
//! | [`max_hoop_stress_kink_angle`] | `gkmet1.F90`, `gkmet3.F90` (commented out) | the Erdogan-Sih maximum-hoop-stress kink angle |
//! | [`CrackTipBasis`] | `cakg2d.F90` (the 90-degree rotation), `chauxi.F90` (the `invp` transform) | local crack-tip frame and local/global rotation |
//! | [`PlanarCrackTipResult`] | `cakg2d.F90` lines 471-493 | the symmetry and axisymmetric corrections applied to a summed 2-D result |
//! | [`legendre_front_mode`], [`legendre_front_mode_derivative`] | `plegen.F90`, `dplegen.F90` | the `L2`-orthonormal Legendre basis along the crack front |
//! | [`hat_smooth_front`] | `hatSmooth.F90` | quadratic-front hat smoothing of `G(s)` or `K(s)` |
//!
//! Everything above is checked against a closed-form reference — the Williams
//! singular field, Irwin's relation on a centre-cracked infinite plate, the
//! Legendre three-term recurrence and orthonormality, and (for the kink angle) a
//! numerical stationary-point search on the hoop stress using this port's own
//! [`brent`](super::integration::brent) solver.
//!
//! # What is blocked, and on what
//!
//! Classified by reading all 72 files of `bibfor/fracture` at the pinned commit.
//! "JEVEUX-free" below means the file makes no call to upstream's memory manager
//! (`jeveuo`/`wkvect`/`jemarq`) and no call to the element driver (`calcul`).
//!
//! **1. The G-theta domain integral itself — blocked on a solid-mechanics FE
//! framework.** `cgComputeGtheta.F90` (734 lines), `calcG_type.F90` (1953),
//! `cakg2d.F90` (537), `cakg3d.F90` (559), `mecalg.F90`, `mecagl.F90`. These are
//! drivers: they assemble field names, call `calcul` to run an element option
//! over a `LIGREL`, and sum the elementary results with `mesomm`. There is no
//! physics in them that survives removing the framework.
//!
//! **2. The per-Gauss-point G-theta integrand — portable in form, blocked on
//! verification.** `gbilin.F90` (321 lines, 2-D) and `gbil3d.F90` (400 lines,
//! 3-D) are the one real surprise: `gbil3d.F90` is entirely JEVEUX-free and
//! `gbilin.F90` touches it only for the material lookup. Both are pure algebra
//! once you supply the four gradient matrices `dudm`, `dvdm`, `dtdm`, `dfdm` and
//! the elastic constants — they compute the classical term, the thermal term,
//! the body-force term, the dynamic term and three initial-stress terms, and
//! return a scalar. They are **deliberately not ported**: their only meaningful
//! test is that the ring integral of the kernel reproduces a known `G`, and that
//! test needs the quadrature this crate does not have. Porting them now would
//! add ~700 lines of untested transcription — exactly the "plausible-looking
//! module that computes nothing" this port is trying to avoid.
//!
//! **3. Theta-field construction — blocked on mesh topology.** `gcour2.F90`
//! (436), `gcour3.F90` (366), `gcou2d.F90`, `gcharf.F90`, `gcharg.F90`,
//! `gcharm.F90`, `cgComputeLayers.F90`, `cgDiscrField.F90`, `thetapdg.F90`,
//! `xcourb.F90`. These build the virtual crack-extension field by walking rings
//! of elements outward from the crack front and interpolating a radial profile.
//! They need element connectivity, node coordinates, and a crack-front node
//! group.
//!
//! **4. Crack-front smoothing systems — blocked on the front discretisation.**
//! `gkmet1.F90`, `gkmet3.F90`, `gmeth1/2/3.F90`, `gmatr1.F90`, `gmatr2.F90`,
//! `gmatc3.F90`, `gmate3.F90`, `gmatl3.F90`, `gsyste.F90`. The *basis functions*
//! these use are ported (see [`legendre_front_mode`]); the Gram matrices they
//! assemble are 1-D integrals over the crack-front segments, so they are blocked
//! only on having a crack front — a much smaller dependency than (1)-(3), and
//! the natural second phase.
//!
//! **5. Command-language plumbing — out of scope permanently.** `cglect.F90`,
//! `cglecc.F90`, `cgleco.F90`, `cgcrio.F90`, `cgcrtb.F90`, `cgtyfi.F90`,
//! `cgvcmo.F90`, `cgvein.F90`, `cgvemf.F90`, `cgverc.F90`, `cgverho.F90`,
//! `cgVerification.F90`, `cgReadCompor.F90`, `cgComporNodes.F90`,
//! `cgTempNodes.F90`, `cgCreateCompIncr.F90`, `cgExportTableG.F90`,
//! `cgcrio.F90`, `gcsele.F90`, `gcfonc.F90`, `gcchar.F90`, `gchfus.F90`,
//! `gchs2f.F90`, `medomg.F90`, `mefor0.F90`, `mepres.F90`, `gverfo.F90`,
//! `gver2d.F90`, `gveri3.F90`, `gverlc.F90`, `foninf2.F90`, `gimpgs.F90`,
//! `gksimp.F90`. Keyword parsing, `.comm` deck validation, JEVEUX table writing
//! and formatted printing. These reproduce code_aster's *user interface*, not
//! its physics; this workspace has its own.
//!
//! **6. Elastoplastic free energy for `G` — blocked on the material catalogue.**
//! `nmplru.F90` (216 lines) computes the free-energy density and its temperature
//! derivative for the plastic `G`. Its algebra is portable, but it is driven by
//! upstream's tabulated traction curve (`rctrac`/`rcfonc`) and material-field
//! lookups. Deferred to the point where this port has a hardening-curve
//! abstraction.
//!
//! # The smallest FE capability that would unblock the rest
//!
//! In dependency order, smallest first:
//!
//! 1. **A crack front as data** — an ordered list of front points with
//!    curvilinear abscissae `s` and a local basis per point. This alone unblocks
//!    group (4): the Legendre and Lagrange smoothing systems (`gmatr1`,
//!    `gmatr2`, `gsyste`) become 1-D quadratures over front segments, needing
//!    only `SE2`/`SE3` shape functions and a Gauss rule on `[-1, 1]`.
//! 2. **Gauss quadrature plus isoparametric shape functions and Jacobians on
//!    solid elements** (upstream's `elrfvf`/`elrfdf`/`nmgeom`), together with a
//!    displacement field sampled at Gauss points. With that, `gbilin`/`gbil3d`
//!    become both portable *and* testable, because the ring integral of the
//!    kernel over a Westergaard displacement field must return `K^2 / E'`.
//! 3. **Element-ring topology around the front** — "give me the elements whose
//!    distance from the front is in `[R_inf, R_sup]`". That unblocks group (3),
//!    the theta field.
//!
//! Only (1) and (2) are needed for a first working `G`. Item (2) is the real
//! cost, and it is a general finite-element capability, not a fracture one.
//!
//! # Provenance and honesty notes
//!
//! - The read-only upstream clone used here is a **partial** one: it carries
//!   `bibfor/{fracture,comport,comport_prep,lc,metallurgy,algorith,nonlinear,
//!   modelisa,utilitai,utilifor,include}` but **not** `bibfor/te` (the element
//!   routines), `catalo/`, `code_aster/Commands/` or `astest/`. Consequences:
//!   the element routine that fills `chauxi`'s `ka` argument per modelisation is
//!   **not visible**, and neither is the regression suite. So the mapping from
//!   `D_PLAN` / `C_PLAN` / `AXIS` to Kolosov's `kappa` in [`CrackPlaneState`] is
//!   taken from the standard result (Kolosov/Muskhelishvili), corroborated by
//!   upstream's own inverse `nu = (3 - ka)/4` in `chauxi.F90`, and **verified
//!   here** by checking that the near-tip stress it produces is the same
//!   `1/sqrt(2 pi r)` singularity in plane strain and in plane stress — a check
//!   that fails if `kappa` and the plane Lame constant are mismatched.
//! - The kink-angle formula in [`max_hoop_stress_kink_angle`] exists upstream
//!   **only as commented-out code** in `gkmet1.F90` and `gkmet3.F90`. It is
//!   ported because it is the criterion those files were reaching for and it has
//!   a clean analytical reference, but a reader should know it is not live
//!   upstream.
//! - **Paris-law fatigue crack growth is not ported, because it is not in the
//!   upstream source.** A search of the whole clone for `PARIS`, `LOI_PROPA`,
//!   `DELTA_K_SEUIL` and `PROPA_FISS` returns nothing in any Fortran or Python
//!   file. Upstream's crack-advance law lives in the command layer, which this
//!   clone does not carry. Writing one here would have been invention dressed as
//!   a port.
//!
//! # Units
//!
//! Raw `f64` throughout, matching the rest of [`super`]. SI: lengths in metres,
//! stresses and moduli in pascals, energy release rate `G` in J/m^2 (= Pa m),
//! stress intensity factors in Pa m^(1/2), angles in radians. Poisson's ratio,
//! Kolosov's `kappa` and the Legendre abscissa ratio are dimensionless.

use outram_foam_basic_lib::primitives::{SymmTensor, Tensor, Vector3};

use crate::error::{OffbeatError, Result};
use crate::rheology::aster::kinematics::AsterVoigt;

// =====================================================================
// Stress state and elastic constants
// =====================================================================

/// Which two-dimensional idealisation the crack-tip field is evaluated under.
///
/// # Why this is a separate type and not a boolean
///
/// The plane state changes *two* constants at once — Kolosov's `kappa` and the
/// effective modulus `E'` — and getting one right while getting the other wrong
/// produces a near-tip field that still looks singular and is still smooth, but
/// carries the wrong energy. Requiring the caller to name the state at every
/// entry point is deliberate.
///
/// # The mapping
///
/// | Variant | code_aster `MODELISATION` | `kappa` | `E'` |
/// |---|---|---|---|
/// | [`PlaneStrain`](Self::PlaneStrain) | `D_PLAN` | `3 - 4 nu` | `E / (1 - nu^2)` |
/// | [`PlaneStress`](Self::PlaneStress) | `C_PLAN` | `(3 - nu) / (1 + nu)` | `E` |
/// | [`Axisymmetric`](Self::Axisymmetric) | `AXIS` | `3 - 4 nu` | `E / (1 - nu^2)` |
/// | [`ThreeDimensional`](Self::ThreeDimensional) | `3D` | `3 - 4 nu` | `E / (1 - nu^2)` |
///
/// Axisymmetric and three-dimensional both behave as plane strain here, and for
/// the same reason: a crack front constrains the material in the plane normal to
/// itself, so the asymptotic field in that plane is a plane-strain one. That is
/// an *asymptotic* statement about the crack-front neighbourhood, not a claim
/// that the whole body is in plane strain. Near a free surface, where the true
/// state relaxes towards plane stress, it is wrong — a limitation, documented
/// rather than hidden.
///
/// # Units
///
/// Dimensionless selector. `kappa` is dimensionless; `E'` is in pascals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrackPlaneState {
    /// Plane strain (`D_PLAN`): the out-of-plane strain vanishes, the
    /// out-of-plane stress does not.
    PlaneStrain,
    /// Plane stress (`C_PLAN`): the out-of-plane stress vanishes, the
    /// out-of-plane strain does not. The thin-sheet idealisation.
    PlaneStress,
    /// Axisymmetric (`AXIS`): a circumferential crack in a body of revolution.
    /// Plane-strain-like in the meridian plane.
    Axisymmetric,
    /// Fully three-dimensional (`3D`), evaluated in the plane normal to the
    /// crack front. Plane-strain-like there.
    ThreeDimensional,
}

impl CrackPlaneState {
    /// The upstream `MODELISATION` token this corresponds to.
    ///
    /// Preserved verbatim so a code_aster user can find the matching deck
    /// keyword, per the naming convention in `docs/code-aster-port-scoping.md`
    /// section 4.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        match self {
            Self::PlaneStrain => "D_PLAN",
            Self::PlaneStress => "C_PLAN",
            Self::Axisymmetric => "AXIS",
            Self::ThreeDimensional => "3D",
        }
    }

    /// Whether the state behaves as plane strain for `kappa` and `E'`.
    ///
    /// True for everything except [`PlaneStress`](Self::PlaneStress). See the
    /// type documentation for why axisymmetric and 3-D fall on this side.
    #[must_use]
    pub const fn is_plane_strain_like(self) -> bool {
        !matches!(self, Self::PlaneStress)
    }
}

/// Isotropic linear-elastic constants at the crack tip.
///
/// # What it holds
///
/// Young's modulus `E` in pascals and Poisson's ratio `nu`, dimensionless.
/// Everything the near-tip field needs — the shear modulus `mu`, the plane Lame
/// constant, Kolosov's `kappa`, the effective modulus `E'` — is derived from
/// these two, so there is no way for a caller to supply an inconsistent set.
///
/// # Valid range
///
/// `E > 0` and `-1 < nu < 0.5`. The upper bound excludes incompressibility,
/// where the plane-strain Lame constant diverges; the lower bound is the
/// thermodynamic limit for an isotropic solid. Both are enforced by
/// [`new`](Self::new).
///
/// # Units
///
/// `young` in pascals (Pa); `poisson` dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearElasticConstants {
    /// Young's modulus `E`, in pascals.
    pub young: f64,
    /// Poisson's ratio `nu`, dimensionless.
    pub poisson: f64,
}

impl LinearElasticConstants {
    /// Build and validate a pair of isotropic elastic constants.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if `young <= 0`, or if `poisson` is outside
    /// the open interval `(-1, 0.5)`. `nu = 0.5` is rejected rather than
    /// clamped: at that value the plane-strain Lame constant is infinite and
    /// every quantity downstream is meaningless, so failing loudly is the honest
    /// behaviour.
    pub fn new(young: f64, poisson: f64) -> Result<Self> {
        if !(young > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "Young's modulus",
                value: young,
                unit: "Pa",
                reason: "must be strictly positive",
            });
        }
        if !(poisson > -1.0 && poisson < 0.5) {
            return Err(OffbeatError::Unphysical {
                quantity: "Poisson's ratio",
                value: poisson,
                unit: "-",
                reason: "must lie in the open interval (-1, 0.5); at 0.5 the \
                         plane-strain Lame constant is infinite",
            });
        }
        Ok(Self { young, poisson })
    }

    /// The shear modulus `mu = E / (2 (1 + nu))`, in pascals.
    ///
    /// Upstream's `mu` argument to `chauxi`. Independent of the plane state.
    #[must_use]
    pub fn shear_modulus(self) -> f64 {
        self.young / (2.0 * (1.0 + self.poisson))
    }

    /// Kolosov's constant `kappa`, dimensionless.
    ///
    /// `3 - 4 nu` in plane strain (and axisymmetric, and 3-D at the front);
    /// `(3 - nu) / (1 + nu)` in plane stress. This is upstream's `ka` argument
    /// to `chauxi.F90`, whose curved-crack branch inverts it as
    /// `nu = (3 - ka) / 4` — i.e. upstream itself takes the plane-strain form as
    /// the definition there.
    ///
    /// # Range
    ///
    /// For `nu` in `(-1, 0.5)`: `kappa` runs over `(1, 7)` in plane strain and
    /// over `(1, 5/3)` in plane stress. It is never negative, and `kappa = 1` is
    /// the incompressible plane-strain limit.
    #[must_use]
    pub fn kolosov_kappa(self, state: CrackPlaneState) -> f64 {
        if state.is_plane_strain_like() {
            3.0 - 4.0 * self.poisson
        } else {
            (3.0 - self.poisson) / (1.0 + self.poisson)
        }
    }

    /// The effective modulus `E'` appearing in Irwin's relation, in pascals.
    ///
    /// `E / (1 - nu^2)` in plane strain, `E` in plane stress. **This is the
    /// single most common sign-of-error in fracture post-processing**: for
    /// `nu = 0.3` the two differ by 9.9%, which is small enough to look like a
    /// mesh effect and large enough to invalidate a comparison.
    ///
    /// It appears in `G_I = K_I^2 / E'` and, inverted, in `K_J = sqrt(G E')`.
    #[must_use]
    pub fn effective_modulus(self, state: CrackPlaneState) -> f64 {
        if state.is_plane_strain_like() {
            self.young / (1.0 - self.poisson * self.poisson)
        } else {
            self.young
        }
    }

    /// The Lame constant `lambda` to use in the **two-dimensional** Hooke's law
    /// `sigma_ij = lambda tr(eps_2D) delta_ij + 2 mu eps_ij`, in pascals.
    ///
    /// Plane strain gives the true three-dimensional `lambda = E nu /
    /// ((1 + nu)(1 - 2 nu))`. Plane stress gives the *reduced* constant
    /// `lambda* = E nu / (1 - nu^2) = 2 lambda mu / (lambda + 2 mu)`, which is
    /// what you get after eliminating the out-of-plane strain from the
    /// three-dimensional law under `sigma_zz = 0`.
    ///
    /// Using the unreduced `lambda` in a plane-stress calculation is the
    /// companion error to using the wrong `kappa`, and the two conveniently
    /// cancel in the singular stress amplitude while leaving the displacement
    /// field wrong — which is why the verification here checks the stress
    /// *and* the crack-opening displacement.
    #[must_use]
    pub fn plane_lame_lambda(self, state: CrackPlaneState) -> f64 {
        let (e, nu) = (self.young, self.poisson);
        if state.is_plane_strain_like() {
            e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu))
        } else {
            e * nu / (1.0 - nu * nu)
        }
    }
}

// =====================================================================
// Irwin's relation
// =====================================================================

/// The three stress intensity factors at a point on the crack front.
///
/// # What they mean
///
/// The amplitudes of the three independent singular modes of the Williams
/// expansion: opening (`I`), in-plane shear (`II`) and anti-plane shear (`III`).
/// Each multiplies a universal angular field, so a single number per mode
/// characterises the whole near-tip state.
///
/// # Units
///
/// Pa m^(1/2) — pascals times the square root of a metre. In two dimensions
/// `k3` is identically zero and is carried only so one type serves both cases.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StressIntensityFactors {
    /// Mode I (opening), in Pa m^(1/2).
    pub k1: f64,
    /// Mode II (in-plane sliding shear), in Pa m^(1/2).
    pub k2: f64,
    /// Mode III (anti-plane tearing shear), in Pa m^(1/2). Zero in 2-D.
    pub k3: f64,
}

impl StressIntensityFactors {
    /// A pure mode-I state of amplitude `k1` (Pa m^(1/2)).
    #[must_use]
    pub const fn mode_i(k1: f64) -> Self {
        Self {
            k1,
            k2: 0.0,
            k3: 0.0,
        }
    }

    /// A general state. All three amplitudes in Pa m^(1/2).
    #[must_use]
    pub const fn new(k1: f64, k2: f64, k3: f64) -> Self {
        Self { k1, k2, k3 }
    }
}

/// The energy release rate split by mode, plus the total.
///
/// # What it means
///
/// Under linear elasticity the three modes contribute *additively* to the energy
/// release rate — there is no cross term, because the three angular fields are
/// orthogonal over a circuit around the tip. That additivity is exactly what
/// upstream relies on when it forms `G_IRWIN` as a sum of squares in
/// `calcG_type.F90::addValues` and `gkmet1.F90`.
///
/// # Units
///
/// All four fields in J/m^2 (equivalently Pa m).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModeEnergyRelease {
    /// Mode-I contribution `K_I^2 / E'`, in J/m^2.
    pub mode_i: f64,
    /// Mode-II contribution `K_II^2 / E'`, in J/m^2.
    pub mode_ii: f64,
    /// Mode-III contribution `K_III^2 / (2 mu)`, in J/m^2.
    pub mode_iii: f64,
    /// The sum of the three, in J/m^2.
    pub total: f64,
}

/// Split the energy release rate by mode via Irwin's relation.
///
/// # What it computes
///
/// ```text
/// G_I   = K_I^2   / E'
/// G_II  = K_II^2  / E'
/// G_III = K_III^2 / (2 mu)
/// G     = G_I + G_II + G_III
/// ```
///
/// `E'` is [`LinearElasticConstants::effective_modulus`] — `E / (1 - nu^2)` in
/// plane strain (and 3-D at the front), `E` in plane stress. The mode-III factor
/// is `(1 + nu) / E = 1 / (2 mu)` and is **the same in every plane state**,
/// because anti-plane shear is a scalar Laplace problem that never sees the
/// in-plane constraint. Applying the plane-state factor to mode III is a
/// plausible-looking error worth naming.
///
/// # Assumptions
///
/// Linear elastic, isotropic, homogeneous material; small-scale yielding; a
/// straight crack front with a self-similar advance. Outside those, `G` from
/// `K` and `G` from the domain integral part company, and the difference is
/// itself diagnostic — which is why upstream reports both `G` and `G_IRWIN` and
/// leaves the comparison to the user.
///
/// # Units
///
/// `k` in Pa m^(1/2), `elastic.young` in Pa; the result in J/m^2.
#[must_use]
pub fn irwin_mode_split(
    k: StressIntensityFactors,
    elastic: LinearElasticConstants,
    state: CrackPlaneState,
) -> ModeEnergyRelease {
    let e_eff = elastic.effective_modulus(state);
    let two_mu = 2.0 * elastic.shear_modulus();

    let mode_i = k.k1 * k.k1 / e_eff;
    let mode_ii = k.k2 * k.k2 / e_eff;
    let mode_iii = k.k3 * k.k3 / two_mu;

    ModeEnergyRelease {
        mode_i,
        mode_ii,
        mode_iii,
        total: mode_i + mode_ii + mode_iii,
    }
}

/// The total energy release rate from the three stress intensity factors.
///
/// A convenience over [`irwin_mode_split`] when only the total is wanted. This
/// is the quantity upstream tabulates as `G_IRWIN`, formed there as a sum of
/// squares of per-mode roots (`calcG_type.F90` line 1599, `cakg2d.F90` line
/// 493).
///
/// # Units
///
/// `k` in Pa m^(1/2), result in J/m^2. See [`irwin_mode_split`] for the
/// assumptions.
#[must_use]
pub fn irwin_energy_release_rate(
    k: StressIntensityFactors,
    elastic: LinearElasticConstants,
    state: CrackPlaneState,
) -> f64 {
    irwin_mode_split(k, elastic, state).total
}

/// The equivalent mode-I stress intensity factor of an energy release rate.
///
/// # What it computes
///
/// `K_eq = sqrt(G E')`. This is upstream's `KJ` output, formed in
/// `calcG_type.F90::addValues` as `sqrt(gth(2))` after the element has already
/// multiplied by `E'`. It answers: *what pure mode-I loading would release
/// energy at this rate?* — the standard way to compare a mixed-mode or
/// elastic-plastic result against a mode-I toughness `K_Ic`.
///
/// A negative `G` is returned as zero, matching upstream's guard
/// (`if (gth(2) >= 0) ... else 0`). A negative energy release rate is not
/// physical; it arises numerically when the domain integral is evaluated on a
/// ring too small or too distorted to resolve the field, and upstream chose to
/// clip rather than fail. That behaviour is **reproduced, not corrected**.
///
/// # Units
///
/// `g` in J/m^2, `elastic.young` in Pa; result in Pa m^(1/2).
#[must_use]
pub fn equivalent_mode_i_factor(
    g: f64,
    elastic: LinearElasticConstants,
    state: CrackPlaneState,
) -> f64 {
    if g >= 0.0 {
        (g * elastic.effective_modulus(state)).sqrt()
    } else {
        0.0
    }
}

// =====================================================================
// Near-tip (Williams / Westergaard) fields — chauxi.F90
// =====================================================================

/// Which singular crack-tip mode a near-tip field belongs to.
///
/// Enum dispatch, not trait objects, per the workspace rule: the set of
/// crack-opening modes is closed by elasticity itself and cannot grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrackOpeningMode {
    /// Mode I — opening. Upstream's auxiliary field `u1`.
    Opening,
    /// Mode II — in-plane sliding shear. Upstream's `u2`.
    InPlaneShear,
    /// Mode III — anti-plane tearing shear. Upstream's `u3`.
    AntiPlaneShear,
}

impl CrackOpeningMode {
    /// The mode number, 1, 2 or 3, matching the `K1`/`K2`/`K3` table columns
    /// upstream writes.
    #[must_use]
    pub const fn number(self) -> usize {
        match self {
            Self::Opening => 1,
            Self::InPlaneShear => 2,
            Self::AntiPlaneShear => 3,
        }
    }
}

/// A near-tip displacement field and its gradient, in the local crack-tip basis.
///
/// # Frame
///
/// Both members are expressed in the **local crack-tip basis**: `x` along the
/// crack-propagation direction (ahead of the tip), `y` normal to the crack
/// plane, `z` along the crack front. Use [`CrackTipBasis`] to rotate into the
/// global frame.
///
/// # Units
///
/// `displacement` in metres per unit stress intensity factor, i.e. m /
/// (Pa m^(1/2)) = m^(1/2) / Pa. `gradient` is that per metre, i.e.
/// 1 / (Pa m^(1/2)). Multiply by a `K` in Pa m^(1/2) with
/// [`scaled`](Self::scaled) to get metres and a dimensionless gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearTipField {
    /// Displacement `u`, local basis.
    pub displacement: Vector3,
    /// Displacement gradient `du_i / dx_j` (row `i`, column `j`), local basis.
    pub gradient: Tensor,
}

impl NearTipField {
    /// Scale a unit-`K` field by an actual stress intensity factor.
    ///
    /// The near-tip fields are linear in `K`, so this is the whole of the
    /// scaling law. `k` in Pa m^(1/2); the result carries metres and a
    /// dimensionless gradient.
    #[must_use]
    pub fn scaled(self, k: f64) -> Self {
        Self {
            displacement: Vector3::new(
                self.displacement.x * k,
                self.displacement.y * k,
                self.displacement.z * k,
            ),
            gradient: self.gradient * k,
        }
    }

    /// The small-strain tensor `eps = (grad u + grad u^T) / 2` of this field.
    ///
    /// Dimensionless once the field has been [`scaled`](Self::scaled) by a `K`.
    #[must_use]
    pub fn small_strain(self) -> SymmTensor {
        self.gradient.symm()
    }

    /// The small-strain tensor in code_aster's Mandel six-vector convention.
    ///
    /// This is the layout upstream's `gbilin.F90` builds by hand as
    /// `epsu(4) = 0.5*(dudm(1,2)+dudm(2,1))*rac2` — the `sqrt(2)` on the shear
    /// entries that makes the six-vector dot product equal the tensor double
    /// contraction. Provided so a future port of the G-theta integrand consumes
    /// the same convention rather than re-deriving it; see [`AsterVoigt`] for
    /// why the scaling is not optional.
    #[must_use]
    pub fn small_strain_mandel(self) -> AsterVoigt {
        AsterVoigt::from_tensor(self.small_strain())
    }
}

/// The unit-`K` near-tip displacement field and its gradient — a port of
/// `chauxi.F90`.
///
/// # What it computes
///
/// The leading (`r^(1/2)`) term of the Williams expansion, normalised so that
/// the field corresponds to a stress intensity factor of exactly 1 Pa m^(1/2)
/// in the requested mode. With `mu` the shear modulus and `kappa` Kolosov's
/// constant:
///
/// `u_x = (1 / (2 mu)) sqrt(r / (2 pi)) cos(t/2) (kappa - cos t)` (mode I)
///
/// `u_y = (1 / (2 mu)) sqrt(r / (2 pi)) sin(t/2) (kappa - cos t)` (mode I)
///
/// `u_x = (1 / (2 mu)) sqrt(r / (2 pi)) sin(t/2) (kappa + 2 + cos t)` (mode II)
///
/// `u_y = (1 / (2 mu)) sqrt(r / (2 pi)) cos(t/2) (2 - kappa - cos t)` (mode II)
///
/// `u_z = (2 / mu) sqrt(r / (2 pi)) sin(t/2)` (mode III)
///
/// transcribed from upstream's `u1l`, `u2l`, `u3l` with upstream's own
/// coefficients `cr1 = 1/(4 mu sqrt(2 pi r))` and `cr2 = sqrt(r/(2 pi))/(2 mu)`.
/// The gradient is upstream's `du#dl`: the polar derivatives converted to local
/// Cartesian components by
/// `d/dx = cos(t) d/dr - (sin(t)/r) d/dt`, `d/dy = sin(t) d/dr + (cos(t)/r) d/dt`.
///
/// # Coordinates
///
/// `r` is the distance from the crack tip in metres, strictly positive — the
/// field is singular at `r = 0` and that is the point of it. `theta` is the
/// angle in radians measured from the crack-propagation direction, with the
/// crack faces at `theta = +/- pi`. The field is *not* periodic in `theta`: it
/// changes sign across `theta = pi`, which is the branch cut representing the
/// crack itself, so passing `theta` outside `[-pi, pi]` is meaningless and
/// rejected.
///
/// # What is deliberately not ported
///
/// Upstream's optional `r_courb` argument adds a higher-order correction for a
/// *curved* crack front (the `A1..D1`, `A2..D2` coefficient block). It is left
/// out: it is a `O(r^(3/2))` correction with no closed-form reference available
/// in this clone to verify it against, and transcribing 60 lines of untested
/// trigonometry would add risk without adding capability. The straight-front
/// leading term is exact and is what the verification below pins.
///
/// # Errors
///
/// [`OffbeatError::Unphysical`] if `r <= 0` (the field is singular there) or if
/// `theta` is outside `[-pi, pi]`.
///
/// # Units
///
/// `r` in metres, `theta` in radians. The returned displacement is in
/// m^(1/2)/Pa and the gradient in 1/(Pa m^(1/2)) — per unit `K`. Multiply by a
/// `K` in Pa m^(1/2) with [`NearTipField::scaled`].
pub fn westergaard_unit_field(
    mode: CrackOpeningMode,
    r: f64,
    theta: f64,
    elastic: LinearElasticConstants,
    state: CrackPlaneState,
) -> Result<NearTipField> {
    if !(r > 0.0) {
        return Err(OffbeatError::Unphysical {
            quantity: "crack-tip radial distance r",
            value: r,
            unit: "m",
            reason: "must be strictly positive; the Williams field is singular \
                     at the tip itself",
        });
    }
    if !(theta.abs() <= std::f64::consts::PI + 1.0e-12) {
        return Err(OffbeatError::Unphysical {
            quantity: "crack-tip polar angle theta",
            value: theta,
            unit: "rad",
            reason: "must lie in [-pi, pi]; the crack faces are the branch cut \
                     at theta = +/- pi and the field is not periodic across it",
        });
    }

    let mu = elastic.shear_modulus();
    let ka = elastic.kolosov_kappa(state);
    let two_pi = std::f64::consts::TAU;

    // Upstream's `cr1` and `cr2` (chauxi.F90 lines 60-61). `cr1` is exactly
    // `cr2 / (2 r)`, which is why it serves as the radial derivative factor.
    let cr1 = 1.0 / (4.0 * mu * (two_pi * r).sqrt());
    let cr2 = (r / two_pi).sqrt() / (2.0 * mu);

    let (c_half, s_half) = ((0.5 * theta).cos(), (0.5 * theta).sin());
    let (c_full, s_full) = (theta.cos(), theta.sin());

    // Displacement, and the polar derivatives (d/dr, d/dtheta), by mode.
    let (u, du_dr, du_dtheta) = match mode {
        CrackOpeningMode::Opening => (
            Vector3::new(cr2 * c_half * (ka - c_full), cr2 * s_half * (ka - c_full), 0.0),
            Vector3::new(cr1 * c_half * (ka - c_full), cr1 * s_half * (ka - c_full), 0.0),
            Vector3::new(
                cr2 * (-0.5 * s_half * (ka - c_full) + c_half * s_full),
                cr2 * (0.5 * c_half * (ka - c_full) + s_half * s_full),
                0.0,
            ),
        ),
        CrackOpeningMode::InPlaneShear => (
            Vector3::new(
                cr2 * s_half * (ka + 2.0 + c_full),
                cr2 * c_half * (2.0 - ka - c_full),
                0.0,
            ),
            Vector3::new(
                cr1 * s_half * (ka + 2.0 + c_full),
                cr1 * c_half * (2.0 - ka - c_full),
                0.0,
            ),
            Vector3::new(
                cr2 * (0.5 * c_half * (ka + 2.0 + c_full) - s_half * s_full),
                cr2 * (-0.5 * s_half * (2.0 - ka - c_full) + c_half * s_full),
                0.0,
            ),
        ),
        CrackOpeningMode::AntiPlaneShear => (
            Vector3::new(0.0, 0.0, 4.0 * cr2 * s_half),
            Vector3::new(0.0, 0.0, 4.0 * cr1 * s_half),
            Vector3::new(0.0, 0.0, 2.0 * cr2 * c_half),
        ),
    };

    // Upstream's polar-to-local-Cartesian conversion (chauxi.F90 `du#dl`).
    let gradient = polar_gradient_to_cartesian(du_dr, du_dtheta, r, theta);

    Ok(NearTipField {
        displacement: u,
        gradient,
    })
}

/// Convert `(du/dr, du/dtheta)` into the local Cartesian gradient `du_i/dx_j`.
///
/// This is upstream's `du#dl` block, common to all three modes:
/// `d/dx = cos(t) d/dr - (sin(t)/r) d/dt` and
/// `d/dy = sin(t) d/dr + (cos(t)/r) d/dt`, with the `z` column zero because the
/// leading Williams term is independent of position along the front.
fn polar_gradient_to_cartesian(
    du_dr: Vector3,
    du_dtheta: Vector3,
    r: f64,
    theta: f64,
) -> Tensor {
    let (c, s) = (theta.cos(), theta.sin());
    let dx = |dr: f64, dt: f64| c * dr - s / r * dt;
    let dy = |dr: f64, dt: f64| s * dr + c / r * dt;

    Tensor::new(
        dx(du_dr.x, du_dtheta.x),
        dy(du_dr.x, du_dtheta.x),
        0.0,
        dx(du_dr.y, du_dtheta.y),
        dy(du_dr.y, du_dtheta.y),
        0.0,
        dx(du_dr.z, du_dtheta.z),
        dy(du_dr.z, du_dtheta.z),
        0.0,
    )
}

/// The Cauchy stress of a near-tip field, by isotropic Hooke's law.
///
/// # What it computes
///
/// `sigma_ij = lambda_plane tr(eps_2D) delta_ij + 2 mu eps_ij` for the in-plane
/// components, with `lambda_plane` from
/// [`LinearElasticConstants::plane_lame_lambda`]. The out-of-plane components
/// follow the plane state:
///
/// - plane strain (and axisymmetric, and 3-D at the front):
///   `sigma_zz = lambda tr(eps_2D)`, the reaction that enforces `eps_zz = 0`;
/// - plane stress: `sigma_zz = 0` by definition.
///
/// The anti-plane shears `sigma_xz` and `sigma_yz` are `2 mu eps_xz` and
/// `2 mu eps_yz` in every state, because mode III does not couple to the
/// in-plane constraint.
///
/// # Why this exists
///
/// Not because a caller needs it — because it is the *verification oracle*.
/// Applying Hooke to the displacement field must return the Williams singular
/// stress `sigma_yy = K_I / sqrt(2 pi r)` on the crack plane ahead of the tip,
/// and that value is **independent of the plane state**. So the test that
/// `near_tip_stress` gives the same singularity in plane strain and plane stress
/// is a direct check that `kappa` and `lambda_plane` are mutually consistent —
/// the check the missing element routines would otherwise have to supply.
///
/// # Units
///
/// Input field per unit `K` gives a stress in 1/m^(1/2) — multiply the field by
/// a `K` in Pa m^(1/2) first (via [`NearTipField::scaled`]) to get pascals.
#[must_use]
pub fn near_tip_stress(
    field: NearTipField,
    elastic: LinearElasticConstants,
    state: CrackPlaneState,
) -> SymmTensor {
    let eps = field.small_strain();
    let mu = elastic.shear_modulus();
    let lambda = elastic.plane_lame_lambda(state);
    let trace_2d = eps.xx + eps.yy;

    let szz = if state.is_plane_strain_like() {
        lambda * trace_2d
    } else {
        0.0
    };

    SymmTensor::new(
        lambda * trace_2d + 2.0 * mu * eps.xx,
        2.0 * mu * eps.xy,
        2.0 * mu * eps.xz,
        lambda * trace_2d + 2.0 * mu * eps.yy,
        2.0 * mu * eps.yz,
        szz,
    )
}

// =====================================================================
// Crack-kink direction — the maximum-hoop-stress criterion
// =====================================================================

/// The crack-kink angle predicted by the maximum-hoop-stress criterion.
///
/// # What it computes
///
/// The angle `theta_c`, in radians, at which the near-tip hoop stress
/// `sigma_theta_theta` is stationary and maximal, which the Erdogan-Sih
/// criterion takes as the direction a crack turns to under mixed mode I/II
/// loading. The stationarity condition is
///
/// `K_I sin(theta) + K_II (3 cos(theta) - 1) = 0`
///
/// whose relevant root, written with `t = tan(theta_c / 2)`, is
///
/// `t = (K_I - sqrt(K_I^2 + 8 K_II^2)) / (4 K_II)`.
///
/// # Relation to upstream
///
/// That expression is exactly what `gkmet1.F90` and `gkmet3.F90` carry — **as
/// commented-out code**, guarded by `abs(K_II) >= 1e-12`:
///
/// ```text
/// betas(i) = 2*atan2(0.25*(k1s/k2s - sign(1,k2s)*sqrt((k1s/k2s)**2 + 8)), 1)
/// ```
///
/// It is not live in the pinned commit, so this is a port of an *inactive*
/// upstream expression plus its published source, not of running upstream
/// behaviour. A reader should weigh it accordingly.
///
/// # Difference from upstream's form, and why
///
/// Upstream evaluates `K_I/K_II - sign(K_II) sqrt((K_I/K_II)^2 + 8)`, a
/// difference of two nearly equal large numbers whenever `K_II << K_I` — which
/// is the near-mode-I regime most calculations sit in. It therefore needs the
/// `1e-12` guard *and* loses several significant figures well before the guard
/// fires. This port picks whichever of the two algebraically identical forms is
/// cancellation-free for the sign of `K_I`:
///
/// - `K_I >= 0`: `t = -2 K_II / (K_I + sqrt(K_I^2 + 8 K_II^2))` — both terms in
///   the denominator are non-negative, and there is no division by `K_II` at
///   all, so pure mode I returns exactly zero with no guard.
/// - `K_I < 0`: `t = (K_I - sqrt(K_I^2 + 8 K_II^2)) / (4 K_II)` — both terms in
///   the numerator are negative. This branch does divide by `K_II`, which is why
///   `K_II = 0` with `K_I < 0` is the one rejected case.
///
/// Multiplying the first form's numerator and denominator by
/// `K_I + sqrt(K_I^2 + 8 K_II^2)` recovers the second, so this is a numerical
/// restatement, not a change of behaviour. The tests below sweep the two against
/// upstream's literal expression and against an independent numerical
/// stationary-point search, and quantify how much accuracy upstream's form
/// loses as `K_II` shrinks.
///
/// # Sign convention
///
/// `theta_c` is measured from the crack-propagation direction, positive
/// anticlockwise, in `(-pi, pi)`. A positive `K_II` turns the crack **clockwise**
/// (negative angle) — this is the standard convention and the one upstream's
/// expression produces.
///
/// # Assumptions and limits
///
/// Linear elastic, small-scale yielding, mode I/II only. Mode III is ignored:
/// the maximum-hoop-stress criterion has no accepted three-dimensional
/// extension, and pretending otherwise would be worse than declining. The
/// criterion is also a *local* one — it says nothing about whether the crack
/// grows, only about which way it would turn if it did.
///
/// # Errors
///
/// [`OffbeatError::Unphysical`] if `k2 == 0` and `k1 <= 0`. That is a closed
/// crack under pure compression, where the hoop stress is nowhere tensile, has
/// no maximum, and the criterion does not apply. `k1 = k2 = 0` is rejected by
/// the same test. Every other input is admissible.
///
/// # Units
///
/// `k1`, `k2` in Pa m^(1/2) (only their ratio matters); result in radians.
pub fn max_hoop_stress_kink_angle(k1: f64, k2: f64) -> Result<f64> {
    let root = (k1 * k1 + 8.0 * k2 * k2).sqrt();

    // Pick the cancellation-free branch for the sign of K_I; see the doc above.
    let tan_half = if k1 >= 0.0 {
        -2.0 * k2 / (k1 + root)
    } else {
        if k2 == 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "mixed-mode stress intensity pair (K_I, K_II)",
                value: k1,
                unit: "Pa m^(1/2)",
                reason: "the maximum-hoop-stress criterion is undefined for a \
                         non-positive K_I with zero K_II — a closed crack in \
                         pure compression has no hoop-stress maximum",
            });
        }
        (k1 - root) / (4.0 * k2)
    };

    if !tan_half.is_finite() {
        // Reachable only for K_I = K_II = 0, where `root` is zero and the first
        // branch evaluates 0/0.
        return Err(OffbeatError::Unphysical {
            quantity: "mixed-mode stress intensity pair (K_I, K_II)",
            value: k1,
            unit: "Pa m^(1/2)",
            reason: "an unloaded crack tip has no hoop-stress maximum and no \
                     kink direction",
        });
    }
    Ok(2.0 * tan_half.atan())
}

/// The near-tip hoop stress `sigma_theta_theta` times `sqrt(2 pi r)`.
///
/// # What it computes
///
/// The angular part of the mode I/II hoop stress,
///
/// `sqrt(2 pi r) sigma_tt = cos(theta/2) [ K_I cos^2(theta/2) - (3/2) K_II sin(theta) ]`
///
/// stripped of its `1/sqrt(r)` singularity so it can be compared at a fixed
/// radius. This is the function [`max_hoop_stress_kink_angle`] maximises, and it
/// is exposed so a caller can check that claim rather than take it on trust.
///
/// # Units
///
/// `k1`, `k2` in Pa m^(1/2), `theta` in radians; result in Pa m^(1/2). Divide by
/// `sqrt(2 pi r)` for a stress in pascals at radius `r` metres.
#[must_use]
pub fn scaled_hoop_stress(k1: f64, k2: f64, theta: f64) -> f64 {
    let c_half = (0.5 * theta).cos();
    c_half * (k1 * c_half * c_half - 1.5 * k2 * theta.sin())
}

// =====================================================================
// Crack-tip local basis
// =====================================================================

/// An orthonormal frame attached to a point on the crack front.
///
/// # The three directions
///
/// - **`x` — propagation.** The direction the crack would extend in, lying in
///   the crack plane, normal to the front.
/// - **`y` — normal.** Normal to the crack plane; the direction the faces
///   separate in under mode I.
/// - **`z` — tangent.** Tangent to the crack front. Degenerate in 2-D, where it
///   is the out-of-plane axis.
///
/// This is the frame [`westergaard_unit_field`] returns its fields in, and the
/// frame upstream's `chauxi.F90` calls "the local basis".
///
/// # Relation to upstream
///
/// In two dimensions `cakg2d.F90` builds it from a single stored vector, with a
/// comment worth preserving: *"ATTENTION, ON NE SE SERT PAS DU VECTEUR NORMAL DE
/// BASEFOND MAIS ON FAIT TOURNER DE 90 DEGRES LE VECTEUR DE PROPA"* — it does
/// **not** use the stored normal, it rotates the propagation vector by 90
/// degrees. That is what [`from_propagation_direction_2d`](Self::from_propagation_direction_2d)
/// reproduces, including the specific rotation sense
/// `(n_x, n_y) = (-t_y, t_x)` from lines 267-279.
///
/// In three dimensions `chauxi.F90` rotates the field with `invp`, the inverse
/// passage matrix, as `du_global(i,j) = sum_kl invp(k,i) du_local(k,l) invp(l,j)`
/// — which for an orthonormal frame is exactly `P G P^T` with `P` the
/// local-to-global rotation. [`local_to_global_gradient`](Self::local_to_global_gradient)
/// is that expression.
///
/// # Units
///
/// Dimensionless. All three stored vectors are unit vectors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrackTipBasis {
    /// Columns are the local basis vectors expressed in global coordinates, so
    /// this maps local components to global ones.
    rotation: Tensor,
}

/// Tolerance below which a direction vector is treated as degenerate.
///
/// Chosen well above rounding but far below any physically meaningful direction:
/// a crack-front tangent shorter than this in a normalised geometry means the
/// two front points coincide.
const DIRECTION_TOLERANCE: f64 = 1.0e-12;

impl CrackTipBasis {
    /// Build a two-dimensional crack-tip frame from the propagation direction.
    ///
    /// # Method
    ///
    /// Normalises the in-plane propagation direction `t = (t_x, t_y)` and takes
    /// the crack-plane normal as `n = (-t_y, t_x)` — the 90-degree anticlockwise
    /// rotation, matching `cakg2d.F90` lines 267-279 exactly. The front tangent
    /// is `+z`, out of the plane, so that `(x, y, z)` is right-handed.
    ///
    /// Any `z` component of `direction` is ignored; this is the planar case by
    /// construction.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if the in-plane part of `direction` has
    /// magnitude below `1e-12` and so defines no direction.
    ///
    /// # Units
    ///
    /// `direction` in any consistent length unit — only its direction is used.
    pub fn from_propagation_direction_2d(direction: Vector3) -> Result<Self> {
        let norm = (direction.x * direction.x + direction.y * direction.y).sqrt();
        if !(norm > DIRECTION_TOLERANCE) {
            return Err(OffbeatError::Unphysical {
                quantity: "crack propagation direction (in-plane magnitude)",
                value: norm,
                unit: "-",
                reason: "must be non-degenerate; a zero-length vector defines \
                         no crack-tip frame",
            });
        }
        let t = Vector3::new(direction.x / norm, direction.y / norm, 0.0);
        let n = Vector3::new(-t.y, t.x, 0.0);
        let z = Vector3::new(0.0, 0.0, 1.0);
        Ok(Self {
            rotation: Tensor::from_cols(t, n, z),
        })
    }

    /// Build a three-dimensional crack-tip frame from the front tangent and the
    /// crack-plane normal.
    ///
    /// # Method
    ///
    /// Both inputs are normalised. The normal is then made exactly orthogonal to
    /// the tangent by removing its tangential component (Gram-Schmidt), because
    /// a normal extracted from a discretised crack surface is only approximately
    /// perpendicular to a tangent extracted from a discretised front — and an
    /// almost-orthonormal frame silently corrupts the mode decomposition. The
    /// propagation direction is then `n x t`, completing a right-handed frame in
    /// which the local `x`, `y`, `z` are propagation, normal and tangent.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if either input is degenerate, or if the two
    /// are parallel to within `1e-12` after normalisation — in which case they
    /// span no plane and no frame exists.
    ///
    /// # Units
    ///
    /// Both inputs in any consistent length unit; only directions are used.
    pub fn from_front_tangent_and_normal(tangent: Vector3, normal: Vector3) -> Result<Self> {
        let t = unit_or_error(tangent, "crack-front tangent")?;
        let n_raw = unit_or_error(normal, "crack-plane normal")?;

        // Gram-Schmidt: strip the component of the normal along the front.
        let along = n_raw.dot(t);
        let n_ortho = Vector3::new(
            n_raw.x - along * t.x,
            n_raw.y - along * t.y,
            n_raw.z - along * t.z,
        );
        let n = unit_or_error(n_ortho, "crack-plane normal, orthogonalised against the front tangent")?;

        // Right-handed (propagation, normal, tangent).
        let p = n.cross(t);
        Ok(Self {
            rotation: Tensor::from_cols(p, n, t),
        })
    }

    /// The crack-propagation direction, a unit vector in global coordinates.
    #[must_use]
    pub fn propagation_direction(self) -> Vector3 {
        self.rotation.col_x()
    }

    /// The crack-plane normal, a unit vector in global coordinates.
    #[must_use]
    pub fn crack_plane_normal(self) -> Vector3 {
        self.rotation.col_y()
    }

    /// The crack-front tangent, a unit vector in global coordinates.
    #[must_use]
    pub fn front_tangent(self) -> Vector3 {
        self.rotation.col_z()
    }

    /// Rotate a vector from the local crack-tip frame into global coordinates.
    ///
    /// Units are whatever the vector carries — this is a rotation, not a
    /// scaling.
    #[must_use]
    pub fn local_to_global_vector(self, v: Vector3) -> Vector3 {
        self.rotation.mat_vec(v)
    }

    /// Rotate a vector from global coordinates into the local crack-tip frame.
    #[must_use]
    pub fn global_to_local_vector(self, v: Vector3) -> Vector3 {
        self.rotation.transpose().mat_vec(v)
    }

    /// Rotate a second-order tensor (a displacement gradient, a stress) from the
    /// local crack-tip frame into global coordinates: `P G P^T`.
    ///
    /// This is upstream's `chauxi.F90` transformation
    /// `du_global(i,j) = sum_kl invp(k,i) du_local(k,l) invp(l,j)`, written
    /// without index gymnastics.
    #[must_use]
    pub fn local_to_global_gradient(self, g: Tensor) -> Tensor {
        self.rotation.mat_mul(g).mat_mul(self.rotation.transpose())
    }

    /// Rotate a second-order tensor from global coordinates into the local
    /// crack-tip frame: `P^T G P`. The inverse of
    /// [`local_to_global_gradient`](Self::local_to_global_gradient).
    #[must_use]
    pub fn global_to_local_gradient(self, g: Tensor) -> Tensor {
        self.rotation.transpose().mat_mul(g).mat_mul(self.rotation)
    }

    /// Rotate a whole near-tip field into global coordinates.
    ///
    /// Convenience over calling
    /// [`local_to_global_vector`](Self::local_to_global_vector) and
    /// [`local_to_global_gradient`](Self::local_to_global_gradient) separately,
    /// and the operation `chauxi.F90` performs on every auxiliary field before
    /// it enters the domain integral.
    #[must_use]
    pub fn field_to_global(self, field: NearTipField) -> NearTipField {
        NearTipField {
            displacement: self.local_to_global_vector(field.displacement),
            gradient: self.local_to_global_gradient(field.gradient),
        }
    }
}

/// Normalise a vector or report it as degenerate.
fn unit_or_error(v: Vector3, what: &'static str) -> Result<Vector3> {
    let m = v.mag();
    if !(m > DIRECTION_TOLERANCE) {
        return Err(OffbeatError::Unphysical {
            quantity: what,
            value: m,
            unit: "-",
            reason: "must be non-degenerate; a zero-length vector defines no \
                     direction",
        });
    }
    Ok(Vector3::new(v.x / m, v.y / m, v.z / m))
}

// =====================================================================
// 2-D summed-result post-processing — cakg2d.F90
// =====================================================================

/// The five quantities upstream's two-dimensional `CALC_K_G` sums out of the
/// element loop, and the corrections applied to them afterwards.
///
/// # What the five are
///
/// `cakg2d.F90` calls `mesomm` to sum five elementary values (`fic(1..5)`) into
/// `valg(1..5)`:
///
/// | Slot | Meaning |
/// |---|---|
/// | `valg(1)` | `G`, the energy release rate from the domain integral |
/// | `valg(2)` | the mode-I *Irwin root*, `K_I / sqrt(E')` |
/// | `valg(3)` | the mode-II Irwin root, `K_II / sqrt(E')` |
/// | `valg(4)` | `K_I` from the interaction integral |
/// | `valg(5)` | `K_II` from the interaction integral |
///
/// and then forms `G_IRWIN = valg(2)^2 + valg(3)^2` (line 493) — the same
/// construction `calcG_type.F90` line 1599 uses in 3-D with three modes.
///
/// **Only the post-processing is ported.** Producing the five numbers is the
/// blocked FE work; this type is what you do with them once you have them, and
/// it is genuinely free of any mesh dependency.
///
/// # Why keep the roots separate from `K_I`, `K_II`
///
/// Because `G` and `G_IRWIN` are computed by different routes — the direct
/// domain integral and the interaction integral respectively — and their
/// *disagreement* is the standard diagnostic for an under-resolved ring. Folding
/// them together would throw that away, and upstream deliberately reports both.
///
/// # Units
///
/// `g` and `g_irwin` in J/m^2; `k1`, `k2` in Pa m^(1/2); the Irwin roots in
/// (J/m^2)^(1/2) = Pa^(1/2) m^(1/2)... more usefully, `K / sqrt(E')`, whose
/// square is an energy release rate.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlanarCrackTipResult {
    /// `valg(1)` — energy release rate from the domain integral, J/m^2.
    pub g: f64,
    /// `valg(2)` — mode-I Irwin root `K_I / sqrt(E')`.
    pub mode_i_root: f64,
    /// `valg(3)` — mode-II Irwin root `K_II / sqrt(E')`.
    pub mode_ii_root: f64,
    /// `valg(4)` — mode-I stress intensity factor, Pa m^(1/2).
    pub k1: f64,
    /// `valg(5)` — mode-II stress intensity factor, Pa m^(1/2).
    pub k2: f64,
}

impl PlanarCrackTipResult {
    /// `G_IRWIN`, the energy release rate reconstructed from the per-mode roots.
    ///
    /// `mode_i_root^2 + mode_ii_root^2`, transcribing `cakg2d.F90` line 493
    /// (`girwin = valg(1+1)*valg(1+1) + valg(1+2)*valg(1+2)`).
    ///
    /// In an exact linear-elastic calculation this equals [`g`](Self::g). The
    /// gap between them measures discretisation error at the front, and is the
    /// number to look at before trusting either.
    ///
    /// Units: J/m^2.
    #[must_use]
    pub fn g_irwin(self) -> f64 {
        self.mode_i_root * self.mode_i_root + self.mode_ii_root * self.mode_ii_root
    }

    /// Apply upstream's `SYME = 'OUI'` correction for a symmetric half model.
    ///
    /// # What it does
    ///
    /// Doubles `G`, the mode-I root and `K_I`; zeroes the mode-II root and
    /// `K_II`. Transcribes `cakg2d.F90` lines 485-491 exactly.
    ///
    /// # Why doubling is right for *both* `G` and `K` — and what it implies
    ///
    /// This looks inconsistent at first sight: `G` goes as the *square* of a
    /// stress intensity factor, so doubling both cannot be right. It is,
    /// because the halving lives in the **domain of integration**, not in the
    /// field. Meshing half the body halves the ring, and all five summed
    /// quantities are integrals of a fixed integrand over that ring, so all five
    /// come out halved and all five are recovered by the same factor of two.
    ///
    /// That in turn tells you something about slot 2: the doubling is only
    /// consistent if the Irwin root is **linear** in the ring — i.e. it is
    /// `K_I / sqrt(E')` produced directly by the element, not `sqrt(G_I)`
    /// obtained by taking a square root afterwards, which would need a factor of
    /// `sqrt(2)` instead. The element routine that fills these five slots is not
    /// in the available upstream clone (see the module documentation), so this is
    /// an inference from the correction's internal consistency, not something
    /// read off the source.
    ///
    /// A consequence worth stating, because it surprises: **`g_irwin()` does not
    /// equal `g` on the uncorrected half-model result.** With every slot at half
    /// its full-model value, `g_irwin()` is quartered while `g` is halved, so the
    /// raw half-model `G_IRWIN` reads exactly `g / 2`. The two agree only after
    /// this correction has been applied. The test below pins that factor of two
    /// rather than leaving it to be rediscovered.
    ///
    /// Mode II vanishing is not an approximation: a load symmetric about the
    /// crack plane cannot produce in-plane shear at the tip.
    #[must_use]
    pub fn with_symmetric_half_model(self) -> Self {
        Self {
            g: 2.0 * self.g,
            mode_i_root: 2.0 * self.mode_i_root,
            mode_ii_root: 0.0,
            k1: 2.0 * self.k1,
            k2: 0.0,
        }
    }

    /// Apply upstream's axisymmetric normalisation by the crack-tip radius.
    ///
    /// # What it does
    ///
    /// Divides **all five** slots by `r_tip`, the radial coordinate of the crack
    /// tip. Transcribes `cakg2d.F90` lines 479-483
    /// (`if (is_axi) valg(i) = valg(i)/rcmp(1)`).
    ///
    /// # Why
    ///
    /// In an axisymmetric model the element integrals carry the `2 pi r`
    /// Jacobian, so the summed result is an energy release for the whole
    /// revolution rather than the per-unit-length quantity `G` is defined as.
    /// Dividing by `r_tip` converts back. Upstream applies it to the `K` slots
    /// too, which is consistent because those `K` values are likewise integrals
    /// over the revolved ring.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if `r_tip <= 0`. A crack tip on the axis of
    /// revolution has no axisymmetric `G` — the correction is a division by
    /// zero, and upstream would produce an infinity there.
    ///
    /// # Units
    ///
    /// `r_tip` in metres.
    pub fn with_axisymmetric_normalisation(self, r_tip: f64) -> Result<Self> {
        if !(r_tip > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "crack-tip radial coordinate in an axisymmetric model",
                value: r_tip,
                unit: "m",
                reason: "must be strictly positive; a tip on the axis of \
                         revolution has no axisymmetric energy release rate",
            });
        }
        Ok(Self {
            g: self.g / r_tip,
            mode_i_root: self.mode_i_root / r_tip,
            mode_ii_root: self.mode_ii_root / r_tip,
            k1: self.k1 / r_tip,
            k2: self.k2 / r_tip,
        })
    }

    /// The stress intensity factors as a [`StressIntensityFactors`], with
    /// `k3 = 0`.
    #[must_use]
    pub fn stress_intensity_factors(self) -> StressIntensityFactors {
        StressIntensityFactors::new(self.k1, self.k2, 0.0)
    }
}

// =====================================================================
// Crack-front Legendre basis — plegen.F90 / dplegen.F90
// =====================================================================

/// Highest Legendre degree upstream's `plegen.F90` supports.
///
/// Degrees 0 through 7 are hard-coded there; anything else hits an
/// `ASSERT(.false.)`. Reproduced as an error rather than a panic.
pub const MAX_LEGENDRE_FRONT_DEGREE: usize = 7;

/// The `L2`-orthonormal Legendre basis function along the crack front.
///
/// # What it computes
///
/// `phi_n(s) = sqrt((2n + 1) / L) P_n(xi)` with `xi = 2 s / L - 1`, where `P_n`
/// is the standard Legendre polynomial and `L` the crack-front length. Ported
/// from `plegen.F90`.
///
/// # What it is for
///
/// The G-theta method does not compute `G(s)` pointwise along a
/// three-dimensional crack front; it computes the projections
/// `<G, theta_i>` of `G` onto a family of virtual extension fields, then solves
/// a small linear system for the coefficients of `G(s)` in the same basis. When
/// `LISSAGE = 'LEGENDRE'`, this is that basis. The *assembly and solve* are
/// blocked on having a crack front (see the module documentation, group 4); the
/// basis itself is closed-form and is here.
///
/// # Why the normalisation matters
///
/// The `sqrt((2n + 1) / L)` factor makes the family orthonormal in `L2(0, L)`:
/// `integral_0^L phi_m phi_n ds = delta_mn`. That is what makes the Gram matrix
/// upstream assembles well-conditioned — without it the system degrades rapidly
/// with degree. It is verified below rather than asserted.
///
/// # Errors
///
/// [`OffbeatError::Unphysical`] if `front_length <= 0`;
/// [`OffbeatError::NotImplemented`] if `degree` exceeds
/// [`MAX_LEGENDRE_FRONT_DEGREE`], matching upstream's assertion.
///
/// # Units
///
/// `s` and `front_length` in metres, both measured along the front, with `s`
/// expected in `[0, L]` (values outside are evaluated by extrapolation, as
/// upstream does, without complaint). The result has units of m^(-1/2), so that
/// a coefficient times the basis function integrates to a length-independent
/// quantity.
pub fn legendre_front_mode(degree: usize, s: f64, front_length: f64) -> Result<f64> {
    let xi = legendre_abscissa(s, front_length)?;
    let normalisation = ((2 * degree + 1) as f64 / front_length).sqrt();
    Ok(normalisation * legendre_polynomial(degree, xi)?)
}

/// The derivative with respect to arc length of [`legendre_front_mode`].
///
/// # What it computes
///
/// `d phi_n / ds = (2 / L) sqrt((2n + 1) / L) P_n'(xi)`, the chain rule applied
/// through `xi = 2 s / L - 1`. Ported from `dplegen.F90`, whose `coef` is
/// exactly that `(2/L) sqrt((2n+1)/L)` prefactor.
///
/// Needed because the virtual extension field's *gradient* enters the G-theta
/// bilinear form, not only its value.
///
/// # Errors
///
/// As [`legendre_front_mode`].
///
/// # Units
///
/// `s`, `front_length` in metres; result in m^(-3/2).
pub fn legendre_front_mode_derivative(degree: usize, s: f64, front_length: f64) -> Result<f64> {
    let xi = legendre_abscissa(s, front_length)?;
    let normalisation = (2.0 / front_length) * ((2 * degree + 1) as f64 / front_length).sqrt();
    Ok(normalisation * legendre_polynomial_derivative(degree, xi)?)
}

/// Map arc length `s` in `[0, L]` onto the reference abscissa `xi` in `[-1, 1]`.
///
/// Upstream's `ksi = 2*s/l - 1`, common to `plegen.F90` and `dplegen.F90`.
fn legendre_abscissa(s: f64, front_length: f64) -> Result<f64> {
    if !(front_length > 0.0) {
        return Err(OffbeatError::Unphysical {
            quantity: "crack-front length",
            value: front_length,
            unit: "m",
            reason: "must be strictly positive; the Legendre basis is defined \
                     on a front of finite length",
        });
    }
    Ok(2.0 * s / front_length - 1.0)
}

/// The standard Legendre polynomial `P_n(x)` on `[-1, 1]`, degrees 0 to 7.
///
/// Transcribed from `plegen.F90`'s `pleg2..pleg7` macros, with `P_0 = 1` and
/// `P_1 = x` inline there. Kept in the same closed form rather than replaced by
/// the recurrence, so the port stays a transcription; the recurrence is used in
/// the tests as an *independent* check.
fn legendre_polynomial(degree: usize, x: f64) -> Result<f64> {
    let (x2, x4, x6) = (x * x, x.powi(4), x.powi(6));
    Ok(match degree {
        0 => 1.0,
        1 => x,
        2 => (3.0 * x2 - 1.0) / 2.0,
        3 => x * (5.0 * x2 - 3.0) / 2.0,
        4 => (35.0 * x4 - 30.0 * x2 + 3.0) / 8.0,
        5 => x * (63.0 * x4 - 70.0 * x2 + 15.0) / 8.0,
        6 => (231.0 * x6 - 315.0 * x4 + 105.0 * x2 - 5.0) / 16.0,
        7 => x * (429.0 * x6 - 693.0 * x4 + 315.0 * x2 - 35.0) / 16.0,
        _ => return Err(unsupported_degree(degree)),
    })
}

/// The derivative `P_n'(x)`, degrees 0 to 7.
///
/// Transcribed from `dplegen.F90`'s `dpleg2..dpleg7` macros, with `P_0' = 0` and
/// `P_1' = 1` inline there.
fn legendre_polynomial_derivative(degree: usize, x: f64) -> Result<f64> {
    let (x2, x3, x4, x5, x6) = (x * x, x.powi(3), x.powi(4), x.powi(5), x.powi(6));
    Ok(match degree {
        0 => 0.0,
        1 => 1.0,
        2 => 3.0 * x,
        3 => (15.0 * x2 - 3.0) / 2.0,
        4 => (35.0 * x3 - 15.0 * x) / 2.0,
        5 => 15.0 * (21.0 * x4 - 14.0 * x2 + 1.0) / 8.0,
        6 => 21.0 * (33.0 * x5 - 30.0 * x3 + 5.0 * x) / 8.0,
        7 => 7.0 * (429.0 * x6 - 495.0 * x4 + 135.0 * x2 - 5.0) / 16.0,
        _ => return Err(unsupported_degree(degree)),
    })
}

/// The error upstream raises as `ASSERT(.false.)` for an out-of-range degree.
fn unsupported_degree(degree: usize) -> OffbeatError {
    let _ = degree;
    OffbeatError::NotImplemented(
        "the Legendre crack-front basis above degree 7 (upstream's \
         plegen.F90/dplegen.F90 hard-code degrees 0 to 7 and assert on anything \
         higher)",
    )
}

// =====================================================================
// Crack-front hat smoothing — hatSmooth.F90
// =====================================================================

/// Smooth a nodal `G(s)` or `K(s)` along a quadratic crack front — a port of
/// `hatSmooth.F90`.
///
/// # What it computes
///
/// Given values at the `2 m - 1` nodes of a chain of `m - 1` three-node
/// (quadratic) front segments, it replaces them with a hat-function-weighted
/// average. Corner-node values become
///
/// - first: `(2 v_0 + v_1) / 3`
/// - interior `i`: `(lg_i v_{2i-1} + v_{2i} + ld_i v_{2i+1}) / 3`, with
///   `lg_i = 2 le_i / (le_i + le_{i+1})` and `ld_i = 2 le_{i+1} / (le_i + le_{i+1})`
///   where `le` are the corner-to-corner segment lengths
/// - last: `(v_{n-2} + 2 v_{n-1}) / 3`
///
/// and mid-side values become the mean of their two neighbouring smoothed corner
/// values.
///
/// # Why it exists
///
/// The raw per-node `G` from a G-theta calculation on a quadratic front
/// oscillates between corner and mid-side nodes — an artefact of the quadratic
/// interpolation, not physics. This is the fixed three-point filter upstream
/// applies to remove it.
///
/// # Properties, and one limitation worth stating
///
/// `lg_i + ld_i = 2` identically, so the interior stencil reproduces a constant
/// exactly. The **end** stencils do not reproduce a linear function: `(2 v_0 +
/// v_1)/3` is biased towards the interior by one third of the end slope. That is
/// upstream's behaviour and it is reproduced, not corrected — but a user reading
/// a smoothed `G(s)` should expect the two end values to be pulled inward.
///
/// # Errors
///
/// [`OffbeatError::Mesh`] if `abscissae` and `values` differ in length, if the
/// length is even, or if it is below 3 — a quadratic front needs an odd node
/// count of at least one segment.
///
/// # Units
///
/// `abscissae` are curvilinear abscissae along the front in metres; `values`
/// carry whatever the smoothed quantity does (J/m^2 for `G`, Pa m^(1/2) for
/// `K`) and are modified in place.
pub fn hat_smooth_front(abscissae: &[f64], values: &mut [f64]) -> Result<()> {
    let nno = values.len();
    if abscissae.len() != nno {
        return Err(OffbeatError::Mesh(format!(
            "crack-front hat smoothing: {} abscissae for {nno} values",
            abscissae.len()
        )));
    }
    if nno < 3 || nno % 2 == 0 {
        return Err(OffbeatError::Mesh(format!(
            "crack-front hat smoothing needs an odd node count of at least 3 \
             (a chain of quadratic segments), got {nno}"
        )));
    }
    let nnos = (nno + 1) / 2;

    // Corner-to-corner segment lengths, upstream's `le`.
    let le: Vec<f64> = (0..nnos - 1)
        .map(|i| (abscissae[2 * i + 2] - abscissae[2 * i]).abs())
        .collect();

    let mut smooth = vec![0.0; nnos];
    smooth[0] = (2.0 * values[0] + values[1]) / 3.0;
    for i in 0..nnos.saturating_sub(2) {
        let sum = le[i] + le[i + 1];
        if !(sum > 0.0) {
            return Err(OffbeatError::Mesh(format!(
                "crack-front hat smoothing: zero-length segment pair at corner \
                 node {}",
                i + 1
            )));
        }
        let lg = 2.0 * le[i] / sum;
        let ld = 2.0 * le[i + 1] / sum;
        smooth[i + 1] =
            (lg * values[2 * i + 1] + values[2 * i + 2] + ld * values[2 * i + 3]) / 3.0;
    }
    smooth[nnos - 1] = (values[nno - 2] + 2.0 * values[nno - 1]) / 3.0;

    for i in 0..nnos {
        values[2 * i] = smooth[i];
    }
    for i in 0..nnos - 1 {
        values[2 * i + 1] = 0.5 * (smooth[i] + smooth[i + 1]);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
