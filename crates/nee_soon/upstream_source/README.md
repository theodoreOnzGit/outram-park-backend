# Upstream source

**N/A — original work.** `nee_soon` is the workspace's own
integration/coupling layer (composing Monte Carlo transport, deterministic/TH,
and nuclear data; PRKE + surrogates), specific to OUTRAM PARK's own
architecture (see the workspace-root `docs/architecture.md`). It has no
single upstream repository it forks or translates — it depends on, and
coordinates, other in-workspace crates (`teh-o-prke`, `openmc-libs`,
`njoy-outram-park-fork`, `openfoam-appbuilder-lib`), each of which has its own
`upstream_source/README.md` where relevant.
