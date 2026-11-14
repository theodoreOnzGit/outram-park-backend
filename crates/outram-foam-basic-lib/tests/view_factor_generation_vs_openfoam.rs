//! `viewFactorsGen` port differential against ESI OpenFOAM-v2506 (gh:#1540, under gh:#1529).
//!
//! # Method
//! `verification_and_validation/upstream_view_factor_gh1529/scripts/run.sh` builds closed cases with
//! upstream `blockMesh` (cubeA: 4x4x4 unit cube, plate4/8/16: 8x8x1 plates in a closed box,
//! boxE: 2 x 1 x 0.5 m with 5x3x2 cells), all six patches in group `viewFactorWall`, no
//! agglomeration, and runs the REAL `viewFactorsGen` (`-DNO_CGAL`, serial, defaults
//! `GaussQuadTol 0.01`, `distTol 8`, `alpha 0.21`, `writePrecision 17`). The upstream
//! `polyMesh/{points,faces,boundary}` and the files it wrote (`F`, `globalFaceFaces`, `mapDist`)
//! are committed gzipped under `reference/vfgen/<case>/`.
//!
//! This test regenerates the three files from the upstream mesh with
//! `view_factor_generation::generate_view_factors` and the writers, and compares (1) the visible
//! lists exactly, (2) every `F` entry bit for bit, (3) the three files byte for byte. The counts
//! and worst relative difference are PINNED.
//!
//! # Results
//! `verification_and_validation/upstream_view_factor_gh1529/README.md`, section "viewFactorsGen".

use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_basic_lib::view_factor_generation::{
    generate_view_factors, trace_candidate_rays, write_label_list_list_file, write_map_dist_file,
    write_scalar_list_list_file, ViewFactorGenInput, ViewFactorGenSettings,
};
use outram_foam_basic_lib::view_factor_radiation::{parse_label_list_list, parse_scalar_list_list};

const DIR: &str = "verification_and_validation/upstream_view_factor_gh1529/reference/vfgen";

fn gunzip_file(case: &str, name: &str) -> String {
    let path = format!(
        "{}/{}/{}/{}.gz",
        env!("CARGO_MANIFEST_DIR"),
        DIR,
        case,
        name
    );
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let body = &data[10..data.len() - 8];
    String::from_utf8(miniz_oxide::inflate::decompress_to_vec(body).expect("inflate")).unwrap()
}

/// Text after the `FoamFile { }` header with parentheses separated out.
fn body_tokens(text: &str) -> Vec<String> {
    let p = text.find("}\n").expect("FoamFile end");
    let body = &text[p + 2..];
    let mut clean = String::new();
    for line in body.lines() {
        if line.starts_with("//") {
            continue;
        }
        clean.push_str(line);
        clean.push('\n');
    }
    clean
        .replace('(', " ( ")
        .replace(')', " ) ")
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn parse_points(text: &str) -> Vec<Vector3> {
    let t = body_tokens(text);
    let n: usize = t[0].parse().unwrap();
    let nums: Vec<f64> = t[1..]
        .iter()
        .filter_map(|w| w.parse::<f64>().ok())
        .collect();
    assert_eq!(nums.len(), 3 * n);
    (0..n)
        .map(|i| Vector3::new(nums[3 * i], nums[3 * i + 1], nums[3 * i + 2]))
        .collect()
}

fn parse_faces(text: &str) -> Vec<Vec<usize>> {
    let t = body_tokens(text);
    let n: usize = t[0].parse().unwrap();
    // tokens: n ( k ( a b c d ) k ( ... ) )
    let nums: Vec<usize> = t[1..]
        .iter()
        .filter_map(|w| w.parse::<usize>().ok())
        .collect();
    let mut out = Vec::with_capacity(n);
    let mut p = 0;
    for _ in 0..n {
        let k = nums[p];
        out.push(nums[p + 1..p + 1 + k].to_vec());
        p += 1 + k;
    }
    out
}

/// `(startFace, nFaces)` per patch, in file order.
fn parse_patches(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut n = 0;
    for line in text.lines() {
        let t: Vec<&str> = line
            .split(|c: char| c.is_whitespace() || c == ';')
            .filter(|s| !s.is_empty())
            .collect();
        if t.len() >= 2 && t[0] == "nFaces" {
            n = t[1].parse().unwrap();
        }
        if t.len() >= 2 && t[0] == "startFace" {
            out.push((t[1].parse().unwrap(), n));
        }
    }
    out
}

struct Outcome {
    rows: usize,
    lists_equal: usize,
    pairs_up: usize,
    pairs_rust: usize,
    pairs_common: usize,
    common_identical: usize,
    worst_rel: f64,
    f_bytes_equal: bool,
    gff_bytes_equal: bool,
    map_bytes_equal: bool,
}

fn run(case: &str) -> Outcome {
    let points = parse_points(&gunzip_file(case, "points"));
    let faces = parse_faces(&gunzip_file(case, "faces"));
    let patches = parse_patches(&gunzip_file(case, "boundary"));
    assert!(patches.len() == 6 || patches.len() == 3);
    let patch_faces: Vec<Vec<Vec<usize>>> = patches
        .iter()
        .map(|&(start, n)| faces[start..start + n].to_vec())
        .collect();
    let g = generate_view_factors(
        &ViewFactorGenInput {
            points,
            patch_faces,
        },
        ViewFactorGenSettings::default(),
    )
    .expect("generate");
    let up_f_text = gunzip_file(case, "F");
    let up_gff_text = gunzip_file(case, "globalFaceFaces");
    let up_f = parse_scalar_list_list(&up_f_text).expect("F");
    let up_gff = parse_label_list_list(&up_gff_text).expect("gff");
    let mut o = Outcome {
        rows: up_f.len(),
        lists_equal: 0,
        pairs_up: 0,
        pairs_rust: 0,
        pairs_common: 0,
        common_identical: 0,
        worst_rel: 0.0,
        f_bytes_equal: false,
        gff_bytes_equal: false,
        map_bytes_equal: false,
    };
    for i in 0..up_f.len() {
        if g.global_face_faces[i] == up_gff[i] {
            o.lists_equal += 1;
        }
        o.pairs_up += up_gff[i].len();
        o.pairs_rust += g.global_face_faces[i].len();
        // F entries of the columns both sides list
        for (k, j) in g.global_face_faces[i].iter().enumerate() {
            if let Some(m) = up_gff[i].iter().position(|x| x == j) {
                let (a, b) = (g.f[i][k], up_f[i][m]);
                o.pairs_common += 1;
                if a.to_bits() == b.to_bits() {
                    o.common_identical += 1;
                } else {
                    o.worst_rel = o.worst_rel.max(((a - b) / b).abs());
                }
            }
        }
    }
    o.f_bytes_equal = write_scalar_list_list_file(&g.f) == up_f_text;
    o.gff_bytes_equal = write_label_list_list_file(&g.global_face_faces) == up_gff_text;
    o.map_bytes_equal = write_map_dist_file(g.f.len()) == gunzip_file(case, "mapDist");
    o
}

/// Convex enclosures: everything identical, files byte for byte.
#[test]
fn convex_cases_are_bit_and_byte_identical_to_viewfactorsgen() {
    for (case, rows, pairs) in [
        ("cubeA", 96, 7680),
        ("plate4", 160, 17152),
        ("plate8", 160, 17152),
        ("plate16", 160, 17152),
        ("boxE", 62, 3132),
    ] {
        let o = run(case);
        println!(
            "{case}: rows {} lists-equal {} pairs up {} rust {} common {} bit-identical {} worst-rel {:e} F-bytes {} gff-bytes {} map-bytes {}",
            o.rows,
            o.lists_equal,
            o.pairs_up,
            o.pairs_rust,
            o.pairs_common,
            o.common_identical,
            o.worst_rel,
            o.f_bytes_equal,
            o.gff_bytes_equal,
            o.map_bytes_equal
        );
        assert_eq!(o.rows, rows, "{case}");
        assert_eq!(o.lists_equal, rows, "{case} visible lists");
        assert_eq!(o.pairs_up, pairs, "{case}");
        assert_eq!(o.common_identical, pairs, "{case} F bits");
        assert!(
            o.f_bytes_equal && o.gff_bytes_equal && o.map_bytes_equal,
            "{case} bytes"
        );
    }
}

/// Non-convex L enclosure (rays blocked at the re-entrant corner, in-plane faces whose candidacy
/// is rounding noise). ~~gh:#1564: 119/126 rows, 7 upstream-only pairs + 1 Rust-only, with
/// brute-force visibility~~ **CORRECTED 2026-11-14** - with the octree `findLine` port
/// (`outram_foam_basic_lib::esi_octree`) all 126 visible lists, all 10,886 `F` entries and all
/// three files are identical to upstream's, byte for byte. Cause and numbers:
/// `verification_and_validation/upstream_view_factor_gh1529/README.md`, "gh:#1564".
#[test]
fn lshape_non_convex_case_is_bit_and_byte_identical_to_viewfactorsgen() {
    let o = run("lshape");
    println!(
        "lshape: rows {} lists-equal {} pairs up {} rust {} common {} bit-identical {} worst-rel {:e}",
        o.rows, o.lists_equal, o.pairs_up, o.pairs_rust, o.pairs_common, o.common_identical, o.worst_rel
    );
    assert_eq!(o.rows, 126);
    assert_eq!(o.lists_equal, 126);
    assert_eq!(o.pairs_up, 10886);
    assert_eq!(o.pairs_rust, 10886);
    assert_eq!(o.common_identical, 10886);
    assert!(o.f_bytes_equal && o.gff_bytes_equal && o.map_bytes_equal);
}

fn gunzip_probe(name: &str) -> String {
    let path = format!(
        "{}/verification_and_validation/upstream_view_factor_gh1529/reference/probe_1564/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let body = &data[10..data.len() - 8];
    String::from_utf8(miniz_oxide::inflate::decompress_to_vec(body).expect("inflate")).unwrap()
}

/// Ray-by-ray differential against upstream's own `findLine` (gh:#1564). `lshape_rays.txt.gz` is
/// the `RAY` output of a printf-instrumented ESI `viewFactorsGen` (`probe_1564/`): for each of the
/// 12,278 candidate rays the start, end, hit flag, hit face and hit point at `%.17g`. All must
/// equal `trace_candidate_rays` bit for bit. (The brute-force search of the first version
/// differed on 161 rays.)
#[test]
fn lshape_every_candidate_ray_first_hit_equals_upstream_find_line() {
    let points = parse_points(&gunzip_file("lshape", "points"));
    let faces = parse_faces(&gunzip_file("lshape", "faces"));
    let patches = parse_patches(&gunzip_file("lshape", "boundary"));
    let patch_faces: Vec<Vec<Vec<usize>>> = patches
        .iter()
        .map(|&(start, n)| faces[start..start + n].to_vec())
        .collect();
    let rays = trace_candidate_rays(&ViewFactorGenInput {
        points,
        patch_faces,
    })
    .expect("trace");
    let text = gunzip_probe("lshape_rays.txt.gz");
    let lines: Vec<&str> = text.lines().filter(|l| l.starts_with("RAY ")).collect();
    assert_eq!(lines.len(), rays.len());
    assert_eq!(rays.len(), 12278);
    let f = |t: &str| t.parse::<f64>().unwrap().to_bits();
    let mut bad = 0;
    for (line, r) in lines.iter().zip(&rays) {
        // RAY i j s x y z e x y z hit H idx I agg A p x y z
        let t: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(
            (t[1].parse::<usize>().unwrap(), t[2].parse::<usize>().unwrap()),
            (r.source, r.target)
        );
        let same_ends = f(t[4]) == r.start.x.to_bits()
            && f(t[5]) == r.start.y.to_bits()
            && f(t[6]) == r.start.z.to_bits()
            && f(t[8]) == r.end.x.to_bits()
            && f(t[9]) == r.end.y.to_bits()
            && f(t[10]) == r.end.z.to_bits();
        assert!(same_ends, "ray {} -> {} start/end", r.source, r.target);
        let up_hit = t[12] == "1";
        let same_hit = match r.hit {
            None => !up_hit,
            Some((face, p)) => {
                up_hit
                    && t[16].parse::<usize>().unwrap() == face
                    && f(t[18]) == p.x.to_bits()
                    && f(t[19]) == p.y.to_bits()
                    && f(t[20]) == p.z.to_bits()
            }
        };
        if !same_hit {
            bad += 1;
        }
    }
    println!("lshape rays: {} compared, {} differ", rays.len(), bad);
    assert_eq!(bad, 0);
}
