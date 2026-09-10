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
use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use crate::geometry::universe::Universe;
use crate::material::material::{Material, NuclideComponent};

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
    assert!(total_v > 0.0, "homogenise_by_volume: total volume must be > 0");

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
        SurfaceKind::XPlane(XPlane { x0: -h, bc: BoundaryType::Reflective }),
        SurfaceKind::XPlane(XPlane { x0: h, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: -h, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: h, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: -h, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: h, bc: BoundaryType::Reflective }),
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
        let n0 = h.components.iter().find(|c| c.nuclide_idx == 0).unwrap().atom_density;
        let n1 = h.components.iter().find(|c| c.nuclide_idx == 1).unwrap().atom_density;
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
            1.4934, 1.7534, 2.0, 3.0, 0, 1, 2, BoundaryType::Reflective, 600.0,
        );
        // Start in the coolant, heading straight out.
        let mut r = Position::new(2.5, 0.0, 0.0);
        let u = Direction::new(1.0, 0.0, 0.0);
        let path = g.locate(r, u, usize::MAX).expect("located in coolant");
        let db = g.distance_to_boundary(&path);
        assert!(
            db.distance.is_finite() && db.distance <= 0.5 + 1e-9,
            "distance to the r=3 boundary from r=2.5 should be ~0.5, got {}",
            db.distance
        );
        r = crate::geometry::position::stream(r, u, db.distance);
        if let crate::geometry::geometry::Crossing::Surface(i) = db.crossing {
            let (_, _, alive) = g.cross_surface(i, r, u);
            assert!(alive, "root sphere must be reflective (alive after crossing)");
        } else {
            panic!("expected a surface crossing at the root sphere, got {:?}", db.crossing);
        }
    }

    /// **Known-failing regression for `op-mzvp.2.11`.** `run_keff_csg` leaks
    /// ~87 % of neutrons on this four-region concentric-sphere reflective
    /// geometry: a near-tangent transmissive crossing of a curved surface plus
    /// the fixed 1e-9 nudge lands the neutron back on the wrong side, `locate`
    /// re-picks the cell it was leaving, and the next `distance_to_boundary`
    /// finds no forward surface → the neutron streams to infinity. A single
    /// internal sphere (`fuel ball + one reflective shell`) works; the failure
    /// needs an all-concentric-sphere geometry to trigger reliably. Unignore
    /// once the surface-tracking fix lands.
    #[test]
    #[ignore = "op-mzvp.2.11: run_keff_csg near-tangent curved-surface crossing leaks"]
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
            r_inner, r_fuel, 2.0, 3.0, 0, 1, 2, BoundaryType::Reflective, 600.0,
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
        let rep = run_keff_reactor_physics(&g, &[fuel, graphite, coolant], &nucs, &cfg)
            .expect("Ok");
        assert!(
            rep.leakage_total.mean < 1.0e-3,
            "reflective pebble leaked {} per source neutron — op-mzvp.2.11",
            rep.leakage_total.mean
        );
    }

    #[test]
    fn pebble_geometry_locates_every_region() {
        use crate::geometry::position::{Direction, Position};
        let g = fhr_pebble_geometry(
            1.4934, 1.7534, 2.0, 3.0, 0, 1, 2, BoundaryType::Reflective, 600.0,
        );
        let u = Direction::new(1.0, 0.0, 0.0);
        let leaf_mat = |r: f64| {
            g.locate(Position::new(r, 0.0, 0.0), u, usize::MAX)
                .and_then(|p| p.material)
        };
        assert_eq!(leaf_mat(0.5), Some(1), "inner graphite");
        assert_eq!(leaf_mat(1.6), Some(0), "homogenised fuel shell");
        assert_eq!(leaf_mat(1.9), Some(1), "graphite shell");
        assert_eq!(leaf_mat(2.5), Some(2), "coolant");
    }
}
