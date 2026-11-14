# scripts/upstream: rebuild the upstream reference builds with one command

**Research, education and V&V only** (`RESPONSIBLE_USE.md`). AI-written, not
human-reviewed. Only open-source upstream code is fetched (OpenFOAM and GeN-Foam,
both GPL-3.0).

The upstream-parity work (epic gh:#1336) compares ports against OpenFOAM ESI
v2506 and GeN-Foam 652b3da0cc55. Both live outside the repo in
`/home/user/upstream-build`, which a container reset wipes. One command
rebuilds the subset the V&V records used, at the **same paths**:

```bash
scripts/upstream/build-esi-v2506-and-genfoam.sh          # everything, resumable
scripts/upstream/build-esi-v2506-and-genfoam.sh status   # what is done, timings, disk
scripts/upstream/build-esi-v2506-and-genfoam.sh closure  # or any single stage
```

It re-runs itself under `nice -n 10`, compiles with `WM_NCOMPPROCS=2` (override
with `WM_NCOMPPROCS=3 ...`), stamps every finished library under
`/home/user/upstream-build/.stamps` and skips stamped work, so an interrupted run
continues where it stopped and a finished tree re-runs in about a second. Logs:
`/home/user/upstream-build/logs/<stage>.log`, per-stage seconds in
`logs/timings.tsv`. `FORCE=1 <stage>` redoes a stage; `KEEP_OBJECTS=1` keeps
object files (default deletes them after each link, as the earlier build did);
the run aborts if `/` has less than `MIN_FREE_GB` (default 5) free.

## What it builds (paths as before the reset)

| stage | result |
|---|---|
| `prereqs` | installs `flex`/`libfl-dev`/`zlib1g-dev` if missing (as root, `apt-get`); flex is the usual absentee |
| `fetch-esi` | `OpenFOAM-v2506.tgz` into `/home/user/upstream-build/dl`, extracted to `/home/user/upstream-build/OpenFOAM-v2506` (`api=2506 patch=0`) |
| `env` | `/home/user/upstream-build/scripts/with-of.sh` and `of-env.sh` (dummy Pstream, `WM_COMPILE_OPTION=Opt`, `-O3`, double precision, 32-bit labels, `FOAM_USER_*` = `/root/OpenFOAM/user-v2506`, `gh1363/bin` on `PATH`) |
| `core` | wmake tools, `OpenFOAM`, `fileFormats`, `surfMesh`, `meshTools`, `finiteVolume`, `blockMesh` and `extrudeModel` libs, `dynamicMesh`; apps `blockMesh`, `checkMesh` |
| `prereq-libs` | `decompositionMethods`, `distributed`, `lagrangian/basic`, `sampling`, `decompose`, `reconstruct` |
| `closure` | the 19-library thermo / turbulence / fvOptions closure of `crates/outram-foam-appbuilder-lib/verification_and_validation/esi_thermo_turbulence_closure_build.md` |
| `meshing` | `fvMotionSolvers`, `libsnappyHexMesh`, apps `snappyHexMesh` and `surfaceFeatureExtract` |
| `solvers` | real `rhoCentralFoam`, `sonicFoam`, `rhoPimpleFoam`, `pimpleFoam` in `/home/user/upstream-build/gh1363/bin` (scratch copies with `thermoTools`, `topoChangerFvMesh`, `atmosphericModels`, `regionFaModels`, `finiteArea` pruned from the link line, diffs beside them in `gh1363/closure/scratch/<solver>/Make/*.diff`) |
| `dynmesh` | app `moveDynamicMesh` (`libdynamicFvMesh` and `libfvMotionSolvers` come from `solvers`/`meshing`); the `dumpMeshMotion` probe of gh:#1380 is built by `crates/outram-foam-mesh/verification_and_validation/dynamic_mesh_gh1380/run_esi.sh` |
| `fetch-genfoam` | GeN-Foam `652b3da0cc558543b89e880812d910bf122b356c` from `gitlab.com/foam-for-nuclear/GeN-Foam` (shallow fetch by SHA) into `/home/user/upstream-build/GeN-Foam-src` |
| `genfoam` | `libCommon`, `libMultiRegion`, `libNeutronicsSolvers`, `libFFSSrc2`, `libFFSSolvers2` and the `GeN-Foam` binary into `/root/OpenFOAM/user-v2506/platforms/linux64GccDPInt32Opt` |
| `fetch-cfmesh`, `cfmesh` | cfMesh `e944f6ba695f` (`DAFoam/cfmesh`, cloned over git into the cfMesh crate's gitignored `upstream_source/cfmesh_dafoam`, override with `CF_SRC`) with the committed three-part v2506 API patch (`crates/outram-park-fork-cfmesh/verification_and_validation/upstream_cfmesh_gh1385/patches/cfmesh-e944f6ba695f-openfoam-v2506.patch`, applied idempotently), `libmeshLibrary` and the executables `cartesianMesh`, `cartesian2DMesh`, `tetMesh`, `pMesh` into the OpenFOAM tree's `platforms/` (`FOAM_LIBBIN`, `FOAM_APPBIN`). cfMesh was built against ESI v2506 before the reset (not OpenFOAM-dev), so no other OpenFOAM is needed. Run its binaries with `OMP_NUM_THREADS=1` (`with-of.sh` sets it): the library compiles with `-fopenmp` |
| `verify` | laminar `pimpleFoam` cavity (`blockMesh` then 20 steps, must reach `End`), `-help` of `snappyHexMesh`, `surfaceFeatureExtract`, `checkMesh`, `GeN-Foam`, `sonicFoam`, `rhoPimpleFoam`, `rhoCentralFoam` |

Edits to upstream sources are `Make/options` and `Make/files` only, never a
`.C` or `.H`; each is derived from a kept `.orig` copy (so a re-run never
edits an edit) with a `.diff` beside it: the `snappyHexMesh` application loses
its dummy kahip/metis/scotch link line; the solver copies lose the link
libraries above; GeN-Foam's link lines lose `thermoTools`, `genericPatchFields`,
`ThermoMechanicsSolvers`, `offbeat*`, `sciantix`, `OFSolvers` (not built), and
`FFSSrc2` loses `-g` (debug info only). The `dumps` drivers of
`upstream_openfoam_gh1363` and the `gh1361` scripts build themselves against
this tree as before; they are not run by this script.

## Cost (measured 2026-11-12/13, 4 cores shared with a cargo gate job and another agent)

| item | measured |
|---|---|
| total compile time at `WM_NCOMPPROCS=2`, `nice 10` | 110 minutes of stage time (sum of `logs/timings.tsv`), about 1 h 50 min wall including one failed attempt; load average 3 to 7 |
| largest stages | `finiteVolume` 36 min, `OpenFOAM` 13 min, `meshTools` 8 min, `libFFSSrc2` 8 min; the 19-library closure about 25 min |
| disk, final | `/home/user/upstream-build` 1.1 GB (OpenFOAM tree 450 MB of which `platforms/` 184 MB, GeN-Foam source 561 MB, tarball 52 MB, scratch and logs 8 MB) plus 13 MB of GeN-Foam libraries and binary under `/root/OpenFOAM` |
| disk, peak | free space on `/` fell from 20 GB to 16 GB; objects are deleted after each link, so the transient peak is one library (finiteVolume, a few hundred MB) |

## Sources and mirrors

The official hosts `dl.openfoam.com` and `develop.openfoam.com` were **denied
by the agent network proxy** (HTTP 403 on CONNECT) on 2026-11-12, and a GitHub
archive URL for GeN-Foam also returned 403 (GeN-Foam is fetched from GitLab
instead: the git endpoint works, its web archive URL returned 429);
`dl.openfoam.com` is still tried first. The script falls back to SourceForge
(`sourceforge.net/projects/openfoam/files/v2506/OpenFOAM-v2506.tgz`, redirected
to a `*.dl.sourceforge.net` mirror), which worked. The tarball's sha256 is
pinned in the script (`63d26f48...`) from that copy; a different hash warns, and
the tree must still report `api=2506 patch=0`. `ftp.gnu.org` was also denied
on the same day; nothing here needs it. No `ThirdParty` tree is needed (dummy
Pstream, no scotch/metis/MPI/CGAL).

## Not rebuilt (not requested, not needed by the records above)

`OpenFOAM-dev` and `of-dev-env.sh`, the cfMesh build (`crates/outram-park-fork-cfmesh/.../upstream_cfmesh_gh1385/scripts/build-cfmesh.sh`
and `build-of.sh` still exist), `polyDualMesh-dev`, the `ops1362` build, and every
`gh136x` run directory and driver binary (those are produced by their own
scripts). Not built at all, as before: `thermoTools`, `finiteArea`,
`regionFaModels`, `genericPatchFields`, `offbeat`, `sciantix`, thermo-mechanics.

## Verification evidence from the rebuild

`verify` passes (above). In addition, the rebuilt GeN-Foam binary ran
`Tutorials/guidedCases/1_reactorSlab_1D_1Gr_neutronicDiffusion` as shipped and
printed `keff_ = 1.0000037`, the value the gh:#1361 record
(`genfoam_vs_upstream_gh1361/README.md`, prediction S0) gives for the shipped
case, which is an independent check that the rebuilt binary is the one the
records were measured with. Bit-identity of every record number was **not**
re-checked; a new gcc or a different sourceforge snapshot could in principle
change round-off. Re-run the relevant `run_*.sh` before trusting a number.

## Bookkeeping status

| axis | state |
|---|---|
| implemented / tested | [ ] |
| human-reviewed | [ ] |
