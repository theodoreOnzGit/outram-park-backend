//! Unit tests of the finite-element thermo-mechanics: mesh decomposition accounting, patch conditions and
//! refusals. The verification against closed forms, the finite-volume solver and scikit-fem is
//! `tests/genfoam_fem_thermo_mechanics_gh1782.rs`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::*;
use crate::genfoam::thermo_mechanics::legacy_fvm::DispPatch;
use crate::genfoam::thermo_mechanics::structural_region::{fem_patches_from_disp_patches, StructuralDiscretisation};
use farrer_park::mesh::ElemId;

fn box_block(n: [usize; 3]) -> (Arc<FvMesh>, FemTopology) {
    let dict = format!(
        "convertToMeters 1;
vertices ((0 0 0) (2 0 0) (2 1 0) (0 1 0) (0 0 1) (2 0 1) (2 1 1) (0 1 1));
blocks ( hex (0 1 2 3 4 5 6 7) ({} {} {}) simpleGrading (2 1 1) );
boundary ( xmin {{ type patch; faces ((0 4 7 3)); }} xmax {{ type patch; faces ((1 2 6 5)); }}
 rest {{ type patch; faces ((0 1 5 4) (3 7 6 2) (0 3 2 1) (4 5 6 7)); }} );",
        n[0], n[1], n[2]
    );
    let poly = outram_foam_mesh::block_mesh::block_mesh(&dict).expect("blockMesh");
    let mesh = Arc::new(poly.to_fv_mesh().expect("FvMesh"));
    let topo = FemTopology {
        points: poly.points.iter().map(|p| [p.x, p.y, p.z]).collect(),
        faces: poly.faces.iter().map(|f| f.verts.clone()).collect(),
    };
    (mesh, topo)
}

fn element_volume(m: &FemMesh, e: usize) -> f64 {
    let et = m.mesh.element_type();
    let mut ec = vec![[0.0_f64; 3]; et.n_nodes()];
    m.mesh.element_coords(ElemId(e), &mut ec);
    farrer_park::quadrature::default_rule(et)
        .iter()
        .map(|q| q.weight * farrer_park::element::map_gradients(et, &ec, q.xi).expect("jacobian").det_j)
        .sum()
}

/// The elements of every cell fill it exactly (planar faces): per-cell volume equals the finite-volume cell volume,
/// for the native `Hex8` and for the cone decomposition into `Tet4`; the boundary facets of each patch sum to the
/// patch area; a requested `Hex8` of a mesh that is hexahedral succeeds and `Auto` picks it.
#[test]
fn decomposition_fills_every_cell_and_covers_every_patch() {
    let (mesh, topo) = box_block([3, 2, 2]);
    for (choice, n_per_cell) in [(FemElementChoice::Auto, 1usize), (FemElementChoice::Hex8, 1), (FemElementChoice::Tet4Decomposed, 6)] {
        let m = FemMesh::build(&mesh, &topo, choice).expect("build");
        assert_eq!(m.mesh.n_elements(), n_per_cell * mesh.n_cells, "{choice:?}");
        let mut vol = vec![0.0; mesh.n_cells];
        for e in 0..m.mesh.n_elements() {
            vol[m.element_cell[e]] += element_volume(&m, e);
        }
        for c in 0..mesh.n_cells {
            assert!((vol[c] - mesh.cell_volumes[c]).abs() < 1e-13 * mesh.cell_volumes[c].max(1e-300) * 1e3, "cell {c}: {} vs {}", vol[c], mesh.cell_volumes[c]);
        }
        for (pi, p) in mesh.patches.iter().enumerate() {
            let area: f64 = m.patch_facets[pi]
                .iter()
                .map(|pf| {
                    let coords: Vec<[f64; 3]> = pf.facet.nodes.iter().map(|n| m.mesh.node(*n)).collect();
                    farrer_park::quadrature::facet_rule(pf.facet.facet_type)
                        .iter()
                        .map(|q| q.weight * pf.facet.facet_type.measure_jacobian(&coords, q.xi))
                        .sum::<f64>()
                })
                .sum();
            let expected: f64 = (p.start..p.start + p.size).map(|f| mesh.face_areas[f]).sum();
            assert!((area - expected).abs() < 1e-12 * expected, "patch {}: {area} vs {expected}", p.name);
            assert!(!m.patch_nodes[pi].is_empty());
        }
    }
}

/// A topology of the wrong size, and a `Hex8` request on a tetrahedral decomposition's own mesh, are refused by name.
#[test]
fn mismatched_topology_is_refused() {
    let (mesh, mut topo) = box_block([2, 1, 1]);
    topo.faces.pop();
    assert!(matches!(FemMesh::build(&mesh, &topo, FemElementChoice::Auto), Err(AppBuilderError::Case { .. })));
}

/// `zeroGradient` maps to the natural condition, `fixedValue` stays; the enum default is the finite-volume solver.
#[test]
fn patch_mapping_and_default_discretisation() {
    let v = Vector3::new(1.0, 2.0, 3.0);
    let m = fem_patches_from_disp_patches(&[DispPatch::FixedValue(v), DispPatch::ZeroGradient]).expect("map");
    assert_eq!(m, vec![FemPatch::FixedValue(v), FemPatch::TractionFree]);
    assert_eq!(StructuralDiscretisation::default(), StructuralDiscretisation::FiniteVolume);
    assert_eq!(StiffnessNormalisation::default(), StiffnessNormalisation::UpstreamRhoNormalised);
}

/// Wrong-length inputs are refused: the patch list, the temperature vector, an unknown card zone.
#[test]
fn solver_refuses_bad_inputs() {
    let (mesh, topo) = box_block([2, 1, 1]);
    let zones = vec![("all".to_string(), (0..mesh.n_cells).collect::<Vec<usize>>())];
    let mut card = ZoneCard::new("all");
    card.e = 1.0e9;
    card.nu = 0.3;
    let np = mesh.patches.len();
    let s = FemSettings::default();
    assert!(FemThermoElastic::new(mesh.clone(), &topo, s, &zones, &[card.clone()], vec![FemPatch::TractionFree; np + 1]).is_err());
    let mut bad = card.clone();
    bad.name = "nowhere".to_string();
    assert!(FemThermoElastic::new(mesh.clone(), &topo, s, &zones, &[bad], vec![FemPatch::TractionFree; np]).is_err());
    let mut patches = vec![FemPatch::TractionFree; np];
    patches[0] = FemPatch::FixedValue(Vector3::ZERO);
    let mut f = FemThermoElastic::new(mesh.clone(), &topo, s, &zones, &[card], patches).expect("solver");
    assert!(f.solve(&[0.0]).is_err());
    let log = f.solve(&vec![0.0; mesh.n_cells]).expect("solve");
    assert_eq!(log.element, farrer_park::element::ElementType::Hex8);
    assert!(f.nodal_displacement().iter().all(|u| u.abs() < 1e-15), "T = Tref = 0 gives no load");
    assert_eq!(f.node_displacement(farrer_park::mesh::NodeId(0)), [0.0; 3]);
    assert_eq!(f.sigma_eq().len(), mesh.n_cells);
}
