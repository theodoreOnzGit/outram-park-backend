// Main transport loop — one particle history.
//
// C++ source: `src/physics.cpp` (1249 LOC), `include/openmc/physics.h`.
//
// The transport loop for one particle:
//   1. Sample distance to next collision: d_col = −ln(ξ) / Σ_t(E)
//   2. Find distance to nearest geometry boundary: d_geom
//   3. If d_col < d_geom → stream to collision point; call `collision()`
//   4. Else → stream to boundary; cross surface or leak
//   5. Repeat until particle is dead (absorbed, leaked, or weight below cut-off)
//
// Key functions from `physics.cpp`:
//   - `transport_history_based(particle)` — outer loop
//   - `collision(particle)`               — sample reaction type, call scatter/fission/capture
//   - `distance_to_boundary(particle)`    — geometry query
//   - `cross_surface(particle)`           — apply boundary condition
//
// TODO: implement transport_history once Particle + geometry + material are ported.
