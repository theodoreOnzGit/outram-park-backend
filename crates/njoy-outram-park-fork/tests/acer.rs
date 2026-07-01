//! ACER integration tests: build a continuous-energy ACE table from
//! reconstructed cross sections and verify its structure round-trips.
//!
//! Covers the first ACER increment (ESZ + MTR/LQR/TYR/LSIG/SIG blocks):
//! - U-235 (fissile, MAT=9228) — exercises fission + capture partials.
//! - H-2  (light, MAT=128)     — minimal reaction set.
//!
//! The decisive checks are structural and self-consistent:
//! - NXS/JXS agree with the XSS layout (lengths, locators, END word);
//! - the ESZ total equals elastic + Σ partials at every grid point;
//! - the Type-1 ASCII file the writer emits parses back to the same arrays.
//!
//! Run with:
//! ```bash
//! cargo test -p njoy-outram-park-fork --test acer --release
//! ```

use std::fs::File;

use njoy_outram_park_fork::{
    ace::{angular::parse_elastic_angular, jxs, nxs, AceTable},
    endf::tape::Tape,
    reconr::{reconr, ReconrConfig},
};

fn fixture(name: &str) -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources");
    p.push(name);
    p
}

fn build(name: &str, mat: i32) -> AceTable {
    let tape = Tape::read(File::open(fixture(name)).unwrap()).unwrap();
    let cfg = ReconrConfig { mat, tolerance: 0.001, temperature: 0.0 };
    let res = reconr(&tape, &cfg).unwrap();
    AceTable::from_reconr(&res, 0.0, 0)
}

/// Build with the elastic angular distribution included (parses MF=4/MT=2).
fn build_with_angular(name: &str, mat: i32) -> AceTable {
    let tape = Tape::read(File::open(fixture(name)).unwrap()).unwrap();
    let cfg = ReconrConfig { mat, tolerance: 0.001, temperature: 0.0 };
    let res = reconr(&tape, &cfg).unwrap();
    let sec = tape.section(mat, 4, 2).expect("MF=4/MT=2 present");
    let ang = parse_elastic_angular(sec).unwrap();
    AceTable::from_reconr_with_angular(&res, 0.0, 0, &ang)
}

/// Build the full table: cross sections + elastic angular + secondary energy
/// distributions (TYR / LDLW / DLW).
fn build_full(name: &str, mat: i32) -> AceTable {
    use njoy_outram_park_fork::ace::energy::build_emissions;
    let tape = Tape::read(File::open(fixture(name)).unwrap()).unwrap();
    let cfg = ReconrConfig { mat, tolerance: 0.001, temperature: 0.0 };
    let res = reconr(&tape, &cfg).unwrap();
    let partials: Vec<(i32, f64)> =
        res.sections.iter().map(|s| (i32::from(s.mt), s.qi)).collect();
    let emissions = build_emissions(&tape, mat, res.material.awr, &partials);
    let ang = tape.section(mat, 4, 2).map(|s| parse_elastic_angular(s).unwrap());
    AceTable::from_reconr_full(&res, 0.0, 0, ang.as_ref(), &emissions)
}

/// A minimal parsed Type-1 ACE table: just the arrays we need to validate.
struct ParsedAce {
    nxs: Vec<i32>,
    jxs: Vec<i32>,
    xss: Vec<f64>,
}

/// Parse a Type-1 ASCII ACE table written by [`AceTable::write_to`].
///
/// Layout: 2 header lines, 4 IZAW lines, 6 lines of NXS(16)+JXS(32) (8 ints
/// each), then the XSS data (4 values per line). Numeric fields are
/// whitespace-separable because every field is right-justified in its column.
fn parse_type1(text: &str) -> ParsedAce {
    let lines: Vec<&str> = text.lines().collect();
    // Skip 2 header + 4 IZAW lines.
    let mut ints: Vec<i32> = Vec::new();
    for line in &lines[6..12] {
        for tok in line.split_whitespace() {
            ints.push(tok.parse().unwrap());
        }
    }
    assert_eq!(ints.len(), 48, "expected 16 NXS + 32 JXS integers");
    let nxs = ints[..16].to_vec();
    let jxs = ints[16..].to_vec();

    let mut xss: Vec<f64> = Vec::new();
    for line in &lines[12..] {
        for tok in line.split_whitespace() {
            xss.push(tok.parse().unwrap());
        }
    }
    ParsedAce { nxs, jxs, xss }
}

#[test]
fn u235_ace_header_and_dimensions() {
    let ace = build("n-092_U_235-ENDF8.0.endf", 9228);

    assert_eq!(ace.nxs[nxs::ZA], 92235, "ZA");
    assert_eq!(ace.nxs[nxs::Z], 92, "Z");
    assert_eq!(ace.nxs[nxs::A], 235, "A");
    assert!(ace.zaid.starts_with("92235.00c") || ace.zaid.trim() == "92235.00c");
    assert!(ace.nxs[nxs::NES] > 1000, "U-235 union grid should be large");
    assert!(ace.nxs[nxs::NTR] >= 2, "at least fission + capture partials");

    // XSS length self-consistency.
    assert_eq!(ace.nxs[nxs::LEN_XSS] as usize, ace.xss.len());
    assert_eq!(ace.jxs[jxs::ESZ], 1, "ESZ starts at word 1");
    assert_eq!(ace.jxs[jxs::END] as usize, ace.xss.len(), "END = last word");
    assert_eq!(ace.xss_is_int.len(), ace.xss.len(), "type mask covers XSS");
}

#[test]
fn esz_total_equals_elastic_plus_partials() {
    let ace = build("n-092_U_235-ENDF8.0.endf", 9228);
    let nes = ace.nxs[nxs::NES] as usize;
    let ntr = ace.nxs[nxs::NTR] as usize;

    // ESZ columns (0-based offsets within the block at word jxs::ESZ=1).
    let energy = &ace.xss[0..nes];
    let total = &ace.xss[nes..2 * nes];
    let disap = &ace.xss[2 * nes..3 * nes];
    let elastic = &ace.xss[3 * nes..4 * nes];

    // Grid is ascending and expressed in MeV (top below the 20 MeV ceiling).
    assert!(energy.windows(2).all(|w| w[1] >= w[0]), "energy grid ascending");
    // In MeV the top is ~20–30; in eV it would be ~2–3e7. The loose bound just
    // confirms the unit conversion happened.
    assert!(*energy.last().unwrap() < 1000.0, "energies are in MeV, not eV");
    assert!(energy[0] > 0.0);

    // Re-sum the SIG partials at each grid point and compare to the ESZ total.
    let sig0 = ace.jxs[jxs::SIG] as usize; // 1-based
    for j in 0..nes {
        let mut sum = elastic[j];
        for i in 0..ntr {
            // Each SIG entry is [IE, NE, σ(1..NE)] laid contiguously; IE=1, NE=nes.
            let base = (sig0 - 1) + i * (2 + nes);
            let ie = ace.xss[base] as usize;
            let ne = ace.xss[base + 1] as usize;
            assert_eq!(ie, 1);
            assert_eq!(ne, nes);
            sum += ace.xss[base + 2 + j];
        }
        let t = total[j];
        assert!(
            (t - sum).abs() <= 1e-5 * t.abs().max(1.0),
            "total[{j}]={t} != elastic+Σpartials={sum}"
        );
        assert!(disap[j] <= t + 1e-9, "disappearance ≤ total");
        assert!(elastic[j] <= t + 1e-9, "elastic ≤ total");
        assert!(total[j] >= 0.0 && disap[j] >= 0.0 && elastic[j] >= 0.0);
    }
}

#[test]
fn type1_file_roundtrips() {
    let ace = build("n-001_H_002-ENDF8.0.endf", 128);

    let mut buf: Vec<u8> = Vec::new();
    ace.write_to(&mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let parsed = parse_type1(&text);

    // Integer arrays must match exactly.
    assert_eq!(parsed.nxs, ace.nxs.to_vec(), "NXS round-trip");
    assert_eq!(parsed.jxs, ace.jxs.to_vec(), "JXS round-trip");

    // XSS length matches; values match (reals to 11 significant figures, ints
    // exactly since they were written as i20).
    assert_eq!(parsed.xss.len(), ace.xss.len(), "XSS length round-trip");
    for (i, (&got, &want)) in parsed.xss.iter().zip(ace.xss.iter()).enumerate() {
        let tol = if ace.xss_is_int[i] { 0.0 } else { 1e-9 * want.abs().max(1.0) };
        assert!(
            (got - want).abs() <= tol,
            "xss[{i}] round-trip: got {got}, want {want}"
        );
    }
}

#[test]
fn h2_minimal_table_is_valid() {
    let ace = build("n-001_H_002-ENDF8.0.endf", 128);
    assert_eq!(ace.nxs[nxs::ZA], 1002, "H-2 ZA");
    assert!(ace.nxs[nxs::NES] > 1, "has an energy grid");
    // H-2 has elastic + capture at least → at least one MTR partial (capture).
    assert!(ace.nxs[nxs::NTR] >= 1, "at least one non-elastic reaction");
    assert_eq!(ace.jxs[jxs::MTR] != 0, ace.nxs[nxs::NTR] > 0);
}

/// Walk the AND block for the elastic reaction and assert every stored
/// distribution is a valid tabulated cosine pdf/cdf. Returns the number of
/// anisotropic incident energies found.
fn check_elastic_and_block(ace: &AceTable) -> usize {
    let land = ace.jxs[jxs::LAND];
    let and = ace.jxs[jxs::AND];
    assert!(land > 0 && and > 0, "LAND/AND locators set");

    // LAND(1): elastic locator, relative to the AND block (1-based).
    let land1 = ace.xss[(land - 1) as usize] as i64;
    assert_eq!(land1, 1, "elastic data at the start of the AND block");

    let and0 = (and - 1) as usize; // 0-based start of AND block
    let ne = ace.xss[and0] as usize;
    assert!(ne >= 1);
    let e = &ace.xss[and0 + 1..and0 + 1 + ne]; // incident energies [MeV]
    let locs = &ace.xss[and0 + 1 + ne..and0 + 1 + 2 * ne]; // per-energy locators

    assert!(e.windows(2).all(|w| w[1] >= w[0]), "incident energies ascending");

    let mut anisotropic = 0;
    for (j, &lf) in locs.iter().enumerate() {
        let l = lf as i64;
        if l == 0 {
            continue; // isotropic at this energy
        }
        assert!(l < 0, "tabulated distributions use negative locators (newfor=1)");
        anisotropic += 1;

        // Data lives at AND + |l| - 1: [JJ, NP, μ(NP), pdf(NP), cdf(NP)].
        let base = and0 + (l.unsigned_abs() as usize) - 1;
        let jj = ace.xss[base] as i64;
        assert_eq!(jj, 2, "JJ = lin-lin");
        let np = ace.xss[base + 1] as usize;
        assert!(np >= 2, "energy {j}: need at least two cosine points");

        let cos = &ace.xss[base + 2..base + 2 + np];
        let pdf = &ace.xss[base + 2 + np..base + 2 + 2 * np];
        let cdf = &ace.xss[base + 2 + 2 * np..base + 2 + 3 * np];

        // Cosines span and stay within [−1, 1], ascending.
        assert!(cos.windows(2).all(|w| w[1] > w[0]), "cosines strictly ascending");
        assert!(cos[0] >= -1.0 - 1e-9 && *cos.last().unwrap() <= 1.0 + 1e-9);
        // pdf non-negative; cdf monotone from 0 to 1.
        assert!(pdf.iter().all(|&p| p >= -1e-12), "pdf non-negative");
        assert!((cdf[0]).abs() < 1e-9, "cdf starts at 0");
        assert!((cdf[np - 1] - 1.0).abs() < 1e-6, "cdf ends at 1");
        assert!(cdf.windows(2).all(|w| w[1] >= w[0] - 1e-9), "cdf monotone");
    }
    anisotropic
}

#[test]
fn u235_elastic_angular_block_is_valid() {
    let ace = build_with_angular("n-092_U_235-ENDF8.0.endf", 9228);
    // Elastic-only angular: no non-elastic angular reactions counted in NR.
    assert_eq!(ace.nxs[nxs::NR], 0);
    assert_eq!(ace.jxs[jxs::END] as usize, ace.xss.len());
    assert_eq!(ace.nxs[nxs::LEN_XSS] as usize, ace.xss.len());
    let n = check_elastic_and_block(&ace);
    assert!(n > 10, "U-235 elastic is anisotropic at many energies, got {n}");
}

#[test]
fn h2_elastic_angular_block_is_valid() {
    let ace = build_with_angular("n-001_H_002-ENDF8.0.endf", 128);
    let n = check_elastic_and_block(&ace);
    assert!(n >= 1, "H-2 elastic is anisotropic above ~100 eV");
}

/// Walk the LDLW/DLW blocks and assert every producer's law is well-formed.
/// Returns (number of Law 3 entries, number of Law 4 entries).
fn check_dlw_block(ace: &AceTable) -> (usize, usize) {
    let nr = ace.nxs[nxs::NR] as usize;
    let ldlw = ace.jxs[jxs::LDLW];
    let dlw = ace.jxs[jxs::DLW];
    assert!(nr > 0 && ldlw > 0 && dlw > 0, "DLW present when NR>0");

    let dlw0 = (dlw - 1) as usize; // 0-based DLW start
    let (mut n_law3, mut n_law4) = (0, 0);

    for i in 0..nr {
        // LDLW[i] is a DLW-relative 1-based locator to the law-validity header.
        let loc = ace.xss[(ldlw - 1) as usize + i] as i64;
        assert!(loc >= 1, "LDLW locator ≥ 1");
        let h = dlw0 + (loc as usize) - 1; // 0-based header position

        let lnw = ace.xss[h] as i64;
        let law = ace.xss[h + 1] as i64;
        let idat = ace.xss[h + 2] as i64;
        let nr_app = ace.xss[h + 3] as i64;
        let ne_app = ace.xss[h + 4] as i64;
        assert_eq!(lnw, 0, "single law (LNW=0)");
        assert!(law == 3 || law == 4, "LAW ∈ {{3,4}}, got {law}");
        assert_eq!(nr_app, 0);
        assert_eq!(ne_app, 2, "applicability over an energy range");
        // Probabilities P=1 at both ends.
        assert!((ace.xss[h + 7] - 1.0).abs() < 1e-9 && (ace.xss[h + 8] - 1.0).abs() < 1e-9);

        let d = dlw0 + (idat as usize) - 1; // 0-based law-data position
        assert_eq!(idat, loc + 9, "IDAT = header + 9 words");

        if law == 3 {
            n_law3 += 1;
            // Law 3: [ldat1, ldat2]; slope (A/(A+1))² ∈ (0,1).
            let ldat2 = ace.xss[d + 1];
            assert!(ldat2 > 0.0 && ldat2 < 1.0, "Law 3 slope in (0,1), got {ldat2}");
        } else {
            n_law4 += 1;
            // Law 4: [NR, NE, E_in(NE), L(NE), dists…]. NR=0 here (lin-lin).
            let nr4 = ace.xss[d] as i64;
            let ne_pos = d + 1 + if nr4 == 0 { 0 } else { 2 * nr4 as usize };
            let ne = ace.xss[ne_pos] as usize;
            assert!(ne >= 1, "Law 4 has incident energies");
            let e_in = &ace.xss[ne_pos + 1..ne_pos + 1 + ne];
            let l = &ace.xss[ne_pos + 1 + ne..ne_pos + 1 + 2 * ne];
            assert!(e_in.windows(2).all(|w| w[1] >= w[0]), "E_in ascending");
            // Follow each distribution locator and validate its pdf/cdf.
            for &lf in l {
                let dp = dlw0 + (lf as usize) - 1;
                let intt = ace.xss[dp] as i64;
                assert!(intt == 1 || intt == 2, "INTT ∈ {{1,2}}");
                let np = ace.xss[dp + 1] as usize;
                assert!(np >= 1);
                let cdf = &ace.xss[dp + 2 + 2 * np..dp + 2 + 3 * np];
                assert!((cdf[np - 1] - 1.0).abs() < 1e-5, "cdf ends at 1");
                assert!(cdf.windows(2).all(|w| w[1] >= w[0] - 1e-9), "cdf monotone");
            }
        }
    }
    (n_law3, n_law4)
}

#[test]
fn h2_unsupported_law6_producer_is_skipped_gracefully() {
    // H-2's only neutron producer, MT16 (n,2n), uses ENDF MF=6 LAW=6 (n-body
    // phase space), which is not yet ported. It must be skipped cleanly: NR=0,
    // no DLW block, and the rest of the table still valid and self-consistent.
    let ace = build_full("n-001_H_002-ENDF8.0.endf", 128);
    assert_eq!(ace.nxs[nxs::NR], 0, "LAW=6 producer skipped ⇒ no NR");
    assert_eq!(ace.jxs[jxs::LDLW], 0, "no DLW block");
    assert_eq!(ace.jxs[jxs::END] as usize, ace.xss.len());
    assert_eq!(ace.nxs[nxs::LEN_XSS] as usize, ace.xss.len());
}

#[test]
fn u235_full_table_mixes_law3_and_law4() {
    // U-235: discrete inelastic levels (MT51-90) → Law 3; continuum / (n,xn)
    // (MT16/17/91/5) → Law 4 from MF=6. Fission (MT18) is excluded (needs ν̄).
    let ace = build_full("n-092_U_235-ENDF8.0.endf", 9228);
    assert!(ace.nxs[nxs::NR] > 10, "many producers, got {}", ace.nxs[nxs::NR]);
    let (n3, n4) = check_dlw_block(&ace);
    assert!(n3 > 5, "several discrete-level Law 3 producers, got {n3}");
    assert!(n4 >= 1, "at least one Law 4 producer, got {n4}");

    // TYR must be non-zero for exactly the NR producers.
    let tyr0 = (ace.jxs[jxs::TYR] - 1) as usize;
    let ntr = ace.nxs[nxs::NTR] as usize;
    let nonzero = ace.xss[tyr0..tyr0 + ntr].iter().filter(|&&t| t != 0.0).count();
    assert_eq!(nonzero, ace.nxs[nxs::NR] as usize, "TYR non-zero ⇔ producer");
}

#[test]
fn u235_discrete_levels_carry_mf4_angular() {
    // LAND has NR+1 entries (elastic + producers). Discrete inelastic levels
    // (Law 3) now carry their MF=4 angular distribution, so several producer LAND
    // locators must be > 0 (into the AND block), not all isotropic (0).
    let ace = build_full("n-092_U_235-ENDF8.0.endf", 9228);
    let nr = ace.nxs[nxs::NR] as usize;
    let land0 = (ace.jxs[jxs::LAND] - 1) as usize;
    let producer_land = &ace.xss[land0 + 1..land0 + 1 + nr]; // skip elastic entry
    let anisotropic = producer_land.iter().filter(|&&l| l > 0.0).count();
    assert!(
        anisotropic > 5,
        "discrete levels should carry MF=4 angular, got {anisotropic} anisotropic producers"
    );
    // Every LAND locator is a valid AND-relative index or 0/isotropic.
    assert!(producer_land.iter().all(|&l| l >= 0.0), "no negative producer LAND");
}

#[test]
fn full_table_roundtrips_through_file() {
    // The complete table (ESZ+SIG+AND+DLW) must survive a Type-1 write/parse.
    let ace = build_full("n-092_U_235-ENDF8.0.endf", 9228);
    let mut buf: Vec<u8> = Vec::new();
    ace.write_to(&mut buf).unwrap();
    let parsed = parse_type1(&String::from_utf8(buf).unwrap());
    assert_eq!(parsed.nxs, ace.nxs.to_vec());
    assert_eq!(parsed.jxs, ace.jxs.to_vec());
    assert_eq!(parsed.xss.len(), ace.xss.len());
    for (i, (&got, &want)) in parsed.xss.iter().zip(ace.xss.iter()).enumerate() {
        let tol = if ace.xss_is_int[i] { 0.0 } else { 1e-9 * want.abs().max(1.0) };
        assert!((got - want).abs() <= tol, "xss[{i}]: got {got}, want {want}");
    }
}

#[test]
fn angular_table_roundtrips_through_file() {
    // The whole table (now including LAND/AND) must survive a Type-1 write/parse.
    let ace = build_with_angular("n-001_H_002-ENDF8.0.endf", 128);
    let mut buf: Vec<u8> = Vec::new();
    ace.write_to(&mut buf).unwrap();
    let parsed = parse_type1(&String::from_utf8(buf).unwrap());

    assert_eq!(parsed.nxs, ace.nxs.to_vec());
    assert_eq!(parsed.jxs, ace.jxs.to_vec());
    assert_eq!(parsed.xss.len(), ace.xss.len());
    for (i, (&got, &want)) in parsed.xss.iter().zip(ace.xss.iter()).enumerate() {
        let tol = if ace.xss_is_int[i] { 0.0 } else { 1e-9 * want.abs().max(1.0) };
        assert!((got - want).abs() <= tol, "xss[{i}]: got {got}, want {want}");
    }
}
