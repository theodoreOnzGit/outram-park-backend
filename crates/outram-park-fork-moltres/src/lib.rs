//! # outram-park-fork-moltres
//!
//! MSR neutronics + thermal-hydraulics on the outram-foam finite-volume layer — physics formulation from the LGPL Moltres code (circulating-fuel group diffusion + delayed-neutron precursor drift + salt TH), reimplemented on outram-foam rather than MOOSE/PETSc. SCAFFOLD: no human V&V. Not affiliated with the Moltres/ARFC project.
//!
//! > **⚠️ Scaffold — unverified until validated.** Skeleton crate; the port is
//! > in progress (MSRE digital-twin epic `op-6w0`). No human V&V. Not for
//! > nuclear facility operation, reactor control, safety-critical, or licensing
//! > decisions. Independent OUTRAM PARK fork.
#![forbid(unsafe_code)]
