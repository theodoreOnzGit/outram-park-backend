// SPDX-License-Identifier: AGPL-3.0-only
//! **OpenMC's pincell depletion example on this crate** (gh:#1626 arm 2).
//!
//! The model of OpenMC `e4febe9`'s `examples/pincell_depletion/run_depletion.py`
//! (UO2 / He / Zircaloy-4 / borated water with H in H2O, reflective pitch
//! 1.25984 cm, infinite in z), the same ACE library OpenMC reads after
//! conversion (`verification_and_validation/spectrum_depletion/openmc_inputs/
//! pincell_ace_to_hdf5.py`), `chain_simple.xml`, and the same depletion:
//! Predictor, five 1-day steps at 174 W/cm, 1000 particles x [10 + 90].
//! The depletion operator is [`CoupledOperator`], OpenMC's `CoupledOperator`
//! in `reaction_rate_mode = "flux"` with the CCFE-709 group bounds; the
//! integrator is PETIR's Predictor with CRAM-48, as upstream.
//!
//! The initial compositions, the fuel area and the group bounds are read
//! from the OpenMC run's own output (`ref_pincell_depletion.py`), so both
//! codes start from the same numbers. The comparison is
//! `tests/pincell_depletion_vs_openmc.rs`.
//!
//! GUI-free CLI glue; its arithmetic is in `depletion::coupled_operator`,
//! which `tests/coupled_operator.rs` reaches.
//!
//! ```text
//! RAYON_NUM_THREADS=4 cargo run --release -p outram-mc-libs --example pincell_depletion_vs_openmc -- \
//!   --lib <ace lib> --ref <openmc pincell.tsv> --chain <chain_simple.xml> --seeds 1 16 --threads 4 --out <port.tsv>
//! ```

use njoy_outram_park_fork::acer::ce_decode::decode_ce;
use njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml;
use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::coupled_operator::{reaction_from_ace, ChainNuclideData, CoupledNormalization, CoupledOperator};
use outram_mc_libs::depletion::integrators::{CramSolver, DepletionIntegrator, Integrator};
use outram_mc_libs::material::material_sab::{assign_material_sab, MaterialSab};
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::prelude::*;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

/// The pincell, infinite in z, as upstream's example builds it.
fn geometry() -> Geometry {
    let half = 1.25984 / 2.0;
    let cyl = |r: f64| SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r, bc: BoundaryType::Transmissive });
    let r = BoundaryType::Reflective;
    let surfaces = vec![
        cyl(0.39218),
        cyl(0.40005),
        cyl(0.45720),
        SurfaceKind::XPlane(XPlane { x0: -half, bc: r }),
        SurfaceKind::XPlane(XPlane { x0: half, bc: r }),
        SurfaceKind::YPlane(YPlane { y0: -half, bc: r }),
        SurfaceKind::YPlane(YPlane { y0: half, bc: r }),
    ];
    let hs = |surface_idx: usize, sense: HalfSpaceSense| RegionToken::HalfSpace { surface_idx, sense };
    let (o, i, and) = (HalfSpaceSense::Outside, HalfSpaceSense::Inside, RegionToken::Intersection);
    let cells = vec![
        Cell::material(1, vec![hs(0, i)], 0, 293.6),
        Cell::material(2, vec![hs(0, o), hs(1, i), and.clone()], 1, 293.6),
        Cell::material(3, vec![hs(1, o), hs(2, i), and.clone()], 2, 293.6),
        Cell::material(
            4,
            vec![hs(2, o), hs(3, o), and.clone(), hs(4, i), and.clone(), hs(5, o), and.clone(), hs(6, i), and],
            3,
            293.6,
        ),
    ];
    Geometry { surfaces, cells, universes: vec![Universe { id: 0, cell_indices: vec![0, 1, 2, 3] }], lattices: vec![], root_universe: 0 }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lib = PathBuf::from(flag(&args, "--lib").expect("--lib"));
    let reference = std::fs::read_to_string(flag(&args, "--ref").expect("--ref")).expect("read --ref");
    let chain_path = flag(&args, "--chain").expect("--chain");
    let out_path = flag(&args, "--out").expect("--out");
    let si = args.iter().position(|a| a == "--seeds").expect("--seeds a b");
    let threads: usize = flag(&args, "--threads").map_or(4, |t| t.parse().expect("--threads"));
    let (s0, s1): (u64, u64) = (args[si + 1].parse().unwrap(), args[si + 2].parse().unwrap());

    // OpenMC's initial state: compositions, fuel area, group bounds, atoms.
    let mut edges: Vec<f64> = Vec::new();
    let mut comps: Vec<(String, String, f64)> = Vec::new();
    let mut volume = 0.0;
    let mut initial: Vec<(String, f64)> = Vec::new();
    for line in reference.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f.as_slice() {
            ["edge", _, e] => edges.push(e.parse().unwrap()),
            ["comp", m, n, d] => comps.push(((*m).into(), (*n).into(), d.parse().unwrap())),
            ["volume", v] => volume = v.parse().unwrap(),
            ["atoms", "flux", "1", "0", n, a] => initial.push(((*n).into(), a.parse().unwrap())),
            _ => {}
        }
    }
    let mat_names: Vec<String> = comps.iter().fold(Vec::new(), |mut v, c| {
        if !v.contains(&c.0) {
            v.push(c.0.clone());
        }
        v
    });
    assert_eq!(mat_names.len(), 4, "four materials");

    let xml = DepletionChainXml::parse(&std::fs::read_to_string(&chain_path).expect("chain")).expect("chain xml");
    let chain = DepletionChain::from_chain_xml(&xml, 0.0253).expect("chain");
    let chain_names: Vec<String> = chain.nuclide_names().iter().map(|s| (*s).to_owned()).collect();

    // Transport nuclides: every composition nuclide, then the chain's with data.
    let table = |n: &str| lib.join("293.6K").join(format!("{n}.ace.gz"));
    let mut names: Vec<String> = Vec::new();
    for c in &comps {
        if !names.contains(&c.1) {
            names.push(c.1.clone());
        }
    }
    for n in &chain_names {
        if !names.contains(n) && table(n).is_file() {
            names.push(n.clone());
        }
    }
    let t0 = std::time::Instant::now();
    let sab_raw = njoy_outram_park_fork::acer::read::read(lib.join("293.6K/HinH2O.ace.gz")).expect("HinH2O");
    let nuclides: Vec<Nuclide> = names.iter().map(|n| Nuclide::from_ace_file(table(n), n).unwrap_or_else(|e| panic!("{n}: {e}"))).collect();
    let h_in_h2o = ThermalScattering::from_ace(&sab_raw, "c_H_in_H2O").expect("S(a,b)");
    eprintln!("{} nuclides loaded in {:.1} s", nuclides.len(), t0.elapsed().as_secs_f64());

    let data: Vec<ChainNuclideData> = chain_names
        .iter()
        .map(|n| {
            let q = xml.nuclides.iter().find(|x| &x.name == n).and_then(|x| x.reactions.iter().find(|r| r.kind == "fission")).map_or(0.0, |r| r.q_ev);
            match names.iter().position(|m| m == n) {
                None => ChainNuclideData { fission_q: q, ..Default::default() },
                Some(t) => {
                    let raw = njoy_outram_park_fork::acer::read::read(table(n)).unwrap();
                    let ace = decode_ce(&raw).unwrap();
                    ChainNuclideData {
                        transport: Some(t),
                        gamma: reaction_from_ace(&raw, &ace, 102),
                        fission: reaction_from_ace(&raw, &ace, 18),
                        n2n: reaction_from_ace(&raw, &ace, 16),
                        fission_q: q,
                    }
                }
            }
        })
        .collect();

    let materials: Vec<Material> = mat_names
        .iter()
        .enumerate()
        .map(|(id, m)| Material {
            id: id as i32 + 1,
            name: m.clone(),
            temperature: 293.6,
            components: comps
                .iter()
                .filter(|c| &c.0 == m)
                .map(|c| NuclideComponent { nuclide_idx: names.iter().position(|x| x == &c.1).unwrap(), atom_density: c.2 })
                .collect(),
        })
        .collect();
    // H in H2O per material, as OpenMC binds it (gh:#462): every material
    // holding H-1 is the moderator here, and gets a bound copy of H-1
    // appended after the transport nuclides; the free entry stays for the
    // others (none hold H-1).
    let h1 = names.iter().position(|n| n == "H1").expect("H1 in the model");
    let entries = MaterialSab::for_materials_containing(&materials, h1, 0, &[]);
    assert_eq!(entries.len(), 1, "only the water holds H-1");
    let (materials, nuclides) = assign_material_sab(&materials, &nuclides, &[h_in_h2o], &entries).expect("per-material H(H2O)");
    let n0: Vec<f64> = chain_names.iter().map(|n| initial.iter().find(|(m, _)| m == n).map(|x| x.1).expect("initial atoms")).collect();

    let geom = Arc::new(geometry());
    let nuclides = Arc::new(nuclides);
    let source = SourceBox { lower: Position::new(-0.62992, -0.62992, -1.0), upper: Position::new(0.62992, 0.62992, 1.0) };
    let mut out = std::fs::OpenOptions::new().create(true).append(true).open(&out_path).expect("out");
    for seed in s0..=s1 {
        let t = std::time::Instant::now();
        let settings = KeffSettings { n_particles: 1000, n_inactive: 10, n_active: 90, seed: seed * 1_000_003, temperature_k: 293.6, ..KeffSettings::default() }
            .with_compute(ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)));
        let mut op = CoupledOperator::new(
            geom.clone(),
            nuclides.clone(),
            materials.clone(),
            source,
            settings,
            vec![0],
            vec![volume],
            chain.clone(),
            data.clone(),
            edges.clone(),
            CoupledNormalization::FissionQ,
        )
        .expect("operator");
        let mut integ = DepletionIntegrator::new(Integrator::Predictor, CramSolver::Cram48, vec![86_400.0; 5], vec![174.0; 5]).unwrap();
        let records = integ.integrate(&mut op, vec![n0.clone()], true).unwrap_or_else(|e| panic!("{e:?}"));
        for rec in &records {
            writeln!(out, "k\tport\t{seed}\t{}\t{:?}\t{:?}", rec.index, rec.k.value, rec.k.std_dev).unwrap();
            for (j, n) in chain_names.iter().enumerate() {
                writeln!(out, "atoms\tport\t{seed}\t{}\t{n}\t{:?}", rec.index, rec.n[0][j]).unwrap();
                let r = rec.rates.0[0].rate_for(n);
                writeln!(out, "rate\tport\t{seed}\t{}\t{n}\t(n,gamma)\t{:?}", rec.index, r.gamma).unwrap();
                writeln!(out, "rate\tport\t{seed}\t{}\t{n}\tfission\t{:?}", rec.index, r.fission).unwrap();
            }
        }
        out.flush().unwrap();
        let ks: Vec<String> = records.iter().map(|r| format!("{:.5}", r.k.value)).collect();
        eprintln!("seed {seed}: k {} ({:.1} s)", ks.join(" "), t.elapsed().as_secs_f64());
    }
    let _ = Path::new(&out_path);
}
