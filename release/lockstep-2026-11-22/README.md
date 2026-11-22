# Lockstep release 2026-11-22/23 (all 51 member crates)

Maintainer request 2026-11-22 ("prep the entire workspace for lockstep
release"), published in two waves at the maintainer's direction ("publish the
passing crates that have nothing to do with neutronics stack", then "publish
now, documenting known failures"). First releases: **kembangan 0.0.1** and
**punggol 0.0.1**. Every other crate's patch version +1
(`scripts/release/bump-versions.sh`, `56dfab087`).

| Wave | Tag | Commit | Crates |
|---|---|---|---|
| 1 | `release-2026-11-22` | `5b3bed31b` | 39: everything not depending on `outram-mc-libs` or `nee_soon`, plus njoy |
| 2 | `release-2026-11-23` | `16dea0ebd` | 12: `outram-mc-libs`, `nee_soon`, `outram-park-fork-liggghts` and every crate that depends on them |

Decay data used without `std`: `openmc-endf-8-depletion-lib-a` / `-b` 1.1.0
(published 2026-11-22, separate repositories).

## Test evidence

- **Full run** (`kovan-cli test`, no fail-fast, long tests on) at `447b47b71`
  on the release host (Intel Xeon W-2195, 36 logical cores, 62.5 GiB, Linux,
  CPU plus llvmpipe software GPU). Stopped after ~700 binaries (all crates
  except `outram-park-fork-liggghts`' last binaries and the alphabetically
  later crates) when the commit was superseded; **no `[test_run]` record was
  written**, so the merge to `main` carries no full `kovan-cli test` record of
  the merged commit (root CLAUDE.md asks for one). The crates the run did not
  reach were tested separately (below).
- **Separately, on develop's tip at the time:** the 12 crates the run had not
  reached (petir, raffles, redhill, the steam tables, teh-o-prke, tuas, the
  MPI crate, the moltres/offbeat/onix/pflotran/thermochimica forks): 158
  binaries, all passing. The fixed tests below, each re-run.
- **Wave 2** was published before its own re-test at `16dea0ebd` finished
  (maintainer: "publish now"). `outram-mc-libs` at `d1e207f18` passed
  `--lib --tests` (952 passed, 0 failed, 88 binaries) on a 4-core Xeon @
  2.30 GHz (gh:#1709); the re-test of all 12 on the release host was still
  running at publish time.
- `cargo publish --dry-run` of each wave's crates was clean (every package
  built from its tarball).

## Known failures (published with them)

| Crate | Test | Issue | State |
|---|---|---|---|
| `outram-blender` 0.0.8 | `tests/gpu_csg_parity.rs` `grid_index_matches_the_cpu_plotter`: unindexed GPU path mismatches the CPU plotter on 9.0 % of pixels (indexed path exact) | #1708 | open; bisected to `3134e3134` (GPU virtual-crossing walk, OpenMC #3934 mirror); three fixes tried and ruled out (recorded on the issue). GPU plotting only; transport is on the CPU. Maintainer: publish with it red. |
| wave 2 (12 crates) | re-test at `16dea0ebd` on the release host incomplete at publish time (all 12 live on crates.io 2026-11-23) | — | see "Test evidence"; results to be added here |

## Fixed during the release (all on develop)

| What | Commit | Issue |
|---|---|---|
| punggol uses PETIR maths in every build; petir's no-platform-libm guard green | `447b47b71` | #1706 |
| kembangan added to kovan's backronym pin; citeproc-js reference regenerated (bib 264 -> 267) | `bfe1ec45e` | — |
| six-factor reader: leakage keeps the per-history divisor after `normalise_tallies` became default | `bfe1ec45e` | #675 |
| shielded-room weight-window test asks for raw sums | `bfe1ec45e` | #675 |
| `publish.sh` gains `HOLD` for a partial lockstep | `23ef63a0b` | — |
| punggol ships its LICENSE | `5b3bed31b` | — |
| outram-mc-libs packages the LCT-008 XML that `src/vv/lct008.rs` embeds | `c3d900d97` | — |
| dhoby-ghaut packages the five-route summary it embeds (copy + drift test) | `fafb896f3` | — |
| nee_soon pebble test made like for like | `46036ef9e`, `21a4c1ce6` | #1707 (closed) |
| mesh-surface tallies: port of OpenMC `raytrace_mesh` / `r_last_current`; collision gates use the registered 8-seed pooled replication | `7fb1cec0c`, `d1e207f18` | #1709 (closed) |

## Versions

| Crate | Version | Wave |
|---|---|---|
| `bedok` | 0.0.4 | 1 (2026-11-22) |
| `bishan` | 0.0.4 | 1 (2026-11-22) |
| `boon-lay` | 0.2.3 | 2 (2026-11-23) |
| `buangkok` | 0.0.3 | 2 (2026-11-23) |
| `bugis` | 0.0.2 | 1 (2026-11-22) |
| `changi` | 0.0.4 | 1 (2026-11-22) |
| `chem-eng-real-time-process-control-simulator` | 0.2.6 | 1 (2026-11-22) |
| `deck-based-open-source-visualisation-engine-for-reactors-dover` | 0.0.3 | 1 (2026-11-22) |
| `dhoby-ghaut` | 0.0.3 | 2 (2026-11-23) |
| `farrer-park` | 0.0.5 | 1 (2026-11-22) |
| `kaki-bukit` | 0.0.4 | 1 (2026-11-22) |
| `kembangan` | 0.0.1 | 2 (2026-11-23) |
| `knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan` | 0.0.5 | 1 (2026-11-22) |
| `kovan-codegen` | 0.0.6 | 1 (2026-11-22) |
| `kovan-common` | 0.0.6 | 1 (2026-11-22) |
| `kovan-discovery` | 0.0.6 | 1 (2026-11-22) |
| `kovan-literature` | 0.0.5 | 1 (2026-11-22) |
| `kovan-metrics` | 0.0.5 | 1 (2026-11-22) |
| `kovan-semantics` | 0.0.5 | 1 (2026-11-22) |
| `kovan-web` | 0.0.5 | 1 (2026-11-22) |
| `nee_soon` | 0.0.6 | 2 (2026-11-23) |
| `njoy-outram-park-fork` | 0.0.8 | 1 (2026-11-22) |
| `outram-blender` | 0.0.8 | 1 (2026-11-22) |
| `outram-foam-appbuilder-lib` | 0.1.8 | 1 (2026-11-22) |
| `outram-foam-basic-lib` | 0.1.9 | 1 (2026-11-22) |
| `outram-foam-cli` | 0.0.3 | 1 (2026-11-22) |
| `outram-foam-mesh` | 0.1.4 | 1 (2026-11-22) |
| `outram-foam-multiphase` | 0.0.6 | 1 (2026-11-22) |
| `outram-foam-turbulence-lib` | 0.1.6 | 1 (2026-11-22) |
| `outram-mc-libs` | 0.0.8 | 2 (2026-11-23) |
| `outram-park-digital-twin-engine` | 0.0.5 | 2 (2026-11-23) |
| `outram-park-fork-cfmesh` | 0.0.5 | 1 (2026-11-22) |
| `outram-park-fork-coolprop` | 0.1.5 | 1 (2026-11-22) |
| `outram-park-fork-dwsim-libs` | 0.0.6 | 1 (2026-11-22) |
| `outram-park-fork-liggghts` | 0.0.4 | 2 (2026-11-23) |
| `outram-park-fork-moltres` | 0.0.3 | 1 (2026-11-22) |
| `outram-park-fork-offbeat` | 0.0.3 | 1 (2026-11-22) |
| `outram-park-fork-onix` | 0.0.3 | 1 (2026-11-22) |
| `outram-park-fork-pflotran` | 0.0.3 | 1 (2026-11-22) |
| `outram-park-fork-thermochimica` | 0.0.3 | 1 (2026-11-22) |
| `outram-park-mpi` | 0.0.3 | 1 (2026-11-22) |
| `pasir-ris` | 0.0.3 | 2 (2026-11-23) |
| `petir` | 0.0.5 | 1 (2026-11-22) |
| `punggol` | 0.0.1 | 2 (2026-11-23) |
| `raffles` | 0.0.5 | 1 (2026-11-22) |
| `redhill` | 0.0.4 | 1 (2026-11-22) |
| `sembawang` | 0.0.3 | 2 (2026-11-23) |
| `tampines` | 0.0.5 | 2 (2026-11-23) |
| `tampines-steam-tables` | 0.3.2 | 1 (2026-11-22) |
| `teh-o-prke` | 0.1.9 | 1 (2026-11-22) |
| `tuas_boussinesq_solver` | 0.1.10 | 1 (2026-11-22) |
