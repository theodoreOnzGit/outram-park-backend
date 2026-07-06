# ACER — ACE library writer (continuous-energy + thermal)

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
- **4f thermal `…t`** ✅ — inelastic (IFENG=0) + coherent/incoherent elastic.
- **4b ν̄ (NU block)** ⬜, continuum correlated angle (Law 44/61) ⬜.
- **Dosimetry / photoatomic / photonuclear classes** ⬜ — `acedo`/`acepa`/`acepn`
  not started.

## Testing

`tests/acer.rs` (NXS/JXS self-consistency, Type-1 round-trip, DLW-walk, ESZ
heating physicality on U-235), `tests/thermal_ace.rs`, `tests/thermal_ace_zrh.rs`.
See `docs/porting-plan.md` §4 for the full V&V trail.

## Caveats

- Not yet a *complete* CE transport library: fission has no NU block (4b) and
  MF=6 continuum producers are still emitted isotropic.
- Photoatomic/photonuclear/dosimetry classes are unported.
- The `run()` driver returns `NotPorted`; use `crate::acer` / `write_ace`.

## References

- NJOY2016 manual §ACER (LA-UR-17-20093)
- `acer.f90`, `acefc.f90`, `aceth.f90` (NJOY2016 2016.79)
- X-5 Monte Carlo Team, MCNP ACE format specification
