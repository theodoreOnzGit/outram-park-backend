//! **The data of neutronics lesson pages 20 onward** (gh:#906, "But the atoms
//! are bound!" and the lattice pages after it): written to
//! `docs/site/science/neutronics/bound-data.js`.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example lesson_bound [out.js]
//! ```
//!
//! - **Vibration spectra** from the LEAPR decks in `reference-data/leapr/`,
//!   read by this crate's deck parser (`leapr::deck::LeaprDeck::parse`): the
//!   continuous spectrum ρ (its spacing and values), the translational
//!   weight, and any discrete oscillators. α-quartz (Si in SiO₂) is a
//!   crystal; H in H₂O is a liquid of molecules.
//! - **Where the thermal tables stop**: MF=7/MT=4 `B(4)`, the upper energy of
//!   each `tsl-*` ENDF/B-VIII.0 evaluation's S(α,β) (ENDF-102 §7.4), read by
//!   `thermr::mf7::parse_mf7`; with its mass ratio, base temperature and,
//!   for a crystal with coherent elastic data, its lowest Bragg edge.
//! - **Graphite's coherent elastic (Bragg) data** at 296 K and 1200 K, from
//!   `thermr::coherent` (`CoherentElastic::s_of_e_at`): the Bragg edges E_i
//!   and the cumulative S(E_i, T) \[eV b\], so σ = S/E between edges, with
//!   reference values of `CoherentElastic::cross_section` for the page's
//!   check.
//!
//! Driver glue (`main`): the parsers it calls are tested in the crate.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::thermr::mf7::parse_mf7;
use std::fmt::Write as _;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../reference-data"))
}

fn nums(v: &[f64]) -> String {
    v.iter().map(|x| format!("{x:.6e}")).collect::<Vec<_>>().join(",")
}

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/neutronics/bound-data.js").to_string()
    });
    let mut decks = String::new();
    for (name, file) in [("Si in alpha-quartz", "tsl-SiO2-alpha-njoy2016-leapr.njoy-input"), ("H in H2O", "tsl-HinH2O-293.6K-njoy2016-leapr.njoy-input")] {
        let text = std::fs::read_to_string(root().join("leapr").join(file)).map_err(|e| format!("{file}: {e}"))?;
        let d = LeaprDeck::parse(&text).map_err(|e| e.to_string())?;
        let t = d.temperatures.first().ok_or("no temperature")?;
        let c = &t.continuous;
        let emax = c.delta_ev * (c.rho.len() as f64 - 1.0);
        let osc: Vec<String> = t.oscillators.iter().map(|o| format!("[{},{}]", o.energy_ev, o.weight)).collect();
        println!("{name}: AWR {}, T {} K, rho {} points x {} eV (to {emax:.4} eV), twt {}, c {}, tbeta {}, oscillators {:?}", d.awr, t.temperature_k, c.rho.len(), c.delta_ev, c.twt, c.c, c.tbeta, osc);
        let _ = write!(decks, "\"{name}\":{{\"file\":\"{file}\",\"awr\":{},\"t\":{},\"delta\":{},\"rho\":[{}],\"twt\":{},\"c\":{},\"tbeta\":{},\"osc\":[{}]}},", d.awr, t.temperature_k, c.delta_ev, nums(&c.rho), c.twt, c.c, c.tbeta, osc.join(","));
    }
    decks.pop();
    let mut tapes = String::new();
    for (name, file) in [("graphite (crystalline)", "tsl-crystalline-graphite.endf"), ("H in H2O", "tsl-HinH2O.endf"), ("H in ZrH", "tsl-HinZrH-ENDF8.0.endf"), ("Si in SiC", "tsl-SiinSiC.endf"), ("C in SiC", "tsl-CinSiC.endf")] {
        let tape = Tape::read(std::fs::File::open(root().join("endf").join(file)).map_err(|e| format!("{file}: {e}"))?).map_err(|e| e.to_string())?;
        let mat = *tape.materials().first().ok_or("no material")?;
        let m = parse_mf7(&tape, mat).map_err(|e| e.to_string())?;
        let ii = m.incoherent_inelastic.as_ref().ok_or("no MT=4")?;
        let emax = ii.b.get(3).copied().unwrap_or(0.0);
        let bragg = m.coherent_elastic.as_ref().and_then(|c| c.bragg_energies_ev.first().copied());
        println!("{name}: MAT {mat}, AWR {}, B(4) = {emax} eV, base T {} K, tabulated {:?} K, lowest Bragg edge {bragg:?} eV", m.awr, ii.temperature_k, ii.tabulated_temperatures_k);
        let _ = write!(
            tapes,
            "\"{name}\":{{\"file\":\"{file}\",\"mat\":{mat},\"awr\":{},\"emax\":{emax},\"t0\":{},\"temps\":[{}],\"bragg0\":{}}},",
            m.awr,
            ii.temperature_k,
            ii.tabulated_temperatures_k.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(","),
            bragg.map(|b| b.to_string()).unwrap_or_else(|| "null".into())
        );
    }
    tapes.pop();
    // Graphite's Bragg edges through the THERMR port.
    let tape = Tape::read(std::fs::File::open(root().join("endf").join("tsl-crystalline-graphite.endf")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mat = *tape.materials().first().ok_or("no material")?;
    let m = parse_mf7(&tape, mat).map_err(|e| e.to_string())?;
    let coh = m.coherent_elastic.as_ref().ok_or("no coherent elastic")?;
    let mut bragg = String::new();
    for t in [296.0, 1200.0] {
        let s = coh.s_of_e_at(t).map_err(|e| e.to_string())?;
        let first = s.iter().find(|p| p.1 > 0.0).map(|p| p.0).unwrap_or(0.0);
        let es: Vec<f64> = [1e-3, 2e-3, 3e-3, 5e-3, 1e-2, 2.53e-2, 0.1, 1.0].to_vec();
        let refs: Vec<f64> = es.iter().map(|&e| coh.cross_section(e, t)).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        println!("graphite {t} K: {} edges, first with S > 0 at {first} eV; sigma_coh at {:?} eV = {:?} b", s.len(), es, refs);
        let _ = write!(
            bragg,
            "\"{t}\":{{\"e\":[{}],\"s\":[{}],\"refE\":[{}],\"ref\":[{}]}},",
            nums(&s.iter().map(|p| p.0).collect::<Vec<_>>()),
            nums(&s.iter().map(|p| p.1).collect::<Vec<_>>()),
            nums(&es),
            refs.iter().map(|x| format!("{x:.15e}")).collect::<Vec<_>>().join(",")
        );
    }
    bragg.pop();
    let file = format!(
        "// Data for the neutronics lessons, pages 20 onward (gh:#906), written by\n\
         // crates/njoy-outram-park-fork/examples/lesson_bound.rs (regenerate with it;\n\
         // do not edit). decks: vibration spectra from reference-data/leapr/ (delta\n\
         // [eV] spacing of rho, rho values, twt/c/tbeta, discrete oscillators [E eV,\n\
         // weight]). tapes: ENDF/B-VIII.0 tsl-* evaluations, MF=7/MT=4 B(4) as emax\n\
         // [eV], base and tabulated temperatures [K], lowest Bragg edge [eV].\n\
         // bragg: graphite coherent elastic, edges e [eV] and cumulative s [eV b]\n\
         // per temperature, with the port's sigma_coh at refE.\n\
         window.BOUND_DATA = {{\"decks\":{{{decks}}},\"tapes\":{{{tapes}}},\"bragg\":{{{bragg}}}}};\n"
    );
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
