# Upstream source

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


**N/A — original work.** `nee_soon` is the workspace's own
integration/coupling layer (composing Monte Carlo transport, deterministic/TH,
and nuclear data; PRKE + surrogates), specific to OUTRAM PARK's own
architecture (see the workspace-root `docs/architecture.md`). It has no
single upstream repository it forks or translates — it depends on, and
coordinates, other in-workspace crates (`teh-o-prke`, `openmc-libs`,
`njoy-outram-park-fork`, `openfoam-appbuilder-lib`), each of which has its own
`upstream_source/README.md` where relevant.
