//! Thermal S(alpha,beta) ACE table vs NJOY2016 — the `...t` tables.
//!
//! The continuous-energy comparator next door (`ace_vs_njoy2016`) covers the
//! incident-neutron sublibrary. The thermal sublibrary is a different ACE
//! format with its own NXS/JXS and blocks (ITIE/ITIX/ITXE/ITCE/ITCX), so it
//! needs its own comparison; nine `tsl-*.endf` tapes in `reference-data/endf/`
//! had none.
//!
//! # Methodology
//!
//! NJOY2016 side, per tape: `reconr` -> `broadr` -> `thermr` -> `acer` with
//! `iopt = 2`. **`iwt = 1` on ACER card 9 is required**, not cosmetic:
//! `aceth.f90:674-676` sets `ifeng = 0` only for `iwt = 1` (`iwt = 0` gives the
//! skewed `ifeng = 1`, `iwt = 2` the continuous-tabular `ifeng = 2`), and this
//! port writes the equiprobable `IFENG = 0` form alone. Comparing against an
//! `IFENG = 1` reference would be comparing two different representations.
//!
//! **Ours is built on NJOY's own incident-energy grid**, read out of the
//! reference table's ITIE block, and with NJOY's own `NIEB`/`NIL` dimensions.
//! That is deliberate and is the same rule the CE comparator follows: this
//! port's thermal writer takes its grid from the caller, so choosing our own
//! would measure the grid choice rather than the physics. Every quantity below
//! is therefore compared at an energy both tables actually carry.
//!
//! # What is compared
//!
//! | block | quantity |
//! |---|---|
//! | NXS | `IDPNI`, `NIL`, `NIEB`, `IDPNC`, `NCL`, `IFENG` |
//! | ITIE/ITIX | the inelastic cross section on NJOY's own grid |
//! | ITCE/ITCX | coherent-elastic Bragg edge energies and cumulative `S` |
//!
//! ITXE (the equiprobable emission bins) is **not** differenced here; it is
//! reported as a length agreement only. Saying so is the point — see the
//! closing note this program prints.
//!
//! # Usage
//!
//! ```bash
//! cargo build --release -p njoy-outram-park-fork --example thermal_ace_vs_njoy2016
//! ./target/release/examples/thermal_ace_vs_njoy2016 <njoy.ace> \
//!     --tsl tsl-013_Al_027-ENDF8.0.endf --mat 53 --temp-k 293.6 --natom 1
//! ```

fn main() {
    use njoy_outram_park_fork::{
        acer::{
            thermal::{jxs, nxs, ThermalAceOptions},
            AceTable,
        },
        endf::tape::Tape,
        thermr::mf7::parse_mf7,
    };

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: thermal_ace_vs_njoy2016 <njoy.ace> --tsl <file> --mat <MAT> \
             --temp-k <K> [--natom <n>]"
        );
        std::process::exit(2);
    }
    let ace_path = args[1].clone();
    let flag = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let tsl = flag("--tsl").expect("--tsl <file> is required");
    let mat: i32 = flag("--mat").expect("--mat is required").parse().unwrap();
    let temp_k: f64 = flag("--temp-k").unwrap_or_else(|| "293.6".into()).parse().unwrap();
    let natom: f64 = flag("--natom").unwrap_or_else(|| "1".into()).parse().unwrap();

    // Inventory-only mode: `-` in place of the ACE path. Needed because the
    // NJOY deck cannot be written until the evaluation's own base temperature
    // is known, and asking for any other temperature silently compares a
    // different scattering law (Al-27's T0 is 20 K, not 293.6 K).
    if ace_path == "-" {
        let tape = Tape::read_file(
            &njoy_outram_park_fork::reference_data::reference_endf_dir().join(&tsl),
        )
        .unwrap_or_else(|e| panic!("read tsl {tsl}: {e:?}"));
        let mf7 = parse_mf7(&tape, mat).unwrap_or_else(|e| panic!("parse MF=7: {e:?}"));
        let (t0, lat, lasym, lln) = match mf7.incoherent_inelastic.as_ref() {
            Some(ii) => (ii.temperature_k, ii.lat, ii.lasym, ii.lln),
            None => (f64::NAN, -1, -1, -1),
        };
        let elastic = match (
            mf7.coherent_elastic.as_ref(),
            mf7.incoherent_elastic.as_ref(),
        ) {
            (Some(ce), None) => format!("coherent({})", ce.bragg_energies_ev.len()),
            (Some(ce), Some(_)) => format!("mixed({})", ce.bragg_energies_ev.len()),
            (None, Some(_)) => "incoherent".to_string(),
            (None, None) => "none".to_string(),
        };
        println!(
            "INVENTORY tsl={tsl} mat={mat} t0={t0} lat={lat} lasym={lasym} lln={lln} elastic={elastic}"
        );
        return;
    }

    // ---- NJOY's table -----------------------------------------------------
    let text = std::fs::read_to_string(&ace_path)
        .unwrap_or_else(|e| panic!("read {ace_path}: {e}"));
    let lines: Vec<&str> = text.lines().collect();
    let ints: Vec<i32> = lines[6..12]
        .iter()
        .flat_map(|l| l.split_whitespace())
        .map(|t| t.parse::<i32>().expect("nxs/jxs int"))
        .collect();
    let (t_nxs, t_jxs) = (&ints[..16], &ints[16..]);
    let t_xss: Vec<f64> = lines[12..]
        .iter()
        .flat_map(|l| l.split_whitespace())
        .map(|t| t.parse::<f64>().expect("xss real"))
        .collect();
    // JXS locators are 1-based into XSS.
    let at = |loc: i32| -> usize { (loc - 1) as usize };

    let ifeng = t_nxs[nxs::IFENG];
    let nieb = t_nxs[nxs::NIEB] as usize;
    let nil = t_nxs[nxs::NIL];
    let idpnc = t_nxs[nxs::IDPNC];

    println!("=== NJOY thermal table: {ace_path} ===");
    println!(
        "  IDPNI={} NIL={} NIEB={} IDPNC={} NCL={} IFENG={}",
        t_nxs[nxs::IDPNI], nil, nieb, idpnc, t_nxs[nxs::NCL], ifeng
    );
    if ifeng != 0 {
        println!(
            "\nREFUSING TO COMPARE: NJOY's table is IFENG={ifeng}; this port writes the\n\
             equiprobable IFENG=0 form only. Re-run ACER with iwt=1 (aceth.f90:674-676).\n\
             A number produced here would be comparing two different representations."
        );
        println!("SUMMARY tsl={tsl} mat={mat} ifeng={ifeng} verdict=NOT_COMPARABLE");
        return;
    }

    // NJOY's own inelastic incident-energy grid, and its cross section.
    let itie = at(t_jxs[jxs::ITIE]);
    let nei = t_xss[itie] as usize;
    let njoy_e: Vec<f64> = t_xss[itie + 1..itie + 1 + nei].to_vec(); // MeV
    let itix = at(t_jxs[jxs::ITIX]);
    let njoy_xs: Vec<f64> = t_xss[itix..itix + nei].to_vec();

    // ---- ours, on NJOY's grid --------------------------------------------
    let tape = Tape::read_file(&njoy_outram_park_fork::reference_data::reference_endf_dir().join(&tsl))
        .unwrap_or_else(|e| panic!("read tsl {tsl}: {e:?}"));
    let mf7 = parse_mf7(&tape, mat).unwrap_or_else(|e| panic!("parse MF=7: {e:?}"));

    // The comparison's own inputs, printed before any number is produced: a
    // temperature or a storage flag that does not match what NJOY processed
    // makes every difference below uninterpretable.
    println!("\n=== MF=7 inventory ({tsl}) ===");
    if let Some(ii) = mf7.incoherent_inelastic.as_ref() {
        println!(
            "  incoherent-inelastic: T0={} K  LAT={}  LASYM={}  LLN={}",
            ii.temperature_k, ii.lat, ii.lasym, ii.lln
        );
        if (ii.temperature_k - temp_k).abs() > 1.0 {
            println!(
                "  NOTE: requested {temp_k} K but the evaluation's base temperature is {} K",
                ii.temperature_k
            );
        }
        if ii.lln != 0 {
            println!("  WARNING: LLN != 0 — S is stored as ln S and is not undone by this port");
        }
    } else {
        println!("  incoherent-inelastic: ABSENT");
    }
    match (
        mf7.coherent_elastic.as_ref(),
        mf7.incoherent_elastic.as_ref(),
    ) {
        (Some(ce), _) => println!(
            "  coherent-elastic: {} Bragg edges, {} tabulated temperatures",
            ce.bragg_energies_ev.len(),
            ce.temperatures_k.len()
        ),
        (None, Some(_)) => println!("  incoherent-elastic only"),
        (None, None) => println!("  elastic: ABSENT"),
    }

    const EMEV: f64 = 1.0e6;
    let grid_ev: Vec<f64> = njoy_e.iter().map(|&e| e * EMEV).collect();
    let emax_ev = grid_ev.last().copied().unwrap_or(4.0);
    let opts = ThermalAceOptions {
        n_outgoing: nieb,
        n_cosines: (nil + 1) as usize,
        natom,
        emax_ev,
    };
    let ours = match AceTable::thermal_from_mf7(&mf7, temp_k, "x", 0, &grid_ev, opts) {
        Ok(t) => t,
        Err(e) => {
            println!("\nOUR BUILD FAILED: {e:?}");
            println!("SUMMARY tsl={tsl} mat={mat} verdict=BUILD_FAILED");
            return;
        }
    };

    let o_nxs = &ours.nxs;
    let o_jxs = &ours.jxs;
    let o_xss = &ours.xss;
    println!("\n=== NXS agreement ===");
    let mut nxs_ok = true;
    for (label, i) in [
        ("IDPNI", nxs::IDPNI),
        ("NIL", nxs::NIL),
        ("NIEB", nxs::NIEB),
        ("IDPNC", nxs::IDPNC),
        ("NCL", nxs::NCL),
        ("IFENG", nxs::IFENG),
    ] {
        let (a, b) = (o_nxs[i], t_nxs[i]);
        if a != b {
            nxs_ok = false;
        }
        println!("  {label:<6} ours {a:>6}  njoy {b:>6}  {}", if a == b { "ok" } else { "DIFFER" });
    }

    // ---- inelastic cross section, same grid, no interpolation ------------
    let o_itix = (o_jxs[jxs::ITIX] - 1) as usize;
    let mut worst_xs = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for k in 0..nei {
        let (a, b) = (o_xss[o_itix + k], njoy_xs[k]);
        if b != 0.0 {
            let d = ((a - b) / b).abs();
            if d > worst_xs.0 {
                worst_xs = (d, njoy_e[k], a, b);
            }
        }
    }
    println!("\n=== inelastic cross section (NJOY's own grid, {nei} points) ===");
    println!(
        "  worst rel {:.3e} at {:.6e} MeV  (ours {:.6e} njoy {:.6e})",
        worst_xs.0, worst_xs.1, worst_xs.2, worst_xs.3
    );
    // A single worst number cannot distinguish "wrong everywhere" from "wrong at
    // one endpoint", and those call for completely different investigations.
    let mut above = (0.0f64, 0.0f64);
    for k in 1..nei {
        let (a, b) = (o_xss[o_itix + k], njoy_xs[k]);
        if b != 0.0 {
            let d = ((a - b) / b).abs();
            if d > above.0 {
                above = (d, njoy_e[k]);
            }
        }
    }
    println!(
        "  worst rel excluding the first grid point: {:.3e} at {:.6e} MeV",
        above.0, above.1
    );
    println!("  sample (MeV, ours, njoy, rel):");
    for k in [0usize, 1, nei / 4, nei / 2, 3 * nei / 4, nei - 1] {
        let (a, b) = (o_xss[o_itix + k], njoy_xs[k]);
        let r = if b != 0.0 { ((a - b) / b).abs() } else { f64::NAN };
        println!("    {:.4e}  {:.6e}  {:.6e}  {:.2e}", njoy_e[k], a, b, r);
    }

    // ---- coherent elastic -------------------------------------------------
    let mut worst_be = 0.0f64;
    let mut worst_s = 0.0f64;
    let mut nee = 0usize;
    if idpnc == 4 && t_jxs[jxs::ITCE] != 0 && o_jxs[jxs::ITCE] != 0 {
        let t_itce = at(t_jxs[jxs::ITCE]);
        nee = t_xss[t_itce] as usize;
        let o_itce = (o_jxs[jxs::ITCE] - 1) as usize;
        let o_nee = o_xss[o_itce] as usize;
        if o_nee != nee {
            // Differencing by index across two different edge lists would be the
            // same defect the CE comparator was built to avoid. Match by ENERGY:
            // for each NJOY edge, find ours nearest in relative terms, and
            // compare the cumulative S there.
            println!("\n=== coherent elastic ===");
            println!("  NEE ours {o_nee} njoy {nee} — different edge lists, matching by energy");
            let t_itcx = at(t_jxs[jxs::ITCX]);
            let o_itcx = (o_jxs[jxs::ITCX] - 1) as usize;
            let ours_e: Vec<f64> = (0..o_nee).map(|k| o_xss[o_itce + 1 + k]).collect();
            let mut matched = 0usize;
            let mut worst_pair = (0.0f64, 0.0f64);
            let mut unmatched = 0usize;
            for k in 0..nee {
                let te = t_xss[t_itce + 1 + k];
                // nearest of ours in relative distance
                let mut best = (f64::INFINITY, 0usize);
                for (j, &oe) in ours_e.iter().enumerate() {
                    let d = if te != 0.0 { ((oe - te) / te).abs() } else { (oe - te).abs() };
                    if d < best.0 {
                        best = (d, j);
                    }
                }
                if best.0 > 1.0e-6 {
                    unmatched += 1;
                    continue;
                }
                matched += 1;
                let (a, b) = (o_xss[o_itcx + best.1], t_xss[t_itcx + k]);
                if b != 0.0 {
                    let d = ((a - b) / b).abs();
                    if d > worst_pair.0 {
                        worst_pair = (d, te);
                    }
                }
            }
            println!(
                "  {matched} of {nee} NJOY edges found in ours to 1e-6 relative; {unmatched} not found"
            );
            println!(
                "  cumulative S at matched edges: worst rel {:.3e} at {:.6e} MeV",
                worst_pair.0, worst_pair.1
            );
            worst_s = worst_pair.0;
            worst_be = if unmatched == 0 { 0.0 } else { f64::NAN };
        } else {
            let t_itcx = at(t_jxs[jxs::ITCX]);
            let o_itcx = (o_jxs[jxs::ITCX] - 1) as usize;
            for k in 0..nee {
                let (a, b) = (o_xss[o_itce + 1 + k], t_xss[t_itce + 1 + k]);
                if b != 0.0 {
                    worst_be = worst_be.max(((a - b) / b).abs());
                }
                let (a, b) = (o_xss[o_itcx + k], t_xss[t_itcx + k]);
                if b != 0.0 {
                    worst_s = worst_s.max(((a - b) / b).abs());
                }
            }
            println!("\n=== coherent elastic ({nee} Bragg edges) ===");
            println!("  edge energies worst rel {worst_be:.3e}");
            println!("  cumulative S  worst rel {worst_s:.3e}");
        }
    } else {
        println!("\n=== coherent elastic ===\n  not present on both sides (IDPNC={idpnc})");
    }

    println!(
        "\nNOT COMPARED HERE: the ITXE equiprobable emission bins. Their lengths agree by\n\
         construction once NIEB and NIL match, so a length check proves nothing about the\n\
         values, and this program does not claim otherwise."
    );
    println!(
        "SUMMARY tsl={tsl} mat={mat} temp={temp_k} nei={nei} nee={nee} ifeng={ifeng} \
         nxs={} xs={:.3e} bragg_e={:.3e} bragg_s={:.3e}",
        if nxs_ok { "ok" } else { "DIFFER" },
        worst_xs.0,
        worst_be,
        worst_s
    );
}
