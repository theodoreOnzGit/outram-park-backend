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
    assemble_explicit_triso, HTR10_AXIAL_REFLECTOR_CM, HTR10_CAVITY_ABOVE_BED_CM,
    HTR10_CONTROL_ROD_INNER_CM, HTR10_CONTROL_ROD_OUTER_CM, HTR10_CONUS_HEIGHT_CM,
    HTR10_COOLANT_INNER_CM, HTR10_COOLANT_OUTER_CM, HTR10_CORE_CAVITY_CM, HTR10_CORE_RADIUS_CM,
    HTR10_DISCHARGE_TUBE_RADIUS_CM, HTR10_GRAPHITE_OUTER_CM, HTR10_REFLECTOR_OUTER_CM,
    PAPER_FILLING_FRACTION,
};
use nee_soon::htr10_rmc::{geometry_closures, heavy_metal_per_ball, table1};
use outram_mc_libs::prelude::TrisoSpec;

/// Rings and axial layers of the **reported** case, not the example's own
/// cheap defaults (8 x 12). The V&V record's quoted results and the timed runs
/// are both at 14 x 25, so that is what a manuscript table must describe.
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
    let core = assemble_explicit_triso(rings, layers, 0);
    let bed_height = 2.0 * core.bed_half_height;
    let mut axial = String::from("zone,extentcm,contents,source\n");
    for (zone, extent, contents, source) in [
        (
            "conus (sloping bed floor)",
            HTR10_CONUS_HEIGHT_CM,
            "DUMMY pebbles only",
            "Terry et al. (2005) s2 and Fig. 2; TECDOC-1382 Table 2",
        ),
        (
            "pebble bed, as built",
            bed_height,
            "hex lattice; 57:43 fuelled:dummy",
            "realised: layers x lattice height",
        ),
        (
            "empty cavity above the bed",
            HTR10_CAVITY_ABOVE_BED_CM,
            "helium",
            "Terry et al. (2005) Fig. 2; 221.818 - 123.06 at the benchmark loading",
        ),
        (
            "core cavity (fixed, conus top to cavity top)",
            HTR10_CORE_CAVITY_CM,
            "bed plus void",
            "Terry et al. (2005) Fig. 2; z = 130.0 to 351.818",
        ),
        (
            "axial reflector above the cavity",
            HTR10_AXIAL_REFLECTOR_CM,
            "graphite",
            "Terry et al. (2005); 610 cm model height",
        ),
    ] {
        axial.push_str(&row(&[zone, &format!("{extent:.3}"), contents, source]));
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

    for (name, body) in [
        ("htr10_geometry_radial.csv", &radial),
        ("htr10_geometry_axial.csv", &axial),
        ("htr10_dh_geometry.csv", &dh),
        ("htr10_lattice_realised.csv", &realised),
        ("htr10_geometry_closures.csv", &closures),
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
