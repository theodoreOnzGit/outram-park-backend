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

/// Header of [`measure`]'s row.
pub const MEASURE_HEADER: &str = "shape,tets,mesh,n,layers,dual,rule,cells,neg_cells,max_non_orth_deg,max_skewness,layers_deep,layer_cells,layer1_cells,thickness_median_over_asked,build_s,notes";

/// One build of the design a query describes (the page's own keys, see
/// [`crate::engine::Design::from_query`], plus `rule=edge|corners` for a
/// reactor shape's dual feature points), as one CSV row of
/// [`MEASURE_HEADER`]: the gh:#1796 measurement of every mesh type on
/// both tet sources. `layers_deep` is the deepest layer index found,
/// `layer1_cells` the cells in the wall-nearest layer, and the thickness
/// ratio the median grown column over the asked total (0 with no layer).
pub fn measure(q: &[(String, String)]) -> String {
    use super::engine::{layer_thickness, Design};
    use outram_park_fork_cfmesh::dual::FeaturePointRule;
    let d = Design::from_query(q);
    let mut p = d.params();
    let rule = q.iter().find(|(k, _)| k == "rule").map(|(_, v)| v.as_str());
    if rule == Some("edge") {
        p.reactor_feature_points = FeaturePointRule::OnFeatureEdge;
    }
    let t0 = std::time::Instant::now();
    let r = build(&p);
    let secs = t0.elapsed().as_secs_f64();
    let head = format!(
        "{},{},{},{},{},{:?},{}",
        d.kind.slug(),
        if d.tetgen { "tetgen" } else { "carved" },
        if d.polyhedral { "dual" } else { "tet" },
        d.n_layers,
        // gh:#1829: one layer route, grown first.
        if d.n_layers > 0 { "first" } else { "none" },
        d.dual,
        rule.unwrap_or("corners"),
    );
    match r {
        Ok(b) => {
            let counts = b.layer_index.counts();
            let [asked, _, med, _] = layer_thickness(&d, b.report.layers.as_ref());
            format!(
                "{head},{},{},{:.2},{:.3},{},{},{},{:.4},{:.2},{}",
                b.report.cell_count,
                b.report.n_negative_volume_cells,
                b.report.max_non_orthogonality_deg,
                b.report.max_skewness,
                counts.len() - 1,
                counts[1..].iter().sum::<usize>(),
                counts.get(1).copied().unwrap_or(0),
                if asked > 0.0 { med / asked } else { 0.0 },
                secs,
                b.report.stage_notes.join(" | ").replace(',', ";")
            )
        }
        Err(e) => format!("{head},ERROR,,,,,,,,{secs:.2},{}", e.replace(',', ";")),
    }
}
