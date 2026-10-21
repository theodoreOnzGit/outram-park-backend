//! **The scattering-law numbers of the neutronics maths track** (gh:#906,
//! rungs 3 to 5g, from LEAPR toward THERMR): written to
//! `docs/site/science/neutronics/leapr-data.js`.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example lesson_leapr [out.js]
//! ```
//!
//! Every number is this crate's port of NJOY's LEAPR:
//! - **One discrete oscillator** (`leapr::discrete::bfact`, NJOY's `bfact`,
//!   the manual's eq. 542) of weight 1 at two step sizes, β_i = 2 and 8, and
//!   three kicks α: the zero-phonon weight and the weights of creating
//!   (down-scatter) and absorbing (up-scatter) 1 to 10 phonons. The page's
//!   JavaScript implements eq. 542 itself; `check-physics.js` compares.
//! - **Si in α-quartz at 293.6 K** (its LEAPR deck, `reference-data/leapr/`):
//!   `FrequencyModel::start` (NJOY's `start`) gives the Debye-Waller λ, the
//!   effective-temperature ratio T̄/T and the one-phonon term T₁(β);
//!   `continuous::convol` gives T₂ to T₆; `continuous::phonon_expansion`
//!   gives the asymmetric S(α, −β) at five kicks on a fine β grid, and
//!   `sct::sct_free_gas` the short-collision-time bell at the same points.
//!
//! Driver glue (`main`): the LEAPR routines it calls are tested in the crate.

use petir::mathf::RealMath;
use njoy_outram_park_fork::leapr::continuous::{convol, phonon_expansion};
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::discrete::bfact;
use njoy_outram_park_fork::leapr::frequency::FrequencyModel;
use njoy_outram_park_fork::leapr::sct::sct_free_gas;
use std::fmt::Write as _;

fn nums(v: &[f64]) -> String {
    v.iter().map(|x| format!("{x:.6e}")).collect::<Vec<_>>().join(",")
}

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/neutronics/leapr-data.js").to_string()
    });
    // One oscillator, weight 1 (eq. 542 with w_i = 1).
    let mut osc = String::new();
    for bi in [2.0f64, 8.0] {
        let lam = (bi / 2.0).r_cosh() / (bi / 2.0).r_sinh() / bi;
        for a in [0.1f64, 1.0, 5.0] {
            let x = a / (bi * (bi / 2.0).r_sinh());
            let (b0, bp, bm) = bfact(x, a * lam, bi);
            let _ = write!(osc, "{{\"bi\":{bi},\"a\":{a},\"b0\":{b0:.15e},\"up\":[{}],\"down\":[{}]}},", nums(&bp[..10]), nums(&bm[..10]));
            println!("oscillator beta_i {bi}, alpha {a}: zero-phonon {b0:.5e}, first down {:.5e}, first up {:.5e}", bm[0], bp[0]);
        }
    }
    osc.pop();
    // Si in alpha-quartz.
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../reference-data/leapr/tsl-SiO2-alpha-njoy2016-leapr.njoy-input")).map_err(|e| e.to_string())?;
    let deck = LeaprDeck::parse(&text).map_err(|e| e.to_string())?;
    let mut input = deck.input_at(0).map_err(|e| e.to_string())?;
    let tev = input.tev();
    let c = &input.continuous;
    let freq = FrequencyModel::start(&c.rho, c.delta_ev, tev, c.tbeta);
    let mut tn = vec![freq.t1.clone()];
    for _ in 2..=6 {
        let last = tn.last().cloned().unwrap_or_default();
        let (next, _) = convol(&freq.t1, &last, freq.t1.len() + last.len() - 1, freq.deltab);
        tn.push(next);
    }
    let kicks = [0.05f64, 0.3, 1.0, 5.0, 20.0];
    let betas: Vec<f64> = (0..=600).map(|i| i as f64 * 0.05).collect();
    let (sc, arat) = (input.scale(), input.arat);
    input.alpha = kicks.iter().map(|a| a * arat / sc).collect();
    input.beta = betas.iter().map(|b| b / sc).collect();
    let sab = phonon_expansion(&input, &freq);
    let mut cols = String::new();
    for (j, &a) in kicks.iter().enumerate() {
        let s: Vec<f64> = (0..betas.len()).map(|k| sab.get(k, j)).collect();
        let sct: Vec<f64> = betas.iter().map(|&b| sct_free_gas(a * c.tbeta, b, freq.tbar)).collect();
        let _ = write!(cols, "{{\"a\":{a},\"s\":[{}],\"sct\":[{}]}},", nums(&s), nums(&sct));
    }
    cols.pop();
    let tns: Vec<String> = tn.iter().map(|t| format!("[{}]", nums(t))).collect();
    println!("quartz {} K: deltab {:.5}, lambda {:.5}, tbar {:.5}, T1 {} points; S at {} kicks x {} betas", input.temperature_k, freq.deltab, freq.f0, freq.tbar, freq.t1.len(), kicks.len(), betas.len());
    let file = format!(
        "// Scattering-law numbers for the neutronics maths track (gh:#906), written by\n\
         // crates/njoy-outram-park-fork/examples/lesson_leapr.rs (regenerate with it; do\n\
         // not edit), all from OUTRAM PARK's LEAPR port. osc: bfact for one oscillator\n\
         // (weight 1, step bi = hbar omega/kT, kick a): zero-phonon weight b0, up[k-1]\n\
         // and down[k-1] the weights of absorbing and creating k phonons. quartz: Si in\n\
         // alpha-quartz at {T} K from its LEAPR deck: deltab, lambda, tbar = Tbar/T, tn[n-1]\n\
         // = T_n on beta = 0, deltab, 2 deltab, ...; s = S(alpha, -beta) from\n\
         // phonon_expansion (no zero-phonon term) and sct = sct_free_gas, on beta = 0, 0.05,\n\
         // ..., 30.\n\
         window.LEAPR_DATA = {{\"osc\":[{osc}],\"quartz\":{{\"t\":{T},\"tev\":{tev},\"deltab\":{},\"lambda\":{},\"tbar\":{},\"tbeta\":{},\"tn\":[{}],\"beta\":[{}],\"cols\":[{cols}]}}}};\n",
        freq.deltab,
        freq.f0,
        freq.tbar,
        c.tbeta,
        tns.join(","),
        nums(&betas),
        T = input.temperature_k
    );
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
