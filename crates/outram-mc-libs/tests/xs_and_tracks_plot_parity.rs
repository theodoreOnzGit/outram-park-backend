// SPDX-License-Identifier: GPL-3.0-only

//! **Code-to-code verification of the non-geometry plotting port**
//! (`outram_mc_libs::plotter`) against OpenMC's own Python functions:
//! `openmc.plot_xs`, `openmc.Track.plot`, `openmc.Tracks.plot`
//! (OpenMC `d7d3284a1`).
//!
//! # Methodology
//!
//! The port computes in Rust the arrays OpenMC hands to matplotlib and emits a
//! standalone matplotlib script that makes the same calls. The reference is
//! OpenMC itself, run by
//! `verification_and_validation/python_plotting_parity/xs_and_tracks/openmc_inputs/reference_driver.py`
//! on **the same ACE files**: NJOY2016's ENDF/B-VIII.0 U-234/U-235/U-238 at
//! 293.6 K from the `reference-data/ace` submodule, and a C-12 table the
//! workspace's own `njoy-outram-park-fork` wrote (committed as
//! `data/C12.ace.gz`; the same bytes feed both sides, so its provenance does
//! not enter the comparison). OpenMC reads them with
//! `IncidentNeutron.from_ace -> export_to_hdf5 -> plot_xs(ce_cross_sections=...)`;
//! this crate reads them with `IncidentNeutronData::from_ace`.
//!
//! Two gates, both fixed before the first run:
//!
//! 1. **Data identity, no Python needed:** every plotted line's x and y arrays
//!    hash (FNV-1a 64 over the little-endian float64 bytes) to the values the
//!    reference driver recorded in `reference/line_fingerprints.tsv`, with the
//!    same labels, in the same order. Bit identity, not a tolerance.
//! 2. **Pixel identity, when `OUTRAM_PYTHON` names a Python with numpy and
//!    matplotlib (3.11.2 for the committed references):** each emitted script,
//!    run with `MPLBACKEND=Agg`, writes a PNG with **0 differing pixels**
//!    against the committed reference PNG, decoded with
//!    `outram_mc_libs::geometry::plot::decode_png`. Without `OUTRAM_PYTHON`
//!    the test prints SKIP.
//!
//! Negative control: the same script with one plotted value changed
//! (`y[len/2] *= 1.5`) must differ in fingerprint and, when Python is
//! available, in pixels.
//!
//! # Results (2026-09-26, first run, nothing adjusted)
//!
//! 13 figures (11 `plot_xs`, `Tracks.plot`, `Track.plot`), 53 plotted lines:
//! **53/53 lines bit-identical (all fingerprints match; max abs and max
//! relative difference 0), 0 differing pixels on all 13 PNGs** (640x480, and
//! 800x500 for the `figsize=(8, 5)` case; matplotlib 3.11.2, numpy 2.5.3,
//! OpenMC 0.16.1.dev25+gd7d3284a1). Negative control: the fingerprint changes,
//! and scaling U-235's total at 1 eV by 1.5 gives **79 differing pixels**.
//! Ablation: removing upstream's `np.isclose` end-point snap from
//! `Tabulated1D` (see `plotter::function1d`) fails gate 1 on the very first
//! line (U-235 total), so the gate is sensitive to that level of detail.
//! Full record:
//! `verification_and_validation/python_plotting_parity/xs_and_tracks/README.md`.
//!
//! Setting `OUTRAM_WRITE_VV=1` rewrites the committed Rust-side artefacts:
//! `data/tracks_input.tsv`, `ours/*.png` (with `OUTRAM_PYTHON`) and those
//! `ours/*.py` of at most [`MAX_COMMITTED_SCRIPT`] bytes. The larger scripts
//! (the U-235/U-238 grids make them 1-13 MB) are regenerated into the temp
//! directory on every run instead of being committed; gate 1 asserts they are
//! deterministic.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::plotter::script::fnv1a64_f64;
use outram_mc_libs::plotter::xs::{MaterialDensity, PlotMaterial};
use outram_mc_libs::plotter::{
    plot_xs, track_plot_script, tracks_plot_script, EnergyAxisUnits, FigureKwargs,
    IncidentNeutronData, PlotTarget, PlotTrack, PlotXsOptions, XsFigure, XsLibrary, XsType,
};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

/// Emitted scripts larger than this are not committed (see the module docs).
const MAX_COMMITTED_SCRIPT: usize = 1_000_000;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/python_plotting_parity/xs_and_tracks")
}

fn write_vv() -> bool {
    std::env::var("OUTRAM_WRITE_VV").is_ok_and(|v| v == "1")
}

/// The library both sides read, or `None` (SKIP) without the ACE submodule.
fn library() -> Option<&'static XsLibrary> {
    static LIB: OnceLock<Option<XsLibrary>> = OnceLock::new();
    LIB.get_or_init(|| {
        use njoy_outram_park_fork::reference_data::ace_reference_file_or_skip;
        let mut lib = XsLibrary::new();
        for n in ["U234", "U235", "U238"] {
            let rel = format!("reference-njoy/endf-b-viii.0/293.6K/{n}.ace.gz");
            let p = ace_reference_file_or_skip(&rel, &format!("xs_plot_parity/{n}"))?;
            lib = lib.with_nuclide(IncidentNeutronData::from_ace_file(&p, None).expect(n));
        }
        let c12 = vv_dir().join("data/C12.ace.gz");
        lib = lib.with_nuclide(IncidentNeutronData::from_ace_file(&c12, None).expect("C12"));
        Some(lib)
    })
    .as_ref()
}

fn t(names: &[&str]) -> Vec<XsType> {
    names.iter().map(|s| XsType::from(*s)).collect()
}

fn heu() -> PlotMaterial {
    // Same as the reference driver's `materials()`.
    PlotMaterial::new(1, "HEU metal")
        .with_nuclide_ao("U234", 4.9184e-4)
        .with_nuclide_ao("U235", 4.4994e-2)
        .with_nuclide_ao("U238", 2.4984e-3)
        .with_density(MaterialDensity::Sum)
}

fn graphite() -> PlotMaterial {
    let mut m = PlotMaterial::new(2, "")
        .with_nuclide_ao("C12", 1.0)
        .with_density(MaterialDensity::AtomPerBarnCm(8.5238e-2));
    m.temperature = Some(ThermodynamicTemperature::new::<kelvin>(600.0));
    m
}

/// The eleven `plot_xs` cases, mirroring `reference_driver.py::cases()`.
fn xs_cases(lib: &XsLibrary) -> Vec<(&'static str, XsFigure)> {
    let n = |s: &str| PlotTarget::from(s);
    let def = PlotXsOptions::default();
    let run = |r: Vec<(PlotTarget, Vec<XsType>)>, div: Option<Vec<XsType>>, o: &PlotXsOptions| {
        plot_xs(&r, div.as_deref(), lib, o).expect("plot_xs")
    };
    let mut scatter = t(&["scatter", "inelastic", "nu-scatter"]);
    scatter.push(XsType::Mt(16));
    scatter.extend(t(&["(n,2n)", "damage", "unity", "slowing-down power"]));
    let mut u234 = t(&["fission", "nu-fission", "total"]);
    u234.push(XsType::Mt(19));
    let mut c12 = t(&["total", "elastic", "inelastic", "capture"]);
    c12.push(XsType::Mt(51));
    c12.extend(t(&["(n,a)", "(n,p)", "nu-scatter"]));
    let kev = PlotXsOptions {
        energy_axis_units: EnergyAxisUnits::KeV,
        ..def.clone()
    };
    let mev = PlotXsOptions {
        energy_axis_units: EnergyAxisUnits::MeV,
        figure: FigureKwargs {
            figsize: Some((8.0, 5.0)),
            dpi: None,
        },
        ..def.clone()
    };
    vec![
        (
            "xs_u235_basic",
            run(
                vec![(n("U235"), t(&["total", "elastic", "fission", "capture", "absorption", "nu-fission"]))],
                None,
                &def,
            ),
        ),
        ("xs_u235_scatter_types", run(vec![(n("U235"), scatter)], None, &def)),
        ("xs_u234_partial_fission", run(vec![(n("U234"), u234)], None, &kev)),
        ("xs_c12_types", run(vec![(n("C12"), c12)], None, &def)),
        ("xs_c12_heating", run(vec![(n("C12"), t(&["heating"]))], None, &def)),
        (
            "xs_two_nuclides_mev",
            run(
                vec![(n("U235"), t(&["fission"])), (n("U238"), t(&["fission", "capture"]))],
                None,
                &mev,
            ),
        ),
        (
            "xs_u235_divisor",
            run(
                vec![(n("U235"), t(&["nu-fission", "fission", "elastic"]))],
                Some(t(&["fission", "absorption", "total"])),
                &def,
            ),
        ),
        (
            "xs_u235_divided_by_unity",
            run(vec![(n("U235"), t(&["capture"]))], Some(t(&["unity"])), &def),
        ),
        (
            "xs_material_heu",
            run(
                vec![(
                    PlotTarget::Material(heu()),
                    t(&["total", "elastic", "fission", "nu-fission", "capture", "unity"]),
                )],
                None,
                &def,
            ),
        ),
        (
            "xs_material_graphite_divisor",
            run(
                vec![(PlotTarget::Material(graphite()), t(&["elastic", "total"]))],
                Some(t(&["total", "unity"])),
                &def,
            ),
        ),
        ("xs_element_u", run(vec![(n("U"), t(&["total", "fission", "(n,gamma)"]))], None, &def)),
    ]
}

/// Tracks from a real traced run: a 2 MeV point source at the centre of a
/// 15 cm C-12 sphere (the committed `data/C12.ace.gz`), eight histories.
fn traced_tracks() -> Vec<outram_mc_libs::physics::track_output::Track> {
    use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
    use outram_mc_libs::geometry::geometry::Geometry;
    use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
    use outram_mc_libs::geometry::universe::Universe;
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::physics::fixed_source::{run_fixed_source_traced, FixedSource, FixedSourceSettings};
    use outram_mc_libs::physics::track_output::TrackRecorder;

    let c12 = Nuclide::from_ace_file(vv_dir().join("data/C12.ace.gz"), "C12").expect("C12 nuclide");
    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: 15.0,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::fill(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            CellFill::Material(0),
            Position::ZERO,
        )],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let mat = Material {
        id: 1,
        name: "graphite".into(),
        components: vec![NuclideComponent {
            nuclide_idx: 0,
            atom_density: 8.5238e-2,
        }],
        temperature: 293.6,
    };
    let mut rec = TrackRecorder::new(8, 60);
    run_fixed_source_traced(
        &geom,
        &[mat],
        &[c12],
        &FixedSource::Point {
            r: Position::ZERO,
            energy_ev: 2.0e6,
        },
        &FixedSourceSettings {
            n_particles: 8,
            n_batches: 1,
            seed: 20_260_926,
            ..Default::default()
        },
        None,
        Some(&mut rec),
        None,
        None,
    );
    rec.tracks
}

/// `data/tracks_input.tsv`: `track  particle  x  y  z`, floats as shortest
/// round-trip decimals.
fn tracks_to_tsv(tracks: &[PlotTrack]) -> String {
    let mut s = String::from("# track\tparticle\tx [cm]\ty [cm]\tz [cm]\n");
    for (i, tr) in tracks.iter().enumerate() {
        for (p, states) in tr.particle_tracks.iter().enumerate() {
            for r in states {
                s.push_str(&format!("{i}\t{p}\t{:?}\t{:?}\t{:?}\n", r.x, r.y, r.z));
            }
        }
    }
    s
}

fn tracks_from_tsv(text: &str) -> Vec<PlotTrack> {
    let mut m: BTreeMap<usize, BTreeMap<usize, Vec<Position>>> = BTreeMap::new();
    for row in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let f: Vec<&str> = row.split('\t').collect();
        let p = Position {
            x: f[2].parse().unwrap(),
            y: f[3].parse().unwrap(),
            z: f[4].parse().unwrap(),
        };
        m.entry(f[0].parse().unwrap())
            .or_default()
            .entry(f[1].parse().unwrap())
            .or_default()
            .push(p);
    }
    m.into_values()
        .map(|parts| PlotTrack {
            particle_tracks: parts.into_values().collect(),
        })
        .collect()
}

/// The committed track input. The first source particle carries two particle
/// tracks (the first two recorded histories), so `Track.plot` draws more than
/// one line; the rest carry one each.
fn plot_tracks() -> Vec<PlotTrack> {
    let path = vv_dir().join("data/tracks_input.tsv");
    if write_vv() || !path.exists() {
        let rec = traced_tracks();
        let mut v: Vec<PlotTrack> = Vec::new();
        let first = PlotTrack {
            particle_tracks: rec[..2]
                .iter()
                .flat_map(|t| PlotTrack::from(t).particle_tracks)
                .collect(),
        };
        v.push(first);
        v.extend(rec[2..].iter().map(PlotTrack::from));
        std::fs::write(&path, tracks_to_tsv(&v)).expect("write tracks_input.tsv");
    }
    tracks_from_tsv(&std::fs::read_to_string(&path).expect("tracks_input.tsv"))
}

/// Every emitted script, by case name, plus the per-line data it plots.
struct Emitted {
    name: &'static str,
    script: String,
    /// `(label, x, y)`; for 3-D tracks `x` is x‖y‖z concatenated and `y` empty.
    lines: Vec<(String, Vec<f64>, Vec<f64>)>,
}

fn emit_all(lib: &XsLibrary) -> Vec<Emitted> {
    let mut out = Vec::new();
    for (name, fig) in xs_cases(lib) {
        out.push(Emitted {
            name,
            script: fig.to_python_script(&format!("{name}.png")),
            lines: fig.lines.iter().map(|l| (l.label.clone(), l.x.clone(), l.y.clone())).collect(),
        });
    }
    let tracks = plot_tracks();
    let flat = |tr: &[PlotTrack]| -> Vec<(String, Vec<f64>, Vec<f64>)> {
        tr.iter()
            .flat_map(|t| t.particle_tracks.iter())
            .map(|st| {
                let mut v: Vec<f64> = st.iter().map(|p| p.x).collect();
                v.extend(st.iter().map(|p| p.y));
                v.extend(st.iter().map(|p| p.z));
                (String::new(), v, Vec::new())
            })
            .collect()
    };
    out.push(Emitted {
        name: "tracks_all",
        script: tracks_plot_script(&tracks, &FigureKwargs::default(), "tracks_all.png"),
        lines: flat(&tracks),
    });
    out.push(Emitted {
        name: "track_first",
        script: track_plot_script(&tracks[0], &FigureKwargs::default(), "track_first.png"),
        lines: flat(&tracks[..1]),
    });
    out
}

fn scratch_dir() -> PathBuf {
    let d = std::env::temp_dir().join("outram_xs_and_tracks_plot_parity");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Reference fingerprints: case -> [(label, n, fnv(x), fnv(y))].
fn reference_fingerprints() -> Option<BTreeMap<String, Vec<(String, usize, String, String)>>> {
    let text = std::fs::read_to_string(vv_dir().join("reference/line_fingerprints.tsv")).ok()?;
    let mut m: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for row in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let f: Vec<&str> = row.split('\t').collect();
        m.entry(f[0].to_string()).or_default().push((
            f[2].to_string(),
            f[3].parse().unwrap(),
            f[4].to_string(),
            f[5].to_string(),
        ));
    }
    Some(m)
}

fn fp(v: &[f64]) -> String {
    format!("{:016x}", fnv1a64_f64(v))
}

/// **Gate 1: bit identity of every plotted array, and deterministic scripts.**
#[test]
fn plotted_data_is_bit_identical_to_openmc() {
    let Some(lib) = library() else {
        println!("SKIP: reference-data/ace not available");
        return;
    };
    let a = emit_all(lib);
    let b = emit_all(lib);
    let dir = scratch_dir();
    let mut total_lines = 0;
    let refs = reference_fingerprints();
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.script, y.script, "{}: script is not deterministic", x.name);
        std::fs::write(dir.join(format!("{}.py", x.name)), &x.script).unwrap();
        if write_vv() && x.script.len() <= MAX_COMMITTED_SCRIPT {
            std::fs::write(vv_dir().join(format!("ours/{}.py", x.name)), &x.script).unwrap();
        }
        println!(
            "{:32} {:3} lines, script {:>9} bytes",
            x.name,
            x.lines.len(),
            x.script.len()
        );
        total_lines += x.lines.len();
        let Some(refs) = refs.as_ref() else { continue };
        let r = refs
            .get(x.name)
            .unwrap_or_else(|| panic!("{}: no reference lines recorded", x.name));
        assert_eq!(r.len(), x.lines.len(), "{}: number of plotted lines", x.name);
        for (i, ((label, xs, ys), (rl, rn, rfx, rfy))) in x.lines.iter().zip(r.iter()).enumerate() {
            assert_eq!(label, rl, "{} line {i}: legend label", x.name);
            let n = if ys.is_empty() { xs.len() / 3 } else { xs.len() };
            assert_eq!(n, *rn, "{} line {i} ({label}): number of points", x.name);
            assert_eq!(&fp(xs), rfx, "{} line {i} ({label}): x data differ from OpenMC's", x.name);
            if !ys.is_empty() {
                assert_eq!(&fp(ys), rfy, "{} line {i} ({label}): y data differ from OpenMC's", x.name);
            }
        }
    }
    println!(
        "{} figures, {total_lines} lines; reference fingerprints {}",
        a.len(),
        if refs.is_some() { "all matched" } else { "ABSENT (not compared)" }
    );
}

fn python() -> Option<String> {
    std::env::var("OUTRAM_PYTHON").ok().filter(|s| !s.is_empty())
}

fn render(py: &str, script: &std::path::Path, png: &std::path::Path) {
    let st = std::process::Command::new(py)
        .arg(script)
        .arg(png)
        .env("MPLBACKEND", "Agg")
        .status()
        .expect("run python");
    assert!(st.success(), "{} failed", script.display());
}

fn diff_pixels(a: &std::path::Path, b: &std::path::Path) -> Option<usize> {
    use outram_mc_libs::geometry::plot::decode_png;
    let ia = decode_png(&std::fs::read(a).unwrap()).expect("decode ours");
    let ib = decode_png(&std::fs::read(b).unwrap()).expect("decode reference");
    ia.count_differences(&ib)
}

/// **Gate 2: 0 differing pixels against OpenMC's PNGs** (needs `OUTRAM_PYTHON`).
#[test]
fn emitted_scripts_render_pixel_identical_to_openmc() {
    let Some(py) = python() else {
        println!("SKIP: set OUTRAM_PYTHON to a python with numpy + matplotlib 3.11.2");
        return;
    };
    let Some(lib) = library() else {
        println!("SKIP: reference-data/ace not available");
        return;
    };
    let dir = scratch_dir();
    for e in emit_all(lib) {
        let script = dir.join(format!("{}.py", e.name));
        std::fs::write(&script, &e.script).unwrap();
        let png = dir.join(format!("{}.png", e.name));
        render(&py, &script, &png);
        if write_vv() {
            std::fs::copy(&png, vv_dir().join(format!("ours/{}.png", e.name))).unwrap();
        }
        let reference = vv_dir().join(format!("reference/{}.png", e.name));
        let d = diff_pixels(&png, &reference);
        println!("{:32} differing pixels: {d:?}", e.name);
        assert_eq!(d, Some(0), "{}: rendered PNG differs from OpenMC's", e.name);
    }
}

/// **Negative control: a one-value perturbation must be detected** — by the
/// fingerprint always, and in pixels when Python is available.
#[test]
fn a_perturbed_value_is_detected() {
    let Some(lib) = library() else {
        println!("SKIP: reference-data/ace not available");
        return;
    };
    let mut fig = xs_cases(lib).into_iter().next().expect("first case").1;
    let before = fp(&fig.lines[0].y);
    let n = fig.lines[0].y.len();
    let mid = (0..n).find(|&i| fig.lines[0].x[i] >= 1.0).expect("a point above 1 eV");
    fig.lines[0].y[mid] *= 1.5;
    assert_ne!(fp(&fig.lines[0].y), before, "fingerprint must see a one-value change");
    let Some(py) = python() else {
        println!("SKIP (pixel half): set OUTRAM_PYTHON");
        return;
    };
    let dir = scratch_dir();
    let script = dir.join("negative_control.py");
    std::fs::write(&script, fig.to_python_script("negative_control.png")).unwrap();
    let png = dir.join("negative_control.png");
    render(&py, &script, &png);
    let d = diff_pixels(&png, &vv_dir().join("reference/xs_u235_basic.png")).expect("same size");
    println!(
        "negative control: U235 total at E = {} eV scaled by 1.5 -> {d} differing pixels",
        fig.lines[0].x[mid]
    );
    assert!(d > 0, "a visible perturbation produced no differing pixels");
}

/// Tracks recorded by this crate's transport convert to the plotting type.
#[test]
fn recorded_tracks_convert_to_plot_tracks() {
    let c12 = vv_dir().join("data/C12.ace.gz");
    if !c12.exists() {
        println!("SKIP: data/C12.ace.gz missing");
        return;
    }
    let rec = traced_tracks();
    assert_eq!(rec.len(), 8);
    for t in &rec {
        let p = PlotTrack::from(t);
        assert_eq!(p.particle_tracks.len(), 1);
        assert_eq!(p.particle_tracks[0].len(), t.states.len());
        assert!(t.states.len() >= 2, "a track has at least birth and death");
    }
}

/// Regenerates `data/C12.ace.gz`' contents (the ACE both sides read) with this
/// workspace's NJOY port. Deterministic: two runs gave byte-identical files.
/// Run with `--ignored`; it writes `C12.ace` to the temp directory, which is
/// then `gzip -9 -n`-ed into `data/`.
#[test]
#[ignore]
fn regenerate_c12_ace() {
    use njoy_outram_park_fork::interface::NuclearDataLibrary;
    let endf = njoy_outram_park_fork::reference_data::reference_endf("n-006_C_012-ENDF8.0.endf")
        .expect("C-12 tape");
    let lib = NuclearDataLibrary::from_file(&endf, 625)
        .unwrap()
        .reconstruct(0.001)
        .unwrap()
        .broaden(ThermodynamicTemperature::new::<kelvin>(293.6))
        .unwrap();
    let out = std::env::temp_dir().join("C12.ace");
    lib.write_ace(&out).unwrap();
    println!("wrote {}", out.display());
}
