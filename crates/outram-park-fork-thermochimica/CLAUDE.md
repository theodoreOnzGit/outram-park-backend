# CLAUDE.md — outram-park-fork-thermochimica

Guidance for Claude Code (and other AI assistants) working in this crate. The
workspace root `CLAUDE.md` binds here as everywhere; this file adds what is
specific to this crate. Added 2026-10-20: the member-crate rule requires one,
and this crate had none.

## What it is

An independent pure-Rust translation of ORNL **Thermochimica**
(<https://github.com/ORNL-CEES/thermochimica>, BSD-3-Clause, reference commit
`0c35c8d`). It does Gibbs-energy-minimisation (CALPHAD) thermochemistry for
molten fluoride salts: fission-product speciation, UF3/UF4 redox partitioning
and solubility, for MSRE fission-product migration.

**Status: SCAFFOLD, no human V&V.** Checked 2026-10-20:
- the whole port is `src/gem.rs`, with 6 unit tests;
- `src/lib.rs` only re-exports;
- no other crate depends on it (no consumer in any `crates/*/Cargo.toml`).

Do not describe it as verified or validated.

It belongs to the MSRE digital-twin group, with `outram-park-fork-moltres` and
`outram-park-fork-onix` (`docs/claude-md/members-and-dependencies.md`; scoping
in `docs/reactor-scoping/msre.md`).

## Provenance (keep it)

- `src/gem.rs` carries the attribution header. It names upstream, the commit,
  and every Fortran routine consulted, with `File.f90:line` citations
  (`GEMSolver.f90`, `GEMNewton.f90`, `GEMLineSearch.f90`,
  `CompChemicalPotential.f90`, `CompMolFraction.f90`,
  `CompExcessGibbsEnergyRKMP.f90`). Keep it, and extend the list whenever
  another routine is ported.
- `LICENSE.thermochimica` and `upstream_source/README.md` keep the BSD-3
  notice. The reference clone itself
  (`upstream_source/thermochimica/`) is gitignored; only the README is
  tracked.
- The crate is GPL-3.0-only (workspace default). BSD-3 combines one way into
  GPL-3.0. #973 (workspace relicence to AGPL-3.0) will change the crate
  licence. The BSD-3 notice stays either way.

## Working rules for this crate

- **Upstream is the specification.** When the solver misbehaves, read the
  matching Thermochimica routine before changing anything (root `CLAUDE.md`,
  "Debugging a port").
- **Thermodynamic data is input, never tuned.** Molten-fluoride databases
  (e.g. MSTDB-TC) are supplied separately with documented provenance. Do not
  adjust a Gibbs parameter to make a speciation result match.
- **Search before adding.** `outram-park-fork-dwsim-libs` has a sibling
  `thermo::gibbs_multiphase`. Check it before writing another minimiser, and
  say which one a caller should use.
- Release builds only: `cargo test -p outram-park-fork-thermochimica --release`.
- Regenerate `docs/outram-park-fork-thermochimica-api.md` with
  `kovan-cli api-docs outram-park-fork-thermochimica` when a public doc comment
  changes.
