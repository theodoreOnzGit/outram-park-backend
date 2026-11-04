//! **The data of the Fission products lesson "Breathing: how the release
//! leaves the vessel"** (`atops-breathing.html`; gh:#1207, maintainer
//! direction 2026-11-04: "add the lesson on breathing for TRISO-ATOPS and the
//! assumption it makes"). Writes `breathing-data.js` in
//! `docs/site/science/fission-products/`.
//!
//! ```bash
//! cargo run --release -p dhoby-ghaut --example lesson_breathing [out_dir]
//! ```
//!
//! What is compared, with the same kernel release R(t) and the same
//! temperatures on both sides (see `breath.rs`): TRISO-ATOPS's product
//! `frac(t) × R(t)`, as its port computes it, against a well-mixed gas
//! volume stepped by boon-lay's `live_pools::step` (gas leaves at the rate
//! the ideal gas law gives while the core heats, nothing leaves while it
//! cools).
//!
//! **Predictions, written 2026-11-04 before the first run** (nuclides
//! Cs-137, Sr-90, I-131, Kr-88; the illustrative heat-up from 900 °C to a
//! peak of 1400, 1600 or 1800 °C in 50 h, held 50 h, back down over 200 h):
//!
//! - B1. Without decay the well-mixed gas never carries out more than the
//!   product, at any venting sample, nuclide or peak. Derived beforehand: an
//!   atom released at s leaves with share 1 − T(s)/Tmax(t) ≤ 1 − T0/Tmax(t),
//!   which is what the product gives every atom.
//! - B2. The `live_pools` run (no decay, 200 substeps per 5 h interval)
//!   agrees with the written-out formula (`mixed_formula`, 200 points) to
//!   1e-6 absolute at every sample.
//! - B3. At the end, Cs-137 and Sr-90 carried out by the well-mixed gas are
//!   less than half the product at every peak: most of their kernel release
//!   happens in the hold, when no gas leaves.
//! - B4. With decay in the gas, Kr-88 (T½ 2.84 h) is carried out at less
//!   than 0.6 of its no-decay value, I-131 (8.0 d) within 5 % of it, Cs-137
//!   and Sr-90 within 1e-3 relative.
//! - B5. Neither model carries anything out after 100 h (the end of the
//!   hold): the product's last venting sample is at 100 h, and the
//!   well-mixed curve is flat from 100 h to 300 h.
//! - B6. 400 substeps change the well-mixed results by less than 1e-6
//!   relative against 200.
//!
//! **Results, first run 2026-11-04 (predictions above unedited):**
//!
//! - B1: **missed in the port's own numbers, at 1-3 early samples per case**
//!   (5 h for Cs-137, I-131 and Kr-88; 5-15 h for Sr-90). There the
//!   well-mixed gas carries out slightly more than the product, e.g. Cs-137
//!   at 1600 °C, 5 h: 4.64e-4 against 4.38e-4. The cause is the port's
//!   vented fraction, not the product. `coolant_release` drops half of the
//!   first 5 h step (the trapezoid starts from dT/dt = 0; lesson "Accidents"
//!   records it): 0.0266 at 5 h against the ideal gas's 0.0563 (970 °C). With the
//!   ideal-gas fraction 1 − T0/Tmax, the product is above the well-mixed gas
//!   at every venting sample (`check-breathing.js`), as derived.
//! - B2 held: `live_pools` against the written-out formula, worst 3.95e-9.
//! - B3 held. Well-mixed over product at the end (100 h): Cs-137 0.168,
//!   0.230 and 0.364 (peaks 1400/1600/1800 °C); Sr-90 0.059, 0.047 and
//!   0.045. Also, not predicted: I-131 0.206, 0.059 and 0.164; Kr-88 0.041,
//!   0.021 and 0.059 (with decay).
//! - B4 held: Kr-88 with decay at 0.188, 0.345 and 0.354 of no decay; I-131
//!   at 0.950, 0.969 and 0.975 (within 5 %); Cs-137 and Sr-90 1.0000.
//! - B5 held: the last venting sample is 100 h in every case, and the
//!   well-mixed curve does not change from 100 to 300 h (0 to the last bit).
//! - B6 held: at most 2.2e-7 relative.
//!
//! **So the product is conservative here**, by a factor of 3 to 50 on this
//! history. Its assumption is that everything released by t was in the gas
//! from the start and leaves with the whole cumulative vented share,
//! including what the fuel gives up during the hold, when no gas leaves.
//!
//! Driver glue (`main`, `native::run`, formatting): writes what `breath.rs`
//! computes; its output is checked by `check-breathing.js` and the functions
//! are reached by `tests/lesson_breathing.rs`. Research, education and V&V
//! only (`RESPONSIBLE_USE.md`): an illustrative history on one MHTGR Case A
//! node, not a plant calculation.

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
#[path = "breath.rs"]
mod breath;

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> Result<(), String> {
    native::run()
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
mod native {
    use super::breath::*;
    use std::fmt::Write as _;

    pub const PEAKS: [f64; 3] = [1400.0, 1600.0, 1800.0];

    fn e(x: f64) -> String {
        format!("{x:.6e}")
    }
    fn arr(v: &[f64]) -> String {
        format!("[{}]", v.iter().map(|x| e(*x)).collect::<Vec<_>>().join(","))
    }
    fn opt(v: &[Option<f64>]) -> String {
        format!("[{}]", v.iter().map(|x| x.map_or("null".into(), e)).collect::<Vec<_>>().join(","))
    }

    pub fn run() -> Result<(), String> {
        let out_dir = std::env::args().nth(1).unwrap_or_else(|| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/fission-products").to_string()
        });
        let mut js = String::new();
        writeln!(js, "// The data of the Fission products lesson \"Breathing: how the release leaves the vessel\".").unwrap();
        writeln!(js, "// GENERATED by crates/dhoby-ghaut/examples/lesson_breathing/main.rs:").unwrap();
        writeln!(js, "//   cargo run --release -p dhoby-ghaut --example lesson_breathing").unwrap();
        writeln!(js, "// Do not edit; checked by check-breathing.js. Fractions of the kernel inventory; hours; °C.").unwrap();
        writeln!(js, "window.BREATHING_DATA = {{ peaks: {PEAKS:?}, cases: [").unwrap();
        for name in NUCLIDES {
            for peak in PEAKS {
                let b = breath(name, peak, 200);
                let fine = breath(name, peak, 400);
                let formula = mixed_formula(&b.hours, &b.t_c, &b.release, 200);
                let b1 = b.atops.iter().zip(&b.mixed_no_decay).filter(|(a, m)| a.is_some_and(|a| **m > a + 1e-15)).count();
                let b2 = b.mixed_no_decay.iter().zip(&formula).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
                let n = b.hours.len() - 1;
                let i100 = b.hours.iter().position(|h| *h == 100.0).expect("100 h sample");
                let last_vent = b.hours.iter().zip(&b.atops).filter(|(_, a)| a.is_some()).map(|(h, _)| *h).fold(0.0, f64::max);
                let atops_end = b.atops[i100].expect("100 h vents");
                let flat = (b.mixed[n] - b.mixed[i100]).abs();
                let b6 = b.mixed.iter().zip(&fine.mixed).map(|(x, y)| if y.abs() > 0.0 { (x - y).abs() / y.abs() } else { 0.0 }).fold(0.0, f64::max);
                println!(
                    "{name} peak {peak}: R(end) {:.4e}; ATOPS {atops_end:.4e}; mixed {:.4e} (no decay {:.4e}); ratio mixed/ATOPS {:.4}; decay/no-decay {:.4}; B1 violations {b1}; B2 {b2:.2e}; B5 last vent {last_vent} h, mixed change 100-300 h {flat:.2e}; B6 {b6:.2e}; vent at 100 h port {:.5} exact {:.5}",
                    b.release[n], b.mixed[n], b.mixed_no_decay[n], b.mixed[n] / atops_end, b.mixed[n] / b.mixed_no_decay[n],
                    b.vent[i100].unwrap_or(f64::NAN), b.vent_exact[i100]
                );
                writeln!(js, "  {{ name: \"{name}\", peak: {peak}, hours: {}, t: {}, release: {}, vent: {}, ventExact: {}, atops: {}, mixed: {}, mixedNoDecay: {}, formula: {}, b1: {b1}, b2: {}, b6: {} }},",
                    arr(&b.hours), arr(&b.t_c), arr(&b.release), opt(&b.vent), arr(&b.vent_exact), opt(&b.atops), arr(&b.mixed), arr(&b.mixed_no_decay), arr(&formula), e(b2), e(b6)).unwrap();
            }
        }
        writeln!(js, "] }};").unwrap();
        let path = std::path::Path::new(&out_dir).join("breathing-data.js");
        std::fs::write(&path, &js).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {} ({} bytes)", path.display(), js.len());
        Ok(())
    }
}
