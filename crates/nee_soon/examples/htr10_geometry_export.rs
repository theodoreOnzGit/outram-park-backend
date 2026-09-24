//! **Export the HTR-10 RMC case geometry as CSV** — for the double-heterogeneity
//! manuscript (`publications/outram_park/.../outram_park_double_heterogeneity_arxiv`).
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_geometry_export
//! OUTRAM_HTR10_GEOM_OUT=/path/to/data cargo run --release ...
//! ```
//!
//! # Why this is an exporter and not a table someone typed
//!
//! Most of this geometry is **not a constant**. The hex pitch is solved so the
//! axially-clipped ball realises the paper's fuel-zone volume fraction; the
//! TRISO pitch is solved from a particle count; the tile, cell and universe
//! counts fall out of the assembly. Transcribing any of them into a manuscript
//! would create a second copy that drifts silently from the model the
//! eigenvalue was computed with — which is exactly the failure this workspace's
//! rules exist to prevent. So every realised number below is read back out of
//! [`assemble_explicit_triso`] after it has built the real geometry, and the
//! published constants are emitted with the citation that justifies them.
//!
//! # What it writes
//!
//! | File | Contents |
//! |---|---|
//! | `htr10_geometry_radial.csv` | radial zone boundaries, core outward |
//! | `htr10_geometry_axial.csv` | axial extents of the R-Z model |
//! | `htr10_dh_geometry.csv` | the three double-heterogeneity levels: TRISO particle, pebble, bed |
//! | `htr10_lattice_realised.csv` | what the assembly actually built, at the reported case size |
//! | `htr10_geometry_closures.csv` | over-determined quantities: paper's stated value vs our derived one |
//!
//! # NOT a validation artefact
//!
//! This example runs **no transport** and computes no eigenvalue. It reports
//! the geometry the model is built from. The eigenvalue, its residual against
//! the RMC reference and every caveat on quoting it are in
//! `crates/outram-mc-libs/verification_and_validation/htr10_rmc/README.md`.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use nee_soon::htr10_rmc::bed::HexBedCell;
use nee_soon::htr10_rmc::core_model::{
    assemble_explicit_triso, HTR10_CONTROL_ROD_INNER_CM, HTR10_CONTROL_ROD_OUTER_CM,
    HTR10_COOLANT_INNER_CM, HTR10_COOLANT_OUTER_CM, HTR10_CORE_RADIUS_CM,
    HTR10_DISCHARGE_TUBE_RADIUS_CM, HTR10_GRAPHITE_OUTER_CM, HTR10_REFLECTOR_OUTER_CM,
    PAPER_FILLING_FRACTION,
};
use nee_soon::htr10_rmc::materials::{htr10_material_set, nuclide_name, Htr10MaterialConfig};
use nee_soon::htr10_rmc::{geometry_closures, heavy_metal_per_ball, table1};
use outram_mc_libs::geometry::surface::SurfaceKind;
use outram_mc_libs::pebble_beds::htr10::Htr10Nuclides;
use outram_mc_libs::prelude::TrisoSpec;

/// Rings and axial layers of the **reported** case, not the example's own
/// cheap defaults (8 x 12). The V&V record's quoted results and the timed runs
/// are both at 14 x 25, so that is what a manuscript table must describe.
/// Temperature \[K\] every material is built at -- the same value
/// `htr10_rmc_keff` uses.
const TEMP_K: f64 = 300.15;

const REPORTED_RINGS: usize = 14;
const REPORTED_LAYERS: usize = 25;

fn env_usize(k: &str, d: usize) -> usize {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

/// Every CSV header below is **letters only, no underscores or digits**, and
/// avoids the name of any LaTeX macro.
///
/// That is not a style choice: the manuscript typesets these through
/// `csvsimple-l3`'s `head to column names`, which turns each header into a
/// LaTeX macro name, and a macro name cannot contain an underscore. The
/// existing `htr10_timing.csv` (`wallseconds`, `peakrssmb`) follows the same
/// rule for the same reason. A header with an underscore compiles to a
/// baffling error rather than to a table.
///
/// For the same reason the value columns are `magnitude` and `remark`
/// rather than the obvious `value` and `note`: `head to column names`
/// would otherwise redefine LaTeX's own counter primitive and a macro
/// several document classes define.
///
/// The free-text fields likewise avoid `&`, `%` and `_`, which are
/// LaTeX's alignment tab, comment character and subscript. Escaping
/// them would put backslashes in a CSV that is also read by people and
/// by `scripts/`, so the wording avoids them instead: the reference is
/// cited as "Li, Yu and Wei", not "Li, Yu & Wei".
///
/// Escape a field for RFC-4180 CSV: quote it if it holds a comma or a quote.
fn csv_field(s: &str) -> String {
    // Belt and braces: some of these strings come from the library rather than
    // from this file (the closure descriptions do), so a future edit upstream
    // could reintroduce a character that silently breaks the typeset table.
    // `%` would comment out the rest of the row and `&` would open a new
    // column -- both produce a WRONG TABLE rather than an error, which is the
    // dangerous kind of failure. Substituting here means the CSV is correct
    // whatever upstream says.
    let safe = s
        .replace('%', " pct")
        .replace('&', "and")
        .replace('_', "-")
        // `^` is LaTeX's superscript and `~` a non-breaking space; `#`, `$`,
        // `{`, `}` and a backslash are specials too. `slope^2` in the surface
        // dump reached the typeset table as a bare `^` and produced
        // "Missing $ inserted" -- caught only by actually compiling the
        // tables, which is why that check is part of the workflow.
        .replace('^', "")
        .replace('~', "-")
        .replace('#', "no.")
        .replace('$', "")
        .replace('{', "(")
        .replace('}', ")")
        .replace('\\', "/")
        // A comma inside a field would have to be quoted, and csvsimple
        // SILENTLY DROPS a row whose fields are quoted -- measured: the
        // radial table rendered 2 rows of 8 and the DH table 7 of 22, with
        // pdflatex reporting no error at all. A dropped row in a typeset
        // geometry table is the worst possible failure here, so the wording
        // carries no commas. `htr10_timing.csv` and `htr10_machine.csv`
        // already follow this rule ("24 (8 P + 16 E; hybrid)").
        .replace(',', ";");
    safe.replace("  ", " ")
}

fn row(fields: &[&str]) -> String {
    fields
        .iter()
        .map(|f| csv_field(f))
        .collect::<Vec<_>>()
        .join(",")
}

fn main() {
    let out = PathBuf::from(
        std::env::var("OUTRAM_HTR10_GEOM_OUT").unwrap_or_else(|_| "htr10_geometry_export".into()),
    );
    fs::create_dir_all(&out).expect("cannot create the output directory");

    let rings = env_usize("OUTRAM_HTR10_RINGS", REPORTED_RINGS);
    let layers = env_usize("OUTRAM_HTR10_LAYERS", REPORTED_LAYERS);

    // ---------------------------------------------------------------- radial
    let mut radial = String::from("zone,rinnercm,routercm,material,source\n");
    for (zone, ri, ro, material, source) in [
        (
            "pebble bed (active core)",
            0.0,
            HTR10_CORE_RADIUS_CM,
            "hex lattice of pebbles; 57:43 fuelled:dummy",
            "IAEA-TECDOC-1382; 180 cm core diameter",
        ),
        (
            "inner side reflector",
            HTR10_CORE_RADIUS_CM,
            HTR10_CONTROL_ROD_INNER_CM,
            "graphite; TECDOC Table 4-3 zone 22",
            "Terry et al. (2005) Fig. 2",
        ),
        (
            "control-rod boring band",
            HTR10_CONTROL_ROD_INNER_CM,
            HTR10_CONTROL_ROD_OUTER_CM,
            "bored graphite; TECDOC Table 4-3 zones 31-40",
            "Terry et al. (2005) Fig. 2; channel r 102.1 -/+ 13/2",
        ),
        (
            "outer side reflector",
            HTR10_CONTROL_ROD_OUTER_CM,
            HTR10_COOLANT_INNER_CM,
            "graphite; TECDOC Table 4-3 zone 22",
            "Terry et al. (2005) Fig. 2",
        ),
        (
            "cold coolant annulus",
            HTR10_COOLANT_INNER_CM,
            HTR10_COOLANT_OUTER_CM,
            "helium",
            "Terry et al. (2005) Fig. 2; channel r 144.6 -/+ 8.0/2",
        ),
        (
            "reflector beyond the annulus",
            HTR10_COOLANT_OUTER_CM,
            HTR10_GRAPHITE_OUTER_CM,
            "graphite; TECDOC Table 4-3 zone 22",
            "Terry et al. (2005) Fig. 2",
        ),
        (
            "boronated carbon bricks",
            HTR10_GRAPHITE_OUTER_CM,
            HTR10_REFLECTOR_OUTER_CM,
            "boronated carbon; 5 wt pct B4C",
            "Terry et al. (2005) Fig. 2; 380 cm outer diameter",
        ),
        (
            "fuel discharge tube",
            0.0,
            HTR10_DISCHARGE_TUBE_RADIUS_CM,
            "dummy pebbles (conus lower radius)",
            "Terry et al. (2005) s2",
        ),
    ] {
        radial.push_str(&row(&[
            zone,
            &format!("{ri:.3}"),
            &format!("{ro:.3}"),
            material,
            source,
        ]));
        radial.push('\n');
    }

    // ----------------------------------------------------------------- axial
    //
    // READ FROM THE ASSEMBLY, not from the named constants.
    //
    // The first version of this table listed the five constants that have
    // names -- conus, bed, cavity, core cavity, axial reflector -- and was
    // WRONG by omission: it had no row for the 191.8 cm of graphite below the
    // conus floor, and no total. It described the top half of a model that is
    // 580 cm tall, so a reader checking axial leakage would have been working
    // from a reactor that does not exist.
    //
    // The model is SYMMETRIC IN EXTENT about z = 0 (the outer reflector
    // cylinder runs +/- `refl_half_height`) and ASYMMETRIC IN CONTENTS: above
    // the bed sit the helium cavity then the axial reflector; below it the
    // conus of dummy pebbles, then solid graphite all the way to the floor.
    // That asymmetry is the whole point of the table, and only the assembly
    // knows it.
    let core = assemble_explicit_triso(rings, layers, 0);
    let bed_height = 2.0 * core.bed_half_height;
    let mut axial = String::from("zone,ztopcm,zbotcm,extentcm,contents,source\n");
    for (zone, ztop, zbot, contents, source) in [
        (
            "axial reflector (above cavity)",
            core.refl_half_height,
            core.cavity_top,
            "graphite; TECDOC zone 22",
            "Terry et al. (2005); 610 cm model height",
        ),
        (
            "empty core cavity",
            core.cavity_top,
            core.bed_half_height,
            "helium",
            "Terry et al. (2005) Fig. 2",
        ),
        (
            "pebble bed as built",
            core.bed_half_height,
            -core.bed_half_height,
            "hex lattice; 57:43 fuelled:dummy",
            "realised: layers x lattice height",
        ),
        (
            "conus (sloping bed floor)",
            -core.bed_half_height,
            core.conus_floor,
            "DUMMY pebbles only",
            "Terry et al. (2005) s2",
        ),
        (
            "bottom reflector (below conus)",
            core.conus_floor,
            -core.refl_half_height,
            "graphite; TECDOC zone 22",
            "assembled: symmetric outer cylinder",
        ),
    ] {
        axial.push_str(&row(&[
            zone,
            &format!("{ztop:.4}"),
            &format!("{zbot:.4}"),
            &format!("{:.4}", ztop - zbot),
            contents,
            source,
        ]));
        axial.push('\n');
    }

    // ------------------------------------------------- double heterogeneity
    let spec = TrisoSpec::HTR10_LI2014;
    let cell = HexBedCell::from_paper();
    let mut dh = String::from("level,quantity,magnitude,units,source\n");
    let mut push_dh = |level: &str, q: &str, v: String, u: &str, s: &str| {
        dh.push_str(&row(&[level, q, &v, u, s]));
        dh.push('\n');
    };

    // Level 1 -- the coated particle.
    for (name, r_outer, r_inner) in [
        ("fuel kernel (UO2) outer radius", spec.kernel, 0.0),
        ("buffer (porous PyC) outer radius", spec.buffer, spec.kernel),
        ("inner PyC outer radius", spec.ipyc, spec.buffer),
        ("silicon carbide outer radius", spec.sic, spec.ipyc),
        (
            "outer PyC outer radius (whole particle)",
            spec.opyc,
            spec.sic,
        ),
    ] {
        push_dh(
            "1 particle",
            name,
            format!("{:.4}", r_outer),
            "cm",
            "IAEA-TECDOC-1382; adjudicated radii (90 um buffer)",
        );
        if r_inner > 0.0 {
            push_dh(
                "1 particle",
                &format!("{name} -- layer thickness"),
                format!("{:.4}", r_outer - r_inner),
                "cm",
                "derived from the radii above",
            );
        }
    }
    push_dh(
        "1 particle",
        "UO2 kernel density",
        "10.4".into(),
        "g/cm3",
        "IAEA-TECDOC-1382 part 2 Table 4-17",
    );
    push_dh(
        "1 particle",
        "U-235 enrichment",
        "17.0".into(),
        "wt pct",
        "IAEA-TECDOC-1382 part 2 Table 4-17",
    );

    // Level 2 -- the pebble.
    push_dh(
        "2 pebble",
        "pebble outer radius",
        format!("{:.4}", 0.5 * table1::BALL_DIAMETER_CM),
        "cm",
        "Li et al. (2014) Table 2",
    );
    push_dh(
        "2 pebble",
        "fuelled-zone radius",
        "2.5000".into(),
        "cm",
        "Li et al. (2014) Table 2",
    );
    push_dh(
        "2 pebble",
        "unfuelled graphite shell thickness",
        format!("{:.4}", 0.5 * table1::BALL_DIAMETER_CM - 2.5),
        "cm",
        "derived: pebble radius less fuelled-zone radius",
    );
    push_dh(
        "2 pebble",
        "TRISO particles per pebble (stated)",
        "8335".into(),
        "particles",
        "Li et al. (2014) Table 2",
    );
    push_dh(
        "2 pebble",
        "TRISO packing fraction in the fuelled zone",
        format!("{:.6}", spec.packing_fraction),
        "-",
        "Li et al. (2014) Table 2",
    );
    push_dh(
        "2 pebble",
        "heavy metal per fuelled pebble (derived)",
        format!("{:.4}", heavy_metal_per_ball()),
        "g",
        "derived from kernel count / radius / density / enrichment",
    );

    // Level 3 -- the bed.
    push_dh(
        "3 bed",
        "ball diameter",
        format!("{:.4}", table1::BALL_DIAMETER_CM),
        "cm",
        "Li et al. (2014) Table 2",
    );
    push_dh(
        "3 bed",
        "ball filling fraction (stated)",
        format!("{:.4}", PAPER_FILLING_FRACTION),
        "-",
        "Li et al. (2014) body text",
    );
    push_dh(
        "3 bed",
        "fuelled:dummy ball ratio",
        format!(
            "{:.2}:{:.2}",
            table1::FUEL_BALL_FRACTION,
            table1::MODERATOR_BALL_FRACTION
        ),
        "-",
        "Li et al. (2014) Table 1",
    );
    push_dh(
        "3 bed",
        "paper hex cell pitch (two-ball prism)",
        format!("{:.4}", cell.pitch),
        "cm",
        "reconstructed from the stated 0.61 filling fraction",
    );
    push_dh(
        "3 bed",
        "paper layer height (two close-packed layers)",
        format!("{:.4}", cell.height),
        "cm",
        "reconstructed; the paper states 9.798",
    );

    // ------------------------------------------------- realised lattice
    let mut realised = String::from("quantity,magnitude,units,remark\n");
    for (q, v, u, note) in [
        (
            "lattice rings requested",
            format!("{rings}"),
            "-",
            "a floor: the bed radius is fixed and the lattice is sized to tile it",
        ),
        (
            "axial layers",
            format!("{layers}"),
            "-",
            "reported case size",
        ),
        (
            "hex lattice pitch, realised",
            format!("{:.4}", core.lat_pitch),
            "cm",
            "SOLVED so the axially-clipped ball realises the paper's fuel-zone volume fraction",
        ),
        (
            "axial tile height, realised",
            format!("{:.4}", core.lat_height),
            "cm",
            "half the paper's two-ball prism: one ball per tile",
        ),
        (
            "bed cylinder radius",
            format!("{:.4}", core.bed_radius),
            "cm",
            "inscribed in the tiled hexagon",
        ),
        (
            "bed half-height",
            format!("{:.4}", core.bed_half_height),
            "cm",
            "",
        ),
        (
            "bed full height",
            format!("{:.4}", bed_height),
            "cm",
            "compare the benchmark's 123.576 cm critical loading",
        ),
        (
            "conus floor",
            format!("{:.4}", core.conus_floor),
            "cm",
            "deepest fuelled z; equals the negated bed half-height with no conus",
        ),
        (
            "hex tiles in the bed",
            format!("{}", core.tiles),
            "tiles",
            "",
        ),
        ("geometry cells", format!("{}", core.cells), "cells", ""),
        (
            "geometry universes",
            format!("{}", core.universes),
            "universes",
            "",
        ),
    ] {
        realised.push_str(&row(&[q, &v, u, note]));
        realised.push('\n');
    }

    // ------------------------------------------------- geometry closures
    let mut closures = String::from("quantity,stated,derived,units,reldiffpercent,derivedfrom\n");
    for c in geometry_closures() {
        let _ = writeln!(
            closures,
            "{},{},{},{},{},{}",
            csv_field(c.quantity),
            format_args!("{:.6}", c.stated),
            format_args!("{:.6}", c.derived),
            csv_field(c.units),
            format_args!("{:+.3}", 100.0 * c.relative()),
            csv_field(c.derived_from),
        );
    }

    // ------------------------------------------------------- materials
    //
    // Built through `htr10_material_set`, the SAME call the eigenvalue example
    // makes, so this table cannot describe a different material set from the
    // one k_eff was computed with. Atom densities are atoms/barn-cm.
    let nuclides = Htr10Nuclides {
        u235: 0,
        u238: 1,
        o16: 2,
        c_free: 3,
        c_graphite: 4,
        si28: 5,
        b10: 6,
        // Appended 2026-09-23: slots 0..=6 keep their indices.
        c_sic: 7,
        si29: 8,
        si30: 9,
    };
    let cfg = Htr10MaterialConfig::benchmark_default(TEMP_K);
    let mats = htr10_material_set(nuclides, cfg);
    let mut materials = String::from("matindex,matid,material,nuclide,atomdensity,temperaturek\n");
    for (idx, m) in mats.iter().enumerate() {
        if m.components.is_empty() {
            // Helium is deliberately modelled as a void, as the reference's
            // own model omits it. A blank row is the honest record -- dropping
            // the material entirely would hide a modelling choice.
            materials.push_str(&row(&[
                &format!("{idx}"),
                &format!("{}", m.id),
                &m.name,
                "(none - modelled as void)",
                "0",
                &format!("{:.2}", m.temperature),
            ]));
            materials.push('\n');
            continue;
        }
        for c in &m.components {
            materials.push_str(&row(&[
                &format!("{idx}"),
                &format!("{}", m.id),
                &m.name,
                nuclide_name(nuclides, c.nuclide_idx),
                &format!("{:.6e}", c.atom_density),
                &format!("{:.2}", m.temperature),
            ]));
            materials.push('\n');
        }
    }

    // -------------------------------------------------------- CSG surfaces
    // `index` would collide with LaTeX's own \index primitive under
    // csvsimple's `head to column names`, exactly as `value` and `note`
    // would -- see the header rule at the top of this file.
    let mut surfaces = String::from("surfaceid,kind,parameters\n");
    for (i, sk) in core.geometry.surfaces.iter().enumerate() {
        let (kind, params) = match sk {
            SurfaceKind::ZPlane(z) => ("z-plane", format!("z0 = {:.4} cm", z.z0)),
            SurfaceKind::ZCylinder(c) => (
                "z-cylinder",
                format!("r = {:.4} cm about ({:.1}; {:.1})", c.r, c.x0, c.y0),
            ),
            SurfaceKind::ZCone(c) => (
                "z-cone",
                format!("apex z0 = {:.4} cm; slope squared = {:.6}", c.z0, c.r_sq),
            ),
            SurfaceKind::Sphere(sp) => ("sphere", format!("r = {:.6} cm", sp.r)),
            other => ("other", format!("{other:?}")),
        };
        surfaces.push_str(&row(&[&format!("{i}"), kind, &params]));
        surfaces.push('\n');
    }

    // --------------------------------------------------------- run settings
    let mut settings = String::from("setting,magnitude,remark\n");
    for (k, v, note) in [
        (
            "histories per cycle",
            "2000".to_string(),
            "OUTRAM-HTR10-HISTORIES",
        ),
        (
            "inactive cycles",
            "30".to_string(),
            "source convergence; tunable so it can be MEASURED",
        ),
        ("active cycles", "70".to_string(), ""),
        (
            "seed",
            "20260917".to_string(),
            "SINGLE SEED; seed-to-seed sd is 179 pcm",
        ),
        (
            "temperature",
            format!("{TEMP_K:.2}"),
            "K; every material built at this",
        ),
        (
            "lattice rings",
            format!("{rings}"),
            "a floor; the bed radius is fixed",
        ),
        ("axial layers", format!("{layers}"), ""),
        (
            "tracking",
            "hybrid delta / surface".to_string(),
            "delta-tracked bed inside a surface-tracked reflector",
        ),
        (
            "data library",
            "ENDF/B-VIII.0".to_string(),
            "the reference used VII.0; the library term is worth about 1100-1600 pcm",
        ),
        (
            "reflector zone",
            format!("{}", cfg.reflector_zone),
            "TECDOC Table 4-3; zone 22 is the OPTIMISTIC bound",
        ),
        (
            "boron reading",
            "natural".to_string(),
            "Table 2 ppm read as natural boron; B-10 is 19.9 at pct",
        ),
    ] {
        settings.push_str(&row(&[k, &v, note]));
        settings.push('\n');
    }

    for (name, body) in [
        ("htr10_geometry_radial.csv", &radial),
        ("htr10_geometry_axial.csv", &axial),
        ("htr10_dh_geometry.csv", &dh),
        ("htr10_lattice_realised.csv", &realised),
        ("htr10_geometry_closures.csv", &closures),
        ("htr10_materials.csv", &materials),
        ("htr10_surfaces.csv", &surfaces),
        ("htr10_settings.csv", &settings),
    ] {
        let path = out.join(name);
        fs::write(&path, body).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
        println!("wrote {}", path.display());
    }

    println!("\n--- geometry closures (stated vs derived) ---");
    for c in geometry_closures() {
        println!(
            "{:<32} stated {:>12.5} {:<6} derived {:>12.5}  ({:+.3} %)",
            c.quantity,
            c.stated,
            c.units,
            c.derived,
            100.0 * c.relative()
        );
    }
    println!(
        "\nrealised lattice: {} tiles, {} cells, {} universes; pitch {:.4} cm, tile height {:.4} cm",
        core.tiles, core.cells, core.universes, core.lat_pitch, core.lat_height
    );
    println!(
        "bed: radius {:.4} cm, full height {:.4} cm, conus floor {:.4} cm",
        core.bed_radius, bed_height, core.conus_floor
    );
}
