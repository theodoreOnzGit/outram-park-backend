// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! LEAPR **card-deck reader** — parse a `.leapr` text deck into [`LeaprDeck`].
//!
//! [`crate::leapr::input::LeaprInput`] models one LEAPR job *by value*, which is
//! fine when a caller builds the job programmatically. Real evaluations, though,
//! ship the generator input as a free-format card deck: ENDF/B-VIII.0's thermal
//! sublibrary distributes a `tsl-*.leapr` file next to every `tsl-*.endf` tape.
//! This module reads that text so a stored 12 KB deck can regenerate the ~9 MB
//! MF=7 tape, at whatever temperatures are wanted.
//!
//! It replaces NJOY's `read(nsysi,*)` sequence (`leapr.f90:216-372` plus the
//! card-20 comment loop in `endout`, `leapr.f90:3096-3110`), not the physics.
//!
//! ## Fortran list-directed semantics this reproduces
//!
//! The reader is a *record* (line) cursor, not a flat token stream, because that
//! is what `read(nsysi,*)` is:
//!
//! - **Each card starts on a fresh record.** Whatever is left unconsumed on the
//!   previous line is discarded. This matters: card 13 of the ENDF/B-VIII.0
//!   graphite deck reads `0. 0. 1. 0./` while `read(nsysi,*) twt,c,tbeta` wants
//!   only three values, so the fourth `0.` is silently dropped — exactly as
//!   NJOY drops it.
//! - **A list read spans as many records as it needs.** The 150 alpha and 400
//!   beta values of the graphite deck are written 5-per-line with no `/` at all;
//!   the read simply continues onto the next line until it has its count.
//! - **`/` terminates a record early**, leaving every remaining item of the
//!   read list at the value it had before the read. LEAPR presets defaults
//!   before each such read, so an absent item means "default" — modelled here
//!   by [`CardCursor::read_reals`] returning `None` for the untouched slots.
//! - **`r*v` repeat counts and comma separators** are accepted, as list-directed
//!   input allows, although the ENDF/B-VIII.0 decks use neither.
//!
//! ## What is deliberately *not* interpreted
//!
//! The reader records these fields but nothing downstream consumes them yet, so
//! they are carried verbatim rather than acted on: `nout` (the output unit),
//! `iprint`, `smin`, `isabt`/`ilog` (the ENDF writer's storage options), the
//! secondary-scatterer block (`nss`, `b7`, `aws`, `sps`, `mss`), and the
//! pair-correlation cards 17-19 ([`PairCorrelation`]). Parsing them is not the
//! same as supporting them — see [`LeaprDeck::unsupported_features`].
//!
//! ## Units
//! - `alpha`, `beta` — dimensionless ENDF thermal variables.
//! - `temperature_k` — K. A **negative** card-10 temperature is NJOY's
//!   "reuse the previous cards 11-18" marker; [`LeaprDeck::parse`] resolves the
//!   inheritance and stores the absolute value, so every entry of
//!   [`LeaprDeck::temperatures`] is self-contained.
//! - `delta_ev`, `energy_ev` — eV; `dka` — inverse angstroms.

use crate::leapr::input::{ColdOption, ContinuousDist, DiscreteOscillator, ElasticOption, LeaprInput};
use crate::NjoyError;

/// Pair-correlation input (cards 17-19), read when `nsk > 0` or `ncold > 0`.
///
/// Carried through the parse for completeness; the Sköld correction that
/// consumes it (`skold`, leapr.f90:2816-2922) is **not** ported.
#[derive(Debug, Clone, PartialEq)]
pub struct PairCorrelation {
    /// Kappa increment `dka` \[inverse angstroms\]; the tabulated kappa values
    /// are `dka, 2 dka, ... nka dka`.
    pub dka: f64,
    /// The `S(kappa)` values (dimensionless), one per kappa point.
    pub skappa: Vec<f64>,
    /// Coherent fraction `cfrac` (dimensionless), card 19; only read when
    /// `nsk > 0`.
    pub cfrac: f64,
}

/// One resolved temperature block: cards 10-19 for a single temperature.
///
/// "Resolved" means the NJOY negative-temperature inheritance has already been
/// applied — a block whose card 10 was negative carries a *copy* of the
/// preceding block's spectrum and oscillators, so no consumer has to track
/// deck order.
#[derive(Debug, Clone, PartialEq)]
pub struct LeaprTemperature {
    /// Temperature \[K\], always positive (the card-10 sign is consumed by
    /// [`inherited`](Self::inherited)).
    pub temperature_k: f64,
    /// True when card 10 was negative, i.e. cards 11-19 were inherited from the
    /// previous temperature rather than read again.
    pub inherited: bool,
    /// Continuous frequency distribution and translational parameters
    /// (cards 11-13).
    pub continuous: ContinuousDist,
    /// Discrete oscillators (cards 14-16); may be empty.
    pub oscillators: Vec<DiscreteOscillator>,
    /// Pair-correlation data (cards 17-19), present only when `nsk > 0` or
    /// `ncold > 0`.
    pub pair_correlation: Option<PairCorrelation>,
}

/// A parsed LEAPR card deck: every card of a `tsl-*.leapr` job.
///
/// Field names follow the NJOY card variables so a reader can diff this against
/// `leapr.f90:122-372` line by line.
#[derive(Debug, Clone, PartialEq)]
pub struct LeaprDeck {
    /// Card 1 `nout` — the ENDF output unit. Recorded, not acted on.
    pub nout: i32,
    /// Card 2 — the job title.
    pub title: String,
    /// Card 3 `iprint` — print control (0/1/2). Recorded, not acted on.
    pub iprint: i32,
    /// Card 3 `nphon` — phonon-expansion order.
    pub nphon: usize,
    /// Card 4 `mat` — the ENDF MAT number to write.
    pub mat: i32,
    /// Card 4 `za` — `1000*Z + A` for the principal scatterer.
    pub za: f64,
    /// Card 4 `isabt` — 0 symmetric S, 1 asymmetric. Recorded, not acted on.
    pub isabt: i32,
    /// Card 4 `ilog` — 0 stores S, 1 stores `log10(S)`. Recorded, not acted on.
    pub ilog: i32,
    /// Card 4 `smin` — minimum S retained in the file (dimensionless).
    pub smin: f64,
    /// Card 5 `awr` — principal-scatterer mass ratio to the neutron.
    pub awr: f64,
    /// Card 5 `spr` — principal-scatterer free-atom cross section \[barn\].
    pub spr: f64,
    /// Card 5 `npr` — number of principal scattering atoms in the compound.
    pub npr: i32,
    /// Card 5 `iel` — coherent-elastic lattice option.
    pub iel: ElasticOption,
    /// Card 5 `ncold` — cold-moderator (Young-Koppel) option.
    pub ncold: ColdOption,
    /// Card 5 `nsk` — S(kappa) option: 0 none, 1 Vineyard, 2 Sköld.
    pub nsk: i32,
    /// Card 6 `nss` — number of secondary scatterers (0 or 1).
    pub nss: i32,
    /// Card 6 `b7` — secondary-scatterer type (0 sct, 1 free, 2 diffusion).
    pub b7: f64,
    /// Card 6 `aws` — secondary-scatterer mass ratio to the neutron.
    pub aws: f64,
    /// Card 6 `sps` — secondary-scatterer free-atom cross section \[barn\].
    pub sps: f64,
    /// Card 6 `mss` — number of secondary atoms in the compound.
    pub mss: i32,
    /// Card 7 `lat` — true when alpha/beta are scaled by `0.0253/tev`.
    pub lat: bool,
    /// Card 8 — the alpha grid (dimensionless, increasing).
    pub alpha: Vec<f64>,
    /// Card 9 — the beta grid (dimensionless, increasing).
    pub beta: Vec<f64>,
    /// Cards 10-19, one resolved block per temperature, in deck order.
    pub temperatures: Vec<LeaprTemperature>,
    /// Card 20 — the MF=1/451 descriptive comment lines, quotes stripped.
    pub comments: Vec<String>,
}

impl LeaprDeck {
    /// Parse a LEAPR card deck.
    ///
    /// Accepts the file exactly as ENDF/B-VIII.0 distributes it, including the
    /// leading `leapr` module-name line and the trailing `stop`; both are
    /// optional. Parsing stops at `stop` or at end of input.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if the deck ends before a required card, if a
    /// mandatory value is missing where NJOY presets no default (`mat`, `za`,
    /// `awr`, `spr`, `npr`, `nss`, `nalpha`, `nbeta`, the temperature, `delta`,
    /// `ni`, `twt`, `c`, `tbeta`, `nd`), if `nalpha`/`nbeta`/`ni`/`nd` are not
    /// positive where required, or if `iel`/`ncold` carry a code outside the set
    /// NJOY accepts.
    pub fn parse(text: &str) -> Result<Self, NjoyError> {
        let mut cur = CardCursor::new(text);

        // --- card 1: nout (the module-name line, if present, is skipped by the
        //     cursor constructor) ---
        let nout = cur.read_reals(1)?[0].ok_or_else(|| card_err("card 1: nout"))? as i32;

        // --- card 2: title ---
        let title = cur.read_text()?;

        // --- card 3: ntempr, iprint, nphon (defaults 1, 1, 100) ---
        let c3 = cur.read_reals(3)?;
        let ntempr = c3[0].unwrap_or(1.0) as usize;
        let iprint = c3[1].unwrap_or(1.0) as i32;
        let nphon = c3[2].unwrap_or(100.0).max(1.0) as usize;
        if ntempr == 0 {
            return Err(card_err("card 3: ntempr must be >= 1"));
        }

        // --- card 4: mat, za, isabt, ilog, smin (defaults 0, 0, 1e-75) ---
        let c4 = cur.read_reals(5)?;
        let mat = c4[0].ok_or_else(|| card_err("card 4: mat"))? as i32;
        let za = c4[1].ok_or_else(|| card_err("card 4: za"))?;
        let isabt = c4[2].unwrap_or(0.0) as i32;
        let ilog = c4[3].unwrap_or(0.0) as i32;
        let smin = c4[4].unwrap_or(1.0e-75);

        // --- card 5: awr, spr, npr, iel, ncold, nsk (defaults 0, 0, 0) ---
        let c5 = cur.read_reals(6)?;
        let awr = c5[0].ok_or_else(|| card_err("card 5: awr"))?;
        let spr = c5[1].ok_or_else(|| card_err("card 5: spr"))?;
        let npr = c5[2].ok_or_else(|| card_err("card 5: npr"))? as i32;
        let iel_code = c5[3].unwrap_or(0.0) as i32;
        let ncold_code = c5[4].unwrap_or(0.0) as i32;
        let nsk = c5[5].unwrap_or(0.0) as i32;
        let iel = ElasticOption::from_code(iel_code)
            .ok_or_else(|| card_err("card 5: iel outside 0..=6"))?;
        let ncold = ColdOption::from_code(ncold_code)
            .ok_or_else(|| card_err("card 5: ncold outside 0..=4"))?;

        // --- card 6: nss, b7, aws, sps, mss (defaults 0, 0, 0, 0) ---
        let c6 = cur.read_reals(5)?;
        let nss = c6[0].ok_or_else(|| card_err("card 6: nss"))? as i32;
        let b7 = c6[1].unwrap_or(0.0);
        let aws = c6[2].unwrap_or(0.0);
        let sps = c6[3].unwrap_or(0.0);
        let mss = c6[4].unwrap_or(0.0) as i32;

        // --- card 7: nalpha, nbeta, lat (lat default 0) ---
        let c7 = cur.read_reals(3)?;
        let nalpha = positive_count(c7[0], "card 7: nalpha")?;
        let nbeta = positive_count(c7[1], "card 7: nbeta")?;
        let lat = c7[2].unwrap_or(0.0) as i32 == 1;

        // --- cards 8, 9: the alpha and beta grids ---
        let alpha = cur.read_reals_required(nalpha, "card 8 (alpha grid)")?;
        let beta = cur.read_reals_required(nbeta, "card 9 (beta grid)")?;

        // --- cards 10-19, once per temperature ---
        let mut temperatures: Vec<LeaprTemperature> = Vec::with_capacity(ntempr);
        for itemp in 0..ntempr {
            let temp = cur.read_reals(1)?[0].ok_or_else(|| card_err("card 10: temperature"))?;
            // A negative temperature reuses the previous block's cards 11-19.
            // NJOY also reads the block for itemp == 1 regardless of sign.
            let reuse = temp < 0.0 && itemp > 0;
            if reuse {
                let prev = temperatures[itemp - 1].clone();
                temperatures.push(LeaprTemperature {
                    temperature_k: temp.abs(),
                    inherited: true,
                    ..prev
                });
                continue;
            }

            // --- card 11: delta, ni ---
            let c11 = cur.read_reals(2)?;
            let delta_ev = c11[0].ok_or_else(|| card_err("card 11: delta"))?;
            let ni = positive_count(c11[1], "card 11: ni")?;
            if ni < 2 {
                return Err(card_err("card 11: ni must be >= 2"));
            }

            // --- card 12: rho(E) ---
            let rho = cur.read_reals_required(ni, "card 12 (rho)")?;

            // --- card 13: twt, c, tbeta ---
            let c13 = cur.read_reals(3)?;
            let twt = c13[0].ok_or_else(|| card_err("card 13: twt"))?;
            let c = c13[1].ok_or_else(|| card_err("card 13: c"))?;
            let tbeta = c13[2].ok_or_else(|| card_err("card 13: tbeta"))?;

            // --- cards 14-16: discrete oscillators ---
            let nd = cur.read_reals(1)?[0].ok_or_else(|| card_err("card 14: nd"))? as i32;
            if nd < 0 {
                return Err(card_err("card 14: nd must be >= 0"));
            }
            let nd = nd as usize;
            let mut oscillators = Vec::with_capacity(nd);
            if nd > 0 {
                let bdel = cur.read_reals_required(nd, "card 15 (oscillator energies)")?;
                let adel = cur.read_reals_required(nd, "card 16 (oscillator weights)")?;
                for (energy_ev, weight) in bdel.into_iter().zip(adel) {
                    oscillators.push(DiscreteOscillator { energy_ev, weight });
                }
            }

            // --- cards 17-19: pair correlation ---
            let pair_correlation = if nsk > 0 || ncold != ColdOption::None {
                let c17 = cur.read_reals(2)?;
                let nka = positive_count(c17[0], "card 17: nka")?;
                let dka = c17[1].ok_or_else(|| card_err("card 17: dka"))?;
                let skappa = cur.read_reals_required(nka, "card 18 (s(kappa))")?;
                let cfrac = if nsk > 0 {
                    cur.read_reals(1)?[0].ok_or_else(|| card_err("card 19: cfrac"))?
                } else {
                    0.0
                };
                Some(PairCorrelation { dka, skappa, cfrac })
            } else {
                None
            };

            temperatures.push(LeaprTemperature {
                temperature_k: temp.abs(),
                inherited: false,
                continuous: ContinuousDist {
                    delta_ev,
                    rho,
                    twt,
                    c,
                    tbeta,
                },
                oscillators,
                pair_correlation,
            });
        }

        // --- card 20: comments, until a record that supplies no text ---
        let mut comments = Vec::new();
        while let Some(line) = cur.read_comment()? {
            comments.push(line);
        }

        Ok(LeaprDeck {
            nout,
            title,
            iprint,
            nphon,
            mat,
            za,
            isabt,
            ilog,
            smin,
            awr,
            spr,
            npr,
            iel,
            ncold,
            nsk,
            nss,
            b7,
            aws,
            sps,
            mss,
            lat,
            alpha,
            beta,
            temperatures,
            comments,
        })
    }

    /// The number of temperature blocks (NJOY `ntempr`).
    pub fn ntempr(&self) -> usize {
        self.temperatures.len()
    }

    /// Every temperature \[K\] the deck generates, in deck order.
    pub fn temperatures_k(&self) -> Vec<f64> {
        self.temperatures.iter().map(|t| t.temperature_k).collect()
    }

    /// Build the [`LeaprInput`] for temperature block `index`, ready to hand to
    /// [`crate::leapr::continuous::phonon_expansion`].
    ///
    /// `arat` is set to 1 (the principal scatterer). LEAPR's secondary-scatterer
    /// pass re-runs the same grids with `arat = aws/awr`; that pass is not
    /// driven from here.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if `index` is past the end of
    /// [`temperatures`](Self::temperatures).
    pub fn input_at(&self, index: usize) -> Result<LeaprInput, NjoyError> {
        let t = self
            .temperatures
            .get(index)
            .ok_or_else(|| card_err("temperature index out of range"))?;
        Ok(LeaprInput {
            alpha: self.alpha.clone(),
            beta: self.beta.clone(),
            lat: self.lat,
            arat: 1.0,
            nphon: self.nphon,
            temperature_k: t.temperature_k,
            continuous: t.continuous.clone(),
            oscillators: t.oscillators.clone(),
        })
    }

    /// Build a [`LeaprInput`] for an **arbitrary** temperature \[K\], reusing
    /// the frequency spectrum and oscillators of temperature block `index`.
    ///
    /// This is the point of keeping the deck: the phonon spectrum rho(E) is a
    /// property of the lattice, not of the temperature, so S(alpha, beta) can be
    /// generated at a temperature the shipped tape never tabulated (e.g. an
    /// HTR-10 operating point) instead of interpolating between tabulated ones.
    ///
    /// The caller owns the physics judgement: rho(E) from an ab-initio lattice
    /// calculation at one temperature is only approximately valid at another
    /// (thermal expansion and anharmonicity are not modelled), and the deck's
    /// alpha/beta grids were chosen for its own temperature list. Nothing here
    /// is validated outside the deck's tabulated temperatures.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if `index` is out of range or
    /// `temperature_k <= 0`.
    pub fn input_at_temperature(
        &self,
        index: usize,
        temperature_k: f64,
    ) -> Result<LeaprInput, NjoyError> {
        if temperature_k <= 0.0 || !temperature_k.is_finite() {
            return Err(card_err("temperature must be positive and finite"));
        }
        let mut input = self.input_at(index)?;
        input.temperature_k = temperature_k;
        Ok(input)
    }

    /// Deck features that parse cleanly but that the ported physics does **not**
    /// implement, as human-readable strings; empty when the deck is fully
    /// supported.
    ///
    /// Call this before trusting a regenerated S(alpha, beta): the parser is
    /// deliberately more permissive than the kernels, so "it parsed" is not
    /// "it will be computed correctly".
    pub fn unsupported_features(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.ncold != ColdOption::None {
            out.push(format!(
                "ncold = {} (cold H2/D2 Young-Koppel orchestrator is ported but not reference-validated)",
                self.ncold.code()
            ));
        }
        if self.nsk != 0 {
            out.push(format!(
                "nsk = {} (Vineyard/Skold pair-correlation correction is not ported)",
                self.nsk
            ));
        }
        if self.nss != 0 {
            out.push(format!(
                "nss = {} (secondary-scatterer mixed-moderator merge is not ported)",
                self.nss
            ));
        }
        if self.isabt != 0 {
            out.push("isabt = 1 (asymmetric S output) is not implemented".to_string());
        }
        if self.ilog != 0 {
            out.push("ilog = 1 (log10 S output) is not implemented".to_string());
        }
        out
    }
}

/// A card-level parse error with a fixed message prefix.
fn card_err(msg: &str) -> NjoyError {
    NjoyError::EndfParse(format!("leapr deck: {msg}"))
}

/// Require a value that must be a positive count.
fn positive_count(v: Option<f64>, what: &str) -> Result<usize, NjoyError> {
    match v {
        Some(x) if x >= 1.0 => Ok(x as usize),
        _ => Err(card_err(&format!("{what} must be present and >= 1"))),
    }
}

/// A record cursor over a LEAPR deck, reproducing Fortran list-directed reads.
///
/// Each `read_*` call begins on a fresh record (line) and may span further
/// records; anything left on the last record it touches is discarded, which is
/// how `read(nsysi,*)` behaves.
#[derive(Debug, Clone)]
pub struct CardCursor {
    /// The deck's records, with the optional leading module-name line and
    /// everything from a `stop` line onward already removed.
    lines: Vec<String>,
    /// Index of the next unread record.
    pos: usize,
}

impl CardCursor {
    /// Build a cursor positioned on card 1 of the LEAPR job inside `text`.
    ///
    /// The files ENDF/B-VIII.0 ships as `tsl-*.leapr` are not all bare decks, so
    /// this locates the job rather than assuming it starts at line 1:
    ///
    /// - everything from a `stop` record onward is dropped (NJOY's job
    ///   terminator);
    /// - the cursor then starts **after** the `leapr` module card, wherever it
    ///   sits. That card may be quoted (`'leapr'`, as in `tsl-SiO2-beta.leapr`),
    ///   may follow other NJOY modules in a multi-module job (`reconr` ...
    ///   `broadr` ... `leapr` ..., as in `tsl-013_Al_027.leapr`), or may sit
    ///   inside a shell heredoc (`cat>input <<EOF`, as in `tsl-HinH2O.leapr`).
    ///   Only a record whose entire content is `leapr` counts, so the word
    ///   appearing in a comment cannot trigger it.
    /// - if there is no such card at all, the cursor starts at line 1 and the
    ///   text is treated as a bare deck.
    ///
    /// Trailing NJOY modules *after* the LEAPR job are harmless: parsing stops
    /// at the card-20 comment terminator and never looks further.
    pub fn new(text: &str) -> Self {
        let mut lines: Vec<String> = Vec::new();
        for raw in text.lines() {
            if raw.trim().eq_ignore_ascii_case("stop") {
                break;
            }
            lines.push(raw.to_string());
        }
        let start = lines
            .iter()
            .position(|l| {
                let t = l.trim().trim_matches(|c| c == '\'' || c == '"').trim();
                t.eq_ignore_ascii_case("leapr")
            })
            .map(|i| i + 1)
            .unwrap_or(0);
        Self { lines, pos: start }
    }

    /// Records not yet consumed.
    pub fn records_left(&self) -> usize {
        self.lines.len().saturating_sub(self.pos)
    }

    /// Read up to `n` values, starting on a fresh record.
    ///
    /// Returns exactly `n` slots: `Some(v)` for a value the deck supplied,
    /// `None` for a slot the read never reached because a `/` terminated the
    /// record — which in LEAPR means "keep the preset default".
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if the deck runs out of records while the read
    /// is still short of `n` values and no `/` ended it.
    pub fn read_reals(&mut self, n: usize) -> Result<Vec<Option<f64>>, NjoyError> {
        let mut out: Vec<Option<f64>> = Vec::with_capacity(n);
        let mut terminated = false;
        while out.len() < n && !terminated {
            let line = self.next_record()?.to_string();
            for tok in tokenize(&line) {
                match tok {
                    Token::Slash => {
                        terminated = true;
                        break;
                    }
                    Token::Value { repeat, text } => {
                        let v = parse_real(&text)
                            .ok_or_else(|| card_err(&format!("cannot parse number `{text}`")))?;
                        for _ in 0..repeat {
                            if out.len() == n {
                                break;
                            }
                            out.push(Some(v));
                        }
                        if out.len() == n {
                            break;
                        }
                    }
                }
            }
        }
        while out.len() < n {
            out.push(None);
        }
        Ok(out)
    }

    /// Read exactly `n` values, erroring if the record set supplies fewer
    /// (a `/` short of the count is an error here, not a default).
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] naming `what` if fewer than `n` values arrive.
    pub fn read_reals_required(&mut self, n: usize, what: &str) -> Result<Vec<f64>, NjoyError> {
        let raw = self.read_reals(n)?;
        let mut out = Vec::with_capacity(n);
        for (i, v) in raw.into_iter().enumerate() {
            match v {
                Some(x) => out.push(x),
                None => {
                    return Err(card_err(&format!(
                        "{what}: only {i} of {n} values supplied"
                    )))
                }
            }
        }
        Ok(out)
    }

    /// Read a quoted (or bare) text card, e.g. card 2's title.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if the deck has no record left.
    pub fn read_text(&mut self) -> Result<String, NjoyError> {
        let line = self.next_record()?.to_string();
        Ok(strip_text_card(&line))
    }

    /// Read one card-20 comment record.
    ///
    /// Returns `Ok(None)` at the terminator — a record that supplies no text, in
    /// practice a bare `/` or a blank line, which leaves NJOY's sentinel `'$'`
    /// in place and ends the comment loop. Also returns `Ok(None)` at end of
    /// deck, so a deck that simply stops is not an error.
    pub fn read_comment(&mut self) -> Result<Option<String>, NjoyError> {
        if self.pos >= self.lines.len() {
            return Ok(None);
        }
        let line = self.next_record()?.to_string();
        let t = line.trim();
        if t.is_empty() || t == "/" {
            return Ok(None);
        }
        Ok(Some(strip_text_card(&line)))
    }

    /// The next record, advancing the cursor.
    fn next_record(&mut self) -> Result<&str, NjoyError> {
        let line = self
            .lines
            .get(self.pos)
            .ok_or_else(|| card_err("deck ended early"))?;
        self.pos += 1;
        Ok(line)
    }
}

/// One list-directed input item.
enum Token {
    /// The `/` record terminator.
    Slash,
    /// A value, possibly with an `r*` repeat count.
    Value {
        /// How many times the value repeats (1 unless written `r*v`).
        repeat: usize,
        /// The value's text, exponent letter not yet normalised.
        text: String,
    },
}

/// Split a record into list-directed items. Separators are whitespace and
/// commas; `/` ends the record; `r*v` is a repeat count.
fn tokenize(line: &str) -> Vec<Token> {
    let mut out = Vec::new();
    for raw in line.split(|c: char| c.is_whitespace() || c == ',') {
        if raw.is_empty() {
            continue;
        }
        // A `/` may be glued to the preceding value, as in `296/`.
        let mut rest = raw;
        while !rest.is_empty() {
            match rest.find('/') {
                Some(0) => {
                    out.push(Token::Slash);
                    return out;
                }
                Some(i) => {
                    push_value(&mut out, &rest[..i]);
                    out.push(Token::Slash);
                    return out;
                }
                None => {
                    push_value(&mut out, rest);
                    rest = "";
                }
            }
        }
    }
    out
}

/// Push a value item, splitting an `r*v` repeat count if present.
fn push_value(out: &mut Vec<Token>, text: &str) {
    if let Some((count, value)) = text.split_once('*') {
        if let Ok(r) = count.parse::<usize>() {
            out.push(Token::Value {
                repeat: r,
                text: value.to_string(),
            });
            return;
        }
    }
    out.push(Token::Value {
        repeat: 1,
        text: text.to_string(),
    });
}

/// Parse one numeric token, accepting Fortran `d`/`D` exponents and the
/// exponent-letter-free `1.0+5` / `1.0-5` forms ENDF-adjacent decks sometimes
/// use.
fn parse_real(text: &str) -> Option<f64> {
    let norm = text.replace(['d', 'D'], "e");
    if let Ok(v) = norm.parse::<f64>() {
        return Some(v);
    }
    // `1.0+5` style: insert the missing `e` before a sign that follows a digit
    // or a decimal point.
    let bytes = norm.as_bytes();
    for i in 1..bytes.len() {
        if (bytes[i] == b'+' || bytes[i] == b'-')
            && (bytes[i - 1].is_ascii_digit() || bytes[i - 1] == b'.')
        {
            let mut fixed = String::with_capacity(norm.len() + 1);
            fixed.push_str(&norm[..i]);
            fixed.push('e');
            fixed.push_str(&norm[i..]);
            return fixed.parse::<f64>().ok();
        }
    }
    None
}

/// Strip a text card down to its content: cut at the first `/` that is **not**
/// inside quotes, then remove the surrounding quotes, keeping interior spacing.
///
/// Quote tracking matters: `'----ENDF/B-VIII   MATERIAL 31   '/` must cut at the
/// trailing `/`, not at the one in "ENDF/B".
///
/// **Deliberate divergence from Fortran, documented rather than hidden.** For an
/// *undelimited* character item, list-directed input stops at the first blank —
/// so `H in H2O, CAB Model ... / TITLE` (the card-2 title of
/// `tsl-HinH2O.leapr`) would give NJOY just `H`. Keeping the whole text up to
/// the `/` is more useful and cannot affect any computed quantity, since the
/// title and the card-20 comments are descriptive only. Quoted cards — the
/// common case, and every card the graphite deck uses — are unaffected.
fn strip_text_card(line: &str) -> String {
    let mut in_quote: Option<char> = None;
    let mut cut = line.len();
    for (i, c) in line.char_indices() {
        match (in_quote, c) {
            (None, '\'') | (None, '"') => in_quote = Some(c),
            (Some(q), c) if c == q => in_quote = None,
            (None, '/') => {
                cut = i;
                break;
            }
            _ => {}
        }
    }
    let t = line[..cut].trim();
    if t.len() >= 2 {
        let b = t.as_bytes();
        let first = b[0];
        let last = b[t.len() - 1];
        if (first == b'\'' && last == b'\'') || (first == b'"' && last == b'"') {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A miniature but structurally complete deck: two temperatures, the second
    /// inheriting, one discrete oscillator, and a `/`-shortened card 4.
    const MINI: &str = "\
leapr
24
'mini test'/
2 1 20/
30 130/
11.898 4.73918 1 1 0/
0/
3 4 1/
0.1 0.2
0.4
1.0 2.0 3.0
4.0
296/
0.001 5/
0.0 1.0 2.0
1.0 0.0
0.05 0.0 0.9/
1/
0.2/
0.05/
-400.0/
' a comment line '/
/
stop
";

    /// Methodology: parse the miniature deck and check every card lands in the
    /// right field, that the 3 alpha / 4 beta / 5 rho lists are assembled across
    /// line breaks, and that the negative second temperature inherits cards
    /// 11-16. Result (2026-08-13): all assertions below hold.
    #[test]
    fn parses_all_cards_of_a_miniature_deck() {
        let d = LeaprDeck::parse(MINI).expect("mini deck parses");
        assert_eq!(d.nout, 24);
        assert_eq!(d.title, "mini test");
        assert_eq!(d.ntempr(), 2);
        assert_eq!(d.iprint, 1);
        assert_eq!(d.nphon, 20);
        assert_eq!(d.mat, 30);
        assert_eq!(d.za, 130.0);
        // card 4 was `/`-terminated after za, so these keep NJOY's defaults
        assert_eq!(d.isabt, 0);
        assert_eq!(d.ilog, 0);
        assert_eq!(d.smin, 1.0e-75);
        assert_eq!(d.awr, 11.898);
        assert_eq!(d.spr, 4.73918);
        assert_eq!(d.npr, 1);
        assert_eq!(d.iel, ElasticOption::Graphite);
        assert_eq!(d.ncold, ColdOption::None);
        assert_eq!(d.nsk, 0);
        assert_eq!(d.nss, 0);
        assert!(d.lat);
        assert_eq!(d.alpha, vec![0.1, 0.2, 0.4]);
        assert_eq!(d.beta, vec![1.0, 2.0, 3.0, 4.0]);

        let t0 = &d.temperatures[0];
        assert_eq!(t0.temperature_k, 296.0);
        assert!(!t0.inherited);
        assert_eq!(t0.continuous.delta_ev, 0.001);
        assert_eq!(t0.continuous.rho, vec![0.0, 1.0, 2.0, 1.0, 0.0]);
        assert_eq!(t0.continuous.twt, 0.05);
        assert_eq!(t0.continuous.c, 0.0);
        assert_eq!(t0.continuous.tbeta, 0.9);
        assert_eq!(t0.oscillators.len(), 1);
        assert_eq!(t0.oscillators[0].energy_ev, 0.2);
        assert_eq!(t0.oscillators[0].weight, 0.05);
        assert!(t0.pair_correlation.is_none());

        let t1 = &d.temperatures[1];
        assert_eq!(t1.temperature_k, 400.0);
        assert!(t1.inherited);
        assert_eq!(t1.continuous, t0.continuous);
        assert_eq!(t1.oscillators, t0.oscillators);

        assert_eq!(d.comments, vec![" a comment line "]);
        assert!(d.unsupported_features().is_empty());
    }

    /// Methodology: `input_at` must carry the deck grids and the block's own
    /// temperature into a `LeaprInput`, and `input_at_temperature` must override
    /// only the temperature. Result (2026-08-13): both hold; the 400 K input
    /// reuses the 296 K spectrum verbatim.
    #[test]
    fn input_at_builds_a_job_per_temperature() {
        let d = LeaprDeck::parse(MINI).unwrap();
        let a = d.input_at(0).unwrap();
        assert_eq!(a.temperature_k, 296.0);
        assert_eq!(a.alpha, d.alpha);
        assert_eq!(a.beta, d.beta);
        assert_eq!(a.nphon, 20);
        assert!(a.lat);
        assert_eq!(a.arat, 1.0);

        let b = d.input_at(1).unwrap();
        assert_eq!(b.temperature_k, 400.0);
        assert_eq!(b.continuous, a.continuous);

        let c = d.input_at_temperature(0, 523.0).unwrap();
        assert_eq!(c.temperature_k, 523.0);
        assert_eq!(c.continuous, a.continuous);

        assert!(d.input_at(2).is_err());
        assert!(d.input_at_temperature(0, -1.0).is_err());
    }

    /// Methodology: card 13 of the ENDF/B-VIII.0 graphite deck reads
    /// `0. 0. 1. 0./` — four values into a three-value read. Fortran discards
    /// the fourth and starts card 14 on the next record; a naive flat-token
    /// reader would mis-read `nd = 0.` from the leftover and then desynchronise.
    /// Result (2026-08-13): `twt/c/tbeta = 0/0/1`, `nd` comes from the *next*
    /// record, and the oscillator list is empty.
    #[test]
    fn extra_values_on_a_record_are_discarded_not_carried_over() {
        let deck = MINI.replace("0.05 0.0 0.9/\n1/\n0.2/\n0.05/", "0. 0. 1. 0./\n0/");
        let d = LeaprDeck::parse(&deck).expect("deck with an over-long card 13 parses");
        let t0 = &d.temperatures[0];
        assert_eq!(t0.continuous.twt, 0.0);
        assert_eq!(t0.continuous.c, 0.0);
        assert_eq!(t0.continuous.tbeta, 1.0);
        assert!(t0.oscillators.is_empty());
        assert_eq!(d.temperatures[1].temperature_k, 400.0);
    }

    /// Methodology: exercise the list-directed conveniences — comma separators,
    /// `r*v` repeat counts, Fortran `D` exponents and the exponent-letter-free
    /// `1.0+3` form. Result (2026-08-13): all four parse to the expected values.
    #[test]
    fn tokenizer_handles_list_directed_conveniences() {
        let mut cur = CardCursor::new("1.0,2.0 3*4.5\n1.0D-2 1.0+3/\n");
        let a = cur.read_reals(5).unwrap();
        assert_eq!(
            a,
            vec![Some(1.0), Some(2.0), Some(4.5), Some(4.5), Some(4.5)]
        );
        let b = cur.read_reals(4).unwrap();
        assert_eq!(b, vec![Some(0.01), Some(1000.0), None, None]);
    }

    /// Methodology: a deck that stops mid-card must be an error, never a
    /// silently short grid. Result (2026-08-13): both truncations error.
    #[test]
    fn truncated_deck_is_an_error() {
        assert!(LeaprDeck::parse("leapr\n24\n'x'/\n1 1 10/\n").is_err());
        // card 7 promises 3 alphas but only 2 are supplied before EOF
        let short = "leapr\n24\n'x'/\n1 1 10/\n30 130/\n1.0 1.0 1 0 0/\n0/\n3 1 0/\n0.1 0.2\n";
        assert!(LeaprDeck::parse(short).is_err());
    }

    /// Methodology: `unsupported_features` must flag deck options the ported
    /// kernels do not implement, so a caller cannot mistake "parsed" for
    /// "computed". Result (2026-08-13): an `nsk = 2` deck reports the Sköld gap.
    #[test]
    fn unsupported_features_are_reported() {
        // nsk = 2 turns on cards 17-19, which follow card 16.
        let deck = MINI
            .replace("11.898 4.73918 1 1 0/", "11.898 4.73918 1 1 0 2/")
            .replace(
                "0.05/\n-400.0/",
                "0.05/\n3 0.5/\n0.1 0.2 0.3/\n0.7/\n-400.0/",
            );
        let d = LeaprDeck::parse(&deck).expect("nsk deck parses");
        assert_eq!(d.nsk, 2);
        let pc = d.temperatures[0]
            .pair_correlation
            .as_ref()
            .expect("cards 17-19 read");
        assert_eq!(pc.dka, 0.5);
        assert_eq!(pc.skappa, vec![0.1, 0.2, 0.3]);
        assert_eq!(pc.cfrac, 0.7);
        let f = d.unsupported_features();
        assert_eq!(f.len(), 1, "features = {f:?}");
        assert!(f[0].contains("Skold"), "features = {f:?}");
    }
}
