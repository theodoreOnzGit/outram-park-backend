# upstream_source — LIGGGHTS-PUBLIC (gitignored clone, never committed)

`upstream_source/LIGGGHTS-PUBLIC/` is a **local, gitignored** clone of the
upstream this crate ports. Only this README is committed (`.gitignore`:
`/upstream_source/*/`).

| | |
|---|---|
| Upstream | LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz) |
| URL | `https://github.com/CFDEMproject/LIGGGHTS-PUBLIC` |
| Commit | `3d5c00f20519e6bb6eb6756f51f1ad36564e649d` ("Remove logos", 2024-06-07) |
| Accessed | 2026-11-10 (shallow clone, `--depth 1`) |
| Licence | GNU GPL version 2 (`LICENSE`; sources say "version 2 or later"), used here under the "or later" option as GPL-3.0-compatible, as `NOTICE` records |

## Build recipe (reproduces `reference-data/liggghts/` byte for byte)

```bash
git clone --depth 1 https://github.com/CFDEMproject/LIGGGHTS-PUBLIC.git upstream_source/LIGGGHTS-PUBLIC
cd upstream_source/LIGGGHTS-PUBLIC/src
# output-precision patch (reference-data/liggghts/README.md): dump_custom.cpp
sed -i '165s/3\*size_one+1/8*size_one+1/; 170s/"%g "/"%.17g "/' dump_custom.cpp
(cd STUBS && make)          # MPI stubs
make -j1 serial             # g++ -O2 -fPIC, ~40 min on 1 core; produces src/lmp_serial
```

Verified 2026-11-10: re-running `in.oblique_hertz`, `in.oblique_nohist` and
`in.headon_hertz` with this build and converting with `dump2traj.sh` gives
files **byte-identical** to the committed `oblique_hertz.csv`,
`oblique_nohist.csv` and `headon_hertz.csv`.

**Multisphere package (gh:#1415, checked 2026-11-13).** The default
`make serial` build above already compiles the multisphere sources:
`fix_multisphere*.cpp`, `multisphere*.cpp`, `fix_template_multisphere.cpp`,
`fix_template_multiplespheres.cpp`, `input_multisphere.cpp` and
`particleToInsert_multisphere.cpp`. `lmp_serial -h` lists `multisphere`,
`particletemplate/multisphere` and `particletemplate/multiplespheres` among
its fix styles. No package switch and no rebuild are needed for `in.ms_drop`,
`in.ms_collide` and `in.ms_massflow`
(`reference-data/liggghts/make_multisphere_decks.sh`).
