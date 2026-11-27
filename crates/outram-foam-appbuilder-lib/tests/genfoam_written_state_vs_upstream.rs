// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # The port's written time directory against the real binary's (gh:#1931; README `genfoam_vs_upstream_gh1361`, "Round 68")
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Reference: the real GeN-Foam 652b3da (ESI v2506) RAW steady-state `100` directory of `2D_fullCoupling` (`ADJ=1`, `writePrecision 17`,
//! 84 files, before the transient stage's `-initializeMappedFields`), packed in `refs/fc_ss_written100_r68.txt.gz`.
//!
//! * criterion 1: every numeric token of the real directory re-formatted by `format_g(x, 17)`: identical text;
//! * criterion 2: every field file parsed into the writer's description and written back: byte-identical.
//!
//! Results are recorded in the README (Round 68 results); `--nocapture` prints every count.

use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::io::foam_ascii::{
    format_g, time_name, write_dictionary_file, write_field_file, write_list_file, Data, FieldFile, ListObject, PatchOut, WriteOptions,
};

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let flg = bytes[3];
    let mut at = 10;
    if flg & 4 != 0 {
        at += 2 + usize::from(bytes[at]) + 256 * usize::from(bytes[at + 1]);
    }
    for bit in [8u8, 16u8] {
        if flg & bit != 0 {
            while bytes[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flg & 2 != 0 {
        at += 2;
    }
    decompress_to_vec(&bytes[at..bytes.len() - 8]).unwrap_or_else(|_| panic!("inflate {}", path.display()))
}

/// The packed real directory as `(relative path, text)`, sorted.
fn real_files() -> Vec<(String, String)> {
    let bytes = gunzip(&vv_dir().join("refs/fc_ss_written100_r68.txt.gz"));
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let eol = at + bytes[at..].iter().position(|&b| b == b'\n').expect("header line");
        let header = std::str::from_utf8(&bytes[at..eol]).expect("utf8 header");
        let parts: Vec<&str> = header.split('\t').collect();
        assert_eq!(parts[0], "FILE");
        let len: usize = parts[2].parse().expect("length");
        out.push((parts[1].to_string(), String::from_utf8(bytes[eol + 1..eol + 1 + len].to_vec()).expect("utf8 body")));
        at = eol + 1 + len + 1;
    }
    out
}

/// The part of a file after the header block (`FoamFile { ... }` and the divider line).
fn body(text: &str) -> &str {
    let i = text.find("}\n// * * *").expect("header end");
    let rest = &text[i..];
    let j = rest.find("//\n").expect("divider end");
    &rest[j + 3..]
}

fn is_number_token(t: &str) -> bool {
    let c = t.chars().next().unwrap_or(' ');
    (c.is_ascii_digit() || c == '-' || c == '.') && t.chars().any(|ch| ch.is_ascii_digit()) && t.parse::<f64>().is_ok()
}

#[test]
fn number_tokens_of_the_real_directory_reformat_identically() {
    let (mut n, mut bad) = (0usize, Vec::new());
    for (path, text) in real_files() {
        for tok in body(&text).split(|c: char| c.is_whitespace() || "(){};".contains(c)).filter(|t| is_number_token(t)) {
            n += 1;
            let x: f64 = tok.parse().expect("number");
            let again = format_g(x, 17);
            if again != tok {
                bad.push(format!("{path}: {tok} -> {again}"));
            }
        }
    }
    println!("criterion 1: {n} numeric tokens, {} differences", bad.len());
    assert!(bad.is_empty(), "{:?}", &bad[..bad.len().min(10)]);
    assert!(n > 30_000);
    // time names, pinned from `printf '%.6g'`
    for (t, want) in [(100.0, "100"), (0.2, "0.2"), (1e-6, "1e-06"), (123_456_789.0, "1.23457e+08"), (0.1 + 0.2, "0.3"), (1.0 / 3.0, "0.333333")] {
        assert_eq!(time_name(t, 6), want);
    }
}

// ---- a parser of the written field-file subset into the writer's own description --------------------------------

fn n_cmpt_of(type_name: &str) -> usize {
    match type_name {
        "scalar" => 1,
        "vector" => 3,
        "symmTensor" => 6,
        "tensor" => 9,
        other => panic!("type {other}"),
    }
}

fn numbers(s: &str) -> Vec<f64> {
    s.split(|c: char| c.is_whitespace() || c == '(' || c == ')').filter(|t| !t.is_empty()).map(|t| t.parse().unwrap_or_else(|_| panic!("number {t}"))).collect()
}

/// `uniform x` (one entry) or `nonuniform List<type> <list>`.
fn parse_entry(value: &str) -> Data {
    let v = value.trim();
    if let Some(rest) = v.strip_prefix("uniform") {
        let vals = numbers(rest);
        return Data::flat(vals.len(), vals);
    }
    let rest = v.strip_prefix("nonuniform List<").expect("nonuniform");
    let (ty, rest) = rest.split_once('>').expect("type");
    let nc = n_cmpt_of(ty);
    let open = rest.find('(').expect("(");
    let close = rest.rfind(')').expect(")");
    Data::flat(nc, numbers(&rest[open + 1..close]))
}

fn header_value(text: &str, key: &str) -> String {
    let line = text.lines().find(|l| l.trim_start().starts_with(key) && l.starts_with("    ")).unwrap_or_else(|| panic!("header {key}"));
    line.trim().trim_start_matches(key).trim().trim_end_matches(';').trim_matches('"').to_string()
}

fn parse_field_file(text: &str) -> FieldFile {
    let class = header_value(text, "class");
    let location = header_value(text, "location");
    let object = header_value(text, "object");
    let b = body(text);
    let dim_line = b.lines().find(|l| l.starts_with("dimensions")).expect("dimensions");
    let dims: Vec<i32> = dim_line.split('[').nth(1).expect("[").split(']').next().expect("]").split_whitespace().map(|t| t.parse().expect("dim")).collect();
    let mut dimensions = [0i32; 7];
    dimensions.copy_from_slice(&dims);
    let ii = b.find("internalField").expect("internalField");
    let after = &b[ii + "internalField".len()..];
    let semi = after.find(';').expect("; of internalField");
    let internal = parse_entry(&after[..semi]);
    let bi = b.find("boundaryField").expect("boundaryField");
    let bf = &b[bi + "boundaryField".len()..];
    let open = bf.find('{').expect("{");
    let mut patches = Vec::new();
    let mut rest = &bf[open + 1..];
    loop {
        let Some(po) = rest.find('{') else { break };
        let name = rest[..po].trim().to_string();
        let pc = rest[po..].find('}').expect("}") + po;
        let mut ptype = String::new();
        let mut entries = Vec::new();
        for stmt in rest[po + 1..pc].split(';') {
            let s = stmt.trim();
            if s.is_empty() {
                continue;
            }
            let (k, v) = s.split_once(char::is_whitespace).expect("key value");
            if k == "type" {
                ptype = v.trim().to_string();
            } else {
                entries.push((k.to_string(), parse_entry(v)));
            }
        }
        patches.push(PatchOut { name, ptype, entries });
        rest = &rest[pc + 1..];
    }
    FieldFile { class, location, object, dimensions, oriented: b.contains("\noriented        oriented;"), internal, patches }
}

fn is_field_file(text: &str) -> bool {
    ["volScalarField", "volVectorField", "volTensorField", "surfaceScalarField", "structure"].iter().any(|c| text.contains(&format!("    class       {c};")))
}

#[test]
fn every_field_file_of_the_real_directory_is_written_back_byte_for_byte() {
    let opts = WriteOptions::precision_17();
    let (mut files, mut bytes, mut bad) = (0usize, 0usize, Vec::new());
    for (path, text) in real_files() {
        if is_field_file(&text) {
            let f = parse_field_file(&text);
            let out = write_field_file(&f, &opts).unwrap_or_else(|e| panic!("{path}: {e}"));
            files += 1;
            bytes += text.len();
            if out != text {
                let at = out.bytes().zip(text.bytes()).position(|(a, b)| a != b).unwrap_or(out.len().min(text.len()));
                bad.push(format!("{path}: first difference at byte {at}: ...{}", &text[at.saturating_sub(40)..(at + 40).min(text.len())].replace('\n', "\\n")));
            }
        }
    }
    println!("criterion 2 (fields): {files} files, {bytes} bytes, {} files differ", bad.len());
    assert!(bad.is_empty(), "{bad:#?}");
    assert!(files >= 70);
}

fn parse_lists(text: &str) -> Vec<Vec<f64>> {
    // `N ( (a b c) ... )` or a list of lists `N ( M ( x ... ) ... )`: the numbers of each top-level element
    let b = body(text);
    let open = b.find('(').expect("(");
    let close = b.rfind(')').expect(")");
    let inner = &b[open + 1..close];
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in inner.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            _ => cur.push(c),
        }
        let _ = depth;
    }
    out.push(numbers(&cur));
    out
}

#[test]
fn list_objects_and_dictionaries_of_the_real_directory_are_written_back_byte_for_byte() {
    let opts = WriteOptions::precision_17();
    let mut checked = 0;
    for (path, text) in real_files() {
        let class = header_value_opt(&text, "class");
        let object = header_value_opt(&text, "object");
        let location = header_value_opt(&text, "location");
        let (Some(class), Some(object), Some(location)) = (class, object, location) else { continue };
        let out = match class.as_str() {
            "vectorField" => {
                let nums = parse_lists(&text).remove(0);
                let v: Vec<[f64; 3]> = nums.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
                write_list_file(&class, &location, &object, &ListObject::Plain(Data::vector(&v)), &opts)
            }
            "scalarFieldField" => {
                let fields = parse_field_field(&text);
                write_list_file(&class, &location, &object, &ListObject::FieldField(fields), &opts)
            }
            "dictionary" if path.starts_with("uniform/") => {
                let entries = parse_dictionary(&text);
                write_dictionary_file(&location, &object, &entries)
            }
            _ => continue,
        };
        checked += 1;
        assert!(out == text, "{path}: written text differs (first at byte {})", out.bytes().zip(text.bytes()).position(|(a, b)| a != b).unwrap_or(0));
    }
    println!("criterion 2 (lists and dictionaries): {checked} files byte-identical");
    assert!(checked >= 6, "points x2, Trad, time, reactorState, functionObjectProperties");
}

fn header_value_opt(text: &str, key: &str) -> Option<String> {
    text.lines().find(|l| l.starts_with(&format!("    {key} "))).map(|l| l.trim().trim_start_matches(key).trim().trim_end_matches(';').trim_matches('"').to_string())
}

fn parse_dictionary(text: &str) -> Vec<(String, String)> {
    body(text)
        .split(';')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() || s.starts_with("//") {
                return None;
            }
            let (k, v) = s.split_once(char::is_whitespace).expect("key value");
            Some((k.to_string(), v.trim().to_string()))
        })
        .collect()
}

fn parse_field_field(text: &str) -> Vec<Data> {
    let b = body(text);
    let open = b.find('(').expect("(");
    let close = b.rfind(')').expect(")");
    let inner = &b[open + 1..close];
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(o) = inner[at..].find('(') {
        let start = at + o;
        let end = start + inner[start..].find(')').expect(")");
        out.push(Data::scalar(numbers(&inner[start + 1..end])));
        at = end + 1;
    }
    out
}

// ---- Round 69 (gh:#1935): the baffle-less polyMesh files and the point-kinetics extras ------------------------------------------------------------

use outram_foam_appbuilder_lib::io::foam_ascii_mesh::{parse_boundary_patches, write_poly_mesh, PolyMeshOut};

/// The numbers inside the outermost parentheses of a list file body, one per element (`N(a b c d)` lines for faces: the labels).
fn label_lines(text: &str) -> Vec<Vec<usize>> {
    let b = body(text);
    let open = b.find("\n(\n").expect("list start") + 3;
    let close = b.rfind("\n)").expect("list end");
    b[open..close].lines().map(|l| l.split(|c: char| !c.is_ascii_digit()).filter(|t| !t.is_empty()).map(|t| t.parse().expect("label")).collect::<Vec<usize>>()).collect()
}

fn mesh_from_real_files(real: &[(String, String)]) -> PolyMeshOut {
    let get = |n: &str| &real.iter().find(|(p, _)| *p == format!("fluidRegion.baffleLess/polyMesh/{n}")).unwrap_or_else(|| panic!("real {n}")).1;
    let points: Vec<[f64; 3]> = parse_lists(get("points")).remove(0).chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    // the faces: `N(a b c d)` per line (the leading count is part of the line, drop it)
    let faces: Vec<Vec<usize>> = label_lines(get("faces")).into_iter().map(|mut l| {
        l.remove(0);
        l
    }).collect();
    let owner: Vec<usize> = label_lines(get("owner")).into_iter().flatten().collect();
    let neighbour: Vec<usize> = label_lines(get("neighbour")).into_iter().flatten().collect();
    let cell_labels: Vec<usize> = {
        let t = get("cellZones");
        let at = t.find("cellLabels").expect("cellLabels");
        let open = t[at..].find("\n(\n").expect("list") + at + 3;
        let close = t[open..].find("\n)").expect("end") + open;
        t[open..close].lines().map(|l| l.trim().parse().expect("label")).collect()
    };
    PolyMeshOut {
        points,
        faces,
        owner,
        neighbour,
        n_cells: 144,
        patches: parse_boundary_patches(get("boundary")).expect("patches"),
        cell_zones: vec![("zone0".to_string(), cell_labels)],
        n_face_zones: 0,
        n_point_zones: 0,
    }
}

/// README round 69, criterion 1 (X1): the 8 baffle-less `polyMesh` files of the real raw directory, parsed into the writer's description and
/// written back: 0 differing bytes.
#[test]
fn the_baffle_less_poly_mesh_files_of_the_real_directory_are_written_back_byte_for_byte() {
    let real = real_files();
    let m = mesh_from_real_files(&real);
    let files = write_poly_mesh(&m, "100/fluidRegion.baffleLess/polyMesh", &WriteOptions::precision_17()).expect("canonical mesh");
    let (mut bytes, mut bad) = (0usize, Vec::new());
    for (name, text) in &files {
        let (_, want) = real.iter().find(|(p, _)| *p == format!("fluidRegion.baffleLess/polyMesh/{name}")).expect("real file");
        bytes += want.len();
        if text != want {
            let at = text.bytes().zip(want.bytes()).position(|(a, b)| a != b).unwrap_or(text.len().min(want.len()));
            bad.push(format!("{name}: first difference at byte {at}: ...{}", &want[at.saturating_sub(40)..(at + 40).min(want.len())].replace('\n', "\\n")));
        }
    }
    println!("criterion 1 (baffle-less polyMesh): {} files, {bytes} bytes, {} differ", files.len(), bad.len());
    assert_eq!(files.len(), 8);
    assert!(bad.is_empty(), "{bad:#?}");
}

/// README round 69, criterion 1: the point-kinetics `110` extras (`reactorState` with `precursorPowers`, `defaultPrec`, `oneGroupFlux`) written back.
#[test]
fn the_point_kinetics_extras_of_the_real_110_directory_are_written_back_byte_for_byte() {
    let bytes = gunzip(&vv_dir().join("refs/fc_pk_written110_r69.txt.gz"));
    let mut at = 0;
    let (mut checked, opts) = (0, WriteOptions::precision_17());
    while at < bytes.len() {
        let eol = at + bytes[at..].iter().position(|&b| b == b'\n').expect("header line");
        let parts: Vec<&str> = std::str::from_utf8(&bytes[at..eol]).expect("utf8").split('\t').collect();
        let len: usize = parts[2].parse().expect("length");
        let text = String::from_utf8(bytes[eol + 1..eol + 1 + len].to_vec()).expect("utf8 body");
        at = eol + 1 + len + 1;
        match parts[1] {
            "uniform/reactorState" => {
                let location = header_value_opt(&text, "location").expect("location");
                let out = write_dictionary_file(&location, "reactorState", &parse_dictionary(&text));
                assert!(out == text, "reactorState differs");
                checked += 1;
            }
            "neutroRegion/defaultPrec" | "neutroRegion/oneGroupFlux" | "neutroRegion/meshPhi" => {
                let out = write_field_file(&parse_field_file(&text), &opts).expect("write");
                assert!(out == text, "{} differs", parts[1]);
                checked += 1;
            }
            _ => {}
        }
    }
    println!("criterion 1 (PK extras): {checked} files byte-identical");
    assert_eq!(checked, 4);
}
