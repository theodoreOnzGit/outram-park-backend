# GROUPR — multigroup cross sections and matrices

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §GROUPR); upstream Fortran: `groupr.f90` (~12.7k lines,
> NJOY2016 commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`).

## Theory

GROUPR collapses continuous-energy data onto a **multigroup** structure for
deterministic transport. A group cross section is the flux-weighted average

$$\sigma_{x,g} = \frac{\int_g \sigma_x(E)\,\phi(E)\,dE}{\int_g \phi(E)\,dE}$$

with $\phi(E)$ a weighting spectrum (built-in analytic weights — $1/E$,
Maxwellian, thermal + $1/E$ + fission — or a user tabulation, optionally
self-shielded via the Bondarenko $\sigma_0$ from UNRESR). Beyond vector cross
sections GROUPR builds group-to-group scattering matrices (with Legendre order
for anisotropy), photon-production matrices, ratio quantities ($\bar\mu$,
$\bar\nu$, inverse velocity, photon yield), fission as a full group-to-group
matrix, delayed-neutron spectra, and anisotropic thermal scattering. Output is a
**GENDF** tape, the input to CCCCR/MATXSR/DTFR/POWR/WIMSR.

## Port status — PARTIAL (AI draft, untrusted until reviewed)

This is a self-contained partial port: the **input/selector/data** layer is
ported and unit-tested; the **numeric group-averaging engine** is explicitly
`NotPorted` (it needs a PENDF tape + flux integration that cannot be exercised
standalone). Files:

| File | Ports | Upstream |
|---|---|---|
| `input.rs` | Card deck `GrouprInput` + `parse`, selector enums (`WeightSelection`, `WeightOption`, `PrintOption`, `SmoothingOption`), `neutron_group_from_ign` adapter, `ReactionRequest` | `ruinb` `groupr.f90:1044-1130`; card reads `:628`, `:993`; option tables `:126-215` |
| `photon_groups.rs` | `PhotonGroupStructure` enum + `photon_group_structure(igg)` boundary tables (eV) | `gengpg` `groupr.f90:4651-4863` |
| `weights.rs` | `AnalyticWeight` closed-form weights (`iwt=2,3,4,6,7,11,12`); `ThermalFissionParams`; preserved built-in TAB1 tables (`iwt=5,8,9`) | `genwtf` `groupr.f90:4865-5113`; `getwtf` `:5115-5307` |
| `unresolved.rs` | Bondarenko flux (`genflx_bondarenko`), URR self-shielded table + retrieval (`UnresolvedTable::store`/`shield`) | `genflx` `:5309-5684`; `stounr`/`getunr` `:6802-6994` |
| `urr_pendf.rs` | PENDF MF=2/MT=152 tape reader: `read_urr_table` (LIST-body decode) + `read_urr_from_tape` (tape-locate) | `stounr` `:6802-6875` |
| `pendf_feed.rs` | PENDF MF=3/MF=13 smooth cross-section tape reader: `classify_mtd` + `read_pendf_cross_section` (ordinary single-TAB1 case only) | `getsig` init branch `:6646-6753` |
| `mod.rs` | Module map, re-exports, `run()` pipeline skeleton | `subroutine groupr` `:97-1042` |

### Ported (self-contained, tested)

- **Input card deck** — cards 1-10 modelled as `GrouprInput` with named selector
  enums; a free-format `GrouprInput::parse` reads the numeric cards, conditional
  group-grid cards (6/7), the analytic weight card (8c), the reaction list
  (card 9) and the next-material pointer (card 10).
- **Photon (gamma) group structures** — all built-in `igg = 2..10` tables
  (CSEWG-94, LANL-12/24/48, Steiner-21, Straker-22, VITAMIN-C-36/E-38/J-42),
  in eV, including the VITAMIN-C `eg8` splice. `igg = 0` = none; `igg = 1` =
  read-in (`NotPorted`).
- **Analytic weighting functions** — constant, `1/E`, thermal+1/E+fission,
  thermal-1/E-fission+fusion (with optional temperature dependence), and
  VITAMIN-E, evaluated from the `getwtf` closed forms. Built-in *tabulated*
  weight point tables (EPRI-CELL, fast-reactor, CLAW) are preserved verbatim.

### Neutron group structures — live in ERRORR (not duplicated)

The neutron `gengpn` tables (`ign` selector, `groupr.f90:1599-4649`) were already
ported in `src/errorr/groups.rs`. GROUPR **re-exports** them
(`groupr::neutron_group_structure`, `groupr::NeutronGroupStructure`) and maps
`ign → structure` with `groupr::neutron_group_from_ign` (forbidding the
ERRORR-only `ign = -1`). No boundary table is copied.

### NotPorted (explicit gap list — do not fabricate)

**Note (op-3ut, 2026-07-20):** this list predates the op-bsz self-shielding
pass and the op-3ut tape-feeder pass, both landed since it was written, and is
stale on several entries below — `genflx`/`getunr`/`stounr` (see
`unresolved.rs`/`urr_pendf.rs`), `panel`/`displa` (see `panel.rs`/`matrix.rs`),
and `getsig` (see `pendf_feed.rs`, ordinary MF=3/MF=13 case only) are now
**partially ported**; the rest of this list has not been re-audited item by
item. A `mod.rs`-level `run()` NotPorted entry point remains accurate — the
end-to-end pipeline is not wired together.

The numeric engine and its dependencies return `NjoyError::NotPorted`:

- reaction retrieval & feed functions: `getmf6`, `getff`, `getfwt`, `getflx`,
  `getyld`, `getsig` (MF=10 photon-production + `MT=257/258/259` derived
  quantities only — the ordinary MF=3/MF=13 case is ported, see
  `pendf_feed.rs`), `getdis` (anisotropic-CM only — see `matrix.rs`),
  `getgfl`, `getgyl`, `getsed`, `anased`;
- quadrature / accumulation: `epanel`, `gengr`, `glmol` (the vector/matrix
  `panel`/`displa` reduction is ported — see `panel.rs`/`matrix.rs`);
- flux calculator & URR self-shielding: `getfwt`; the slowing-down branch of
  `genflx` (`nflmax > 0`) — the Bondarenko branch and `getunr`/`stounr` are
  ported, see `unresolved.rs`/`urr_pendf.rs`;
- kinematics / matrix machinery: `cm2lab`, `f6cm`, `f6lab`, `ll2lab`, `getaed`,
  `aedi`, `getco`, `gam102`, `conver`, `hnab`;
- the GENDF matrix-record writer is ported (see `matrix.rs`/`gendf.rs`); the
  vector-record writer was already ported before this note.

Also `NotPorted` in the input layer: the flux-calculator card 8a (`iwt < 0`),
the TAB1 weight card 8b (`iwt = 1`), the resonance-flux card 8d (`iwt = 0`), the
card-9a extended residual format (`mfd = -1`), and *evaluation* of the built-in
tabulated weights (`iwt = 5, 8, 9`) which needs the ENDF TAB1 (`terpa`)
interpolator. The `run()` entry point returns `NotPorted("groupr")`.

## Testing (V&V — methodology + results, 2026-07-15, commit `ac5adf5`)

Inline `#[cfg(test)]` tests in each file, run via
`crates/njoy-outram-park-fork/scripts/test.sh groupr` (12 GB cap, `--release`).

- **`photon_groups.rs`** — `igg` round-trip (0..10); every built-in structure's
  group count, strict-ascending ordering, and endpoint energies vs the Fortran
  `eg*` tables; `igg=0` empty / `igg=1` NotPorted; the VITAMIN-C 36-group splice
  (removes `eg8(7)=0.075 MeV`, drops `eg8(39)=20 MeV`) reproduced exactly.
- **`weights.rs`** — analytic-only `from_iwt` selection; constant/`1/E` values;
  `iwt=4` piecewise continuity at both breakpoints (rel err < 1e-12); `iwt=6`
  segment values + the 14 MeV fusion bump; VITAMIN-E six-segment values (rel err
  < 1e-6); temperature dependence active only for `iwt=7`; the preserved TAB1
  tables' length/header/monotone-`x` structure.
- **`input.rs`** — the `ign` adapter round-trips against ERRORR's authoritative
  `NeutronGroupStructure::ign` (1..36, cannot drift); every card-2 selector
  round-trips through its Fortran integer; the default deck matches NJOY's init
  block; a representative U-235 VITAMIN-J deck parses field-by-field; conditional
  cards 6 + 8c parse; NotPorted weight cards (8a/8b/8d) are reported honestly.
- **`mod.rs`** — `run()` → `NotPorted("groupr")`; the neutron + photon surfaces
  both resolve (ign=17 → 176 boundaries; igg=10 → 43 boundaries).

Test counts: see the porting agent's hand-off / CI. The **numeric group
averaging is untested here by design** — its V&V gate (reproduce upstream group
cross sections + a scattering matrix for a reference nuclide against the Fortran
oracle) is deferred until the engine is ported.

## Caveats

- **AI-generated draft — untrusted until human-reviewed** per crate CLAUDE.md.
  A human must check the transcribed `eg*`/`w*` data tables and the `getwtf`
  closed-form constants against `groupr.f90` before any use.
- **Not required by OpenMC CE** — Phase 5, deterministic/sensitivity workflows.
- Self-shielded matrices depend on UNRESR/PURR outputs; port those first for URR
  nuclides.
- Huge surface area — the engine must be ported feed-function by feed-function,
  verifying each against the oracle.

## References

- NJOY2016 manual §GROUPR (LA-UR-17-20093)
- `groupr.f90` (NJOY2016, commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`)
- Boltzmann constant `bk = 8.617333262e-5` eV/K (`phys.f90:22`)
