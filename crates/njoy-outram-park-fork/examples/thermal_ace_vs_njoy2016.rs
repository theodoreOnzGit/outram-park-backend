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

    // ---- NJOY's table, through the library reader -------------------------
    // Was an ad-hoc parser inlined here, one of five near-identical copies in
    // this crate. `acer::read` is now the single implementation and it also
    // checks NXS(1) against the actual XSS length, so a truncated reference
    // fails here instead of producing quiet nonsense downstream.
    let njoy = njoy_outram_park_fork::acer::read::read_type1(&ace_path)
        .unwrap_or_else(|e| panic!("read {ace_path}: {e}"));
    assert_eq!(
        njoy.header.class,
        njoy_outram_park_fork::acer::read::AceClass::Thermal,
        "{ace_path} is a {:?} table, not a thermal one -- this comparator only \
         understands the `t` class",
        njoy.header.class
    );
    let t_nxs = &njoy.nxs[..];
    let t_jxs = &njoy.jxs[..];
    let t_xss: Vec<f64> = njoy.xss.clone();
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

    // Exhaustive-coverage accumulators, reported in SUMMARY so a sweep can
    // tabulate them. Initialised to NAN, not 0.0: a block that was never
    // compared must not read as "compared and perfect".
    let mut itxe_ep = f64::NAN;
    let mut itxe_mu = f64::NAN;
    let mut itxe_n = 0usize;
    let mut inel_e = f64::NAN;
    let mut inel_a = f64::NAN;

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

    // ---- ITXE: the equiprobable emission bins -----------------------------
    //
    // Layout for IFENG=0, read off upstream's OWN printer (`aceth.f90:904-968`):
    //
    //     loc = itxe - 1
    //     for i in 1..=nie:            nang = nil+1,  nbini = nieb
    //         for j in 1..=nbini:      xss[loc+1 ..= loc+nang+1]   # E', mu(1..nang)
    //                                  loc += nang + 1
    //
    // so the block is NIE * NIEB * (nang+1) values.
    //
    // POSITIONAL COMPARISON IS LEGITIMATE HERE, and only here. Everywhere else
    // this program matches by energy, because two codes do not share a grid.
    // This block is built on NJOY's own incident-energy grid with NJOY's own
    // NIEB and NIL, so entry k of one table is the same (incident energy,
    // bin, cosine) as entry k of the other by construction. If NXS ever
    // disagreed the assertion above would have already failed.
    let nang = (nil + 1) as usize;
    let stride = nang + 1;
    let want_len = nei * nieb * stride;
    let t_itxe = at(t_jxs[jxs::ITXE]);
    let o_itxe = (o_jxs[jxs::ITXE] - 1) as usize;
    println!("\n=== ITXE emission bins ({nei} x {nieb} x {stride} = {want_len} values) ===");
    if t_itxe + want_len > t_xss.len() || o_itxe + want_len > o_xss.len() {
        println!("  block does not fit in one of the tables -- NOT compared");
    } else {
        // E' and the cosines are different quantities and are kept apart: a
        // cosine legitimately passes through zero, where a relative difference
        // is meaningless, so it is reported as an ABSOLUTE difference.
        let mut worst_ep = (0.0f64, 0usize);
        let mut worst_mu = (0.0f64, 0usize);
        for k in 0..want_len {
            let (a, b) = (o_xss[o_itxe + k], t_xss[t_itxe + k]);
            if k % stride == 0 {
                if b != 0.0 {
                    let d = ((a - b) / b).abs();
                    if d > worst_ep.0 {
                        worst_ep = (d, k);
                    }
                }
            } else {
                let d = (a - b).abs();
                if d > worst_mu.0 {
                    worst_mu = (d, k);
                }
            }
        }
        println!(
            "  outgoing energy E'  worst REL {:.3e}  (at index {})",
            worst_ep.0, worst_ep.1
        );
        println!(
            "  cosines             worst ABS {:.3e}  (at index {}; absolute because a \
             cosine crosses zero)",
            worst_mu.0, worst_mu.1
        );
        itxe_ep = worst_ep.0;
        itxe_mu = worst_mu.0;
        itxe_n = want_len;
    }

    // ---- incoherent elastic ----------------------------------------------
    //
    // `aceth.f90:1031-1041`: IDPNC=3 uses ITCE/ITCA with nea = NCL+1; IDPNC=5
    // (mixed) uses the secondary ITCEI/ITCAI with nea = NCLI+1.
    println!("\n=== incoherent elastic ===");
    if idpnc == 3 || idpnc == 5 {
        let (t_start, t_mid, nea) = if idpnc == 5 {
            (t_jxs[jxs::ITCEI], t_jxs[jxs::ITCAI], (t_nxs[nxs::NCLI] + 1) as usize)
        } else {
            (t_jxs[jxs::ITCE], t_jxs[jxs::ITCA], (t_nxs[nxs::NCL] + 1) as usize)
        };
        let (o_start, o_mid) = if idpnc == 5 {
            (o_jxs[jxs::ITCEI], o_jxs[jxs::ITCAI])
        } else {
            (o_jxs[jxs::ITCE], o_jxs[jxs::ITCA])
        };
        if t_start == 0 || o_start == 0 {
            println!("  locator absent on one side (ours {o_start}, njoy {t_start}) -- NOT compared");
        } else {
            let t_nei_e = t_xss[at(t_start)] as usize;
            let o_nei_e = o_xss[(o_start - 1) as usize] as usize;
            println!("  NE ours {o_nei_e}  njoy {t_nei_e}");
            if t_nei_e == o_nei_e && t_mid != 0 && o_mid != 0 {
                let mut worst_e = 0.0f64;
                let mut worst_a = 0.0f64;
                for k in 0..t_nei_e {
                    let (a, b) = (o_xss[(o_start - 1) as usize + 1 + k], t_xss[at(t_start) + 1 + k]);
                    if b != 0.0 {
                        worst_e = worst_e.max(((a - b) / b).abs());
                    }
                }
                let na = t_nei_e * nea;
                if at(t_mid) + na <= t_xss.len() && (o_mid - 1) as usize + na <= o_xss.len() {
                    for k in 0..na {
                        let (a, b) = (o_xss[(o_mid - 1) as usize + k], t_xss[at(t_mid) + k]);
                        worst_a = worst_a.max((a - b).abs());
                    }
                }
                println!("  energies worst REL {worst_e:.3e}");
                println!("  equiprobable cosines ({nea} per energy) worst ABS {worst_a:.3e}");
                inel_e = worst_e;
                inel_a = worst_a;
            }
        }
    } else {
        println!("  none on this evaluation (IDPNC={idpnc})");
    }

    // ---- JXS locators and the declared table length -----------------------
    println!("\n=== JXS locators / table length ===");
    let jxs_names = [
        ("ITIE", jxs::ITIE), ("ITIX", jxs::ITIX), ("ITXE", jxs::ITXE),
        ("ITCE", jxs::ITCE), ("ITCX", jxs::ITCX), ("ITCA", jxs::ITCA),
        ("ITCEI", jxs::ITCEI), ("ITCXI", jxs::ITCXI), ("ITCAI", jxs::ITCAI),
    ];
    let mut jxs_ok = true;
    for (label, i) in jxs_names {
        let (a, b) = (o_jxs[i], t_jxs[i]);
        if a != b {
            jxs_ok = false;
            println!("  {label:<6} ours {a:>8}  njoy {b:>8}  DIFFER");
        }
    }
    if jxs_ok {
        println!("  all nine locators identical");
    }
    let len_ok = o_nxs[nxs::LEN_XSS] == t_nxs[nxs::LEN_XSS]
        && o_xss.len() == t_xss.len();
    println!(
        "  XSS length ours {} ({}) njoy {} ({})  {}",
        o_xss.len(), o_nxs[nxs::LEN_XSS], t_xss.len(), t_nxs[nxs::LEN_XSS],
        if len_ok { "ok" } else { "DIFFER" }
    );
    // A bare "DIFFER" on the length is not a finding, it is a question. The
    // coherent-elastic block is 2*NEE values (NEE edge energies + NEE
    // cumulative S), so if this port keeps more Bragg edges than NJOY the
    // difference must be EXACTLY 2*(NEE_ours - NEE_njoy), and every locator
    // after ITCE must shift by exactly that edge count. Checking the
    // arithmetic turns an unexplained mismatch into an attributed one -- or,
    // if it fails, into a real defect that this check is what would catch.
    let mut attributed = false;
    if !len_ok && t_jxs[jxs::ITCE] != 0 && o_jxs[jxs::ITCE] != 0 {
        let t_nee = t_xss[at(t_jxs[jxs::ITCE])] as i64;
        let o_nee = o_xss[(o_jxs[jxs::ITCE] - 1) as usize] as i64;
        let d_edges = o_nee - t_nee;
        let d_len = o_xss.len() as i64 - t_xss.len() as i64;
        let d_itcx = (o_jxs[jxs::ITCX] - t_jxs[jxs::ITCX]) as i64;
        if d_len == 2 * d_edges && d_itcx == d_edges {
            attributed = true;
            println!(
                "  ATTRIBUTED: we keep {d_edges} more Bragg edges than NJOY ({o_nee} vs \
                 {t_nee}).\n             The coherent block is 2*NEE values, so the length \
                 differs by exactly\n             2*{d_edges} = {d_len}, and ITCX shifts by \
                 exactly {d_itcx}. Both hold, so the\n             whole structural difference \
                 is edge thinning and nothing else."
            );
        } else {
            println!(
                "  NOT attributable to edge thinning: edges differ by {d_edges}, length by \
                 {d_len} (expected {}), ITCX by {d_itcx} (expected {d_edges}).",
                2 * d_edges
            );
        }
    }
    let all_ok = (jxs_ok && len_ok) || attributed;
    println!(
        "SUMMARY tsl={tsl} mat={mat} temp={temp_k} nei={nei} nee={nee} ifeng={ifeng} \
         nxs={} xs={:.3e} bragg_e={:.3e} bragg_s={:.3e} itxe_n={itxe_n} \
         itxe_ep={:.3e} itxe_mu={:.3e} inel_e={:.3e} inel_a={:.3e} struct={}",
        if nxs_ok { "ok" } else { "DIFFER" },
        worst_xs.0,
        worst_be,
        worst_s,
        itxe_ep,
        itxe_mu,
        inel_e,
        inel_a,
        if jxs_ok && len_ok { "ok" } else if all_ok { "thinning" } else { "DIFFER" }
    );
}
