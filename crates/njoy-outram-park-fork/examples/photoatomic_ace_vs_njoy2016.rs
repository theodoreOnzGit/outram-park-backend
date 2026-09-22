//! Compare this port's photo-atomic ACE table against NJOY2016's own
//! `acer iopt=4` output, block by block.
//!
//! ## Methodology
//!
//! Both sides start from the **same** ENDF photo-atomic evaluation. NJOY's
//! table is read with [`njoy_outram_park_fork::acer::read`], this port's is
//! built by [`photoatomic_ace`] and put through the same writer, so the
//! comparison is value-to-value on the written form (ESZG in natural logs)
//! rather than on an intermediate nobody ships.
//!
//! Reported per block: the count compared, the worst absolute and relative
//! difference, and where it sits. NXS and JXS are compared exactly — a
//! one-word disagreement there is a different table, not a tolerance question.
//!
//! ```text
//! cargo run --release -p njoy-outram-park-fork \
//!     --example photoatomic_ace_vs_njoy2016 -- <tape.endf> <mat> <njoy.ace>
//! ```
//!
//! ## Results
//!
//! Recorded in
//! `verification_and_validation/acer_photoatomic_vs_njoy2016.md`.

use njoy_outram_park_fork::acer::photoatomic::{photoatomic_ace, PhotoatomicOptions};
use njoy_outram_park_fork::acer::read;
use njoy_outram_park_fork::endf::tape::Tape;

fn worst(label: &str, mine: &[f64], njoy: &[f64]) -> f64 {
    assert_eq!(
        mine.len(),
        njoy.len(),
        "{label}: block lengths differ ({} vs {})",
        mine.len(),
        njoy.len()
    );
    let mut wa = 0.0f64;
    let mut wr = 0.0f64;
    let mut at = 0usize;
    for i in 0..mine.len() {
        let a = (mine[i] - njoy[i]).abs();
        let r = if njoy[i] != 0.0 { a / njoy[i].abs() } else { a };
        if r > wr {
            wr = r;
            at = i;
        }
        wa = wa.max(a);
    }
    println!(
        "  {label:<12} n={:<7} worst abs {:.3e}  worst rel {:.3e} at [{}] \
         (mine {:.12e} njoy {:.12e})",
        mine.len(),
        wa,
        wr,
        at,
        mine.get(at).copied().unwrap_or(0.0),
        njoy.get(at).copied().unwrap_or(0.0)
    );
    wr
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!(
            "usage: photoatomic_ace_vs_njoy2016 <tape.endf> <mat> <njoy.ace> [comment] [date]"
        );
        std::process::exit(2);
    }
    let tape = Tape::read_file(std::path::Path::new(&args[0])).expect("read ENDF tape");
    let mat: i32 = args[1].parse().expect("mat");
    let njoy = read::read(&args[2]).expect("read NJOY ACE");

    let opts = PhotoatomicOptions {
        suffix: 0.0,
        comment: args.get(3).cloned().unwrap_or_default(),
        date: args.get(4).cloned().unwrap_or_default(),
        ..Default::default()
    };
    let built = photoatomic_ace(&tape, mat, &opts, None).expect("build photoatomic ACE");
    let nes = built.nes;
    let nflo = built.nflo;
    println!(
        "built: nes={nes} nflo={nflo} z={} len2={} (fluorescence {})",
        built.z,
        built.nxs[0],
        if built.fluorescence_missing {
            "not processed -- no relaxation tape, as upstream"
        } else {
            "processed"
        }
    );
    let [built_e, built_inc, built_coh, built_abs, built_pair] = built.eszg();
    let (built_e, built_inc, built_coh, built_abs, built_pair) = (
        built_e.to_vec(),
        built_inc.to_vec(),
        built_coh.to_vec(),
        built_abs.to_vec(),
        built_pair.to_vec(),
    );
    let mine = built.into_raw(&opts, njoy_outram_park_fork::acer::read::AceFileType::Type1Ascii);

    println!("NXS mine {:?}", &mine.nxs[..4]);
    println!("NXS njoy {:?}", &njoy.nxs[..4]);
    assert_eq!(mine.nxs, njoy.nxs, "NXS differs");
    assert_eq!(mine.jxs, njoy.jxs, "JXS differs");
    println!("NXS and JXS identical.");
    println!("ZAID mine {:?} njoy {:?}", mine.header.zaid, njoy.header.zaid);
    println!("AWR  mine {} njoy {}", mine.header.awr, njoy.header.awr);

    let jinc = mine.jxs[1] as usize;
    let jcoh = mine.jxs[2] as usize;
    let jflo = mine.jxs[3] as usize;
    let lhnm = mine.jxs[4] as usize;
    let cut = |t: &read::RawAceTable, lo: usize, n: usize| t.xss[lo - 1..lo - 1 + n].to_vec();

    let mut worst_rel: f64 = 0.0;
    for (k, name) in ["log E", "log inc", "log coh", "log pe", "log pair"]
        .iter()
        .enumerate()
    {
        let lo = 1 + k * nes;
        worst_rel = worst_rel.max(worst(name, &cut(&mine, lo, nes), &cut(&njoy, lo, nes)));
    }
    worst_rel = worst_rel.max(worst("S(v,Z)", &cut(&mine, jinc, 21), &cut(&njoy, jinc, 21)));
    worst_rel = worst_rel.max(worst(
        "coh integral",
        &cut(&mine, jcoh, 55),
        &cut(&njoy, jcoh, 55),
    ));
    worst_rel = worst_rel.max(worst(
        "F(v,Z)",
        &cut(&mine, jcoh + 55, 55),
        &cut(&njoy, jcoh + 55, 55),
    ));
    if nflo > 0 {
        worst_rel = worst_rel.max(worst(
            "fluorescence",
            &cut(&mine, jflo, 4 * nflo),
            &cut(&njoy, jflo, 4 * nflo),
        ));
    }
    worst_rel = worst_rel.max(worst("heating", &cut(&mine, lhnm, nes), &cut(&njoy, lhnm, nes)));

    // Attribute a heating disagreement to one of its three terms rather than
    // reporting it as a single number: the incoherent integral, the
    // photoelectric term, and the pair-production excess are separable.
    if std::env::var("OUTRAM_PARK_PHOTOATOMIC_HIST").is_ok() {
        // How many XSS words disagree, and by how much -- a single outlier and
        // a broad drift are different findings.
        let mut buckets = [0usize; 8];
        let mut worst: Vec<(f64, usize)> = Vec::new();
        for i in 0..mine.xss.len() {
            let (a, b) = (mine.xss[i], njoy.xss[i]);
            let r = if b != 0.0 { (a - b).abs() / b.abs() } else { (a - b).abs() };
            let k = if r == 0.0 {
                0
            } else {
                match r {
                    r if r < 1e-12 => 1,
                    r if r < 1e-11 => 2,
                    r if r < 1e-10 => 3,
                    r if r < 1e-9 => 4,
                    r if r < 1e-8 => 5,
                    r if r < 1e-7 => 6,
                    _ => 7,
                }
            };
            buckets[k] += 1;
            worst.push((r, i));
        }
        let names = ["exact", "<1e-12", "<1e-11", "<1e-10", "<1e-9", "<1e-8", "<1e-7", ">=1e-7"];
        println!("  XSS word agreement histogram over {} words:", mine.xss.len());
        for (k, n) in buckets.iter().enumerate() {
            if *n > 0 {
                println!("    {:<8} {n}", names[k]);
            }
        }
        worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!("  worst 12 words (rel, index, block, E if in a grid block):");
        for &(r, i) in worst.iter().take(12) {
            let (blk, at) = if i < 5 * nes {
                (["log E", "log inc", "log coh", "log pe", "log pair"][i / nes], i % nes)
            } else if i + 1 >= lhnm {
                ("heating", i + 1 - lhnm)
            } else {
                ("form factor", i)
            };
            let e = if blk == "heating" || blk.starts_with("log") {
                format!("{:.5e}", built_e[at])
            } else {
                "-".to_string()
            };
            println!("    {r:.3e}  [{i}] {blk} at {at}  E={e}");
        }
    }
    if std::env::var("OUTRAM_PARK_PHOTOATOMIC_SCAN").is_ok() {
        // The incoherent term is separable from the other two, so scan its own
        // relative disagreement across the whole grid rather than reading the
        // column's, which the pair term dilutes by three orders of magnitude.
        println!("  incoherent-term profile (E MeV, rel difference of iheat's contribution):");
        let mut prev_decade = i32::MIN;
        for i in 0..nes {
            let e = built_e[i];
            let (inc, coh, abs_, pair) =
                (built_inc[i], built_coh[i], built_abs[i], built_pair[i]);
            let tot = inc + coh + abs_ + pair;
            let pe = e * abs_;
            let pp = pair * (e - 1.022);
            let a = mine.xss[lhnm - 1 + i] * tot - pe - pp;
            let b = njoy.xss[lhnm - 1 + i] * tot - pe - pp;
            let rel = if b != 0.0 { (a - b).abs() / b.abs() } else { 0.0 };
            let decade = e.log10().floor() as i32;
            if decade != prev_decade {
                prev_decade = decade;
                println!("    E={e:.4e}  inc term mine {a:.9e} njoy {b:.9e}  rel {rel:.3e}");
            }
        }
    }
    if let Ok(k) = std::env::var("OUTRAM_PARK_PHOTOATOMIC_DEBUG") {
        let i: usize = k.parse().expect("index");
        let e = built_e[i];
        let (inc, coh, abs_, pair) = (built_inc[i], built_coh[i], built_abs[i], built_pair[i]);
        let tot = inc + coh + abs_ + pair;
        let hn = njoy.xss[lhnm - 1 + i];
        let hm = mine.xss[lhnm - 1 + i];
        let pe_term = e * abs_;
        let pp_term = pair * (e - 1.022);
        let inc_term_mine = hm * tot - pe_term - pp_term;
        let inc_term_njoy = hn * tot - pe_term - pp_term;
        // The reconstruction below is amplified by (pe+pp)/inc_term, so first
        // state how far the five ESZG columns themselves agree at this index.
        for (k, name) in ["E", "inc", "coh", "pe", "pair"].iter().enumerate() {
            let lm = mine.xss[k * nes + i];
            let ln = njoy.xss[k * nes + i];
            println!(
                "       ESZG {name:<5} log mine {lm:.13e} njoy {ln:.13e}  \
                 value rel {:.3e}",
                (lm - ln).abs()
            );
        }
        println!("  [{i}] E={e:.6e} MeV  inc={inc:.6e} coh={coh:.6e} pe={abs_:.6e} pair={pair:.6e}");
        println!("       total={tot:.6e}  heating mine={hm:.12e} njoy={hn:.12e}");
        println!("       photoelectric term {pe_term:.12e}   pair term {pp_term:.12e}");
        println!(
            "       incoherent term mine {inc_term_mine:.12e} njoy {inc_term_njoy:.12e}  rel {:.3e}",
            (inc_term_mine - inc_term_njoy).abs() / inc_term_njoy.abs()
        );
        println!(
            "       => the incoherent term is {:.3e} of the total heating",
            inc_term_njoy.abs() / (hn * tot).abs()
        );
    }
    println!("WORST RELATIVE over every block: {worst_rel:.3e}");

    // Type-2 carries ESZG linearly, so compare that container separately.
    if let Ok(t2) = std::env::var("OUTRAM_PARK_PHOTOATOMIC_TYPE2") {
        let want = std::fs::read(&t2).expect("read NJOY type-2 ACE");
        let rebuilt = photoatomic_ace(&tape, mat, &opts, None).expect("rebuild");
        let t = rebuilt.into_raw(&opts, njoy_outram_park_fork::acer::read::AceFileType::Type2Binary);
        let got = t.to_type2_bytes();
        let first = got.iter().zip(want.iter()).position(|(a, b)| a != b);
        match first {
            None if got.len() == want.len() => {
                println!("TYPE-2 WRITE: byte-identical to NJOY's file ({} bytes)", want.len())
            }
            _ => {
                let n = first.unwrap_or(want.len().min(got.len()));
                let w = (n.saturating_sub(508)) / 16;
                println!(
                    "TYPE-2 WRITE: first difference at byte {n} (XSS word {w}) of {} (njoy {})",
                    got.len(),
                    want.len()
                );
                let mut count = 0usize;
                for i in 0..t.xss.len().min((want.len() - 508) / 16) {
                    let o = 512 + 16 * i;
                    let nv = f64::from_le_bytes(want[o..o + 8].try_into().unwrap());
                    if nv != t.xss[i] {
                        if count < 8 {
                            println!(
                                "   word {i}: mine {:.17e} njoy {:.17e}  ulps {}",
                                t.xss[i],
                                nv,
                                (t.xss[i].to_bits() as i64 - nv.to_bits() as i64).abs()
                            );
                        }
                        count += 1;
                    }
                }
                println!("   {count} of {} words differ in the binary form", t.xss.len());
            }
        }
    }

    // The written form, byte for byte, is the sharper statement when it holds.
    if let Ok(text) = std::fs::read_to_string(&args[2]) {
        let re = mine.to_type1_string();
        if re == text {
            println!("TYPE-1 WRITE: byte-identical to NJOY's file ({} bytes)", text.len());
        } else {
            let n = re
                .bytes()
                .zip(text.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            println!(
                "TYPE-1 WRITE: first difference at byte {n} of {} (njoy {} bytes)",
                re.len(),
                text.len()
            );
            let lo = n.saturating_sub(60);
            println!("  mine ...{:?}", &re[lo..(n + 60).min(re.len())]);
            println!("  njoy ...{:?}", &text[lo..(n + 60).min(text.len())]);
        }
    }
}
