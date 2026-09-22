# ACER — ACE library writer (continuous-energy + thermal)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §ACER); upstream Fortran: `acer.f90`, `acefc.f90` (~19.7k),
> `aceth.f90`, `acepn.f90`, `acepa.f90`, `acedo.f90`, `acecm.f90`.

## Theory

ACER converts a processed PENDF evaluation into an **ACE** file — the compact,
random-access format used by MCNP and OpenMC. Where ENDF is sequential and
formalism-rich, ACE is pointer-indexed (NXS/JXS locator arrays into one flat XSS
data array) and pre-sampled for fast Monte Carlo lookup. ACER writes several
distinct **class** tables:

| Class | Suffix | Content | Upstream |
|---|---|---|---|
| Fast continuous-energy | `…c` | ESZ, reactions, angular + energy dists, ν̄, heating | `acefc` |
| Thermal S(α,β) | `…t` | inelastic + coherent/incoherent elastic thermal tables | `aceth` |
| Dosimetry | `…y` | cross sections only, for response functions | `acedo` |
| Photoatomic | `…p` | photon interaction (coherent, incoherent, pe, pair) | `acepa` |
| Photonuclear | `…u` | photon-induced reactions | `acepn` |

## How the port implements it (status matrix)

Ported in [`crate::acer`]:

- **4a cross-section core** ✅ — ESZ (union grid, total, disappearance, elastic,
  heating) + MTR/LQR/TYR/LSIG/SIG.
- **4c elastic angular (LAND/AND)** ✅ — MF=4/MT=2 → tabulated-cosine.
- **4d energy dists (LDLW/DLW)** 🟡 — Law 3 (discrete levels) + Law 4 (MF=5 LF=1
  χ, MF=6 LAW=1 neutron); discrete-level angular wired.
- **4e heating (ESZ col 5)** ✅ — `H(E)=KERMA/σ_total` from HEATR H1–H5.
- **4f thermal `…t`** ✅ — inelastic in **all three** forms (IFENG=0
  equiprobable, IFENG=1 skewed, IFENG=2 continuous; 2026-09-22) +
  coherent/incoherent elastic. IFENG=2's ITXE layout reproduces NJOY's point
  counts exactly on Al-27 and graphite.
- **4b ν̄ (NU block)** ✅ *(2026-09-20 — bit-identical to NJOY2016, 347/347 values on U-235)*, continuum correlated angle (Law 44/61) ⬜.
- **Photoatomic class `…p`** ✅ *(2026-09-21 — `acepa`'s `acepho`, `iheat`,
  `alax` and `phoout`; the Type-1 output is **byte-identical** to NJOY2016 on
  the synthetic Z=6 tape, and 71 781 of 71 807 words are at the file's print
  precision on U ENDF/B-VIII.0)*. Fluorescence (JFLO) is translated but
  unexercised — no atomic-relaxation tape is held here.
- **Dosimetry class `…y`** ✅ *(2026-09-21 — `acedo`'s `acedos` and `dosout`;
  **byte-identical** to NJOY2016 on H-1 (2 532 words) and Mn-55 (70 440 words,
  MF=10 isomeric channels included). NJOY's own test suite never runs
  `iopt = 3`.)*
- **Photonuclear class `…u`** ⬜ — `acepn` not started.

## One reader, one writer (2026-09-22)

`read.rs` is the only ACE reader and `RawAceTable::to_type1_string` /
`to_type2_bytes` the only serialisers; `write.rs` is now a thin conversion
from `AceTable`, and the Fortran edit descriptors live once in
`fortran_fmt.rs`. Nothing outside this crate parses or writes ACE.

## Testing

`tests/acer.rs` (NXS/JXS self-consistency, Type-1 round-trip, DLW-walk, ESZ
heating physicality on U-235), `tests/thermal_ace.rs`, `tests/thermal_ace_zrh.rs`,
`tests/acer_photoatomic_vs_njoy2016.rs` and
`tests/acer_dosimetry_vs_njoy2016.rs` (byte equality against NJOY's own files).
See `docs/porting-plan.md` §4 for the full V&V trail.

## Caveats

- Not yet a *complete* CE transport library: ~~fission has no NU block (4b) and~~ **CORRECTED 2026-09-20 — the NU block is written; what remains is**
  MF=6 continuum producers are still emitted isotropic.
- Photoatomic/photonuclear/dosimetry classes are unported.
- The `run()` driver returns `NotPorted`; use `crate::acer` / `write_ace`.

## References

- NJOY2016 manual §ACER (LA-UR-17-20093)
- `acer.f90`, `acefc.f90`, `aceth.f90` (NJOY2016 2016.79)
- X-5 Monte Carlo Team, MCNP ACE format specification
