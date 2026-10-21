//! The IAEA 3-D PWR benchmark case.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `iaea3ds.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//! - **Composition maps:** `src/data/IAEA3DS_*.csv`; see
//!   `src/data/PROVENANCE.md`.
//!
//! # Why this case matters more than the others
//!
//! It is **pure neutronics**. There is no thermal-hydraulic feedback, no fuel
//! rod, no coolant — just a fixed two-group cross-section set on a fixed
//! material map. So it exercises the whole nodal-diffusion stack against a
//! *published eigenvalue* without any of the coupling layer in the way, and it
//! is the first thing in this crate that can be compared to a number someone
//! else computed.
//!
//! # The problem
//!
//! A 17 x 17 x 19 quarter-core PWR on a 10 cm mesh — 170 x 170 x 380 cm —
//! reflective on the low `x` and `y` faces (the quarter-core symmetry planes)
//! and vacuum on the other four. Two energy groups, five materials, fission
//! only in the fast group's daughter: `chi = [1, 0]`.
//!
//! # The cross sections are `nu * Sigma_f`, not `Sigma_f`
//!
//! `constants.nu` is **all ones**, so `sigmavalues.f` already carries the
//! `nu * Sigma_f` product. That is the benchmark's own convention and it is why
//! a `nu` of 1 is not a mistake here. Everything downstream multiplies by `nu`
//! anyway, so the arithmetic works out.
//!
//! The rest reconstructs to the published specification exactly: `D1 = 1.5`,
//! `D2 = 0.4` in fuel via `Sigma_tot = 1/(3D)`, absorption 0.01 and 0.08 in the
//! two groups of outer fuel, and a down-scatter of 0.02 with no up-scatter.
//! Those identities are checked by a test rather than asserted here.

use crate::geometry_ends3d::geometry_ends3d;
use crate::matlab::{Array2, Array3};
use crate::types::{BoundaryCondition, Geometry, Params, SigmaValues};

/// The four axial composition maps, embedded at build time.
const LAYER_BOTTOM_REFLECTOR: &str = include_str!("data/IAEA3DS_1.csv");
const LAYER_LOWER_CORE: &str = include_str!("data/IAEA3DS_2.csv");
const LAYER_UPPER_CORE: &str = include_str!("data/IAEA3DS_3.csv");
const LAYER_TOP_REFLECTOR: &str = include_str!("data/IAEA3DS_4.csv");

/// The benchmark's reference eigenvalue, as `iaea3ds.m`'s header records it.
///
/// Two independent codes are quoted there, agreeing to 1.4 pcm:
///
/// | Code | `k_eff` |
/// |---|---|
/// | PARCS | 1.029096 |
/// | ADPRES | 1.029082 |
///
/// **These come from that header, not from a primary publication** — see
/// `src/data/PROVENANCE.md` before citing them.
pub const REFERENCE_K_EFF_PARCS: f64 = 1.029_096;
/// The second reference eigenvalue; see [`REFERENCE_K_EFF_PARCS`].
pub const REFERENCE_K_EFF_ADPRES: f64 = 1.029_082;

/// Parse one 17-by-17 comma-separated integer map.
///
/// # Panics
/// If the file is not 17 rows of 17 integers.
fn parse_map(text: &str) -> Array2<usize> {
    let rows: Vec<Vec<usize>> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.split(',')
                .map(|c| {
                    c.trim()
                        .parse::<usize>()
                        .unwrap_or_else(|e| panic!("bad material number {c:?}: {e}"))
                })
                .collect()
        })
        .collect();

    assert_eq!(rows.len(), 17, "expected 17 rows, got {}", rows.len());
    let mut a = Array2::<usize>::zeros(17, 17);
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), 17, "row {i} has {} entries, expected 17", row.len());
        for (j, v) in row.iter().enumerate() {
            a.set(i, j, *v);
        }
    }
    a
}

/// `[params, geometry, constants, whichsigma, sigmavalues] = iaea3ds(params)`.
///
/// Builds the complete IAEA-3D case: extents, mesh, boundary conditions, the
/// two-group five-material cross-section set, and the material map.
///
/// # Returns
///
/// `(params, geometry, whichsigma, sigmavalues)`. The reference's `constants`
/// output carries only `chi`, `nu` and `frac_p`, all of which are already on
/// `sigmavalues` or `params`, so it is not returned separately.
///
/// # The mesh is fixed at 17 x 17 x 19
///
/// The reference computes `xscale = maxix/17` and friends as `int64` and uses
/// them to index the maps, which would in principle allow a refined mesh. But
/// the axial layer boundaries are then written as `14*zscale`, `18*zscale`, and
/// the radial lookup is `ceil(ix/maxix*17)` — an identity only at 17. This
/// translation fixes the mesh at the benchmark's own 17 x 17 x 19 and asserts
/// it, rather than reproducing a refinement path the reference never exercises
/// and that its own header (`FOR NODE SIZE = 10 cm`) does not claim.
///
/// # Panics
///
/// If `params.maxix`, `maxiy` or `maxiz` is set to anything other than
/// 17, 17, 19.
pub fn iaea3ds(params: &Params) -> (Params, Geometry, Array3<usize>, SigmaValues) {
    const NX: usize = 17;
    const NY: usize = 17;
    const NZ: usize = 19;

    let mut params = params.clone();
    params.maxix = Some(NX);
    params.maxiy = Some(NY);
    params.maxiz = Some(NZ);
    params.nc = Some(0);
    params.g = 2;

    let es = NX * NY * NZ;

    // ----- mesh -----
    let (xtot, ytot, ztot) = (170.0, 170.0, 380.0);
    let (sx, sy, sz) = (xtot / NX as f64, ytot / NY as f64, ztot / NZ as f64);

    let uniform = |rows: usize, cols: usize, v: usize| {
        let mut a = Array2::<usize>::zeros(rows, cols);
        for i in 0..rows {
            for j in 0..cols {
                a.set(i, j, v);
            }
        }
        a
    };

    let mut geometry = Geometry {
        xtot,
        ytot,
        lx: vec![sx; es],
        ly: vec![sy; es],
        lz: vec![sz; es],
        vi: vec![sx * sy * sz; es],
        // Quarter-core symmetry planes on the low faces.
        xmin: BoundaryCondition::Reflective,
        xmax: BoundaryCondition::Vacuum,
        ymin: BoundaryCondition::Reflective,
        ymax: BoundaryCondition::Vacuum,
        zmin: BoundaryCondition::Vacuum,
        zmax: BoundaryCondition::Vacuum,
        // `geometry_ends3d` fills these below.
        xlows: Some(uniform(NY, NZ, 0)),
        xhis: Some(uniform(NY, NZ, 0)),
        ylows: Some(uniform(NX, NZ, 0)),
        yhis: Some(uniform(NX, NZ, 0)),
        zlows: Some(uniform(NX, NY, 0)),
        zhis: Some(uniform(NX, NY, 0)),
        ..Default::default()
    };

    // ----- cross sections -----
    // Five materials: outer fuel, inner fuel, inner fuel + rod, reflector,
    // reflector + rod. `Sigma_tot = 1/(3D)`, so 1/4.5 is D = 1.5.
    let tot_rows = [
        [1.0 / 4.5, 1.0 / 1.2],
        [1.0 / 4.5, 1.0 / 1.2],
        [1.0 / 4.5, 1.0 / 1.2],
        [1.0 / 6.0, 1.0 / 0.9],
        [1.0 / 6.0, 1.0 / 0.9],
    ];
    let f_rows = [
        [0.0, 0.135],
        [0.0, 0.135],
        [0.0, 0.135],
        [0.0, 0.0],
        [0.0, 0.0],
    ];
    // `s(material, gt, g)` — destination group first. Row `gt`, column `g`, as
    // the reference's `[s11 s12; s21 s22]` literals lay them out.
    let s_rows = [
        [[1.0 / 4.5 - 0.03, 0.0], [0.020, 1.0 / 1.2 - 0.08]],
        [[1.0 / 4.5 - 0.03, 0.0], [0.020, 1.0 / 1.2 - 0.085]],
        [[1.0 / 4.5 - 0.03, 0.0], [0.020, 1.0 / 1.2 - 0.13]],
        [[1.0 / 6.0 - 0.04, 0.0], [0.040, 1.0 / 0.9 - 0.01]],
        [[1.0 / 6.0 - 0.04, 0.0], [0.040, 1.0 / 0.9 - 0.055]],
    ];

    let mut tot = Array2::<f64>::zeros(5, 2);
    let mut f = Array2::<f64>::zeros(5, 2);
    let mut s = Array3::<f64>::zeros(5, 2, 2);
    let mut nu = Array2::<f64>::zeros(5, 2);
    let mut chi = Array2::<f64>::zeros(5, 2);

    for m in 0..5 {
        for g in 0..2 {
            tot.set(m, g, tot_rows[m][g]);
            f.set(m, g, f_rows[m][g]);
            // `constants.nu = ones(5, G)` — `f` already carries nu*Sigma_f.
            nu.set(m, g, 1.0);
        }
        for (gt, row) in s_rows[m].iter().enumerate() {
            for (g, value) in row.iter().enumerate() {
                s.set(m, gt, g, *value);
            }
        }
        // `constants.chi(:,1) = 1` — every fission neutron born fast.
        chi.set(m, 0, 1.0);
    }

    let sigmavalues = SigmaValues {
        tot,
        f,
        s,
        nu,
        chi,
        fp: None,
    };

    // ----- material map -----
    let bottom = parse_map(LAYER_BOTTOM_REFLECTOR);
    let lower = parse_map(LAYER_LOWER_CORE);
    let upper = parse_map(LAYER_UPPER_CORE);
    let top = parse_map(LAYER_TOP_REFLECTOR);

    let mut whichsigma = Array3::<usize>::zeros(NX, NY, NZ);
    for iz in 0..NZ {
        // 1-based axial bands, as the reference writes them: 1, 2..14, 15..18,
        // 19.
        let layer = match iz + 1 {
            1 => &bottom,
            2..=14 => &lower,
            15..=18 => &upper,
            _ => &top,
        };
        for ix in 0..NX {
            for iy in 0..NY {
                whichsigma.set(ix, iy, iz, layer.get(ix, iy));
            }
        }
    }

    geometry_ends3d(&params, &mut geometry, &whichsigma);

    (params, geometry, whichsigma, sigmavalues)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cross-section set reconstructs the published IAEA-3D specification.
    ///
    /// # Methodology
    ///
    /// The reference writes the data in the form the benchmark publishes it —
    /// `Sigma_tot = 1/(3D)` and scattering as `total minus removal` — which
    /// obscures the quantities a reader would check. This recovers those:
    /// diffusion coefficients, absorption cross sections and the down-scatter,
    /// and compares them against the specification's own values.
    ///
    /// | Material | `D1` | `D2` | `Sigma_a1` | `Sigma_a2` | `Sigma_s,1->2` |
    /// |---|---|---|---|---|---|
    /// | outer fuel | 1.5 | 0.4 | 0.010 | 0.080 | 0.020 |
    /// | inner fuel | 1.5 | 0.4 | 0.010 | 0.085 | 0.020 |
    /// | inner + rod | 1.5 | 0.4 | 0.010 | 0.130 | 0.020 |
    /// | reflector | 2.0 | 0.3 | 0.000 | 0.010 | 0.040 |
    /// | reflector + rod | 2.0 | 0.3 | 0.000 | 0.055 | 0.040 |
    ///
    /// Pass criterion: every entry to 1e-12, and no up-scattering anywhere.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Every one of the 25 recovered quantities matched to 1e-12, and no
    /// material up-scatters. The reconstructed table is exactly the one in the
    /// table above.
    ///
    /// **Interpretation.** The data really is the published IAEA-3D set, in
    /// the units the port expects. This matters because the eigenvalue
    /// comparison below would be meaningless if the cross sections had been
    /// mistranscribed into a different problem that happened to give a similar
    /// `k_eff`.
    #[test]
    fn the_cross_sections_reconstruct_the_published_specification() {
        let (_, _, _, sv) = iaea3ds(&Params::default());

        let expected: [(f64, f64, f64, f64, f64); 5] = [
            (1.5, 0.4, 0.010, 0.080, 0.020),
            (1.5, 0.4, 0.010, 0.085, 0.020),
            (1.5, 0.4, 0.010, 0.130, 0.020),
            (2.0, 0.3, 0.000, 0.010, 0.040),
            (2.0, 0.3, 0.000, 0.055, 0.040),
        ];

        for (m, (d1, d2, a1, a2, s12)) in expected.iter().enumerate() {
            // D = 1/(3 Sigma_tot).
            let got_d1 = 1.0 / (3.0 * sv.tot.get(m, 0));
            let got_d2 = 1.0 / (3.0 * sv.tot.get(m, 1));
            // Absorption = total - all out-scatter from that group.
            let out1: f64 = (0..2).map(|gt| sv.s.get(m, gt, 0)).sum();
            let out2: f64 = (0..2).map(|gt| sv.s.get(m, gt, 1)).sum();
            let got_a1 = sv.tot.get(m, 0) - out1;
            let got_a2 = sv.tot.get(m, 1) - out2;
            let got_s12 = sv.s.get(m, 1, 0);

            eprintln!(
                "material {}: D = ({got_d1:.4}, {got_d2:.4}), Sa = ({got_a1:.4}, {got_a2:.4}), Ss12 = {got_s12:.4}",
                m + 1
            );
            assert!((got_d1 - d1).abs() < 1e-12, "D1 of material {}", m + 1);
            assert!((got_d2 - d2).abs() < 1e-12, "D2 of material {}", m + 1);
            assert!((got_a1 - a1).abs() < 1e-12, "Sa1 of material {}", m + 1);
            assert!((got_a2 - a2).abs() < 1e-12, "Sa2 of material {}", m + 1);
            assert!((got_s12 - s12).abs() < 1e-12, "Ss12 of material {}", m + 1);
            // No up-scattering in this benchmark.
            assert_eq!(sv.s.get(m, 0, 1), 0.0, "material {} up-scatters", m + 1);
        }

        // Fission only in the thermal group, and every neutron born fast.
        for m in 0..5 {
            assert_eq!(sv.f.get(m, 0), 0.0);
            assert_eq!(sv.chi.get(m, 0), 1.0);
            assert_eq!(sv.chi.get(m, 1), 0.0);
        }
    }

    /// The material map loads with the expected axial structure.
    ///
    /// # Methodology
    ///
    /// The four maps cover levels 1, 2-14, 15-18 and 19. Structural checks:
    /// the bottom and top levels carry **no fuel**; the core levels do; and the
    /// rodded fuel material 3 appears where the rods sit.
    ///
    /// **Material `0` is not a defect here.** All four maps carry zeros in the
    /// lattice corners, which is the stepped octagonal core outline — the
    /// benchmark's quarter core is not a square. Zero is the port's standard
    /// "no material" marker, so those positions are simply outside the reactor.
    /// The first version of this test asserted every entry was 4 or 5 at the
    /// reflector levels and failed on exactly those corners.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Bottom level `{0, 4}`, core levels `{0, 1, 2, 3}`, top `{0, 4, 5}`, and
    /// **48 void positions at every one of the 19 levels** — the core outline
    /// is a right prism, as the benchmark specifies.
    #[test]
    fn the_material_map_has_the_expected_axial_structure() {
        let (_, _, ws, _) = iaea3ds(&Params::default());

        let level_materials = |iz: usize| {
            let mut seen = std::collections::BTreeSet::new();
            for ix in 0..17 {
                for iy in 0..17 {
                    seen.insert(ws.get(ix, iy, iz));
                }
            }
            seen
        };

        let bottom = level_materials(0);
        let lower = level_materials(6);
        let upper = level_materials(16);
        let top = level_materials(18);
        eprintln!("bottom {bottom:?}, lower core {lower:?}, upper core {upper:?}, top {top:?}");

        // No fuel in either reflector level; 0 is "outside the core".
        assert!(
            bottom.iter().all(|m| *m == 0 || *m == 4),
            "the bottom level should be reflector or void, got {bottom:?}"
        );
        assert!(
            top.iter().all(|m| *m == 0 || *m >= 4),
            "the top level should be reflector or void, got {top:?}"
        );
        assert!(top.contains(&5), "the top reflector holds the withdrawn rods");
        assert!(lower.contains(&1) && lower.contains(&2), "the core needs fuel");
        assert!(
            upper.contains(&3) || lower.contains(&3),
            "a rodded fuel material should appear somewhere"
        );
        // Every material number is one of the five defined, or the void marker.
        for iz in 0..19 {
            for m in level_materials(iz) {
                assert!(m <= 5, "material {m} at level {iz} is undefined");
            }
        }
        // The core outline is the same at every level - the corners are void
        // throughout, not just in the reflectors.
        let void_at = |iz: usize| {
            let mut n = 0;
            for ix in 0..17 {
                for iy in 0..17 {
                    if ws.get(ix, iy, iz) == 0 {
                        n += 1;
                    }
                }
            }
            n
        };
        let n0 = void_at(0);
        eprintln!("void positions per level: {n0} at the bottom");
        for iz in 0..19 {
            assert_eq!(void_at(iz), n0, "level {iz} has a different core outline");
        }
    }

    /// The embedded maps still match the snapshot's CSVs byte for byte, modulo
    /// the stripped byte-order mark.
    ///
    /// # Methodology
    ///
    /// `src/data/PROVENANCE.md` claims the only edit made to the originals was
    /// removing the UTF-8 BOM. This re-parses the embedded text and checks the
    /// shape and value range, which is the part of that claim this crate can
    /// verify on its own — the originals live outside the repository.
    #[test]
    fn the_embedded_maps_parse_to_seventeen_square() {
        for (name, text) in [
            ("IAEA3DS_1", LAYER_BOTTOM_REFLECTOR),
            ("IAEA3DS_2", LAYER_LOWER_CORE),
            ("IAEA3DS_3", LAYER_UPPER_CORE),
            ("IAEA3DS_4", LAYER_TOP_REFLECTOR),
        ] {
            assert!(
                !text.starts_with('\u{feff}'),
                "{name} still carries a byte-order mark"
            );
            let m = parse_map(text);
            assert_eq!((m.rows(), m.cols()), (17, 17), "{name} is not 17x17");
        }
    }


    /// **The benchmark comparison.** IAEA-3D `k_eff` against the published
    /// reference.
    ///
    /// # Methodology
    ///
    /// The complete case built by [`iaea3ds`] is handed to
    /// [`crate::sanodaldiffusion_solverxyz`] — the semi-analytic nodal solver
    /// the reference's own drivers call — with no thermal-hydraulic feedback,
    /// no control-rod motion and no coupling. Just the eigenvalue problem.
    ///
    /// The reference values are the two `iaea3ds.m` quotes in its header,
    /// [`REFERENCE_K_EFF_PARCS`] (1.029096) and [`REFERENCE_K_EFF_ADPRES`]
    /// (1.029082). Read `src/data/PROVENANCE.md` before citing them: they come
    /// from that header, not from a primary publication checked here.
    ///
    /// `params.nodalupd` is set to 6, which is what the reference's own default
    /// `ceil((17 + 17 + 19) / 10)` gives for this mesh — this case is well
    /// clear of the interval-1 instability of defect N1.
    ///
    /// **Pass criterion: within 50 pcm of the PARCS value.** The first run of
    /// this test used a 500 pcm band, chosen before the answer was known to be
    /// a real test without asserting more than a coarse-mesh nodal method can
    /// support. The measured result came in at 1.1 pcm, so the band is tightened
    /// to 50 pcm — the solve is deterministic, so a tight bound is a genuine
    /// regression guard rather than a flakiness risk. It is still ~45x looser
    /// than the observed agreement, leaving room for a legitimate change to the
    /// nodal-update interval or a tolerance.
    ///
    /// # What this does and does not establish
    ///
    /// Passing means the nodal-diffusion stack — cross-section expansion,
    /// diffusion coefficients, the gradient operator, the SANM correction, the
    /// transverse-leakage chain, the expansions, and the eigenvalue iteration —
    /// computes a published reactor's multiplication factor. That is the first
    /// **validation** evidence in this crate, as distinct from the
    /// verification everything else rests on.
    ///
    /// It does **not** validate the thermal-hydraulics, the coupling, or the
    /// transient path: this case exercises none of them. Nor does it establish
    /// the assembly power distribution, which the benchmark also publishes and
    /// which is not compared here.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// | | `k_eff` | difference |
    /// |---|---|---|
    /// | **this port, SANM nodal** | **1.029084** | — |
    /// | PARCS | 1.029096 | **-1.1 pcm** |
    /// | ADPRES | 1.029082 | **+0.2 pcm** |
    ///
    /// Converged in 256 source iterations with 42 nodal rebuilds; final
    /// fission-source residual 9.611e-7, `k_eff` residual 9.272e-10.
    ///
    /// **Interpretation.** The port reproduces the benchmark to **1.1 pcm** of
    /// PARCS and **0.2 pcm** of ADPRES — closer to ADPRES than the two
    /// reference codes are to each other (they differ by 1.4 pcm). At that
    /// level the residual is comparable to the spread between independent
    /// codes solving the same problem, which is the most one can ask of a
    /// comparison against two quoted numbers.
    ///
    /// This is the **first validation evidence in the crate**. Everything
    /// before it was verification — analytical limits, published property
    /// tables, cross-checks between independently transcribed files. This is
    /// the nodal-diffusion stack computing a real reactor's multiplication
    /// factor and agreeing with codes that were not involved in producing it.
    ///
    /// The scope stays narrow, though, and the section above says why: no
    /// thermal-hydraulics, no coupling, no transient, and no comparison against
    /// the benchmark's published assembly powers.
    #[test]
    fn the_eigenvalue_matches_the_published_benchmark() {
        let params = Params {
            // The reference's own default for this mesh; see N1.
            nodalupd: 6,
            ..Default::default()
        };
        let (params, geometry, whichsigma, sigmavalues) = iaea3ds(&params);

        let out = crate::sanodaldiffusion_solverxyz::sanodaldiffusion_solverxyz(
            &geometry,
            &params,
            &sigmavalues,
            &whichsigma,
            None,
            None,
        )
        .expect("the IAEA-3D case should solve");

        let pcm_parcs = (out.k_eff - REFERENCE_K_EFF_PARCS) / REFERENCE_K_EFF_PARCS * 1e5;
        let pcm_adpres = (out.k_eff - REFERENCE_K_EFF_ADPRES) / REFERENCE_K_EFF_ADPRES * 1e5;

        eprintln!("IAEA-3D, 17x17x19, SANM nodal:");
        eprintln!("  k_eff        = {:.6}", out.k_eff);
        eprintln!("  PARCS        = {REFERENCE_K_EFF_PARCS:.6}   ({pcm_parcs:+.1} pcm)");
        eprintln!("  ADPRES       = {REFERENCE_K_EFF_ADPRES:.6}   ({pcm_adpres:+.1} pcm)");
        eprintln!(
            "  termination  = {:?} after {} source iterations, {} nodal rebuilds",
            out.termination, out.iterations, out.nodal_updates
        );
        eprintln!(
            "  residuals    = fission source {:.3e}, k_eff {:.3e}",
            out.residual, out.k_eff_residual
        );

        assert_eq!(
            out.termination,
            crate::sanodaldiffusion_solverxyz::Termination::Converged,
            "the benchmark case must converge"
        );
        assert!(
            pcm_parcs.abs() < 50.0,
            "k_eff = {:.6} is {pcm_parcs:+.1} pcm from the PARCS reference of {REFERENCE_K_EFF_PARCS:.6}",
            out.k_eff
        );
    }

    /// The converged flux is positive everywhere and peaks inside the core.
    ///
    /// # Methodology
    ///
    /// A fundamental-mode flux cannot change sign, and on a quarter core that
    /// is reflective on its low `x` and `y` faces the peak must sit near that
    /// corner rather than at the vacuum boundaries. This is a shape check on
    /// the same solve the eigenvalue test runs, catching a converged-but-wrong
    /// solution that happened to land on a plausible `k_eff`.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **Zero** negative entries out of 10 982, and the fast-group peak at
    /// node `(2, 3, 8)` with a value of 9.911 — well inside the fuelled
    /// region, near the reflective corner radially and at the axial mid-plane.
    ///
    /// **Interpretation.** The solution is a genuine fundamental mode, not a
    /// higher harmonic or a sign-changing artefact that happened to land on a
    /// plausible eigenvalue. The axial peak at level 8 of 19 is slightly below
    /// mid-height, which is what the rods inserted into the upper core
    /// (materials 3 and 5, levels 15-18) should produce — the flux is pushed
    /// downward away from the absorber.
    #[test]
    fn the_converged_flux_is_positive_and_peaks_inside_the_core() {
        let params = Params {
            nodalupd: 6,
            ..Default::default()
        };
        let (params, geometry, whichsigma, sigmavalues) = iaea3ds(&params);
        let out = crate::sanodaldiffusion_solverxyz::sanodaldiffusion_solverxyz(
            &geometry, &params, &sigmavalues, &whichsigma, None, None,
        )
        .unwrap();

        let (nx, ny, nz) = (17usize, 17usize, 19usize);
        let es = nx * ny * nz;
        let flux = |g: usize, ix: usize, iy: usize, iz: usize| {
            out.scalar_flux.get(g * es + ix * ny * nz + iy * nz + iz, 0)
        };

        // Non-negative everywhere; fuelled nodes strictly positive.
        let mut negatives = 0;
        for i in 0..out.scalar_flux.rows() {
            if out.scalar_flux.get(i, 0) < 0.0 {
                negatives += 1;
            }
        }
        eprintln!("negative flux entries: {negatives} of {}", out.scalar_flux.rows());
        assert_eq!(negatives, 0, "the fundamental mode must not change sign");

        // The peak of the fast group, over the fuelled region.
        let mut peak = (0usize, 0usize, 0usize);
        let mut peakval = 0.0;
        for ix in 0..nx {
            for iy in 0..ny {
                for iz in 0..nz {
                    let v = flux(0, ix, iy, iz);
                    if v > peakval {
                        peakval = v;
                        peak = (ix, iy, iz);
                    }
                }
            }
        }
        eprintln!("fast-group peak at {peak:?}, value {peakval:.4}");
        // Near the reflective corner radially, and off the axial ends.
        assert!(peak.0 < 8 && peak.1 < 8, "the peak should sit near the symmetry corner");
        assert!(peak.2 > 1 && peak.2 < nz - 2, "the peak should be off the axial ends");
    }

    /// The mesh and boundary conditions match the benchmark statement.
    #[test]
    fn the_mesh_is_the_benchmarks_own() {
        let (p, g, _, _) = iaea3ds(&Params::default());
        assert_eq!((p.maxix, p.maxiy, p.maxiz), (Some(17), Some(17), Some(19)));
        assert_eq!(p.g, 2);
        // 10 cm radially, 20 cm axially.
        assert!((g.lx[0] - 10.0).abs() < 1e-12);
        assert!((g.ly[0] - 10.0).abs() < 1e-12);
        assert!((g.lz[0] - 20.0).abs() < 1e-12);
        // Quarter-core symmetry on the low faces only.
        assert_eq!(g.xmin, BoundaryCondition::Reflective);
        assert_eq!(g.ymin, BoundaryCondition::Reflective);
        assert_eq!(g.xmax, BoundaryCondition::Vacuum);
        assert_eq!(g.zmin, BoundaryCondition::Vacuum);
    }
}
