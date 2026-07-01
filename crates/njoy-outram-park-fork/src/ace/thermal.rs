//! Thermal scattering **S(α,β)** ACE table writer — **SCAFFOLD (not yet ported)**.
//!
//! This is the thermal counterpart of the continuous-energy ACE writer in
//! [`super`]. It will produce the `…t` ACE tables (class letter `t`) that a
//! Monte-Carlo code uses for bound-atom thermal scattering — graphite, light/
//! heavy water, ZrH, etc. — where the free-gas model BROADR applies is no longer
//! valid and the scattering law S(α,β) governs the kinematics.
//!
//! **Status: stub.** Every entry point here returns
//! [`NjoyError::NotPorted`]. See the porting plan (`docs/porting-plan.md`,
//! Phase 4 — thermal) for sequencing: this is scheduled **after** the
//! continuous-energy ACE library (ESZ/SIG/AND/DLW/NU) is finished.
//!
//! ## Pipeline this depends on
//!
//! ```text
//!   [LEAPR] → THERMR → (this) thermal ACER
//! ```
//!
//! - **LEAPR** (`leapr.f90`, ~3.6k lines) — *generates* an S(α,β) evaluation
//!   (ENDF MF=7) from a physical scattering-law model. Only needed when the
//!   evaluation does not already ship MF=7; many libraries (ENDF/B thermal
//!   sublibrary) provide it, so LEAPR can come later or never.
//! - **THERMR** (`thermr.f90`, ~3.4k lines) — *the prerequisite*. Reads MF=7
//!   S(α,β) and computes the thermal **incoherent-inelastic** scattering cross
//!   section + coupled energy-angle distributions, and the **coherent-elastic**
//!   (Bragg edges) / **incoherent-elastic** cross sections, onto a PENDF tape
//!   (MT=221–250). This module consumes THERMR's output. THERMR must be ported
//!   first (Phase 3 in the plan).
//! - **thermal ACER** (`aceth.f90` + the thermal driver in `acefc.f90`) — what
//!   this module ports: lay THERMR's thermal data into a thermal ACE table.
//!
//! ## Target ACE thermal-table layout (from `aceth.f90`)
//!
//! Distinct from the continuous-energy NXS/JXS. To be reproduced here:
//!
//! - **NXS**: `IDPNI` (inelastic scattering mode), `NIL` (inelastic dimensioning
//!   — # discrete cosines or order), `NIEB` (# incident energies, inelastic),
//!   `IDPNC` (elastic scattering mode), `NCL` (elastic dimensioning), `IFENG`
//!   (energy-distribution form: equiprobable / skewed / continuous), `NCLI`.
//! - **JXS**: `ITIE`/`ITIX` (inelastic energy grid + cross section), `ITXE`
//!   (inelastic energy-angle distributions), `ITCE`/`ITCX`/`ITCA` (coherent-
//!   elastic Bragg energies / cumulative xs / —), `ITCEI`/`ITCXI`/`ITCAI`
//!   (incoherent-elastic energy / xs / angles).
//!
//! ## TODO (Phase 4 — thermal), in dependency order
//!
//! 1. Port **THERMR** (MF=7 reader + S(α,β) → thermal cross sections / dists).
//! 2. Define `ThermalScatteringData` (THERMR's output: inelastic xs + E-angle
//!    dists, coherent Bragg edges, incoherent-elastic xs/angles).
//! 3. Build the ITIE/ITIX/ITXE inelastic blocks.
//! 4. Build the ITCE/ITCX/ITCA coherent-elastic (Bragg) blocks.
//! 5. Build the ITCEI/ITCXI/ITCAI incoherent-elastic blocks.
//! 6. Reuse [`super::write`] for the Type-1 ASCII output (header + NXS/JXS + XSS),
//!    adjusting the ZAID class letter to `t` and the thermal NXS/JXS semantics.

use crate::NjoyError;

/// Placeholder for THERMR's processed thermal-scattering output.
///
/// **Not yet defined.** Will hold the incoherent-inelastic cross section and
/// energy-angle distributions, the coherent-elastic Bragg edges, and the
/// incoherent-elastic cross section/angles for one material at one temperature —
/// everything the thermal ACE blocks are built from. See the [module docs](self).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ThermalScatteringData {
    // TODO(phase-4-thermal): populate from a ported THERMR. Intentionally empty
    // so the type can be referenced while the producer side is scaffolded.
}

/// Write a thermal **S(α,β)** ACE table — **not yet ported**.
///
/// # Errors
/// Always returns [`NjoyError::NotPorted`] until the thermal ACER path
/// (`aceth.f90`) and its THERMR prerequisite are implemented. See the
/// [module docs](self) for the porting sequence.
pub fn write_thermal_ace(_data: &ThermalScatteringData) -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted(
        "thermal S(α,β) ACE writer (aceth.f90) — Phase 4, after the CE ACE library",
    ))
}
