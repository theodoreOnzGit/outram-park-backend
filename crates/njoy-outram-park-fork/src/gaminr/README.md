# GAMINR — multigroup photoatomic data

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §GAMINR); upstream Fortran: `gaminr.f90` (~2k lines).

## Theory

GAMINR is the photon analogue of GROUPR: it produces complete multigroup
**photoatomic** (photon–electron-cloud interaction) data from ENDF photoatomic
evaluations. The photon interaction is decomposed into:

- **coherent (Rayleigh)** scattering — no energy loss, angular redistribution set
  by the atomic **form factor** F(q, Z);
- **incoherent (Compton)** scattering — energy + angle from the Klein–Nishina
  cross section modulated by the **incoherent scattering function** S(q, Z), which
  accounts for electron binding;
- **pair production** — above 1.022 MeV, in the nuclear and electron fields;
- **photoelectric absorption** — with fluorescence.

Cross sections are group-averaged over a photon group structure and weight, and
the coherent/incoherent parts get **Legendre group-to-group scattering matrices**.

## How the port will implement it

**Not yet ported.** Planned: reuse the GROUPR group-averaging core for the vector
cross sections, then add the form-factor / incoherent-scattering-function angular
integrals (Legendre moments) for the coherent and Compton matrices. Depends on a
photoatomic ENDF reader in `crate::endf` (MF=23/27). Complements the `acepa`
photoatomic ACE class (`../acer/README.md`).

## Testing

**TODO.** Gate: reproduce upstream group cross sections + a coherent scattering
matrix for a reference element (e.g. carbon or lead) against the Fortran oracle.

## Caveats

- **Not required by OpenMC CE neutron transport** — Phase 5.
- Needs photoatomic evaluations (a different sublibrary from neutron ENDF).
- Fluorescence/relaxation cascades are an evaluation-data concern, not modelled
  here beyond what the photoelectric cross section carries.

## References

- NJOY2016 manual §GAMINR (LA-UR-17-20093)
- `gaminr.f90` (NJOY2016 2016.79)
- Hubbell et al., atomic form factors & incoherent scattering functions
