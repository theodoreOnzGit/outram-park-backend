//! **V&V — the evaluated MF=6 LAW=1 continuum emission law read as a
//! [`ContinuumEmission`], checked against the ENDF tape's own records.**
//!
//! # Why this exists
//!
//! RECONR reconstructs MF=3 cross sections but no secondary-energy law, so
//! `outram-mc-libs` has been modelling the MT=91 continuum with a **Weisskopf
//! evaporation stand-in**. That is a shape assumption, not the evaluation's
//! data, and MT=91 carries 10–25 % of the collisions in a bare fast metal
//! sphere (measured on Godiva, 2026-09-13; `gh:#192`). Reading the evaluated
//! `f₀(E→E')` is the fix, and this is the gate on reading it correctly.
//!
//! # Methodology
//!
//! [`ContinuumEmission::from_endf_mf6`] reduces the MF=6 LAW=1 neutron
//! subsection to the same [`ChiTabular`] form the MF=5 LF=1 fission spectrum
//! uses, converting MeV → eV. Three invariants are asserted per incident-energy
//! table, on every table of every case, because each is destroyed by a
//! different parsing error:
//!
//! | invariant | what breaks it |
//! |---|---|
//! | `E'` strictly ascending from ≥ 0 | a wrong LIST stride — LAW=1 LANG=1 interleaves `NA` Legendre terms after each `f₀`, and **`NA` varies table to table** (U-238 MT=91 runs `NA = 0` at threshold and `NA = 26` at 30 MeV) |
//! | `∫f₀ dE' = 1` | reading the wrong subsection (MF=6/MT=91 carries NK=3: neutron, photon, recoil) or dropping points |
//! | `E'_max` = the CM kinematic bound `E·(A/(A+1))²` | any of the above, and a MeV↔eV slip |
//!
//! The `E'_max` check is the sharp one: the evaluation places its last outgoing
//! point exactly on the two-body bound, so a mis-strided table misses it by
//! orders of magnitude rather than by a tolerance.
//!
//! # Results (2026-09-13, ENDF/B-VIII.0 from `reference-data/endf/`)
//!
//! All three cases parse and satisfy every invariant:
//!
//! | case | MT | frame | branches | tables (1st) | incident range | total `y` | worst `|∫f₀−1|` |
//! |---|---|---|---|---|---|---|---|
//! | U-238 | 91 | CM | 1 | 96 | 4.356e5 – 3.000e7 eV | 1.000 | 9.99e-16 |
//! | U-238 | 16 | CM | 1 | 37 | 6.179e6 – 3.000e7 eV | 2.000 | 6.66e-16 |
//! | U-235 | 91 | CM | 1 | 64 | 4.356e5 – 3.000e7 eV | 1.000 | 8.88e-16 |
//! | U-235 | 16 | CM | 1 | 39 | 5.321e6 – 3.000e7 eV | 2.000 | 7.77e-16 |
//! | F-19 | 91 | **lab** | 1 | 13 | 5.937e6 – 2.000e7 eV | 1.000 | 4.44e-16 |
//! | F-19 | 16 | **lab** | **2** | 19 | 1.099e7 – 2.000e7 eV | 2.000 | 4.44e-16 |
//!
//! Two conventions show up in one library, and both had to be handled:
//!
//! - **Frame.** F-19 tabulates MT=91 in the **laboratory** frame while both
//!   actinides use the centre of mass. A consumer that assumes one is wrong on
//!   the other, which is why [`ContinuumEmission::cm_frame`] is carried rather
//!   than inferred.
//! - **How (n,2n) is written.** U-238 and U-235 give MT=16 as **one** neutron
//!   subsection of yield 2. F-19 gives it as **two** ZAP=1 subsections of yield
//!   1 each (NK=4), carrying different spectra for the first and second emitted
//!   neutron. Reading only the first subsection — which
//!   [`crate::acer::energy::parse_mf6_law1_neutron`] does, and which this test
//!   caught — emits **one** neutron where F-19's evaluation says two. That is a
//!   50 % multiplicity error on a fluorine (n,2n), i.e. on exactly the nuclide
//!   the FHR cases are built from. Hence
//!   [`ContinuumEmission::branches`] and
//!   [`ContinuumEmission::total_yield_at`].
//!
//! **Out-of-band check of the parse itself.** U-238 MT=91's last neutron table
//! (incident 30 MeV, `ND=0, NA=26, NW=5460, NEP=195`) was re-read straight from
//! the 66-column card images by an independent script that knows nothing of this
//! crate — stride `NA+2`, trapezoid over `(E', f₀)`. It gives
//! `E'_max = 2.97479e7 eV`, `∫f₀ dE' = 1.0000000841` (the evaluation's own
//! normalisation, not ours) and `⟨E'⟩ = 2.65300e7 eV`. The parser returns
//! `⟨E'⟩ = 2.6530e7 eV` and the same `E'_max` — agreement to every digit
//! printed. The recorded means below are pinned from that run.
//!
//! **What the evaluated law says, against the stand-in it replaces.** U-238
//! MT=91 `⟨E'⟩/E` is **0.1803 at 1 MeV, 0.2095 at 2.1 MeV, 0.2067 at 5 MeV**,
//! rising to 0.7396 at 14 MeV and 0.8843 at 30 MeV as the direct/pre-equilibrium
//! component takes over (`NA` climbing 0 → 26 is the same story in the angular
//! terms). The Weisskopf stand-in gives `⟨E'/E⟩ = 0.2787` at 2 MeV after
//! `gh:#192`'s cap — **33 % harder than the evaluation** at the energy where
//! MT=91 is opening. That difference is the reason for this work; it is priced
//! on the transport side, not here.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::nuclear_data::secondary::{ChiEout, ContinuumEmission};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

/// `(tape file, label, AWR, MT=91 is CM-framed, MT=91 table count, MT=16 branch count)`.
const CASES: &[(&str, &str, f64, bool, usize, usize)] = &[
    ("n-092_U_238.endf", "U-238", 236.0058, true, 96, 1),
    ("n-092_U_235.endf", "U-235", 233.0248, true, 64, 1),
    ("n-009_F_019-ENDF8.0.endf", "F-19", 18.8353, false, 13, 2),
];

/// Trapezoid (or histogram) integral of `f₀` over one table — must be 1.
fn area(t: &ChiEout) -> f64 {
    let mut a = 0.0;
    for i in 1..t.e_out.len() {
        let dx = t.e_out[i] - t.e_out[i - 1];
        a += if t.linlin {
            0.5 * (t.pdf[i] + t.pdf[i - 1]) * dx
        } else {
            t.pdf[i - 1] * dx
        };
    }
    a
}

/// The reaction `QI` \[eV\] from the tape's own MF=3/`mt` TAB1 header — the
/// energy the reaction must pay for, which sets the (n,2n) outgoing bound.
fn qi(tape: &Tape, mat: i32, mt: i32) -> f64 {
    use njoy_outram_park_fork::endf::records::SectionCursor;
    let sec = tape
        .section(mat, 3, mt)
        .unwrap_or_else(|| panic!("no MF=3/MT={mt}"));
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont().unwrap();
    cur.read_tab1().unwrap().head.c2
}

/// First moment `∫E'·f₀ dE'` of one table \[eV\].
fn mean(t: &ChiEout) -> f64 {
    let mut m = 0.0;
    for i in 1..t.e_out.len() {
        let (x0, x1) = (t.e_out[i - 1], t.e_out[i]);
        m += if t.linlin {
            (x1 - x0) * (x1 * t.pdf[i] + x0 * t.pdf[i - 1]) / 2.0
        } else {
            t.pdf[i - 1] * (x1 * x1 - x0 * x0) / 2.0
        };
    }
    m
}

#[test]
fn mf6_continuum_emission_satisfies_the_tape_s_own_invariants() {
    for &(file, label, awr, cm91, ne91, nb16) in CASES {
        let Some(path) = reference_endf_or_skip(file, "mf6-continuum") else {
            continue;
        };
        let tape = Tape::read_file(&path).unwrap();
        let mat = tape.materials()[0];

        for mt in [91, 16] {
            let c = ContinuumEmission::from_endf_mf6(&tape, mat, mt)
                .unwrap()
                .unwrap_or_else(|| panic!("{label}: MF=6/MT={mt} must parse"));

            if mt == 91 {
                assert_eq!(c.cm_frame, cm91, "{label} MT=91 frame (ENDF LCT)");
                assert_eq!(
                    c.branches[0].spectrum.incident.len(),
                    ne91,
                    "{label} MT=91 table count"
                );
            } else {
                assert_eq!(c.branches.len(), nb16, "{label} MT=16 neutron subsections");
            }
            assert!(!c.branches.is_empty(), "{label} MT={mt}: at least one branch");

            for (bi, b) in c.branches.iter().enumerate() {
                assert_eq!(
                    b.spectrum.incident.len(),
                    b.spectrum.tables.len(),
                    "{label} MT={mt} branch {bi}: one table per incident energy"
                );
                assert!(
                    b.spectrum.incident.windows(2).all(|w| w[1] > w[0]),
                    "{label} MT={mt} branch {bi}: incident grid must ascend strictly"
                );

                for (e_in, t) in b.spectrum.incident.iter().zip(&b.spectrum.tables) {
                    assert!(
                        t.e_out[0] >= 0.0 && t.e_out.windows(2).all(|w| w[1] > w[0]),
                        "{label} MT={mt} branch {bi} @ {e_in:e} eV: E' must ascend \
                         from >= 0 (a wrong LIST stride is what breaks this)"
                    );
                    assert!(
                        (area(t) - 1.0).abs() < 1.0e-6,
                        "{label} MT={mt} branch {bi} @ {e_in:e} eV: ∫f₀ dE' = {}, want 1",
                        area(t)
                    );
                    assert!(
                        t.cdf[0].abs() < 1.0e-12
                            && (t.cdf[t.cdf.len() - 1] - 1.0).abs() < 1.0e-9
                            && t.cdf.windows(2).all(|w| w[1] >= w[0]),
                        "{label} MT={mt} branch {bi} @ {e_in:e} eV: cdf must run 0 → 1 \
                         monotonically"
                    );
                    let m = mean(t);
                    // `>= 0`, not `> 0`: the threshold table of an actinide MT=91
                    // is a two-point spike at zero outgoing energy (U-238's first
                    // table is `E' = [0, 1e-5] eV`, `f₀ = [2e5, 0]`), whose first
                    // moment is exactly 0.0 in f64. That is the evaluation, not a
                    // parse error.
                    assert!(
                        m >= 0.0 && m <= *t.e_out.last().unwrap(),
                        "{label} MT={mt} branch {bi} @ {e_in:e} eV: ⟨E'⟩ = {m:e} \
                         outside [0, E'_max]"
                    );
                }

                // The sharp one: the evaluation puts its last outgoing point on
                // the two-body CM bound. Checked at the top table, where the
                // Legendre order is highest and a stride error is most likely.
                //
                // MT=91 tops out at the **elastic** bound `E·(A/(A+1))²` with
                // `f₀` already zero at the last few points — the continuum spans
                // the full kinematic range and its own Q enters through the
                // vanishing tail, not a truncated grid. MT=16 does not: its grid
                // stops at `E·(A/(A+1))² + Q·A/(A+1)` with the evaluation's own
                // `QI` (U-238 −6.1528 MeV), because the second neutron must be
                // paid for. Both are asserted, each against its own bound.
                if c.cm_frame {
                    let last = b.spectrum.incident.len() - 1;
                    let e_in = b.spectrum.incident[last];
                    let elastic = e_in * (awr / (awr + 1.0)).powi(2);
                    let bound = if mt == 16 {
                        elastic + qi(&tape, mat, mt) * awr / (awr + 1.0)
                    } else {
                        elastic
                    };
                    let got = *b.spectrum.tables[last].e_out.last().unwrap();
                    assert!(
                        got <= elastic * (1.0 + 1.0e-4),
                        "{label} MT={mt} branch {bi} @ {e_in:e} eV: E'_max = {got:e} \
                         exceeds the elastic CM bound {elastic:e}"
                    );
                    assert!(
                        (got - bound).abs() / bound < 1.0e-4,
                        "{label} MT={mt} branch {bi} @ {e_in:e} eV: E'_max = {got:e}, \
                         CM kinematic bound = {bound:e}"
                    );
                }
            }

            // Total multiplicity: 1 neutron out of MT=91, 2 out of MT=16 —
            // whichever way the evaluation splits it across subsections.
            let want_y = if mt == 16 { 2.0 } else { 1.0 };
            let probe = *c.branches[0].spectrum.incident.last().unwrap();
            let got_y = c.total_yield_at(probe);
            assert!(
                (got_y - want_y).abs() < 1.0e-9,
                "{label} MT={mt}: total yield at {probe:e} eV = {got_y}, want {want_y}"
            );
        }
    }
}

#[test]
fn mf6_continuum_mean_outgoing_energy_matches_the_recorded_values() {
    // (file, label, mt, incident energy [eV], ⟨E'⟩ [eV]).
    const GOLD: &[(&str, &str, i32, f64, f64)] = &[
        ("n-092_U_238.endf", "U-238", 91, 1.0e6, 1.803e5),
        ("n-092_U_238.endf", "U-238", 91, 5.0e6, 1.0336e6),
        ("n-092_U_238.endf", "U-238", 91, 3.0e7, 2.65300e7),
        ("n-092_U_238.endf", "U-238", 16, 1.4e7, 1.8996e6),
        ("n-092_U_235.endf", "U-235", 91, 2.0e6, 6.339e5),
        ("n-092_U_235.endf", "U-235", 16, 1.4e7, 1.9810e6),
        ("n-009_F_019-ENDF8.0.endf", "F-19", 91, 1.4e7, 7.2530e6),
        ("n-009_F_019-ENDF8.0.endf", "F-19", 16, 1.4e7, 1.2325e6),
    ];

    for &(file, label, mt, e_in, want) in GOLD {
        let Some(path) = reference_endf_or_skip(file, "mf6-continuum-mean") else {
            continue;
        };
        let tape = Tape::read_file(&path).unwrap();
        let mat = tape.materials()[0];
        let c = ContinuumEmission::from_endf_mf6(&tape, mat, mt)
            .unwrap()
            .unwrap();
        // The golden energies are tabulated points of the evaluation, so this
        // reads the table itself rather than interpolating between two.
        let b = &c.branches[0];
        let i = b
            .spectrum
            .incident
            .iter()
            .position(|&x| (x - e_in).abs() / e_in < 1.0e-9)
            .unwrap_or_else(|| panic!("{label} MT={mt}: no table at {e_in:e} eV"));
        let got = mean(&b.spectrum.tables[i]);
        let rel = (got - want).abs() / want;
        assert!(
            rel < 5.0e-4,
            "{label} MT={mt} @ {e_in:e} eV: ⟨E'⟩ = {got:e} eV, recorded {want:e} eV \
             (rel {rel:.2e})"
        );
    }
}
