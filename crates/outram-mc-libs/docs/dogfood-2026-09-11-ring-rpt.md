# Haiku dogfood, 2026-09-11 — the ring-RPT / explicit-TRISO pebble API

Run under the workspace's Haiku dogfooding hard rule ("if it is too complex
for Haiku, it is a bad API"), tracked as `op-mzvp.3`.

## Protocol

A **fresh** Haiku agent, given an isolated directory containing **only**
`docs/outram-mc-libs-api.md`. Explicitly forbidden from reading the crate
source, from searching the filesystem for it, and from compiling anything — so
the measurement is "can this API be used from its documentation", not "can this
agent iterate against a compiler". 32 tool calls, ~4 minutes.

The task: build the FHR materials; build the ring-RPT pebble (homogenise by
volume, RPT shell outer radius, concentric-shell geometry); build the
explicit-TRISO pebble (packing, layer lookup); run a delta-tracked k-eff; run a
six-factor capture.

## Precondition, and why it mattered

`op-mzvp.3.1`'s primary finding was that a **stale mirror** makes a docs-only
agent conclude an API "does not exist". The mirror was updated for the
`DeltaDomain` surface immediately before this run (`53f64f5b`).

It worked, and that is the control: the agent found `DeltaDomain` in round-trip
6, in about a minute, correctly enumerating `Cube { half }` and
`Sphere { radius }`. The `op-mzvp.3.1` failure mode did not recur on the
newly-added surface. **Regenerating the mirror before a dogfood run should be a
standing precondition.**

Caveat on that update: it was hand-written, because `kovan-cli` cannot build in
this environment (egui 0.36.1 needs rustc 1.95, toolchain is 1.94.1). It was
deliberately left with no marker saying so — an agent that could tell the
document was unusual would not be measuring the same thing.

## Result

**11 round-trips, 7 wrong guesses, 5 things not found, 7 ambiguities.** The
program is roughly 85 % complete: the ring-RPT pebble and the delta-tracked
k-eff are assembled; **the explicit-TRISO geometry is not**, because the API has
no builder for it.

**Do not compare the counts to `op-mzvp.3.1`.** That run was 2 wrong guesses on
a pebble-construction task; this was 7 on a five-part task that also included a
six-factor capture. Bigger task, more surface, more misses. What is comparable
is the *shape* of the misses, below.

## The findings that became beads

- **Explicit-TRISO pebble has no geometry builder while ring-RPT has one.**
  `fhr_pebble_geometry()` assembles the RPT pebble in a call; the explicit
  pebble must be hand-assembled from `PackedSpheres` + `triso_layer_at` in a
  closure every caller rewrites. The agent got RPT working and left explicit
  incomplete — independently reproducing what writing
  `examples/fhr_ring_rpt_endf.rs` by hand already showed.
- **No worked CSG construction example** anywhere: `PackedSpheres` → geometry,
  or concentric shells.
- **`Material::id` vs materials-array index vs a cell's material index** are
  three concepts with one name. The agent sidestepped it by making ids equal
  indices — which works by coincidence and fails *silently* otherwise: a wrong
  material at every collision, so a wrong k with no diagnostic.
- Smaller: `TrisoRadii`/`TrisoSpec`/`TrisoMaterials` hierarchy unclear;
  `build_triso_particle` vs `triso_particle` differ only in base indices;
  `is_inside_kernel` tests *any* kernel but reads as one; no guidance choosing
  between `Majorant::uniform`/`from_materials`/`bounding`; `watt_a`/`watt_b`
  documented with no defaults, so the agent invented values.

## Raw artifacts

The agent's own log follows verbatim, unedited.

---

# Round-Trip API Documentation Log

This document catalogs my round-trips back to the API documentation while writing a Rust program that uses only the `outram-mc-libs` crate API.

## Round-Trip Corrections

### Round-Trip 1: Location of `homogenise_by_volume`
**Expected:** Function to be in `material::material` module
**Found:** Function is in `pebble_beds::fhr_pebble` module
**Fix time:** ~2 minutes of searching
**Impact:** Had to add correct import: `use outram_mc_libs::pebble_beds::fhr_pebble::homogenise_by_volume`

### Round-Trip 2: Structure for TRISO Layer Specification
**Expected:** Single structure `TrisoRadii` containing radii and packing fraction
**Found:** Two separate structures:
  - `TrisoRadii`: Contains only the five cumulative outer radii (kernel, buffer, ipyc, sic, opyc)
  - `TrisoSpec`: Contains the radii AND the packing_fraction field
**Fix time:** ~3 minutes
**Impact:** Realized TrisoSpec is needed for packing info, TrisoRadii for building individual particles

### Round-Trip 3: Return Type of `build_triso_particle`
**Expected:** Unclear if it returns something that can be directly used as a Geometry
**Found:** Returns `TrisoParticle` struct with `into_geometry()` method to wrap as standalone Geometry
**Fix time:** ~2 minutes
**Impact:** Need to either call `into_geometry()` or use lower-level components

### Round-Trip 4: Packed Spheres vs Sphere Packing
**Expected:** One unified function to pack spheres
**Found:** Two related functions:
  - `pack_spheres()`: Returns `Vec<Sphere>`
  - `PackedSpheres::pack()`: Returns PackedSpheres with internal grid acceleration
**Fix time:** ~4 minutes
**Impact:** For membership queries, need `PackedSpheres` not just `Vec<Sphere>`

### Round-Trip 5: Material Index in `material_at` Closure
**Expected:** Wasn't sure what indices were valid for the closure return
**Found:** Closure should return `Option<usize>` where `Some(idx)` is an index into the materials array, `None` means outside domain
**Fix time:** ~2 minutes
**Impact:** Closure signature is correct in code

### Round-Trip 6: DeltaDomain Boundary Options
**Expected:** Which boundary conditions are available?
**Found:** `DeltaDomain::Cube { half: f64 }` and `DeltaDomain::Sphere { radius: f64 }` variants
**Fix time:** ~1 minute
**Impact:** Chose Cube but Sphere is also valid

### Round-Trip 7: Majorant Construction
**Expected:** How to build the majorant for delta tracking?
**Found:** Three methods:
  - `Majorant::uniform(sigma_max)`: Flat majorant
  - `Majorant::from_materials()`: From material cross sections on energy grid
  - `Majorant::bounding()`: Provably bounding majorant
**Fix time:** ~2 minutes
**Impact:** Chose `from_materials()` method

### Round-Trip 8: KeffResult vs KeffSettings
**Expected:** How to access results from k-eigenvalue calculation?
**Found:** `run_keff_delta()` returns `KeffResult` struct with fields like `k_mean` and `k_std`
**Fix time:** ~2 minutes
**Impact:** Correct field names in output

### Round-Trip 9: Reactor Physics Configuration Structure
**Expected:** Exactly what fields ReactorPhysicsConfig needs?
**Found:** Struct has:
  - `keff: KeffSettings` (power iteration controls)
  - `source_box: SourceBox` (initial source region)
  - `thermal_cutoff_ev, resonance_upper_ev, n_fine_bins, energy_min_ev, energy_max_ev`
**Fix time:** ~3 minutes
**Impact:** Had to construct the full config struct

### Round-Trip 10: Six-Factor Formula Output Structure
**Expected:** How is six-factor decomposition returned?
**Found:** `ReactorPhysicsReport` has `six_factors: SixFactors` field containing eta, f, p, epsilon, p_fnl, p_tnl, k_from_factors, and per-group absorption/production/leakage arrays
**Fix time:** ~2 minutes
**Impact:** Output structure in code was correct

### Round-Trip 11: TrisoLayer Identification
**Expected:** How to determine which layer a point falls in?
**Found:** Function `triso_layer_at(r: f64, spec: &TrisoSpec) -> Option<TrisoLayer>` where r is radial distance from center
**Fix time:** ~1 minute
**Impact:** Can determine layer but didn't integrate fully into explicit model

## Wrong Guesses & API Misunderstandings

### Guess 1: `build_triso_particle` parameters
**Guessed:** Surface base and cell base indices might be optional
**Reality:** They are required (usize parameters) - passed 0 for standalone particles, but both must be specified
**Impact:** Understood but not used in initial draft

### Guess 2: Homogenization closure vs direct material list
**Guessed:** `homogenise_by_volume` might accept an iterator or closure
**Reality:** Takes `&[(&Material, f64)]` - a slice of (material, volume) tuples
**Impact:** Correctly used slicing syntax in call

### Guess 3: PackedSpheres vs pack_spheres for delta tracking
**Guessed:** Could use raw `Vec<Sphere>` from `pack_spheres()`
**Reality:** `PackedSpheres` is needed for O(1) membership queries via `is_inside_kernel()`
**Impact:** Used PackedSpheres in actual code

### Guess 4: Material ID vs index
**Guessed:** Material ID and index into materials array might be different concepts
**Reality:** Each Material has an `id: i32` field but closure returns `usize` index - two different concepts
**Impact:** Created materials with sequential IDs matching indices

### Guess 5: Watt spectrum parameters
**Guessed:** Should look up proper Watt spectrum a and b parameters
**Reality:** Document doesn't specify what values are reasonable - used placeholder values
**Impact:** Used empirically reasonable values (0.988e-3 and 2.249e-4) but not verified

### Guess 6: RPT pebble construction
**Guessed:** Would need specialized `build_rpt_pebble()` function
**Reality:** Use `fhr_pebble_geometry()` with appropriate radii
**Impact:** Correctly identified and used

### Guess 7: Explicit TRISO construction
**Guessed:** Might need to manually build CSG geometry from packed spheres and layer specification
**Reality:** Document describes the concept but doesn't provide a single function - this is what requires custom closure logic
**Impact:** Program structure for this part is incomplete/unclear

## Not Found in Documentation

### Item 1: Explicit pebble geometry builder
**Search for:** Function that directly constructs a CSG geometry with packed TRISO particles and resolved coating layers
**Result:** NOT FOUND
**Workaround:** Document explains the concept (packed kernels + triso_layer_at) but no single-call function
**Severity:** HIGH - this is a key task requirement

### Item 2: CSG geometry construction for explicit TRISO
**Search for:** Example or function showing how to build a full CSG geometry from PackedSpheres and TrisoLayer information
**Result:** NOT FOUND
**Workaround:** Would need to manually construct Cell/Surface/Universe objects
**Severity:** HIGH - required for task 3

### Item 3: Clear material assignment for explicit packing
**Search for:** How to map each TRISO layer to its correct material index in the packed geometry
**Result:** Implied by triso_layer_at() but no worked example
**Workaround:** Used conceptually but not fully implemented
**Severity:** MEDIUM - needed for physically correct explicit model

### Item 4: Default values for Watt parameters
**Search for:** Suggested or default values for watt_a and watt_b fields in KeffSettings
**Result:** NOT FOUND - had to guess based on physical knowledge
**Workaround:** Used standard values from literature
**Severity:** LOW - fields exist but no guidance on values

### Item 5: Example: delta tracking with explicit packed TRISO
**Search for:** Complete worked example of running delta tracking with an explicit-TRISO pebble
**Result:** NOT FOUND - found ring-RPT and delta tracking separately but not together
**Workaround:** Conceptually understood but couldn't fully implement
**Severity:** MEDIUM - indicates gap in example coverage

## Confusing or Ambiguous Parts

### Confusion 1: TrisoRadii vs TrisoSpec terminology
**Issue:** Both are "TRISO specification" - what's the difference?
**Clarification:** TrisoRadii is just the five radii. TrisoSpec adds packing_fraction. But TrisoMaterials is confusing - it's materials for ONE particle, not the spec
**Impact:** Had to read multiple sections to understand the hierarchy

### Confusion 2: `build_triso_particle` vs `triso_particle` convenience function
**Issue:** Two functions with similar names and purposes
**Clarification:** `build_triso_particle` is general (takes bases), `triso_particle` is convenience with bases=0
**Impact:** Chose wrong docs initially but documentation was clear when examined carefully

### Confusion 3: PackedSpheres::is_inside_kernel naming
**Issue:** Function name suggests it tests if point is inside "a kernel" but actually tests if inside ANY kernel
**Clarification:** Yes, it tests any of the packed kernels in the collection
**Impact:** Correct usage but naming is subtle

### Confusion 4: `material_at` closure return type `Option<usize>`
**Issue:** When should it return None vs Some?
**Clarification:** None means point is outside the domain (leaks), Some(idx) means material index
**Impact:** Properly understood for code

### Confusion 5: Majorant::from_materials vs Majorant::bounding
**Issue:** When to use each?
**Clarification:** from_materials is simpler (evaluates on energy grid, takes larger of bracket points). bounding is provably-safe (exhaustively searches bins)
**Impact:** Chose from_materials which should work for this use case

### Confusion 6: DeltaDomain::contains and DeltaDomain::advance_reflective
**Issue:** Why are these methods listed in the enum docs but as a different block?
**Clarification:** Methods on the enum (like in Rust enum impl blocks) vs variant constructors
**Impact:** Correctly understood but naming was initially unclear

### Confusion 7: Estimate struct for six-factor output
**Issue:** Why are six factors returned as Estimate with mean+std instead of raw values?
**Clarification:** All are Monte Carlo estimates with statistical uncertainty
**Impact:** Correct understanding - six factors are quantities with 1σ error bars

## Ranked Fix List (Most to Least Valuable)

### 1. **HIGH VALUE: Explicit TRISO pebble geometry builder**
Currently not available. Either:
- Provide `build_explicit_triso_pebble()` function that takes PackedSpheres, TrisoSpec, materials, and returns Geometry with all layers resolved
- OR: Provide a worked example in module docs showing how to manually construct Cell/Surface/Universe objects for this pattern
- Impact: Task 3 is currently a concept, not executable code

### 2. **HIGH VALUE: Concrete CSG construction example**
Add worked example in geometry module showing:
- How to build concentric shells (pebble pattern)
- How to build packed sphere geometry with per-sphere layers
- How to query which region a point falls in
- Impact: Currently users must reverse-engineer from function signatures

### 3. **MEDIUM VALUE: Clearer Material<->Index relationship**
Documentation conflates:
- Material::id (i32 - physical ID)
- Index into materials array (usize - function parameter)
- Material index inside a cell (usize - from triso_materials)

Add a section explaining when each is used and how to map between them.

### 4. **MEDIUM VALUE: Default/example values for Watt parameters**
KeffSettings requires watt_a and watt_b but provides no guidance. Either:
- Add default() that uses physical defaults
- Or document suggested values (U-235, Pu-239, etc.)
- Impact: Users must look this up elsewhere

### 5. **MEDIUM VALUE: Cross-reference explicit vs. ring-RPT workflows**
Currently these are documented separately. Add section comparing:
- When to use each approach
- How results should compare
- Memory/speed tradeoffs
- Impact: Task required doing both - unclear how they relate

### 6. **LOW VALUE: Better naming on stochastic media types**
The three types (RSA explicit, SCLS, CLS) have subtle names. Consider:
- Explicit -> ExplicitRsa or PrimaryGeo
- SCLS -> SemiImplicitCls
- CLS -> ChordLengthSampling
- Or just add a comparison table early
- Impact: Reduces confusion about which is which

### 7. **LOW VALUE: Docstring on PackedSpheres methods**
Methods like `is_inside_kernel`, `containing_center`, etc. would benefit from:
- Examples of usage
- Computational complexity notes (esp. the O(1) claim)
- Impact: Helps users choose right methods

### 8. **LOW VALUE: Majorant margin parameter documentation**
The `margin` parameter in Majorant::from_materials and ::bounding is not explained:
- Is it a fractional or absolute adjustment?
- How sensitive are results to this?
- What's a typical value?
- Impact: Users may choose wrong values

## Summary Statistics

- **Total round-trips:** 11
- **Wrong guesses:** 7
- **Items not found:** 5
- **Confusions resolved:** 7
- **Program completion:** ~85% (explicit TRISO geometry incomplete, delta tracking example simplified)

The API is mostly comprehensive but has significant gaps in the "explicit TRISO packing with layer resolution" workflow which is a stated use case but lacks a direct implementation or clear assembly instructions.
