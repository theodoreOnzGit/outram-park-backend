//! Compare this port's photo-nuclear ACE table against NJOY2016's
//! `acer iopt = 5`, block by block.
//!
//! ```text
//! cargo run --release -p njoy-outram-park-fork \
//!   --example photonuclear_ace_vs_njoy2016 -- <tape.endf> <mat> <njoy.ace> [comment] [date] [out.ace]
//! ```
//!
//! With `out.ace` the built table is written there as well, which is how the
//! port's own copy in the `reference-data/ace` submodule is produced.
//!
//! Both codes read the same tape. Results are recorded in
//! `verification_and_validation/acer_photonuclear_vs_njoy2016.md`.

use njoy_outram_park_fork::acer::photonuclear::build::{photonuclear_ace, PhotonuclearOptions};
use njoy_outram_park_fork::acer::photonuclear::layout::{jxs, nxs};
use njoy_outram_park_fork::acer::read::{self, AceFileType};
use njoy_outram_park_fork::endf::tape::Tape;

fn cmp(label: &str, a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() {
        println!("  {label:<14} LENGTH ours {} njoy {}", a.len(), b.len());
        return f64::INFINITY;
    }
    let (mut worst, mut at) = (0.0f64, 0usize);
    for i in 0..a.len() {
        let d = if b[i] != 0.0 {
            ((a[i] - b[i]) / b[i]).abs()
        } else {
            (a[i] - b[i]).abs()
        };
        if d > worst {
            worst = d;
            at = i;
        }
    }
    println!(
        "  {label:<14} n={:<6} worst {:.3e} at [{at}] (ours {:.9e} njoy {:.9e})",
        a.len(),
        worst,
        a.get(at).copied().unwrap_or(0.0),
        b.get(at).copied().unwrap_or(0.0)
    );
    worst
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: photonuclear_ace_vs_njoy2016 <tape.endf> <mat> <njoy.ace> [comment] [date]");
        std::process::exit(2);
    }
    let tape = Tape::read_file(std::path::Path::new(&args[0])).expect("read ENDF tape");
    let mat: i32 = args[1].parse().expect("mat");
    let njoy = read::read(&args[2]).expect("read NJOY ACE");
    let opts = PhotonuclearOptions {
        suffix: 0.0,
        comment: args.get(3).cloned().unwrap_or_default(),
        date: args.get(4).cloned().unwrap_or_default(),
        ..Default::default()
    };
    // A photonuclear evaluation has no resonances, so its own MF=3 is the PENDF.
    let built = match photonuclear_ace(&tape, &tape, mat, &opts) {
        Ok(b) => b,
        Err(e) => {
            println!("REFUSED: {e}");
            println!("SUMMARY mat={mat} verdict=NOT_PORTED");
            return;
        }
    };
    println!("built: ZA={} AWR={} NXS(1)={}", built.za, built.awr, built.nxs[0]);
    let mine = built.into_raw(&opts, AceFileType::Type1Ascii);
    for (k, name) in [
        (nxs::LXS, "LXS"), (nxs::ZA, "ZA"), (nxs::NES, "NES"), (nxs::NTR, "NTR"),
        (nxs::NTYPE, "NTYPE"), (nxs::NPIXS, "NPIXS"), (nxs::NEIXS, "NEIXS"),
        (nxs::IZ, "IZ"), (nxs::IA, "IA"), (nxs::TVN, "TVN"),
    ] {
        if mine.nxs[k] != njoy.nxs[k] {
            println!("  NXS {name}: ours {} njoy {}", mine.nxs[k], njoy.nxs[k]);
        }
    }
    for (k, name) in [
        (jxs::ESZ, "ESZ"), (jxs::TOT, "TOT"), (jxs::NON, "NON"), (jxs::ELS, "ELS"),
        (jxs::THN, "THN"), (jxs::MTR, "MTR"), (jxs::LQR, "LQR"), (jxs::LSIG, "LSIG"),
        (jxs::SIG, "SIG"), (jxs::IXSA, "IXSA"), (jxs::IXS, "IXS"),
    ] {
        if mine.jxs[k] != njoy.jxs[k] {
            println!("  JXS {name}: ours {} njoy {}", mine.jxs[k], njoy.jxs[k]);
        }
    }
    if mine.nxs == njoy.nxs && mine.jxs == njoy.jxs {
        println!("NXS and JXS identical.");
    }
    let nes = njoy.nxs[nxs::NES] as usize;
    let cut = |t: &read::RawAceTable, lo: i32, n: usize| -> Vec<f64> {
        let lo = (lo.max(1) - 1) as usize;
        t.xss.get(lo..lo + n).map(|s| s.to_vec()).unwrap_or_default()
    };
    let mut worst = 0.0f64;
    worst = worst.max(cmp("ESZ", &cut(&mine, njoy.jxs[jxs::ESZ], nes), &cut(&njoy, njoy.jxs[jxs::ESZ], nes)));
    worst = worst.max(cmp("TOT", &cut(&mine, njoy.jxs[jxs::TOT], nes), &cut(&njoy, njoy.jxs[jxs::TOT], nes)));
    worst = worst.max(cmp("THN", &cut(&mine, njoy.jxs[jxs::THN], nes), &cut(&njoy, njoy.jxs[jxs::THN], nes)));
    let ntr = njoy.nxs[nxs::NTR] as usize;
    worst = worst.max(cmp("MTR", &cut(&mine, njoy.jxs[jxs::MTR], ntr), &cut(&njoy, njoy.jxs[jxs::MTR], ntr)));
    worst = worst.max(cmp("LQR", &cut(&mine, njoy.jxs[jxs::LQR], ntr), &cut(&njoy, njoy.jxs[jxs::LQR], ntr)));
    let neixs = njoy.nxs[nxs::NEIXS] as usize;
    let ntype = njoy.nxs[nxs::NTYPE] as usize;
    worst = worst.max(cmp("IXSA", &cut(&mine, njoy.jxs[jxs::IXSA], neixs * ntype), &cut(&njoy, njoy.jxs[jxs::IXSA], neixs * ntype)));
    // Per-particle blocks, which isolate a heating disagreement to a particle.
    let ixsa = njoy.jxs[jxs::IXSA] as usize;
    for p in 0..ntype {
        let b = ixsa - 1 + neixs * p;
        let g = |t: &read::RawAceTable, k: usize| -> usize { t.xss[b + k].round() as usize };
        let ipt = njoy.xss[b].round() as i32;
        for (k, name) in [(2usize, "PXS"), (3, "PHN")] {
            let (lo_o, lo_n) = (g(&mine, k), g(&njoy, k));
            // 1-based locator: word `lo` is IE, `lo+1` is NE, `lo+2..` the data.
            let n = njoy.xss[lo_n].round() as usize;
            worst = worst.max(cmp(
                &format!("ipt{ipt} {name}"),
                &mine.xss[lo_o + 1..lo_o + 1 + n],
                &njoy.xss[lo_n + 1..lo_n + 1 + n],
            ));
        }
    }
    println!("XSS length ours {} njoy {}", mine.xss.len(), njoy.xss.len());
    if mine.xss.len() == njoy.xss.len() {
        worst = worst.max(cmp("WHOLE XSS", &mine.xss, &njoy.xss));
    }
    println!("WORST RELATIVE {worst:.3e}");
    if let Some(out) = args.get(5) {
        std::fs::write(out, mine.to_type1_string()).expect("write our table");
        println!("wrote our table to {out}");
    }
    if let Ok(text) = std::fs::read_to_string(&args[2]) {
        let re = mine.to_type1_string();
        if re == text {
            println!("TYPE-1 WRITE: byte-identical to NJOY's file ({} bytes)", text.len());
        } else {
            let n = re.bytes().zip(text.bytes()).take_while(|(a, b)| a == b).count();
            println!("TYPE-1 WRITE: first difference at byte {n} of {} (njoy {})", re.len(), text.len());
            let lo = n.saturating_sub(60);
            println!("  ours ...{:?}", &re[lo..(n + 60).min(re.len())]);
            println!("  njoy ...{:?}", &text[lo..(n + 60).min(text.len())]);
        }
    }
}
