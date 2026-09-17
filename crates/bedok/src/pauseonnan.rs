//! Abort the run if a vector has gone non-finite or complex.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `pauseonnan.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.

use crate::error::{BedokError, Result};

/// `pauseonnan(input)`.
///
/// A debugging guard the solvers call at points where a diverging iterate would
/// otherwise be carried silently into the next sweep. Raises on the first `NaN`
/// it finds; the reference also rejects complex input.
///
/// # Arguments
///
/// - `input` — the values to check, in any units.
///
/// # Errors
///
/// - [`BedokError::NanEncountered`] if any entry is `NaN`.
///
/// # What is checked, and what is not
///
/// The reference tests `any(isnan(input))`, which catches `NaN` but **not**
/// `Inf` — an infinite iterate passes this guard. That is preserved; use
/// `fixinfnan` for the non-finite case, as the reference does.
///
/// The `~isreal(input)` test cannot be reached here: the translation carries
/// `f64` throughout, so there is no complex value to detect.
/// [`BedokError::UnexpectedComplex`] exists to record that the reference makes
/// the check, and would become live if a complex path is ever introduced.
///
/// # Printing
///
/// Before erroring, the reference echoes the offending vector to the console
/// (the bare `input` on its own line). That is reproduced on stderr, since its
/// purpose — showing the user *what* went non-finite — is part of the
/// behaviour rather than incidental.
pub fn pauseonnan(input: &[f64]) -> Result<()> {
    if input.iter().any(|x| x.is_nan()) {
        eprintln!("input =\n{input:?}");
        return Err(BedokError::NanEncountered);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_input_passes() {
        assert!(pauseonnan(&[1.0, 2.0, -3.0]).is_ok());
    }

    #[test]
    fn nan_is_rejected() {
        assert!(matches!(
            pauseonnan(&[1.0, f64::NAN]),
            Err(BedokError::NanEncountered)
        ));
    }

    /// Pins the documented gap: the reference's guard does not look at `Inf`.
    #[test]
    fn infinity_passes_the_guard() {
        assert!(pauseonnan(&[1.0, f64::INFINITY]).is_ok());
    }
}
