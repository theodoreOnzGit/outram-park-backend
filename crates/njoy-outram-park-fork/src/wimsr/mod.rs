// Ported from NJOY2016 `src/wimsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine wimsr`, l.51-244 — the driver.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `WIMSR` — libraries for the WIMS reactor-physics code.
//!
//! Prepares multigroup libraries for the WIMS ("Winfrith Improved Multigroup
//! Scheme") lattice-physics code (WIMS-D / WIMS-E) from GROUPR GENDF output. WIMS
//! uses collision-probability methods for pin-cell and assembly flux solutions
//! and needs cross sections, scattering matrices, and resonance-integral /
//! self-shielding data in its own library format.
//!
//! **Upstream:** `wimsr.f90`. **Manual:** LA-UR-17-20093 §WIMSR.
//!
//! ```text
//! gendf::read_material   wminit          the GENDF's temperature blocks, egb, awr
//! xsecs::xsecs           xsecs + xseco   cross sections, P0 matrix, spectrum
//! resint::resint         resint + rsiout resonance integrals vs T and sigma-0
//! p1scat::p1scat         p1scat + p1sout P1 matrices (ip1opt = 0)
//! wimout::wimout         wimout          the library text
//! ```
//!
//! **Verified** against the NJOY2016 binary on ENDF/B-VIII.0 U-238
//! (`reference-data/wimsr/`, 29 groups, six sigma-zeros, free-gas
//! thermal `MT=221`, `ires = 1`, `isof = 1`, `ip1opt = 0`):
//! `tests/wimsr_u238_njoy_golden.rs`. See `README.md`. [`run`] is the
//! whole-module entry point; the card-deck `run()` of the module table
//! (`crate::modules`) stays [`crate::NjoyError::NotPorted`] because the
//! deck reader is not written — callers build a [`WimsrInput`].

pub mod gendf;
pub mod input;
pub mod p1scat;
pub mod resint;
pub mod wimout;
pub mod xsecs;

use crate::endf::tape::Tape;
use crate::NjoyError;

pub use input::WimsrInput;

/// A WIMSR run: the library lines and the stage results the listing
/// prints (`iprint = 2`).
#[derive(Debug, Clone)]
pub struct WimsrOutput {
    /// The library, one entry per output line (no trailing newline).
    pub lines: Vec<String>,
    pub xsecs: xsecs::XsecsResult,
    pub resint: Option<resint::ResintResult>,
    pub p1: Option<Vec<p1scat::P1Temp>>,
    /// `wminit`'s header: `awr`, `iznum`, `egb` (descending).
    pub awr: f64,
    pub iznum: i32,
    pub egb: Vec<f64>,
}

impl WimsrOutput {
    /// The library as text (`\n`-terminated lines).
    pub fn text(&self) -> String {
        let mut s = self.lines.join("\n");
        s.push('\n');
        s
    }
}

/// Run WIMSR on a GENDF `tape` (`subroutine wimsr`, `wimsr.f90:51-244`).
///
/// # Errors
/// `EndfParse` for a material or group structure not on the tape, for a
/// GENDF without the thermal `mti` matrix at the first temperature
/// (upstream: "use only 0 temps for mat ... mti missing from higher
/// temps" and an empty library), and for `p1scat`'s "no p1 matrices".
pub fn run_gendf(tape: &Tape, inp: &WimsrInput) -> Result<WimsrOutput, NjoyError> {
    if inp.glam.len() < inp.nrg {
        return Err(NjoyError::EndfParse(format!(
            "wimsr: {} goldstein lambdas given, nrg = {}",
            inp.glam.len(),
            inp.nrg
        )));
    }
    let mat = gendf::read_material(tape, inp.mat, inp.ngnd)?;
    let cnt = xsecs::counts(inp, &mat);
    let xs = xsecs::xsecs(inp, &mat, &cnt)?;
    if xs.ntemp == 0 {
        return Err(NjoyError::EndfParse(format!(
            "wimsr::xsecs: use only 0 temps for mat {} — mti missing from higher temps",
            inp.mat
        )));
    }
    let ri = resint::resint(inp, &mat, &cnt, &xs);
    let p1 = p1scat::p1scat(inp, &mat, &cnt, &xs)?;
    let lines = wimout::wimout(inp, mat.awr, mat.iznum, &xs, ri.as_ref(), p1.as_deref());
    Ok(WimsrOutput {
        lines,
        xsecs: xs,
        resint: ri,
        p1,
        awr: mat.awr,
        iznum: mat.iznum,
        egb: mat.egb,
    })
}

/// The module-table entry point: WIMSR's card deck reader is not written,
/// so this stays `NotPorted`; use [`run_gendf`] with a [`WimsrInput`].
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("wimsr (card deck; use wimsr::run_gendf)"))
}
