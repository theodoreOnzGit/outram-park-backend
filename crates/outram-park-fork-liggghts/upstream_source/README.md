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
(`reference-data/liggghts/make_multisphere_decks.sh`). The same binary also
ran the second-pass decks `in.ms_ke`, `in.ms_flags`, `in.ms_orient` and
`in.ms_lost` on 2026-11-14. `compute ke/multisphere` and
`erotate/multisphere` are in its default build.

**Superquadric package (gh:#1415, checked 2026-11-13).** The default
`make serial` build does **not** compile superquadrics. `atom_vec_superquadric.cpp`,
`superquadric.cpp`, `fix_nve_superquadric.cpp`, `fix_template_superquadric.cpp`,
`math_extra_liggghts_superquadric.cpp` and the superquadric branches of
`fix_wall_gran.cpp`, `pair_gran_base.h` and the contact models all sit behind
`SUPERQUADRIC_ACTIVE_FLAG`: `lmp_serial -h` lists the `superquadric` atom style
(an empty shell) but no `nve/superquadric` fix. The references for
`in.sq_drop` and `in.sq_collide`
(`reference-data/liggghts/make_superquadric_decks.sh`) came from a separate
scratch build, so the sphere references above are untouched:

```bash
cp -a upstream_source/LIGGGHTS-PUBLIC/src upstream_source/LIGGGHTS-PUBLIC/lib <scratch>/
cd <scratch>/src                       # dump_custom.cpp already carries the %.17g patch
rm -f Obj_serial/*.o Obj_serial/*.d    # every object must see the flags
make -j3 serial LMP_INC="-DSUPERQUADRIC_ACTIVE_FLAG -DNONSPHERICAL_ACTIVE_FLAG"
```

The `rm` matters: with the objects of the default build left in
`Obj_serial`, make relinks them without the flags. g++ 13.3 (C++17) provides
`std::beta` (`__STDCPP_MATH_SPEC_FUNCS__ = 201003`), so boost is not needed.
The build takes about 10 minutes on 3 cores, and the result lists
`nve/superquadric`, `particletemplate/superquadric` and `erotate/superquadric`.
The scratch copy was deleted once the references were committed.

Rebuilt the same way on 2026-11-13 for the second superquadric pass
(gh:#1415: `in.sq_cyl`, `in.sq_mesh`, `in.sq_ridge`, `in.sq_curv_mean`,
`in.sq_curv_gauss`, `in.sq_pile_check`, `in.sq_pile_every`). As a check of the
recipe, the rebuilt binary reproduced the committed `sq_drop.csv` byte for
byte. The scratch copy was deleted again.

Rebuilt the same way a third time on 2026-11-13 for superquadric insertion
(gh:#1415: `in.sq_pour`, `in.sq_pour_mesh`, `in.sq_cdt`, `in.sq_cdt_curv`). The build lists
`particletemplate/superquadric`, and `fix insert` compiles its `check_obb`
default (1) under the same flag. With glibc maths, the Rust port reproduced
every value of the two new decks, and of every earlier `sq_*.csv` deck through
the path it pins. The default-list pile, which is not upstream's list, stayed
at 624 / 1560, as recorded in V&V 4.17.1. The scratch copy was deleted afterwards.
