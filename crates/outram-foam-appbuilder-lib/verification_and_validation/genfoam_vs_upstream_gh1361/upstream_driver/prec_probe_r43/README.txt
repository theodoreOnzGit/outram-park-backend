gh:#1891 (README round 43): the instrumented upstream precEq.H and its build/run scripts.
Method: copy GeN-Foam-src/src/classes/{common,neutronics} to a scratch tree, replace neutronics/include/precEq.H by this file,
add `#include "primitiveMeshTools.H"` to neutronics/neutronics.H (after volFields.H) for the geometry re-computation line, build
libNeutronicsSolvers with FOAM_USER_LIBBIN pointing to a scratch lib dir (build.sh), and run the real GeN-Foam binary from the
t = 165 s transient case with LD_LIBRARY_PATH starting at that dir (run_probe2.sh). The shipped sources and libraries are untouched.
Outputs (working directory): r43dump_prec_<n>.txt (first 16 precEq calls), r43dump_prec_sol_<n>.txt, r43dump_geom.txt;
scripts/extract_prec_matrix_reference.py turns them into refs/msfr_prec_matrix_r43.ref.txt.gz.
