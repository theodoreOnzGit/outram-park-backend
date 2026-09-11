//! **LEU-COMP-THERM-008** — the B&W critical lattices, a 2.459 w/o UO₂ rod
//! array in borated water. The one benchmark left that tests **U-238 resolved
//! resonance escape at strong self-shielding**.
//!
//! # Why this exists
//!
//! The FHR ring-RPT study (`verification_and_validation/ring_rpt/`,
//! `op-mzvp.2.12`) narrowed a **+4004 pcm** disagreement against its OpenMC
//! reference down to one scalar: the fraction of neutrons that cross 0.625 eV,
//! an effective resonance integral **11 % lower here**. Production per
//! absorption agrees to 0.07 % in the thermal group and 0.02 % in the fast
//! group, so nothing that changes a reaction-rate *ratio* can be the cause.
//!
//! Three measured criticality benchmarks have since been reproduced on this
//! same data path and driver, and none of them touches that scalar:
//!
//! | benchmark | spectrum | U-238 | `p` | result |
//! |---|---|---|---|---|
//! | HEU-MET-FAST-001 (Godiva) | fast | 5 % of HM | — | +57 ± 173 pcm |
//! | IEU-MET-FAST-002 (Jemima) | fast | 83 % / 99.3 % | — | +6 ± 173 pcm |
//! | HEU-SOL-THERM-009 case 1 | thermal | 5 % of HM | ≈ 0.95 | −18 ± 171 pcm |
//! | **this case** | **thermal** | **97.5 % of HM** | **≈ 0.75** | ? |
//!
//! Godiva and Jemima are fast — U-238 is present, in bulk, but no neutron is
//! slowed *through* the resolved resonances by a moderator, so there is no
//! self-shielded resonance integral to get wrong. HST-009 is thermal but its
//! fuel is 90 % HEU in solution: U-238 is 5 % of the heavy metal and the mixture
//! is homogeneous, so its resonance escape is ≈ 0.95 and essentially unshielded.
//!
//! LEU-COMP-THERM-008 is the complement of all three at once: **thermal**, with
//! U-238 at **97.5 %** of the heavy metal, in **lumped** fuel. The run measures
//! how lumped, from its own reconstruction: the fuel's `Σ_t` peaks at
//! **171.1 cm⁻¹ at 6.674 eV**, 434× its 0.39 cm⁻¹ at 5 eV, so the 1.030 cm
//! pellet is **176 mean free paths across** at the 6.67 eV resonance. The
//! resonance flux depression is as strong as it gets.
//!
//! **That is the whole point: this is the discriminator.** A result near 1.0000
//! clears U-238 resonance escape and leaves the reference deck as the only
//! surviving candidate. A result several thousand pcm high reproduces the FHR
//! residual on a *measured* configuration and hands the hunt a target that
//! depends on nobody's deck.
//!
//! # Results (2026-09-11, ENDF/B-VIII.0, 10 000 × [250 + 400] generations)
//!
//! ```text
//! k_eff = 1.02950 ± 0.00061      Δk = +2950 ± 61 pcm from 1.0000
//! ```
//!
//! **The second branch is what happened, and it settles the question.** The FHR
//! pebble's residual is reproduced on a *measured* critical experiment, so it is
//! not the reference deck, and it is not anything specific to that deck's
//! geometry, materials or author. It is this code, and it is **self-shielded
//! U-238 resonance absorption**.
//!
//! The magnitude agrees with the pebble quantitatively, and the prediction was
//! written down before the run. The pebble's six factors imply an effective
//! resonance integral 11 % low (`I_eff(ours)/I_eff(ref) = 0.888`). Carrying that
//! same deficit onto a lattice whose resonance escape is `p ≈ 0.75` predicts
//! `p_ours = 0.75^0.888 = 0.774`, i.e. `+3.2 %` in `k` — **+3200 pcm against the
//! +2950 ± 61 pcm measured**. One deficit, two systems that share nothing but
//! U-238 in a lump, consistent to better than 10 %.
//!
//! Source convergence: the mean `k` over the four quarters of the 250 inactive
//! generations ran 1.0233 / 1.0316 / 1.0286 / 1.0299 — settled after the first
//! quarter — and the two halves of the active block gave 1.02874 and 1.03025, a
//! 151 pcm difference against a 122 pcm 1σ, so 1.2σ and no drift.
//!
//! What this does **not** say is *which* step is wrong. Every cross section
//! involved is verified against NJOY2016's own PENDF — `σ_γ(E)` pointwise to
//! ±0.04 %, its resonance *shape* to 0.12 % worst over six resonances, and its
//! infinitely-dilute resonance integral to +0.00 % — and the tracking method is
//! excluded twice over (delta vs surface-tracked CSG agree to 18 pcm on the
//! pebble, and this case is surface-tracked while the pebble is delta-tracked,
//! yet both are high). So the defect is in what transport *does* with a correct
//! `σ_γ`, not in `σ_γ` itself. See `verification_and_validation/ring_rpt/` and
//! bead `op-mzvp.2.12` for the live hypothesis list.
//!
//! # The model, and where it comes from
//!
//! The three XML files in
//! `verification_and_validation/icsbep/leu-comp-therm-008/` are the OpenMC
//! model from `mit-crpg/benchmarks` (Paul Romano's ICSBEP model collection,
//! MIT licence). They are **parsed at run time**, not transcribed: the thing
//! under test is then the sourced specification itself, and a transcription
//! error — the failure mode this entire study has been chasing — cannot be
//! introduced by this file.
//!
//! Geometry: a 7 × 7 core lattice at 24.5364 cm pitch, each tile either water
//! or one of 22 distinct 15 × 15 pin lattices at 1.63576 cm pitch, inside a
//! vacuum-bounded cylinder `r < 76.200 cm`, `|z| < 81.662 cm`. Pins are
//! `r < 0.514858` UO₂, `0.514858 – 0.602996` Al-6061 clad, water outside.
//! 3 materials; the pyrex / vicor / alumina surfaces in the file are declared
//! but used by no cell (this is the unpoisoned core).
//!
//! ## What is approximated, and by how much
//!
//! Only the **Al-6061 clad's trace alloying elements** are omitted, because
//! this environment has no ENDF tape for them: Mg-24/25/26, Ti-46…50,
//! Cr-50/52/53/54, Fe-54/56/57, Cu-63/65, Zn-64/66/67/68/70 and B-11. Nothing
//! is omitted from the fuel, the moderator, or the boron poison — B-10, the
//! entire worth of the 1511 ppm soluble boron, is present.
//!
//! The run prints the omitted atom density and its share of the clad. To bound
//! its worth **by measurement rather than by assertion**, pass
//! `--clad-omission-bound`: the omitted density is re-added to the clad as
//! **Mn-55**, a stronger thermal absorber than any element on that list (they
//! are structural alloying metals, which is why they are in cladding at all),
//! and the run is repeated. The resulting Δk is a conservative upper bound on
//! the omission, computed from data in hand rather than from a remembered
//! cross section.
//!
//! # The reference value
//!
//! **`k_eff = 1.0000`**, because this is a *critical* configuration. An ICSBEP
//! benchmark model is an experiment measured at delayed critical, reduced to a
//! model whose `k_eff` is 1.0000 to within the evaluated experimental
//! uncertainty — 0.1–0.6 % for a lattice of this kind. The case-specific
//! uncertainty is not in the repository and the handbook is not reachable from
//! this environment, so the reference is treated as **`1.0000 ± 0.006`**, the
//! pessimistic end of that band. That is ample for a 4 % question.
//!
//! This is reasoning from what an ICSBEP critical benchmark *is*, not from a
//! remembered number — the distinction that matters, and the reason the atom
//! densities and geometry are read from a file instead of recalled.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example lct008_keff -- [--clad-omission-bound] [--particles N]
//! ```

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::compute::ComputeType;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use std::collections::BTreeMap;
use std::time::Instant;

/// The benchmark is at room temperature and the model gives no `<temperature>`,
/// so OpenMC's default applies. 293.6 K is also the tabulated temperature of the
/// `H(H2O)` law, so no thermal interpolation is involved.
const TEMP_K: f64 = 293.6;

const MATERIALS_XML: &str =
    include_str!("../verification_and_validation/icsbep/leu-comp-therm-008/materials.xml");
const GEOMETRY_XML: &str =
    include_str!("../verification_and_validation/icsbep/leu-comp-therm-008/geometry.xml");

/// Nuclides this environment has an ENDF/B-VIII.0 tape for, by the OpenMC name
/// the model uses. Anything in the model and not in this table is omitted, and
/// the omission is reported and bounded (see the module docs).
const TAPES: &[(&str, &str)] = &[
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
    ("B10", "n-005_B_010-ENDF8.0.endf"),
    ("O16", "n-008_O_016-ENDF8.0.endf"),
    ("Al27", "n-013_Al_027-ENDF8.0.endf"),
    ("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ("Si29", "n-014_Si_029-ENDF8.0.endf"),
    ("Si30", "n-014_Si_030-ENDF8.0.endf"),
    ("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
    ("U234", "n-092_U_234-ENDF8.0.endf"),
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let clad_bound = args.iter().any(|a| a == "--clad-omission-bound");
    let n_particles = arg_usize(&args, "--particles").unwrap_or(4000);
    let n_inactive = arg_usize(&args, "--inactive").unwrap_or(120);
    let n_active = arg_usize(&args, "--active").unwrap_or(250);

    eprintln!("LEU-COMP-THERM-008 — B&W critical lattice, 2.459 w/o UO₂ in 1511 ppm borated water");
    eprintln!("  specification: mit-crpg/benchmarks OpenMC model, parsed from XML at run time\n");

    let spec = parse_materials(MATERIALS_XML);
    let (nuclides, slots, omitted) = load_nuclides(&spec);
    let (materials, clad_idx) = build_materials(&spec, &slots, &omitted, false);
    report_omissions(&spec, &omitted);
    // `check_geometry`'s hand-written predicate names the material slots by
    // position, so the model's own order is pinned here rather than assumed.
    assert_eq!(
        materials.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "the model's materials are no longer water/fuel/clad as ids 1/2/3"
    );
    assert_eq!(clad_idx, 2, "material 3 is the Al-6061 clad");

    report_self_shielding(&materials[1], &nuclides);

    let geom = build_geometry(&materials, true);
    check_geometry(&geom, &materials);

    let settings = KeffSettings {
        n_particles,
        n_inactive,
        n_active,
        temperature_k: TEMP_K,
        compute: ComputeType::CpuMultiThread(Default::default()),
        ..KeffSettings::default()
    };
    // The fissionable region is the whole pin array; sample the core cylinder.
    let src = SourceBox {
        lower: Position::new(-R_CORE, -R_CORE, Z_LO),
        upper: Position::new(R_CORE, R_CORE, Z_HI),
    };
    eprintln!(
        "\n  {} histories/gen, {} inactive + {} active generations",
        settings.n_particles, settings.n_inactive, settings.n_active
    );

    let t = Instant::now();
    let result = run_keff_csg(&geom, &materials, &nuclides, src, &settings, None);
    eprintln!("  transport: {:.1} s", t.elapsed().as_secs_f64());

    let k = &result.k_by_generation;
    let trace = |lo: usize, hi: usize| -> f64 { k[lo..hi].iter().sum::<f64>() / (hi - lo) as f64 };
    let n_in = settings.n_inactive.min(k.len());
    eprintln!(
        "  source convergence, mean k over inactive quarters: {:.4} {:.4} {:.4} {:.4}",
        trace(0, n_in / 4),
        trace(n_in / 4, n_in / 2),
        trace(n_in / 2, 3 * n_in / 4),
        trace(3 * n_in / 4, n_in),
    );
    let half = n_in + (k.len() - n_in) / 2;
    eprintln!(
        "  active halves: {:.5} then {:.5} (a drift here means more inactive generations)",
        trace(n_in, half),
        trace(half, k.len()),
    );

    println!("\n=== LEU-COMP-THERM-008 ===");
    println!("  k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
    println!("  ICSBEP benchmark model (critical) = 1.0000 ± ~0.006");
    println!(
        "  Δk from the benchmark = {:+.0} ± {:.0} pcm",
        (result.k_mean - 1.0) * 1.0e5,
        result.k_std * 1.0e5
    );

    if clad_bound {
        eprintln!("\n  bounding the clad omission: omitted density re-added as Mn-55 …");
        let (bounded, _) = build_materials(&spec, &slots, &omitted, true);
        let geom2 = build_geometry(&bounded, true);
        let src2 = SourceBox {
            lower: Position::new(-R_CORE, -R_CORE, Z_LO),
            upper: Position::new(R_CORE, R_CORE, Z_HI),
        };
        let r2 = run_keff_csg(&geom2, &bounded, &nuclides, src2, &settings, None);
        println!(
            "\n  clad-omission bound: k = {:.5} ± {:.5}, Δ = {:+.0} ± {:.0} pcm",
            r2.k_mean,
            r2.k_std,
            (r2.k_mean - result.k_mean) * 1.0e5,
            (r2.k_std.powi(2) + result.k_std.powi(2)).sqrt() * 1.0e5
        );
        println!(
            "  (every omitted clad nuclide replaced by an equal atom density of Mn-55,\n   \
             which absorbs more thermal neutrons than any of them — so this is an\n   \
             upper bound on the omission, measured rather than asserted)"
        );
    }

    println!(
        "\n  This is the discriminator: the one benchmark whose fuel is both thermal\n  \
         and strongly self-shielded in the U-238 resolved resonances. Measured\n  \
         2026-09-11 at 10 000 × [250 + 400]: k = 1.02950 ± 0.00061, i.e.\n  \
         +2950 ± 61 pcm — against +4004 pcm on the FHR pebble, and against the\n  \
         +3200 pcm predicted by carrying that pebble's 11 % resonance-integral\n  \
         deficit onto p ≈ 0.75. The residual is this code's self-shielded U-238\n  \
         resonance absorption, reproduced on a measured critical experiment; it\n  \
         is not the pebble's reference deck."
    );
}

fn arg_usize(args: &[String], flag: &str) -> Option<usize> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1)?.parse().ok()
}

// ---------------------------------------------------------------------------
// A minimal XML reader.
//
// The two model files use a strict subset of XML: elements with quoted
// attributes, no namespaces, no entities, no CDATA, and text content that is
// only whitespace-separated numbers. Parsing them here — rather than taking a
// dependency, or transcribing the numbers into Rust — keeps the specification
// the single source of truth while adding nothing to the crate's dependency
// graph.
// ---------------------------------------------------------------------------

/// Every `<tag …>` element in `xml`, as `(attributes, body)`. `body` is empty
/// for a self-closing tag. Matching requires a delimiter after the tag name, so
/// `<material` does not match `<materials`.
fn elements<'a>(xml: &'a str, tag: &str) -> Vec<(&'a str, &'a str)> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(p) = xml[i..].find(&open) {
        let start = i + p;
        let after = start + open.len();
        let next = xml[after..].chars().next().unwrap_or('>');
        if !(next.is_whitespace() || next == '>' || next == '/') {
            i = after;
            continue;
        }
        let gt = start
            + xml[start..]
                .find('>')
                .unwrap_or_else(|| panic!("unterminated <{tag}>"));
        let raw = &xml[after..gt];
        let self_closing = raw.trim_end().ends_with('/');
        let attrs = raw.trim_end().trim_end_matches('/');
        let body = if self_closing {
            ""
        } else {
            let rest = &xml[gt + 1..];
            let e = rest
                .find(&close)
                .unwrap_or_else(|| panic!("unterminated <{tag}> element"));
            &rest[..e]
        };
        out.push((attrs, body));
        i = gt + 1;
    }
    out
}

/// The value of attribute `name` in an attribute string, or `None`.
fn attr<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let mut i = 0usize;
    while let Some(p) = attrs[i..].find(name) {
        let s = i + p;
        let ok_before = s == 0 || attrs[..s].ends_with(char::is_whitespace);
        let rest = attrs[s + name.len()..].trim_start();
        if ok_before && rest.starts_with('=') {
            let v = rest[1..].trim_start();
            let q = v.chars().next()?;
            if q == '"' || q == '\'' {
                let end = v[1..].find(q)? + 1;
                return Some(&v[1..end]);
            }
        }
        i = s + name.len();
    }
    None
}

/// Strip `<!-- … -->`. The geometry file comments out a surface and annotates
/// every other one; a comment body must never be read as markup.
fn strip_comments(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(p) = rest.find("<!--") {
        out.push_str(&rest[..p]);
        match rest[p..].find("-->") {
            Some(e) => rest = &rest[p + e + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

fn num<T: std::str::FromStr>(s: &str, what: &str) -> T {
    s.trim()
        .parse()
        .unwrap_or_else(|_| panic!("{what}: cannot parse {s:?}"))
}

// ---------------------------------------------------------------------------
// Materials
// ---------------------------------------------------------------------------

struct MaterialSpec {
    id: i32,
    name: String,
    nuclides: Vec<(String, f64)>,
    sab: Option<String>,
}

fn parse_materials(xml: &str) -> Vec<MaterialSpec> {
    let xml = strip_comments(xml);
    elements(&xml, "material")
        .into_iter()
        .map(|(a, body)| MaterialSpec {
            id: num(attr(a, "id").expect("material id"), "material id"),
            name: attr(a, "name").unwrap_or("").to_string(),
            nuclides: elements(body, "nuclide")
                .into_iter()
                .map(|(na, _)| {
                    (
                        attr(na, "name").expect("nuclide name").to_string(),
                        num(attr(na, "ao").expect("nuclide ao"), "nuclide ao"),
                    )
                })
                .collect(),
            sab: elements(body, "sab")
                .first()
                .and_then(|(sa, _)| attr(sa, "name"))
                .map(str::to_string),
        })
        .collect()
}

/// Reconstruct every nuclide the model asks for that this environment has a
/// tape for. Returns the nuclide array, name → slot, and the omitted set.
fn load_nuclides(
    spec: &[MaterialSpec],
) -> (Vec<Nuclide>, BTreeMap<String, usize>, BTreeMap<String, f64>) {
    let mut wanted: Vec<&str> = Vec::new();
    let mut omitted: BTreeMap<String, f64> = BTreeMap::new();
    for m in spec {
        for (name, ao) in &m.nuclides {
            match TAPES.iter().find(|(n, _)| n == name) {
                Some(_) => {
                    if !wanted.contains(&name.as_str()) {
                        wanted.push(name);
                    }
                }
                None => *omitted.entry(name.clone()).or_insert(0.0) += ao,
            }
        }
    }

    eprintln!("Reconstructing nuclides (RECONR + BROADR @ {TEMP_K} K):");
    let t0 = Instant::now();
    let sab_needed = spec.iter().any(|m| m.sab.as_deref() == Some("c_H_in_H2O"));
    let sab = sab_needed.then(|| {
        ThermalScattering::from_endf_file(
            reference_endf("tsl-HinH2O.endf")
                .expect("H(H2O) tape")
                .to_str()
                .expect("path"),
            1, // MAT 1 — H in H2O, ENDF/B-VIII.0
            TEMP_K,
            "c_H_in_H2O",
        )
        .expect("H(H2O) S(a,b)")
    });

    let mut sab = sab;
    let mut nuclides = Vec::new();
    let mut slots = BTreeMap::new();
    for name in wanted {
        let file = TAPES.iter().find(|(n, _)| *n == name).expect("tape").1;
        let mut n = load(name, file);
        if name == "H1" {
            if let Some(s) = sab.take() {
                n = n.with_thermal_scattering(s);
                eprintln!("    H1 carries c_H_in_H2O");
            }
        }
        slots.insert(name.to_string(), nuclides.len());
        nuclides.push(n);
    }
    eprintln!("Nuclear data ready in {:.1} s.", t0.elapsed().as_secs_f64());
    (nuclides, slots, omitted)
}

fn load(name: &str, file: &str) -> Nuclide {
    let p = reference_endf(file).unwrap_or_else(|| panic!("missing reference tape {file}"));
    eprint!("  reconstructing {name:<6} … ");
    let t0 = Instant::now();
    let n = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
        .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
    eprintln!("{:.1?}", t0.elapsed());
    n
}

/// Build the transport materials, in the model's own order. Returns the
/// materials and the index of the clad. With `bound_omission`, the omitted
/// clad density is re-added as Mn-55 (see the module docs).
fn build_materials(
    spec: &[MaterialSpec],
    slots: &BTreeMap<String, usize>,
    omitted: &BTreeMap<String, f64>,
    bound_omission: bool,
) -> (Vec<Material>, usize) {
    let mut clad_idx = 0usize;
    let materials = spec
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let mut components: Vec<NuclideComponent> = m
                .nuclides
                .iter()
                .filter_map(|(name, ao)| {
                    slots.get(name).map(|&nuclide_idx| NuclideComponent {
                        nuclide_idx,
                        atom_density: *ao,
                    })
                })
                .collect();
            if m.name.contains("Aluminum") || m.name.contains("Cladding") {
                clad_idx = i;
                if bound_omission {
                    let extra: f64 = m
                        .nuclides
                        .iter()
                        .filter(|(n, _)| omitted.contains_key(n))
                        .map(|(_, ao)| *ao)
                        .sum();
                    let mn = slots["Mn55"];
                    for c in components.iter_mut() {
                        if c.nuclide_idx == mn {
                            c.atom_density += extra;
                        }
                    }
                }
            }
            Material {
                id: m.id,
                name: m.name.clone(),
                temperature: TEMP_K,
                components,
            }
        })
        .collect();
    (materials, clad_idx)
}

/// How optically thick the fuel pellet is where it matters — measured from this
/// run's own reconstruction, not from a recalled cross section.
///
/// The claim this benchmark rests on is that its fuel is *strongly self-shielded*
/// in the U-238 resolved resonances, which is the one regime the other three
/// reproduced benchmarks never enter. That claim is quantitative, so it is
/// measured: `Σ_t` of the fuel at the peak of the 6.67 eV resonance, scanned on
/// a fine grid across the peak, times the pellet diameter.
fn report_self_shielding(fuel: &Material, nuclides: &[Nuclide]) {
    let (mut e_peak, mut s_peak) = (0.0_f64, 0.0_f64);
    // 6.0-7.5 eV at 0.5 meV covers the resonance (Γ ≈ 25 meV) many times over.
    let mut e = 6.0_f64;
    while e <= 7.5 {
        let s = fuel.macro_xs_total(e, nuclides);
        if s > s_peak {
            s_peak = s;
            e_peak = e;
        }
        e += 5.0e-4;
    }
    let s_off = fuel.macro_xs_total(5.0, nuclides);
    eprintln!(
        "  fuel self-shielding: Σ_t peaks at {:.1} cm⁻¹ at {:.3} eV ({:.0}× the {:.2} cm⁻¹ \
         at 5 eV);\n    the {:.3} cm pellet is {:.0} mean free paths across there",
        s_peak,
        e_peak,
        s_peak / s_off,
        s_off,
        2.0 * R_FUEL,
        2.0 * R_FUEL * s_peak,
    );
}

fn report_omissions(spec: &[MaterialSpec], omitted: &BTreeMap<String, f64>) {
    if omitted.is_empty() {
        eprintln!("  every nuclide in the model has a tape — nothing omitted.");
        return;
    }
    let names: Vec<&str> = omitted.keys().map(String::as_str).collect();
    let total: f64 = omitted.values().sum();
    for m in spec {
        let sum: f64 = m.nuclides.iter().map(|(_, ao)| *ao).sum();
        let miss: f64 = m
            .nuclides
            .iter()
            .filter(|(n, _)| omitted.contains_key(n))
            .map(|(_, ao)| *ao)
            .sum();
        if miss > 0.0 {
            eprintln!(
                "  material {} \"{}\": omitted {:.3e} of {:.3e} /b·cm ({:.2} % of its atoms)",
                m.id,
                m.name,
                miss,
                sum,
                100.0 * miss / sum
            );
        }
    }
    eprintln!(
        "  omitted (no tape in this environment): {} — {:.3e} /b·cm in total",
        names.join(", "),
        total
    );
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Core bounding cylinder [cm] — surface 30 in the model, vacuum.
const R_CORE: f64 = 76.200;
/// Core axial extent [cm] — surfaces 10 and 20, both vacuum.
const Z_LO: f64 = -81.662;
const Z_HI: f64 = 81.662;

/// The parsed model, kept so the self-check can re-derive lattice lookups by
/// its own arithmetic instead of trusting `Geometry::locate`.
struct Model {
    /// Core lattice, and each assembly lattice, by id.
    lattices: BTreeMap<i32, RectLattice>,
    /// Universe id → the lattice id filling it, for the assembly wrapper cells.
    wrapper: BTreeMap<i32, i32>,
    /// Universe id → the *universe* id filling it, for the water-tile wrapper.
    uni_fill: BTreeMap<i32, i32>,
    /// Universe id at each universe slot, in `Geometry.universes` order.
    uni_ids: Vec<i32>,
}

// The parse is done once and stashed, because `build_geometry` and
// `check_geometry` both need it and the model is immutable.
static MODEL: std::sync::OnceLock<Model> = std::sync::OnceLock::new();

fn build_geometry(materials: &[Material], reverse_rows: bool) -> Geometry {
    let xml = strip_comments(GEOMETRY_XML);

    // --- surfaces -------------------------------------------------------
    let mut surf_index: BTreeMap<i32, usize> = BTreeMap::new();
    let mut surfaces: Vec<SurfaceKind> = Vec::new();
    for (a, _) in elements(&xml, "surface") {
        let id: i32 = num(attr(a, "id").expect("surface id"), "surface id");
        let ty = attr(a, "type").expect("surface type");
        let c: Vec<f64> = attr(a, "coeffs")
            .expect("surface coeffs")
            .split_whitespace()
            .map(|t| num(t, "surface coeff"))
            .collect();
        let bc = match attr(a, "boundary") {
            Some("vacuum") => BoundaryType::Vacuum,
            Some("reflective") => BoundaryType::Reflective,
            Some(other) => panic!("unhandled boundary {other:?} on surface {id}"),
            None => BoundaryType::Transmissive,
        };
        let s = match ty {
            "z-cylinder" => SurfaceKind::ZCylinder(ZCylinder {
                x0: c[0],
                y0: c[1],
                r: c[2],
                bc,
            }),
            "z-plane" => SurfaceKind::ZPlane(ZPlane { z0: c[0], bc }),
            "sphere" => SurfaceKind::Sphere(Sphere {
                x0: c[0],
                y0: c[1],
                z0: c[2],
                r: c[3],
                bc,
            }),
            other => panic!("unhandled surface type {other:?} on surface {id}"),
        };
        surf_index.insert(id, surfaces.len());
        surfaces.push(s);
    }

    // --- lattices -------------------------------------------------------
    // Universe ids are collected first: a lattice's `<universes>` block names
    // them, and so does every cell's `universe=` attribute.
    let lat_blocks = elements(&xml, "lattice");
    let cell_blocks = elements(&xml, "cell");
    let mut uni_ids: Vec<i32> = Vec::new();
    for (a, _) in &cell_blocks {
        let u: i32 = num(attr(a, "universe").expect("cell universe"), "cell universe");
        if !uni_ids.contains(&u) {
            uni_ids.push(u);
        }
    }
    uni_ids.sort_unstable();
    let uni_index: BTreeMap<i32, usize> =
        uni_ids.iter().enumerate().map(|(i, &u)| (u, i)).collect();

    let mut lat_index: BTreeMap<i32, usize> = BTreeMap::new();
    let mut lattices: Vec<Lattice> = Vec::new();
    let mut model_lattices: BTreeMap<i32, RectLattice> = BTreeMap::new();
    for (a, body) in &lat_blocks {
        let id: i32 = num(attr(a, "id").expect("lattice id"), "lattice id");
        let dim: Vec<usize> = attr(a, "dimension")
            .expect("lattice dimension")
            .split_whitespace()
            .map(|t| num(t, "dimension"))
            .collect();
        assert_eq!(dim.len(), 2, "lattice {id} is not 2-D; not handled here");
        let (nx, ny) = (dim[0], dim[1]);
        let ll: Vec<f64> = block(body, "lower_left");
        let pitch: Vec<f64> = block(body, "pitch");
        let rows: Vec<i32> = block(body, "universes");
        assert_eq!(
            rows.len(),
            nx * ny,
            "lattice {id}: {} entries for a {nx}×{ny} grid",
            rows.len()
        );

        // OpenMC writes lattice rows from the TOP down — the first row is the
        // one at maximum y (`openmc/lattice.py`, `Lattice.universes` docs, and
        // the XML writer's row ordering). This crate's flat index runs with iy
        // increasing along +y. Reversing the rows here is the whole of that
        // conversion, and getting it wrong mirrors the core in y — a defect
        // that produces a perfectly plausible wrong k. `check_geometry` tests
        // the convention against the data itself.
        // Every lattice in this model is centred on its own frame — asserted
        // below — which is what makes the reversal a pure mirror.
        assert!(
            (ll[1] + 0.5 * ny as f64 * pitch[1]).abs() < 1.0e-9,
            "lattice {id} is not centred in y; the mirror argument in \
             `check_geometry` does not apply"
        );
        let mut universes = vec![0usize; nx * ny];
        for row in 0..ny {
            let iy = if reverse_rows { ny - 1 - row } else { row };
            for ix in 0..nx {
                let uid = rows[row * nx + ix];
                universes[nx * iy + ix] = *uni_index
                    .get(&uid)
                    .unwrap_or_else(|| panic!("lattice {id} names unknown universe {uid}"));
            }
        }
        let rect = RectLattice {
            id,
            n: [nx, ny, 1],
            lower_left: Position::new(ll[0], ll[1], 0.0),
            pitch: [pitch[0], pitch[1], 1.0],
            universes,
            outer: None,
        };
        model_lattices.insert(id, rect.clone());
        lat_index.insert(id, lattices.len());
        lattices.push(Lattice::Rect(rect));
    }

    // --- cells ----------------------------------------------------------
    let mat_index: BTreeMap<i32, usize> = materials
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id, i))
        .collect();
    let mut cells: Vec<Cell> = Vec::new();
    let mut universes: Vec<Universe> = uni_ids
        .iter()
        .map(|&id| Universe {
            id,
            cell_indices: Vec::new(),
        })
        .collect();
    let mut wrapper: BTreeMap<i32, i32> = BTreeMap::new();
    let mut uni_fill: BTreeMap<i32, i32> = BTreeMap::new();
    for (a, _) in &cell_blocks {
        let id: i32 = num(attr(a, "id").expect("cell id"), "cell id");
        let uid: i32 = num(attr(a, "universe").expect("cell universe"), "cell universe");
        let region = parse_region(attr(a, "region").expect("cell region"), &surf_index, id);
        let fill = match (attr(a, "material"), attr(a, "fill")) {
            (Some(m), None) => {
                let mid: i32 = num(m, "cell material");
                CellFill::Material(
                    *mat_index
                        .get(&mid)
                        .unwrap_or_else(|| panic!("cell {id} names unknown material {mid}")),
                )
            }
            (None, Some(f)) => {
                let fid: i32 = num(f, "cell fill");
                // A fill id names a lattice if one has that id, else a universe.
                // The two id spaces are disjoint in this model, and that is
                // asserted rather than assumed.
                match (lat_index.get(&fid), uni_index.get(&fid)) {
                    (Some(_), Some(_)) => {
                        panic!("cell {id}: fill {fid} is ambiguous — both a lattice and a universe")
                    }
                    (Some(&l), None) => {
                        wrapper.insert(uid, fid);
                        CellFill::Lattice(l)
                    }
                    (None, Some(&u)) => {
                        uni_fill.insert(uid, fid);
                        CellFill::Universe(u)
                    }
                    (None, None) => panic!("cell {id}: fill {fid} is neither lattice nor universe"),
                }
            }
            (Some(_), Some(_)) => panic!("cell {id} has both a material and a fill"),
            (None, None) => panic!("cell {id} has neither a material nor a fill"),
        };
        let cell = match fill {
            CellFill::Material(m) => Cell::material(id, region, m, TEMP_K),
            other => Cell::fill(id, region, other, Position::ZERO),
        };
        universes[uni_index[&uid]].cell_indices.push(cells.len());
        cells.push(cell);
    }

    let _ = MODEL.set(Model {
        lattices: model_lattices,
        wrapper,
        uni_fill,
        uni_ids: uni_ids.clone(),
    });

    let root = uni_index[&0];
    // Only the canonical build reports; `check_geometry` builds the mirrored
    // variant a second time and there is nothing new to say about it.
    if reverse_rows {
        eprintln!(
            "\n  geometry: {} surfaces, {} cells, {} universes, {} lattices (root = universe 0)",
            surfaces.len(),
            cells.len(),
            universes.len(),
            lattices.len()
        );
    }
    Geometry {
        surfaces,
        cells,
        universes,
        lattices,
        root_universe: root,
    }
}

/// Whitespace-separated numbers inside `<tag> … </tag>`.
fn block<T: std::str::FromStr>(body: &str, tag: &str) -> Vec<T> {
    elements(body, tag)
        .first()
        .unwrap_or_else(|| panic!("missing <{tag}>"))
        .1
        .split_whitespace()
        .map(|t| num(t, tag))
        .collect()
}

/// A region in these files is a space-separated list of signed surface ids, all
/// intersected — no unions, complements or parentheses appear. Anything else is
/// rejected loudly rather than silently mis-parsed.
fn parse_region(text: &str, surf_index: &BTreeMap<i32, usize>, cell_id: i32) -> Vec<RegionToken> {
    let mut tokens = Vec::new();
    for t in text.split_whitespace() {
        assert!(
            t.chars()
                .all(|c| c.is_ascii_digit() || c == '-' || c == '+'),
            "cell {cell_id}: region token {t:?} is not a signed surface id \
             (unions/complements are not handled)"
        );
        let signed: i32 = num(t, "region surface");
        let idx = *surf_index
            .get(&signed.abs())
            .unwrap_or_else(|| panic!("cell {cell_id}: region names unknown surface {signed}"));
        let sense = if signed < 0 {
            HalfSpaceSense::Inside
        } else {
            HalfSpaceSense::Outside
        };
        let first = tokens.is_empty();
        tokens.push(RegionToken::HalfSpace {
            surface_idx: idx,
            sense,
        });
        if !first {
            tokens.push(RegionToken::Intersection);
        }
    }
    assert!(!tokens.is_empty(), "cell {cell_id}: empty region");
    tokens
}

// ---------------------------------------------------------------------------
// Self-check
// ---------------------------------------------------------------------------

/// Check the CSG descent against hand-written arithmetic at 200 000 points, and
/// settle the lattice row convention.
///
/// This is the first nested-lattice model in this crate, and two of its steps
/// are exactly the kind that produce a plausible wrong answer rather than a
/// crash: the two-level tile index arithmetic, and OpenMC's top-down
/// `<universes>` row order against this crate's bottom-up flat index. The pin
/// *maps* are data read from the same file either way, so what is verified here
/// is the machinery around them.
///
/// 1. **The row convention cannot matter for this model, and that is checked,
///    not assumed.** Every lattice here is centred on its own frame
///    (`lower_left = −½·n·pitch`, asserted while parsing) and every lattice
///    frame is itself centred — the core lattice on the origin, each assembly
///    on its core tile's centre. Reversing a centred lattice's rows therefore
///    mirrors its contents about its own mid-plane, and composing that at both
///    levels is exactly a **global mirror of the model about `y = 0`**. The only
///    other geometry is a z-cylinder on the axis and two z-planes, all
///    y-symmetric, so the two conventions give congruent models and identical
///    `k`. The check builds the model both ways and requires
///    `locate(x, y, z)` under one to equal `locate(x, −y, z)` under the other at
///    every sampled point — which both proves the mirror claim and, because the
///    two builds differ in every lattice, exercises the index arithmetic
///    against an independent transformation.
///
///    (An earlier version of this check tried to pick the convention by
///    measuring which one gave a smoother circular core boundary. It returned
///    exactly the same raggedness for both — 1.944 cm — which is not a weak
///    signal but the mirror symmetry above showing up: no metric invariant
///    under reflection can distinguish them. The convention used is still
///    OpenMC's documented top-down row order; it simply has no effect here.)
///
/// 2. **Point agreement.** For each sampled point, an independent predicate
///    computes the core tile, the assembly tile and the pin region by its own
///    arithmetic — with the pin radii read by hand off the model's surface
///    cards — and the resulting material must equal the one
///    `Geometry::locate` descends to, including `None` outside the vacuum
///    boundary.
fn check_geometry(geom: &Geometry, materials: &[Material]) {
    let model = MODEL.get().expect("model");
    let core = model.lattices.get(&99).expect("core lattice 99");

    // --- 1. the row convention is a global y-mirror -----------------------
    {
        let mirrored = build_geometry(materials, false);
        let mut seed = 777_777_u64;
        let mut prn = move || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 11) as f64) / ((1u64 << 53) as f64)
        };
        const M: usize = 50_000;
        let u = Direction::new(0.0, 0.0, 1.0);
        for _ in 0..M {
            let (x, y, z) = (
                (2.0 * prn() - 1.0) * R_CORE * 1.02,
                (2.0 * prn() - 1.0) * R_CORE * 1.02,
                Z_LO - 1.0 + prn() * (Z_HI - Z_LO + 2.0),
            );
            let a = geom
                .locate(Position::new(x, y, z), u, SurfaceToken::NONE)
                .and_then(|p| p.material);
            let b = mirrored
                .locate(Position::new(x, -y, z), u, SurfaceToken::NONE)
                .and_then(|p| p.material);
            assert_eq!(
                a, b,
                "the two row conventions are not mirror images at ({x}, {y}, {z}): {a:?} vs {b:?}"
            );
        }
        eprintln!(
            "  lattice row convention: the two orders are exact y-mirrors of each other \
             ({M} points), so k cannot depend on the choice"
        );
    }

    // --- 1. point agreement ----------------------------------------------
    // Material slots, asserted rather than assumed: the model lists water,
    // fuel and clad as ids 1, 2, 3 and `build_materials` keeps that order.
    const WATER: usize = 0;
    const FUEL: usize = 1;
    const CLAD: usize = 2;
    let expect = |p: Position| -> Option<usize> {
        if p.z <= Z_LO || p.z >= Z_HI || p.x * p.x + p.y * p.y >= R_CORE * R_CORE {
            return None;
        }
        // core tile
        let cx = ((p.x - core.lower_left.x) / core.pitch[0]).floor();
        let cy = ((p.y - core.lower_left.y) / core.pitch[1]).floor();
        assert!(
            cx >= 0.0 && cy >= 0.0 && cx < core.n[0] as f64 && cy < core.n[1] as f64,
            "core cylinder reaches outside the 7×7 lattice at {p:?}"
        );
        let (cx, cy) = (cx as usize, cy as usize);
        let uid = model.uni_ids[core.universes[core.n[0] * cy + cx]];
        // A core tile is either an assembly (its wrapper universe is filled by
        // a lattice) or the all-water tile (filled by pin universe 1).
        let lid = match (model.wrapper.get(&uid), model.uni_fill.get(&uid)) {
            (Some(&l), _) => l,
            (None, Some(&1)) => return Some(WATER),
            _ => panic!("core tile {cx},{cy} is universe {uid}, which fills nothing known"),
        };
        let asm = &model.lattices[&lid];
        let x0 = core.lower_left.x + (cx as f64 + 0.5) * core.pitch[0];
        let y0 = core.lower_left.y + (cy as f64 + 0.5) * core.pitch[1];
        let (lx, ly) = (p.x - x0, p.y - y0);
        let ax = ((lx - asm.lower_left.x) / asm.pitch[0]).floor();
        let ay = ((ly - asm.lower_left.y) / asm.pitch[1]).floor();
        assert!(
            ax >= 0.0 && ay >= 0.0 && ax < asm.n[0] as f64 && ay < asm.n[1] as f64,
            "assembly {lid} does not tile its core cell at {p:?}"
        );
        let (ax, ay) = (ax as usize, ay as usize);
        let pin_uid = model.uni_ids[asm.universes[asm.n[0] * ay + ax]];
        if pin_uid == 1 {
            return Some(WATER); // a water pin cell
        }
        assert_eq!(pin_uid, 2, "unexpected pin universe {pin_uid}");
        // pin universe 2: fuel / clad / water by radius about the pin centre
        let px = asm.lower_left.x + (ax as f64 + 0.5) * asm.pitch[0];
        let py = asm.lower_left.y + (ay as f64 + 0.5) * asm.pitch[1];
        let (dx, dy) = (lx - px, ly - py);
        let r = (dx * dx + dy * dy).sqrt();
        Some(if r < R_FUEL {
            FUEL
        } else if r < R_CLAD {
            CLAD
        } else {
            WATER
        })
    };

    let mut seed = 20_260_911_u64;
    let mut prn = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
    };
    const N: usize = 200_000;
    let (mut inside, mut outside) = (0usize, 0usize);
    let mut tally = [0usize; 3];
    for _ in 0..N {
        let p = Position::new(
            (2.0 * prn() - 1.0) * R_CORE * 1.05,
            (2.0 * prn() - 1.0) * R_CORE * 1.05,
            Z_LO - 2.0 + prn() * (Z_HI - Z_LO + 4.0),
        );
        let want = expect(p);
        let got = geom
            .locate(p, Direction::new(0.0, 0.0, 1.0), SurfaceToken::NONE)
            .and_then(|path| path.material);
        assert_eq!(
            got, want,
            "CSG disagrees with the model at {p:?}: found material {got:?}, model says {want:?}"
        );
        match want {
            Some(m) => {
                inside += 1;
                tally[m] += 1;
            }
            None => outside += 1,
        }
    }
    eprintln!(
        "  geometry check: {N} points, {inside} inside / {outside} outside; \
         volume shares water {:.1} % / fuel {:.1} % / clad {:.1} %",
        100.0 * tally[0] as f64 / inside as f64,
        100.0 * tally[1] as f64 / inside as f64,
        100.0 * tally[2] as f64 / inside as f64,
    );
    assert!(
        inside > N / 10 && outside > N / 100 && tally.iter().all(|&t| t > 0),
        "degenerate sampling"
    );
}

/// Pin radii [cm] — surfaces 1 and 2 in the model.
const R_FUEL: f64 = 0.514858;
const R_CLAD: f64 = 0.602996;
