// High-level geometry routines: particle location and boundary crossing.
//
// C++ source: `src/geometry.cpp` (495 LOC), `include/openmc/geometry.h` (82 LOC).
// Also `src/geometry_aux.cpp` — overlap checks, boundary conditions.
//
// These are the two innermost loops of the transport algorithm:
//   1. `locate_particle(r, u)` — descend the universe hierarchy to find the
//      leaf cell and material at position `r`.
//   2. `distance_to_boundary(particle)` — find the nearest surface crossing
//      and return the distance, the surface crossed, and the next cell.
//
// TODO: implement locate_particle and distance_to_boundary once
// Cell, Universe, and Lattice are fully ported.
