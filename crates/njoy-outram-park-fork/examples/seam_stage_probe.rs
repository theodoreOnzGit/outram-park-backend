//! Which stage loses the U-238 20 keV discontinuity: RECONR or BROADR?
//!
//! Also probes the coordinator's fast-fission hypothesis (op-sdbk, 2026-09-10):
//! does unbounded SIGMA1 smear U-238's MT=18 rise (~1 MeV) or the MT=16 (n,2n)
//! threshold (6.179 MeV)? Each window is dumped after RECONR, after the
//! *unbounded* kernel (`doppler_broaden`, the pre-fix behaviour), and after
//! the upstream-faithful bounded kernel (`broaden_result`), against the tape's
//! own MF=3 values interpolated lin-lin.
use njoy_outram_park_fork::broadr::{broaden_result, broadening_limit, doppler_broaden};
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{eval_lin_lin, reconr, ReconrConfig, ReconrSection};

fn find(secs: &[ReconrSection], mt: i32) -> &ReconrSection {
    secs.iter().find(|s| i32::from(s.mt) == mt).unwrap_or_else(|| panic!("no MT={mt}"))
}

fn dump(tag: &str, pairs: &[(f64, f64)], lo: f64, hi: f64, raw: &[(f64, f64)], max: usize) {
    println!("\n-- {tag} -- points in [{lo:.4e}, {hi:.4e}]  (rel. to tape MF=3 lin-lin):");
    let mut n = 0;
    for &(e, s) in pairs {
        if (lo..=hi).contains(&e) {
            let r = eval_lin_lin(raw, e);
            let rel = if r != 0.0 { (s - r) / r * 100.0 } else { f64::NAN };
            println!("   E = {e:>14.8e}   sigma = {s:>12.6e} b   tape {r:>12.6e}   {rel:>+8.3}%");
            n += 1;
            if n >= max {
                println!("   ...");
                break;
            }
        }
    }
    if n == 0 {
        println!("   (none)");
    }
}

fn main() {
    let path = std::path::Path::new("reference-data/endf/n-092_U_238.endf");
    let tape = Tape::read_file(path).unwrap();
    let mat = tape.materials()[0];
    let raw = |mt: i32| -> Vec<(f64, f64)> {
        let sec = tape.section(mat, 3, mt).unwrap();
        let mut cur = SectionCursor::new(&sec.rows);
        cur.read_cont().unwrap();
        cur.read_tab1().unwrap().pairs
    };
    let raw102 = raw(102);
    let raw18 = raw(18);
    let raw16 = raw(16);

    let t0 = std::time::Instant::now();
    let r = reconr(&tape, &ReconrConfig { mat, tolerance: 1e-3, temperature: 0.0 }).unwrap();
    println!("RECONR: {} s; resonance_upper_limit = {:?}; thnmax = {:.6e} eV",
        t0.elapsed().as_secs_f64(), r.resonance_upper_limit, broadening_limit(&r));

    let t0 = std::time::Instant::now();
    let unb = doppler_broaden(&r.sections, r.material.awr, 600.0);
    println!("unbounded BROADR 600 K: {} s", t0.elapsed().as_secs_f64());
    let t0 = std::time::Instant::now();
    let bnd = broaden_result(&r, 600.0).sections;
    println!("bounded   BROADR 600 K: {} s", t0.elapsed().as_secs_f64());

    println!("\n=============== MT=102 at the resolved/unresolved seam ===============");
    dump("RECONR 0 K", &find(&r.sections, 102).pairs, 1.9999e4, 2.45e4, &raw102, 14);
    dump("UNBOUNDED BROADR 600 K (pre-fix behaviour)", &find(&unb, 102).pairs, 1.9999e4, 2.45e4, &raw102, 14);
    dump("BOUNDED BROADR 600 K (thnmax)", &find(&bnd, 102).pairs, 1.9999e4, 2.45e4, &raw102, 14);
    for e in [2.0e4, 2.05e4, 2.1e4, 2.2e4, 2.3e4] {
        println!("   interp at {e:.3e}: tape {:.5}  reconr {:.5}  unbounded {:.5}  bounded {:.5}",
            eval_lin_lin(&raw102, e), eval_lin_lin(&find(&r.sections, 102).pairs, e),
            eval_lin_lin(&find(&unb, 102).pairs, e), eval_lin_lin(&find(&bnd, 102).pairs, e));
    }

    println!("\n=============== MT=18 fission rise, 0.8-2.0 MeV ===============");
    dump("RECONR 0 K", &find(&r.sections, 18).pairs, 8.0e5, 2.0e6, &raw18, 40);
    dump("UNBOUNDED BROADR 600 K (pre-fix behaviour)", &find(&unb, 18).pairs, 8.0e5, 2.0e6, &raw18, 40);
    dump("BOUNDED BROADR 600 K (thnmax)", &find(&bnd, 18).pairs, 8.0e5, 2.0e6, &raw18, 40);
    // Worst relative deviation of the unbounded kernel over the whole MT=18 grid above 20 keV
    let mut worst = (0.0f64, 0.0f64, 0.0f64);
    for &(e, s) in &find(&unb, 18).pairs {
        if e > 2.0e4 {
            let t = eval_lin_lin(&raw18, e);
            if t > 0.0 {
                let rel = ((s - t) / t).abs();
                if rel > worst.0 { worst = (rel, e, t); }
            }
        }
    }
    println!("\n   unbounded MT=18 worst |rel dev| above 20 keV: {:.3e} at E={:.6e} (tape {:.6e})", worst.0, worst.1, worst.2);

    println!("\n=============== MT=16 (n,2n) threshold 6.179 MeV ===============");
    dump("RECONR 0 K", &find(&r.sections, 16).pairs, 6.1e6, 6.6e6, &raw16, 12);
    dump("UNBOUNDED BROADR 600 K (pre-fix behaviour)", &find(&unb, 16).pairs, 6.1e6, 6.6e6, &raw16, 12);
    dump("BOUNDED BROADR 600 K (thnmax)", &find(&bnd, 16).pairs, 6.1e6, 6.6e6, &raw16, 12);
}
