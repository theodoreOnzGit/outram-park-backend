use super::model::{build, GeomKind, Params};

/// Stable CSV header. Keep this and the row format in lockstep — a
/// committed fixture diffs against both.
pub const CSV_HEADER: &str =
    "case,cleanup,surface_faces,cells,volume,valid,max_non_orth_deg,max_skewness,neg_vol_cells,stage_notes
";

/// Run one case and format its row. Fixed precision so the output is
/// byte-stable across runs and machines.
pub fn run_case(p: &Params) -> String {
    match build(p) {
        Ok(b) => format!(
            "{},{},{},{},{:.6},{},{:.4},{:.6},{},{}
",
            p.kind.slug(),
            p.cleanup.slug(),
            b.surface_faces,
            b.report.cell_count,
            b.report.total_volume,
            b.report.valid,
            b.report.max_non_orthogonality_deg,
            b.report.max_skewness,
            b.report.n_negative_volume_cells,
            b.report.stage_notes.len(),
        ),
        Err(e) => format!(
            "{},{},0,0,0.000000,false,0.0000,0.000000,0,ERROR:{}\n",
            p.kind.slug(),
            p.cleanup.slug(),
            e.replace(',', ";")
        ),
    }
}

/// Every case crossed with every cleanup mode — the sweep that shows
/// what the surface-quality operators are worth to the mesher.
pub fn run_sweep() -> String {
    use super::model::Cleanup;
    let mut out = String::from(CSV_HEADER);
    for kind in GeomKind::PRIMITIVES {
        for cleanup in Cleanup::ALL {
            let p = Params {
                kind,
                cleanup,
                ..Default::default()
            };
            out.push_str(&run_case(&p));
        }
    }
    out
}

/// Run `case` (or every case when `None`) and return the whole CSV.
pub fn run(case: Option<&str>) -> String {
    if case == Some("sweep") {
        return run_sweep();
    }
    let kinds: Vec<GeomKind> = match case {
        Some(name) => match GeomKind::from_slug(name) {
            Some(k) => vec![k],
            None => {
                return format!(
                    "# unknown case {name:?}; known cases: {}\n",
                    GeomKind::ALL
                        .iter()
                        .map(|k| k.slug())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        },
        None => GeomKind::PRIMITIVES.to_vec(),
    };
    let mut out = String::from(CSV_HEADER);
    for kind in kinds {
        let p = Params {
            kind,
            ..Default::default()
        };
        out.push_str(&run_case(&p));
    }
    out
}
