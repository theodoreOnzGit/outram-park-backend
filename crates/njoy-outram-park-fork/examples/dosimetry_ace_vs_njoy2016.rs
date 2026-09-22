//! Compare this port's dosimetry ACE table against NJOY2016's `acer iopt=3`.
//!
//! ## Methodology
//!
//! Both codes read the **same PENDF** — so the comparison isolates `acedos`
//! and does not fold in RECONR or BROADR — and write a Type-1 ACE file. The
//! pass statement is byte equality of that file; anything short of it is
//! reported as the first differing byte and the surrounding text.
//!
//! ```text
//! cargo run --release -p njoy-outram-park-fork \
//!   --example dosimetry_ace_vs_njoy2016 -- <pendf> <mat> <T_K> <njoy.ace> [comment] [date]
//! ```
//!
//! ## Results
//!
//! Recorded in
//! `verification_and_validation/acer_dosimetry_vs_njoy2016.md`.

use njoy_outram_park_fork::acer::dosimetry::{dosimetry_ace, DosimetryOptions};
use njoy_outram_park_fork::acer::read::{self, AceFileType};
use njoy_outram_park_fork::endf::tape::Tape;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        eprintln!(
            "usage: dosimetry_ace_vs_njoy2016 <pendf> <mat> <T_K> <njoy.ace> [comment] [date]"
        );
        std::process::exit(2);
    }
    let tape = Tape::read_file(std::path::Path::new(&args[0])).expect("read PENDF");
    let mat: i32 = args[1].parse().expect("mat");
    let t_k: f64 = args[2].parse().expect("temperature");
    let njoy = read::read(&args[3]).expect("read NJOY ACE");

    let opts = DosimetryOptions {
        suffix: 0.0,
        comment: args.get(4).cloned().unwrap_or_default(),
        date: args.get(5).cloned().unwrap_or_default(),
        ..Default::default()
    };
    let built = dosimetry_ace(&tape, mat, t_k, &opts).expect("build dosimetry ACE");
    println!(
        "built: ZA={} AWR={} ntr={} T={} K kT={:.5e} MeV len2={}",
        built.za, built.awr, built.ntr, built.temperature_k, built.kt_mev, built.nxs[0]
    );
    println!("  reactions: {:?}", (0..built.ntr).map(|i| built.mt(i)).collect::<Vec<_>>());
    let mine = built.into_raw(&opts, AceFileType::Type1Ascii);
    println!("NXS mine {:?}", &mine.nxs[..4]);
    println!("NXS njoy {:?}", &njoy.nxs[..4]);
    println!("JXS mine {:?}", &mine.jxs[..8]);
    println!("JXS njoy {:?}", &njoy.jxs[..8]);
    assert_eq!(mine.nxs, njoy.nxs, "NXS differs");
    assert_eq!(mine.jxs, njoy.jxs, "JXS differs");

    let mut worst = 0.0f64;
    let mut at = 0usize;
    assert_eq!(mine.xss.len(), njoy.xss.len(), "XSS length differs");
    for i in 0..mine.xss.len() {
        let (a, b) = (mine.xss[i], njoy.xss[i]);
        let r = if b != 0.0 { (a - b).abs() / b.abs() } else { (a - b).abs() };
        if r > worst {
            worst = r;
            at = i;
        }
    }
    println!(
        "XSS: {} words, worst relative {worst:.3e} at [{at}] (mine {:.12e} njoy {:.12e})",
        mine.xss.len(),
        mine.xss.get(at).copied().unwrap_or(0.0),
        njoy.xss.get(at).copied().unwrap_or(0.0)
    );

    if let Ok(text) = std::fs::read_to_string(&args[3]) {
        let re = mine.to_type1_string();
        if re == text {
            println!("TYPE-1 WRITE: byte-identical to NJOY's file ({} bytes)", text.len());
        } else {
            let n = re.bytes().zip(text.bytes()).take_while(|(a, b)| a == b).count();
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
