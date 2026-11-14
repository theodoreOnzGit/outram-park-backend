# Predictions: Rust generator -> Rust viewFactor solve -> qr vs ESI (gh:#1541, under gh:#1529 / #1540)

Written and committed 2026-11-14 BEFORE the test `tests/view_factor_chain_vs_openfoam.rs` was first run.
Research, education and V&V only.

Setup. For each closed case the Rust chain starts from the upstream polyMesh only
(`reference/vfgen/<case>/{points,faces,boundary}.gz`): `generate_view_factors` builds F and
globalFaceFaces, `|Sf|` comes from the Rust face area, `ViewFactorModel::solve` gives qr.
Wall T, emissivity and qro are the case INPUTS (T from `vfDumpDict`, emissivity from
`boundaryRadiationProperties`, qro from `0/qr`; read from the upstream dump, not tuned).
The reference is upstream's `qr1` (call 1) and `qr2` (call 2, T x 1.1) in `reference/ref_<case>.txt.gz`.
The chain is NOT given upstream's F, globalFaceFaces or magSf.

1. cubeA, cubeC (constantEmissivity), cubeD (qro = 0; same mesh as cubeA), plate4/8/16, boxE:
   qr bit-identical to ESI on 100 % of faces in BOTH calls. Reason: F and globalFaceFaces are
   byte-identical (#1540) and the solve is bit-identical given F (#1529). Only open item: the Rust
   `|Sf|` must equal upstream `mesh.magSf()` bit for bit; I expect yes (probability ~85 %). If the
   area differs, the miss will be at the 1e-16 level and I will report it, not patch around it.
   Face counts: 96 + 96 + 96 + 3 x 160 + 62 = 830 faces, x 2 calls = 1,660 values.
2. lshape (non-convex): F has 8 pair differences (#1540) between faces in a common plane, F at
   rounding-noise level (ASSUMPTION later found wrong, see README). Prediction: qr is NOT bit-identical on every face (probability ~70 % that
   fewer than 100 % match), but the worst relative difference is < 1e-10. No gate on bit-identity
   for lshape; the measured numbers are recorded and the worst difference is gated at 1e-8.
3. A mismatch in 1 would be a defect in the chain glue (patch order, area), not in either port. I will
   localise it before touching anything.
