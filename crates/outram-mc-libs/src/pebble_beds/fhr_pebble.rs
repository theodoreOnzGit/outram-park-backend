//! FHR (fluoride-salt-cooled high-temperature reactor) TRISO-pebble builders —
//! the geometry and material machinery for the **ring-RPT** code-to-code
//! verification against OpenMC (`op-mzvp`, GitHub #156).
//!
//! Two pebbles that a reactivity-equivalent physical transformation (RPT) makes
//! equivalent:
//!
//! - the **explicit-TRISO** pebble — thousands of five-layer TRISO particles
//!   randomly packed in a graphite matrix; solved by delta (Woodcock) tracking
//!   ([`crate::pebble_beds::keff_delta`]) with [`triso_layer_at`] resolving which
//!   coating a collision candidate falls in, and
//! - the **ring-RPT** pebble — the same TRISO material dissolved into one
//!   homogeneous shell whose *volume equals the total particle volume*
//!   ([`rpt_fuel_outer_radius`]), placed at a chosen inner radius. Plain CSG,
//!   solved by [`crate::physics::transport_csg::run_keff_csg`] /
//!   [`crate::physics::reactor_physics::run_keff_reactor_physics`].
//!
//! # Data-free and fidelity-agnostic
//!
//! Nothing here reads nuclear data or fixes a temperature: the caller supplies
//! the [`Material`]s (atom densities), so the same builders work with embedded
//! ([`crate::material::nuclide::Nuclide::from_core`]) or ENDF-reconstructed
//! ([`crate::material::nuclide::Nuclide::from_endf_file`]) cross sections. The
//! high-fidelity ENDF case lives in `examples/fhr_ring_rpt_endf.rs`.
//!
//! # Homogenisation is done in atom-density space
//!
//! [`homogenise_by_volume`] volume-fraction-averages the constituent materials'
//! atom densities directly — `N_homog[i] = Σ_layer (V_layer / ΣV) · N_layer[i]`.
//! No molar-mass bookkeeping is needed because [`Material`] already carries atom
//! densities, unlike an OpenMC `Material` (mass density + atom/weight fractions),
//! where the reference `rpt_pebble.py` has to convert through moles.

use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use crate::geometry::geometry::Geometry;
use crate::geometry::position::Position;
use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use crate::geometry::universe::Universe;
use crate::material::material::{Material, NuclideComponent};
// Re-exported, not merely imported: `ExplicitTrisoPebble::new` takes a
// `TrisoMaterials`, so a caller who can name the constructor must be able to
// name its argument from the same module. A docs-only dogfood run wrote
// `use outram_mc_libs::pebble_beds::fhr_pebble::{ExplicitTrisoPebble,
// TrisoMaterials, ...}` -- the obvious import -- and hit E0603 because this was
// a private `use`. An API that cannot be called from the module it is
// documented in is not callable.
pub use crate::geometry::triso_particle::TrisoMaterials;
use crate::pebble_beds::sphere_packing::PackedSpheres;

/// The five cumulative outer radii \[cm\] of a TRISO particle (kernel first,
/// OPyC last) plus the volume packing fraction of whole particles in the fuel
/// zone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrisoSpec {
    /// Fuel-kernel outer radius \[cm\].
    pub kernel: f64,
    /// Buffer (porous PyC) outer radius \[cm\].
    pub buffer: f64,
    /// Inner pyrolytic carbon (IPyC / PyC1) outer radius \[cm\].
    pub ipyc: f64,
    /// Silicon-carbide outer radius \[cm\].
    pub sic: f64,
    /// Outer pyrolytic carbon (OPyC / PyC2) outer radius \[cm\] — the whole
    /// particle radius.
    pub opyc: f64,
    /// Volume fraction of whole TRISO particles in the fuel zone \[–\].
    pub packing_fraction: f64,
}

impl TrisoSpec {
    /// The 19.9 % HALEU UCO reference used by the ring-RPT V&V decks
    /// (`openmc_fuel_perf_project/pebble_factory/triso.py`): 215 µm kernel
    /// radius, then 100 / 35 / 35 / 40 µm buffer / IPyC / SiC / OPyC
    /// thicknesses, packed at 30 % by volume. **Reference values, not an
    /// authoritative fuel specification.**
    pub const FHR_HALEU_UCO: Self = Self {
        kernel: 0.0215,
        buffer: 0.0315,
        ipyc: 0.0350,
        sic: 0.0385,
        opyc: 0.0425,
        packing_fraction: 0.30,
    };

    /// Whether the radii are strictly increasing and the packing fraction is in
    /// `(0, 0.64)` (below the random-close-pack ceiling).
    pub fn is_valid(&self) -> bool {
        let r = [self.kernel, self.buffer, self.ipyc, self.sic, self.opyc];
        r[0] > 0.0
            && r.windows(2).all(|w| w[0] < w[1])
            && self.packing_fraction > 0.0
            && self.packing_fraction < 0.64
    }

    /// The five shell volumes \[cm³\] of one particle, kernel first: the kernel
    /// ball then the four coating shells.
    pub fn layer_volumes(&self) -> [f64; 5] {
        let ball = |r: f64| 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
        [
            ball(self.kernel),
            ball(self.buffer) - ball(self.kernel),
            ball(self.ipyc) - ball(self.buffer),
            ball(self.sic) - ball(self.ipyc),
            ball(self.opyc) - ball(self.sic),
        ]
    }
}

/// A TRISO coating layer, returned by [`triso_layer_at`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrisoLayer {
    /// Fuel kernel.
    Kernel,
    /// Buffer (porous PyC).
    Buffer,
    /// Inner pyrolytic carbon.
    Ipyc,
    /// Silicon carbide.
    Sic,
    /// Outer pyrolytic carbon.
    Opyc,
}

/// Which TRISO layer the radial offset `r` \[cm\] from a particle centre falls
/// in, or `None` if `r` is outside the particle (`r >= spec.opyc`, i.e. the
/// point is in the surrounding matrix).
#[inline]
pub fn triso_layer_at(r: f64, spec: &TrisoSpec) -> Option<TrisoLayer> {
    if r < spec.kernel {
        Some(TrisoLayer::Kernel)
    } else if r < spec.buffer {
        Some(TrisoLayer::Buffer)
    } else if r < spec.ipyc {
        Some(TrisoLayer::Ipyc)
    } else if r < spec.sic {
        Some(TrisoLayer::Sic)
    } else if r < spec.opyc {
        Some(TrisoLayer::Opyc)
    } else {
        None
    }
}

/// The explicit-TRISO pebble's point-membership lookup for delta (Woodcock)
/// tracking: randomly-packed, five-layer TRISO particles in a graphite
/// matrix, itself wrapped in a graphite shell and a coolant exterior.
///
/// # Why this exists
///
/// The ring-RPT pebble gets a one-call builder, [`fhr_pebble_geometry`] — CSG
/// surfaces and cells, done. The explicit pebble has no equivalent: "what
/// material is at this point" for a packed TRISO fuel zone means combining
/// [`PackedSpheres::containing_center`] (which particle, if any, contains
/// `p`) with [`triso_layer_at`] (which coating layer, by radius from that
/// particle's centre), falling back to the matrix when no particle contains
/// `p` — plus the two shells outside the fuel zone entirely. Every caller was
/// hand-assembling that as a closure; `examples/fhr_ring_rpt_endf.rs` carried
/// one (`explicit_at`) before this type existed. A **Haiku dogfood run**
/// (`docs/dogfood-2026-09-11-ring-rpt.md`, `op-mzvp.3`) — given only the API
/// docs, no source, no compiler — reproduced the same gap independently: it
/// assembled the ring-RPT pebble in one call and got stuck on the explicit
/// one, because there was nothing to call.
///
/// [`ExplicitTrisoPebble`] packages that lookup once: build it from a
/// [`PackedSpheres`] packing, a [`TrisoSpec`], the per-layer-plus-matrix
/// material indices ([`TrisoMaterials`]), the shell/coolant material indices,
/// and the two zone radii, then call [`ExplicitTrisoPebble::material_at`]
/// wherever the closure used to be. It reproduces that closure's logic
/// exactly — see "Domain" below — nothing more, nothing smarter.
///
/// # Domain
///
/// - `r < r_fuel_zone`: inside a packed particle, the layer at that radius
///   from its centre ([`triso_layer_at`]); otherwise the surrounding
///   graphite matrix (`mats.matrix`).
/// - `r_fuel_zone <= r < r_pebble`: the graphite shell (`shell_mat`).
/// - `r >= r_pebble`: the coolant (`coolant_mat`).
///
/// There is no outer bound here — a delta-tracking domain
/// ([`crate::pebble_beds::keff_delta::DeltaDomain`]) supplies that — so
/// [`ExplicitTrisoPebble::material_at`] never actually returns `None`; the
/// `Option` in its signature is there because that is what
/// [`crate::pebble_beds::keff_delta::run_keff_delta_in`]'s `material_at`
/// parameter requires.
#[derive(Debug, Clone)]
pub struct ExplicitTrisoPebble {
    packed: PackedSpheres,
    spec: TrisoSpec,
    /// Material index per coating layer **and** the surrounding matrix, by
    /// name. Reuses [`TrisoMaterials`] rather than a positional `[usize; 5]`
    /// plus a separate matrix index: the caller cannot then transpose IPyC and
    /// SiC, which is a silent wrong-material bug rather than a compile error,
    /// and a reader of the API meets one existing concept instead of two new
    /// positional ones.
    mats: TrisoMaterials,
    /// Graphite shell, `r_fuel_zone..r_pebble`.
    shell_mat: usize,
    /// Coolant, `r >= r_pebble`.
    coolant_mat: usize,
    /// Outer radius of the packed-TRISO fuel zone \[cm\].
    r_fuel_zone: f64,
    /// Outer radius of the graphite shell / whole pebble \[cm\].
    r_pebble: f64,
}

impl ExplicitTrisoPebble {
    /// Assemble a pebble from an already-packed TRISO fuel zone.
    ///
    /// `packed` should be a packing of `spec.opyc`-radius spheres (whole
    /// TRISO particles) confined to `r_fuel_zone` (see
    /// `crate::pebble_beds::crp_packing::pack_spheres_crp` /
    /// [`PackedSpheres::from_spheres`] in the caller). `mats` names every
    /// material by its layer, including the matrix.
    ///
    /// # Panics
    /// If `r_fuel_zone >= r_pebble`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        packed: PackedSpheres,
        spec: TrisoSpec,
        mats: TrisoMaterials,
        shell_mat: usize,
        coolant_mat: usize,
        r_fuel_zone: f64,
        r_pebble: f64,
    ) -> Self {
        assert!(
            r_fuel_zone < r_pebble,
            "ExplicitTrisoPebble::new: r_fuel_zone {r_fuel_zone} must be < r_pebble {r_pebble}"
        );
        Self {
            packed,
            spec,
            mats,
            shell_mat,
            coolant_mat,
            r_fuel_zone,
            r_pebble,
        }
    }

    /// Material index at `p`, or `None` outside the domain — see the type's
    /// docs for why that never actually happens here.
    ///
    /// This is exactly the `explicit_at` closure `examples/fhr_ring_rpt_endf.rs`
    /// used to hand-write: inside the fuel zone, find the packed particle (if
    /// any) containing `p` via [`PackedSpheres::containing_center`], resolve
    /// its coating layer by radius from that centre via [`triso_layer_at`]
    /// (falling back to OPyC — the outermost layer — if the radius lookup
    /// itself returns `None`, which should not happen given `p` is already
    /// established to be within the particle radius, but the fallback is
    /// part of the logic being reproduced exactly), or the surrounding
    /// matrix if no packed particle contains `p`; outside the fuel zone, the
    /// graphite shell then the coolant.
    pub fn material_at(&self, p: Position) -> Option<usize> {
        let r = p.norm();
        if r < self.r_fuel_zone {
            Some(match self.packed.containing_center(p) {
                Some(c) => {
                    let layer =
                        triso_layer_at((p - c).norm(), &self.spec).unwrap_or(TrisoLayer::Opyc);
                    match layer {
                        TrisoLayer::Kernel => self.mats.kernel,
                        TrisoLayer::Buffer => self.mats.buffer,
                        TrisoLayer::Ipyc => self.mats.ipyc,
                        TrisoLayer::Sic => self.mats.sic,
                        TrisoLayer::Opyc => self.mats.opyc,
                    }
                }
                None => self.mats.matrix,
            })
        } else if r < self.r_pebble {
            Some(self.shell_mat)
        } else {
            Some(self.coolant_mat)
        }
    }
}

/// Volume-fraction homogenisation of a set of materials into one.
///
/// `parts` pairs each constituent [`Material`] with its **volume** (any
/// consistent unit — only the ratios matter). The result carries, for every
/// nuclide appearing in any part, the volume-weighted atom density
/// `N[i] = Σ_p (V_p / ΣV) · N_p[i]` \[atoms/barn·cm\]. Nuclide indices are into
/// the shared global nuclide array and must be consistent across `parts`.
///
/// This is the ring-RPT "dissolve the TRISO into one medium" step. Because
/// [`Material`] is atom-density-based, this is an exact mixture — no molar-mass
/// conversion, unlike `rpt_pebble.py`'s `get_mixed_triso_fuel_material`.
///
/// # Panics
/// If `parts` is empty or the total volume is not positive.
pub fn homogenise_by_volume(
    parts: &[(&Material, f64)],
    id: i32,
    name: &str,
    temperature: f64,
) -> Material {
    assert!(!parts.is_empty(), "homogenise_by_volume: no constituents");
    let total_v: f64 = parts.iter().map(|(_, v)| *v).sum();
    assert!(
        total_v > 0.0,
        "homogenise_by_volume: total volume must be > 0"
    );

    // Accumulate volume-weighted atom density per nuclide index.
    let mut acc: Vec<(usize, f64)> = Vec::new();
    for (mat, v) in parts {
        let w = v / total_v;
        for c in &mat.components {
            match acc.iter_mut().find(|(idx, _)| *idx == c.nuclide_idx) {
                Some((_, n)) => *n += w * c.atom_density,
                None => acc.push((c.nuclide_idx, w * c.atom_density)),
            }
        }
    }
    acc.sort_by_key(|(idx, _)| *idx);

    Material {
        id,
        name: name.to_string(),
        temperature,
        components: acc
            .into_iter()
            .map(|(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect(),
    }
}

/// The outer radius \[cm\] of the ring-RPT homogenised-fuel shell.
///
/// RPT places the dissolved TRISO material as a spherical **shell** from
/// `inner_radius` outward, sized so the shell volume equals the *total volume of
/// all TRISO particles* in the original fuel zone,
/// `V_particles = packing_fraction · (4/3)π·fuel_zone_radius³`. Hence
///
/// ```text
/// r_outer³ = inner_radius³ + packing_fraction · fuel_zone_radius³
/// ```
///
/// (`rpt_pebble.py`, `build_rpt_equivalent_model_triso_kernel_homogenised`).
/// `inner_radius` is the single knob RPT turns to match the explicit pebble's
/// k-eff; the reference value is `1.493359375` cm for this fuel.
///
/// # Panics
/// If the result would exceed `fuel_zone_radius` (the homogenised shell cannot
/// physically fit inside the original fuel zone).
pub fn rpt_fuel_outer_radius(
    inner_radius: f64,
    fuel_zone_radius: f64,
    packing_fraction: f64,
) -> f64 {
    let r3 = inner_radius.powi(3) + packing_fraction * fuel_zone_radius.powi(3);
    let r_outer = r3.cbrt();
    assert!(
        r_outer <= fuel_zone_radius,
        "rpt_fuel_outer_radius: shell outer radius {r_outer} exceeds the fuel zone \
         radius {fuel_zone_radius} — pick a smaller inner_radius"
    );
    r_outer
}

/// Concentric-shell geometry of one FHR pebble, for the CSG drivers.
///
/// Radii \[cm\], increasing:
/// - `r_inner` — inner graphite ball (`0..r_inner`); pass `0.0` for none.
/// - `r_fuel_outer` — fuel region (`r_inner..r_fuel_outer`), material
///   `fuel_mat`. For the ring-RPT pebble this is the homogenised shell; for a
///   simple homogeneous-fuel-zone pebble set `r_inner = 0`.
/// - `r_shell_outer` — graphite shell (`r_fuel_outer..r_shell_outer`).
/// - `r_root` — coolant / reflector region (`r_shell_outer..r_root`), material
///   `coolant_mat`, with `root_bc` on the outer sphere.
///
/// Material indices are into the caller's global material array. `graphite_mat`
/// fills both the inner ball and the outer shell.
///
/// # Panics
/// If the radii are not strictly increasing (ignoring a zero `r_inner`).
#[allow(clippy::too_many_arguments)]
pub fn fhr_pebble_geometry(
    r_inner: f64,
    r_fuel_outer: f64,
    r_shell_outer: f64,
    r_root: f64,
    fuel_mat: usize,
    graphite_mat: usize,
    coolant_mat: usize,
    root_bc: BoundaryType,
    temperature: f64,
) -> Geometry {
    assert!(
        r_inner >= 0.0
            && r_inner < r_fuel_outer
            && r_fuel_outer < r_shell_outer
            && r_shell_outer < r_root,
        "fhr_pebble_geometry: radii must be strictly increasing"
    );

    // Surfaces: [inner?, fuel_outer, shell_outer, root]
    let mut surfaces = Vec::with_capacity(4);
    let has_inner = r_inner > 0.0;
    if has_inner {
        surfaces.push(sphere(r_inner, BoundaryType::Transmissive));
    }
    surfaces.push(sphere(r_fuel_outer, BoundaryType::Transmissive));
    surfaces.push(sphere(r_shell_outer, BoundaryType::Transmissive));
    surfaces.push(sphere(r_root, root_bc));
    let (s_inner, s_fuel, s_shell, s_root) = if has_inner {
        (Some(0usize), 1usize, 2usize, 3usize)
    } else {
        (None, 0usize, 1usize, 2usize)
    };

    let mut cells = Vec::with_capacity(4);
    let mut cell_id = 1;
    // Inner graphite ball.
    if let Some(si) = s_inner {
        cells.push(Cell::material(
            cell_id,
            vec![inside(si)],
            graphite_mat,
            temperature,
        ));
        cell_id += 1;
    }
    // Shell regions follow the crate convention: `outside(inner)` token first,
    // then `inside(outer)`, then `Intersection` (matches `build_triso_particle`;
    // the `distance_to_boundary` walk depends on this ordering).
    let fuel_region = match s_inner {
        Some(si) => vec![outside(si), inside(s_fuel), RegionToken::Intersection],
        None => vec![inside(s_fuel)],
    };
    cells.push(Cell::material(cell_id, fuel_region, fuel_mat, temperature));
    cell_id += 1;
    // Graphite shell: outside fuel_outer & inside shell_outer.
    cells.push(Cell::material(
        cell_id,
        vec![outside(s_fuel), inside(s_shell), RegionToken::Intersection],
        graphite_mat,
        temperature,
    ));
    cell_id += 1;
    // Coolant: outside shell_outer & inside root.
    cells.push(Cell::material(
        cell_id,
        vec![outside(s_shell), inside(s_root), RegionToken::Intersection],
        coolant_mat,
        temperature,
    ));

    let cell_indices = (0..cells.len()).collect();
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe {
            id: 0,
            cell_indices,
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn sphere(r: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r,
        bc,
    })
}

fn inside(surface_idx: usize) -> RegionToken {
    RegionToken::HalfSpace {
        surface_idx,
        sense: HalfSpaceSense::Inside,
    }
}

fn outside(surface_idx: usize) -> RegionToken {
    RegionToken::HalfSpace {
        surface_idx,
        sense: HalfSpaceSense::Outside,
    }
}

/// A homogeneous-medium cube geometry of half-width `h` \[cm\] filled with a
/// single material, reflective on all six faces — the k∞ unit cell for
/// comparing an explicit packing against its homogenised equivalent.
pub fn homogeneous_cube(h: f64, material_idx: usize, temperature: f64) -> Geometry {
    use crate::geometry::surface::{XPlane, YPlane, ZPlane};
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane {
            x0: -h,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::XPlane(XPlane {
            x0: h,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: -h,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: h,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::ZPlane(ZPlane {
            z0: -h,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::ZPlane(ZPlane {
            z0: h,
            bc: BoundaryType::Reflective,
        }),
    ];
    let region = vec![
        outside(0),
        inside(1),
        RegionToken::Intersection,
        outside(2),
        RegionToken::Intersection,
        inside(3),
        RegionToken::Intersection,
        outside(4),
        RegionToken::Intersection,
        inside(5),
        RegionToken::Intersection,
    ];
    Geometry {
        surfaces,
        cells: vec![Cell::material(1, region, material_idx, temperature)],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::material::Material;

    /// The explicit pebble's five coating layers hold the volume shares the
    /// [`TrisoSpec`] says they do — measured through
    /// [`ExplicitTrisoPebble::material_at`], the function transport actually
    /// calls — and the fuel sphere's packing fraction is **not** the packer's
    /// nominal one.
    ///
    /// This closes the approximation the ring-RPT study carries and never
    /// checked: the explicit pebble resolves layers by **nearest centre +
    /// radius** rather than exact CSG, and the packing is generated in a cube
    /// that is then clipped to the fuel sphere. A few percent of misplaced fuel
    /// is worth hundreds of pcm, which is the scale of the disagreement being
    /// chased (`op-mzvp.2.12`).
    ///
    /// Two separate claims, because they failed differently when measured:
    ///
    /// 1. **Layer resolution is exact.** Each layer's share *of the TRISO
    ///    material* must be `(r_i³ − r_{i−1}³)/r_opyc³` — pure geometry, no
    ///    packing statistics in it at all. Measured to better than 0.5 %.
    /// 2. **The clip inflates the packing fraction, and by a knowable amount.**
    ///    Every layer came out `+2.0 … +2.7 %` — uniformly, which is the
    ///    signature of a density offset rather than a layer bug. Sphere *centres*
    ///    are confined to `half − r_particle` while `pf` is quoted over the full
    ///    cube, so the cube's interior is denser than nominal and the inscribed
    ///    ball inherits that: **0.3072 against a nominal 0.300, +2.4 %**. A deck
    ///    specifies `pf` over the fuel *region*, so that is 2.4 % more heavy
    ///    metal than the model being compared against.
    ///    [`PackedSpheres::volume_fraction_in_ball`] is the quantity to use, and
    ///    this test pins that it and `material_at` agree on it.
    #[test]
    fn explicit_pebble_layer_shares_are_exact_and_the_ball_pf_is_not_the_cube_pf() {
        use crate::geometry::position::Position;
        use crate::pebble_beds::crp_packing::pack_spheres_crp;
        use crate::pebble_beds::sphere_packing::PackedSpheres;

        const R_FUEL_ZONE: f64 = 1.9;
        const R_PEBBLE: f64 = 2.0;
        let spec = TrisoSpec::FHR_HALEU_UCO;

        let pack_half = R_FUEL_ZONE + spec.opyc;
        let spheres = pack_spheres_crp(spec.opyc, pack_half, spec.packing_fraction, 20_260_910)
            .expect("TRISO CRP packing");
        let packed = PackedSpheres::from_spheres(spheres, pack_half, spec.opyc);
        let ball_pf = packed.volume_fraction_in_ball(R_FUEL_ZONE, 400_000, 0xC0FFEE);
        let mats = TrisoMaterials {
            kernel: 0,
            buffer: 1,
            ipyc: 2,
            sic: 3,
            opyc: 4,
            matrix: 5,
        };
        let pebble = ExplicitTrisoPebble::new(packed, spec, mats, 6, 7, R_FUEL_ZONE, R_PEBBLE);

        // Uniform-by-volume sampling of the fuel sphere: rejection in its
        // bounding cube, which needs no RNG library and no inverse transform.
        let mut seed = 99_887_766_u64;
        let mut prn = move || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 11) as f64) / ((1u64 << 53) as f64)
        };
        const N: usize = 1_000_000;
        let mut hits = [0usize; 6];
        let mut n = 0usize;
        while n < N {
            let p = Position::new(
                (2.0 * prn() - 1.0) * R_FUEL_ZONE,
                (2.0 * prn() - 1.0) * R_FUEL_ZONE,
                (2.0 * prn() - 1.0) * R_FUEL_ZONE,
            );
            if p.norm() >= R_FUEL_ZONE {
                continue;
            }
            n += 1;
            let m = pebble.material_at(p).expect("inside the fuel zone");
            assert!(
                m < 6,
                "a point inside the fuel zone resolved to material {m}, not a layer"
            );
            hits[m] += 1;
        }

        let triso: f64 = hits[..5].iter().sum::<usize>() as f64 / N as f64;
        let names = ["kernel", "buffer", "IPyC", "SiC", "OPyC"];
        let r = [spec.kernel, spec.buffer, spec.ipyc, spec.sic, spec.opyc];
        let o3 = spec.opyc.powi(3);

        // 1. Layer shares OF THE TRISO MATERIAL -- pure geometry.
        for i in 0..5 {
            let lo = if i == 0 { 0.0 } else { r[i - 1].powi(3) };
            let expect = (r[i].powi(3) - lo) / o3;
            let got = hits[i] as f64 / N as f64 / triso;
            let rel = (got - expect) / expect;
            eprintln!(
                "[explicit pebble] {:<7} share of TRISO {got:.6}  geometry {expect:.6}  {:+.2} %",
                names[i],
                100.0 * rel
            );
            assert!(
                rel.abs() < 0.01,
                "{}: holds {got:.6} of the TRISO material, geometry says {expect:.6} ({:+.2} %) \
                 -- material_at is mis-resolving a coating layer",
                names[i],
                100.0 * rel
            );
        }

        // 2. Two independent estimators of the ball's packing fraction agree ...
        eprintln!(
            "[explicit pebble] ball pf: material_at {triso:.5}, volume_fraction_in_ball \
             {ball_pf:.5}, packer nominal {:.5}",
            spec.packing_fraction
        );
        assert!(
            (triso - ball_pf).abs() < 0.003,
            "material_at says the fuel sphere is {triso:.5} TRISO but \
             volume_fraction_in_ball says {ball_pf:.5}"
        );
        // ... and both say the clip inflates it well past the packer's nominal pf.
        let inflation = triso / spec.packing_fraction - 1.0;
        assert!(
            (0.01..0.05).contains(&inflation),
            "the ball's packing fraction is {triso:.5} against a nominal {:.5} \
             ({:+.2} %). The clip is expected to inflate it by ~2.4 %: if this has \
             gone to zero the packer changed its centre confinement, and \
             examples/fhr_ring_rpt_endf.rs is now over-correcting for it.",
            spec.packing_fraction,
            100.0 * inflation
        );
    }

    fn mat_comp(id: i32, comps: &[(usize, f64)]) -> Material {
        mat(id, comps)
    }

    fn mat(id: i32, comps: &[(usize, f64)]) -> Material {
        Material {
            id,
            name: format!("m{id}"),
            temperature: 600.0,
            components: comps
                .iter()
                .map(|&(nuclide_idx, atom_density)| NuclideComponent {
                    nuclide_idx,
                    atom_density,
                })
                .collect(),
        }
    }

    #[test]
    fn layer_volumes_sum_to_particle_volume() {
        let s = TrisoSpec::FHR_HALEU_UCO;
        let v: f64 = s.layer_volumes().iter().sum();
        let ball = 4.0 / 3.0 * std::f64::consts::PI * s.opyc.powi(3);
        assert!((v - ball).abs() < 1e-15 * ball, "{v} vs {ball}");
    }

    /// Tiny hand-built packing for [`ExplicitTrisoPebble::material_at`]
    /// tests: one TRISO particle centred at the origin, using the reference
    /// [`TrisoSpec::FHR_HALEU_UCO`] radii, in a fuel zone of radius 1.0 cm
    /// wrapped in a graphite shell out to 2.0 cm.
    fn one_particle_pebble() -> (ExplicitTrisoPebble, TrisoSpec) {
        use crate::pebble_beds::sphere_packing::Sphere;
        use crate::geometry::position::Position;

        let spec = TrisoSpec::FHR_HALEU_UCO;
        let r_fuel_zone = 1.0;
        let r_pebble = 2.0;
        let sphere = Sphere {
            center: Position::new(0.0, 0.0, 0.0),
            radius: spec.opyc,
        };
        let packed = PackedSpheres::from_spheres(vec![sphere], r_fuel_zone, spec.opyc);

        const KERNEL_MAT: usize = 0;
        const BUFFER_MAT: usize = 1;
        const IPYC_MAT: usize = 2;
        const SIC_MAT: usize = 3;
        const OPYC_MAT: usize = 4;
        const GRAPHITE_MAT: usize = 5; // matrix AND shell, as in the real pebble
        const COOLANT_MAT: usize = 6;

        let pebble = ExplicitTrisoPebble::new(
            packed,
            spec,
            TrisoMaterials {
                kernel: KERNEL_MAT,
                buffer: BUFFER_MAT,
                ipyc: IPYC_MAT,
                sic: SIC_MAT,
                opyc: OPYC_MAT,
                matrix: GRAPHITE_MAT,
            },
            GRAPHITE_MAT,
            COOLANT_MAT,
            r_fuel_zone,
            r_pebble,
        );
        (pebble, spec)
    }

    #[test]
    fn material_at_kernel_returns_fuel_index() {
        use crate::geometry::position::Position;
        let (pebble, _) = one_particle_pebble();
        // r = 0.02 cm < spec.kernel (0.0215) — inside the kernel.
        assert_eq!(
            pebble.material_at(Position::new(0.02, 0.0, 0.0)),
            Some(0),
            "point inside the TRISO kernel must resolve to the fuel material index"
        );
    }

    #[test]
    fn material_at_coating_layers_return_layer_indices() {
        use crate::geometry::position::Position;
        let (pebble, _) = one_particle_pebble();
        // Same radii as `triso_layer_classification` above, one per layer.
        assert_eq!(
            pebble.material_at(Position::new(0.03, 0.0, 0.0)),
            Some(1),
            "buffer layer"
        );
        assert_eq!(
            pebble.material_at(Position::new(0.034, 0.0, 0.0)),
            Some(2),
            "IPyC layer"
        );
        assert_eq!(
            pebble.material_at(Position::new(0.037, 0.0, 0.0)),
            Some(3),
            "SiC layer"
        );
        assert_eq!(
            pebble.material_at(Position::new(0.041, 0.0, 0.0)),
            Some(4),
            "OPyC layer"
        );
    }

    #[test]
    fn material_at_matrix_returns_graphite() {
        use crate::geometry::position::Position;
        let (pebble, spec) = one_particle_pebble();
        // r = 0.5 cm: well outside the one particle (opyc = spec.opyc) but
        // still inside the 1.0 cm fuel zone — the surrounding matrix.
        assert!(0.5 > spec.opyc);
        assert_eq!(
            pebble.material_at(Position::new(0.5, 0.0, 0.0)),
            Some(5),
            "point in the matrix between packed particles must resolve to graphite"
        );
    }

    #[test]
    fn material_at_past_shell_returns_coolant() {
        use crate::geometry::position::Position;
        let (pebble, _) = one_particle_pebble();
        // r = 2.5 cm > r_pebble (2.0 cm) — past the graphite shell, in the
        // coolant.
        assert_eq!(
            pebble.material_at(Position::new(2.5, 0.0, 0.0)),
            Some(6),
            "point past the graphite shell must resolve to coolant"
        );
    }

    #[test]
    fn triso_layer_classification() {
        let s = TrisoSpec::FHR_HALEU_UCO;
        assert_eq!(triso_layer_at(0.0, &s), Some(TrisoLayer::Kernel));
        assert_eq!(triso_layer_at(0.02, &s), Some(TrisoLayer::Kernel));
        assert_eq!(triso_layer_at(0.03, &s), Some(TrisoLayer::Buffer));
        assert_eq!(triso_layer_at(0.034, &s), Some(TrisoLayer::Ipyc));
        assert_eq!(triso_layer_at(0.037, &s), Some(TrisoLayer::Sic));
        assert_eq!(triso_layer_at(0.041, &s), Some(TrisoLayer::Opyc));
        assert_eq!(triso_layer_at(0.05, &s), None);
    }

    #[test]
    fn homogenise_conserves_nuclide_content() {
        // 2 parts: material A (nuclide 0 @ 1.0) volume 1, material B (nuclide 0
        // @ 3.0, nuclide 1 @ 2.0) volume 3. Expect N0 = 0.25*1 + 0.75*3 = 2.5,
        // N1 = 0.75*2 = 1.5.
        let a = mat(1, &[(0, 1.0)]);
        let b = mat(2, &[(0, 3.0), (1, 2.0)]);
        let h = homogenise_by_volume(&[(&a, 1.0), (&b, 3.0)], 9, "homog", 600.0);
        let n0 = h
            .components
            .iter()
            .find(|c| c.nuclide_idx == 0)
            .unwrap()
            .atom_density;
        let n1 = h
            .components
            .iter()
            .find(|c| c.nuclide_idx == 1)
            .unwrap()
            .atom_density;
        assert!((n0 - 2.5).abs() < 1e-12, "N0 = {n0}");
        assert!((n1 - 1.5).abs() < 1e-12, "N1 = {n1}");
    }

    #[test]
    fn rpt_shell_volume_equals_particle_volume() {
        // fuel zone r = 1.9, pf = 0.3, inner = 1.493359375
        let r_in = 1.493359375;
        let r_fz = 1.9;
        let pf = 0.3;
        let r_out = rpt_fuel_outer_radius(r_in, r_fz, pf);
        let ball = |r: f64| 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
        let shell_v = ball(r_out) - ball(r_in);
        let particle_v = pf * ball(r_fz);
        assert!(
            (shell_v - particle_v).abs() < 1e-9 * particle_v,
            "shell {shell_v} vs particle {particle_v}"
        );
        assert!((r_out - 1.7534).abs() < 1e-3, "r_out = {r_out}");
    }

    /// A neutron streaming radially outward through the pebble must reflect at
    /// the root sphere, not leak — regression for the nested-CSG reflective
    /// boundary.
    #[test]
    fn reflective_root_does_not_leak() {
        use crate::geometry::position::{Direction, Position};
        let g = fhr_pebble_geometry(
            1.4934,
            1.7534,
            2.0,
            3.0,
            0,
            1,
            2,
            BoundaryType::Reflective,
            600.0,
        );
        // Start in the coolant, heading straight out.
        let mut r = Position::new(2.5, 0.0, 0.0);
        let u = Direction::new(1.0, 0.0, 0.0);
        let path = g
            .locate(r, u, crate::geometry::cell::SurfaceToken::NONE)
            .expect("located in coolant");
        let db = g.distance_to_boundary(&path);
        assert!(
            db.distance.is_finite() && db.distance <= 0.5 + 1e-9,
            "distance to the r=3 boundary from r=2.5 should be ~0.5, got {}",
            db.distance
        );
        r = crate::geometry::position::stream(r, u, db.distance);
        if let crate::geometry::geometry::Crossing::Surface(i) = db.crossing {
            assert!(
                g.cross_surface(i, r, u).alive,
                "root sphere must be reflective (alive after crossing)"
            );
        } else {
            panic!(
                "expected a surface crossing at the root sphere, got {:?}",
                db.crossing
            );
        }
    }

    /// Regression for **`op-mzvp.2.11` / GitHub #168** — a fully reflected
    /// pebble must conserve neutrons.
    ///
    /// # Methodology
    ///
    /// Transport the four-region concentric-sphere pebble
    /// (`inner graphite ball / homogenised fuel shell / graphite shell /
    /// coolant shell`, outermost sphere **reflective**) through
    /// [`run_keff_reactor_physics`] — 300 particles, 5 inactive + 10 active
    /// generations, LOW-tier `U235`/`U238`/`C0` core data at 600 K. With every
    /// escape path closed, the tallied leakage per source neutron must be zero
    /// to within the harness tolerance; the pass criterion is
    /// `leakage_total.mean < 1e-3`.
    ///
    /// This is a **harness / conservation check, not physics V&V**: it says the
    /// tracker does not lose neutrons, and says nothing about whether the
    /// eigenvalue is right.
    ///
    /// # Results
    ///
    /// - **Before the fix** (2026-09-10, commit `ee06b5d`): leakage
    ///   **0.8463 per source neutron** — ~85 % of the population lost, k_eff
    ///   ≈ 0.14 against an expected ≈ 1.3.
    /// - **After the fix** (2026-09-10, this change): leakage
    ///   **exactly 0.0 per source neutron**, k_eff **1.30450 ± 0.02347**.
    ///
    /// # Why it used to fail
    ///
    /// It takes *shell* cells — two concentric surfaces per region — to trigger
    /// it. A neutron crossing an internal sphere landed (to within round-off)
    /// exactly on it; `locate` re-derived the sign of `Surface::evaluate` there,
    /// picked the cell the neutron had just **left**, and the next
    /// `distance_to_boundary` then found no forward surface at all — the one it
    /// was sitting on is suppressed as coincident, and the region's *other*
    /// surface was behind it. The flight distance came back `INFINITY`, so the
    /// neutron streamed its sampled `d_col` clean out of the geometry and
    /// "collided" in vacuum, banking fission sites at positions no cell
    /// contains. The fix is to carry the crossed surface **and the side landed
    /// on** ([`crate::geometry::cell::SurfaceToken`]) rather than re-deriving it.
    #[test]
    fn reflective_pebble_transport_does_not_leak() {
        use crate::material::nuclide::Nuclide;
        use crate::physics::keff::KeffSettings;
        use crate::physics::reactor_physics::{run_keff_reactor_physics, ReactorPhysicsConfig};
        use crate::physics::transport_csg::SourceBox;
        use crate::geometry::position::Position;

        let nucs = vec![
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("U238").unwrap(),
            Nuclide::from_core("C0").unwrap(),
        ];
        let fuel = mat_comp(0, &[(0, 4.5e-3), (1, 1.8e-2), (2, 2.0e-2)]);
        let graphite = mat_comp(1, &[(2, 8.0e-2)]);
        let coolant = mat_comp(2, &[(2, 4.0e-2)]);
        let r_inner = 1.4934;
        let r_fuel = rpt_fuel_outer_radius(r_inner, 1.9, 0.30);
        let g = fhr_pebble_geometry(
            r_inner,
            r_fuel,
            2.0,
            3.0,
            0,
            1,
            2,
            BoundaryType::Reflective,
            600.0,
        );
        let cfg = ReactorPhysicsConfig {
            keff: KeffSettings {
                n_particles: 300,
                n_inactive: 5,
                n_active: 10,
                ..KeffSettings::default()
            },
            source_box: SourceBox {
                lower: Position::new(-r_fuel, -r_fuel, -r_fuel),
                upper: Position::new(r_fuel, r_fuel, r_fuel),
            },
            n_fine_bins: 60,
            ..Default::default()
        };
        let rep =
            run_keff_reactor_physics(&g, &[fuel, graphite, coolant], &nucs, &cfg).expect("Ok");
        eprintln!(
            "[op-mzvp.2.11] reflective pebble: k_eff = {:.5} +/- {:.5}, leakage = {:.3e} per source neutron",
            rep.keff.k_mean, rep.keff.k_std, rep.leakage_total.mean
        );
        assert!(
            rep.leakage_total.mean < 1.0e-3,
            "reflective pebble leaked {} per source neutron — op-mzvp.2.11 / GH #168 regressed",
            rep.leakage_total.mean
        );
        // A fully reflected fuelled pebble is strongly multiplying; the collapse
        // to k ~ 0.14 was the leak's signature, so gate on it too.
        assert!(
            rep.keff.k_mean > 1.0,
            "reflective pebble k_eff = {} (expected ~1.3) — op-mzvp.2.11 / GH #168 regressed",
            rep.keff.k_mean
        );
    }

    #[test]
    fn pebble_geometry_locates_every_region() {
        use crate::geometry::position::{Direction, Position};
        let g = fhr_pebble_geometry(
            1.4934,
            1.7534,
            2.0,
            3.0,
            0,
            1,
            2,
            BoundaryType::Reflective,
            600.0,
        );
        let u = Direction::new(1.0, 0.0, 0.0);
        let leaf_mat = |r: f64| {
            g.locate(
                Position::new(r, 0.0, 0.0),
                u,
                crate::geometry::cell::SurfaceToken::NONE,
            )
            .and_then(|p| p.material)
        };
        assert_eq!(leaf_mat(0.5), Some(1), "inner graphite");
        assert_eq!(leaf_mat(1.6), Some(0), "homogenised fuel shell");
        assert_eq!(leaf_mat(1.9), Some(1), "graphite shell");
        assert_eq!(leaf_mat(2.5), Some(2), "coolant");
    }
}
