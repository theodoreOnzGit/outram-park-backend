# faceAreaWeightAMI vs OpenFOAM ESI (gh:#1859)

Research, education and V&V only (`RESPONSIBLE_USE.md`). Date of every number below:
2026-11-27. Upstream: OpenFOAM ESI, `gitlab.com/openfoam/core/openfoam` commit
`6e2595746a396d688edd9890f4aebaaa7ff4ecfd` (`OPENFOAM=2606`), built serial with dummy Pstream,
g++ 13 `-O3` (no `-march`, so no FMA), double precision, 32-bit labels. Rust 1.99.1, release.

## What is compared

* **Port:** `outram_foam_basic_lib::mesh::face_area_weight_ami::face_area_weight_ami`, a
  line-for-line port of `faceAreaWeightAMI::calculate` (serial path), `advancingFrontAMI`,
  `faceAreaIntersect` and `AMIInterpolation::normaliseWeights`, with the octree seed search
  replaced by a brute-force nearest search (lowest index on an exact tie).
* **Driver:** `driver/amiDump.C` calls only unmodified upstream code: a `faceAreaWeightAMI`
  built from a dictionary (`requireMatch`, `reverseTarget`, `lowWeightCorrection`, `triMode`,
  `restartUncoveredSourceFace`, `maxDistance2`, `minCosAngle`, `areaNormalisationMode`), then
  `calculate(srcPatch, tgtPatch)`. It generates each case's geometry and writes it as IEEE-754
  bit patterns beside the outputs, so both codes run on the same input bits.
* **Reference:** `reference/ami_esi_6e25957.txt` (17 cases).
* **Test:** `tests/ami_face_area_weight_vs_openfoam.rs` compares `srcAddress`, `tgtAddress`
  and `srcNonOverlap` exactly (same entries, **same order**) and every `srcWeights`,
  `srcWeightsSum`, `srcMagSf`, `srcCentroids`, `tgtWeights`, `tgtWeightsSum`, `tgtMagSf`
  value bit for bit. Criterion: bit-identity; it fails on any value off by more than 1e-9
  relative and on any non-identical value.

Reproduce: `scripts/build-upstream.sh <esi tree>` (about 20 min), then
`scripts/run.sh <esi tree>`, then
`cargo test --release -p outram-foam-basic-lib --test ami_face_area_weight_vs_openfoam -- --nocapture`.

## Cases

| case | what it exercises |
|---|---|
| `planar_identity` | non-uniform 4x3 grid against itself reversed: identity |
| `planar_identity_reverse_target_fan` | same winding, `reverseTarget`, `tmFan`, `requireMatch false` |
| `strips_2to1`, `strips_partial` | the 1-D cases `overlap_weights_1d` also covers |
| `unstructured_{mesh,fan}_{project,mag}` | perturbed 5x4 vs perturbed 3x3 grid with one hexagon (two collinear vertices): `face::split` general case, both normalisations |
| `unstructured_reversed_target_order` | target face list reversed: source 0 misses target 0, `setNextFaces` nearest-face search, restart from vertices (`srcNonOverlap = [0]`) |
| `unstructured_reversed_no_restart` | as above with `restartUncoveredSourceFace false`: source 0 never addressed |
| `concave_l_{mesh,fan}` | a concave L-shaped hexagon (concave branch of `mostConcaveAngle`) |
| `cylinder_24x3_36x5` | curved seam, 24x3 vs 36x5 facets, target rotated 0.013 rad |
| `cylinder_coincident_lines` | 24x3 vs 36x4, vertex lines coincide every 30 deg (coplanarity snap) |
| `cylinder_min_cos_angle` | `minCosAngle 0.99` rejects pairs |
| `cylinder_identity` | 16x2 against itself reversed |
| `off_seam_low_weight` | a source face 1 m off the seam, `maxDistance2 0.01`, `lowWeightCorrection 0.2` |

## Result (measured 2026-11-27)

**8579 of 8579 values bit-identical in 17 cases, worst relative difference 0; every address
list equal, in the same order.** Per case:

| case | identical / values |
|---|---|
| planar_identity | 108 / 108 |
| planar_identity_reverse_target_fan | 108 / 108 |
| strips_2to1 | 32 / 32 |
| strips_partial | 37 / 37 |
| unstructured_mesh_project | 266 / 266 |
| unstructured_mesh_mag | 266 / 266 |
| unstructured_fan_project | 266 / 266 |
| unstructured_fan_mag | 266 / 266 |
| unstructured_reversed_target_order | 266 / 266 |
| unstructured_reversed_no_restart | 261 / 261 |
| concave_l_mesh | 82 / 82 |
| concave_l_fan | 82 / 82 |
| cylinder_24x3_36x5 | 2604 / 2604 |
| cylinder_coincident_lines | 1872 / 1872 |
| cylinder_min_cos_angle | 1764 / 1764 |
| cylinder_identity | 288 / 288 |
| off_seam_low_weight | 11 / 11 |

Interpretation: on these cases the port computes what upstream computes, operation for
operation (`acos` and `cos` come from PETIR's `libm` port in Rust and glibc in C++, and agreed
on every value that reached an output). The brute-force seed search returned the octree's face
in every search here, including the restart searches from source vertices; a case with an exact
distance tie between two target faces could still order an address list differently (module
docs). **Not covered:** parallel/distributed AMI, `lowWeightCorrection` at interpolation time,
projection onto a surface, and any geometry beyond these 17 cases.
