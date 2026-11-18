# teh-o-prke

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

Point Reactor Kinetics Equations Module for the Teh-O package

Teh-O is the Transport, Eigenvalue and Hybrid Open Source Solver. It is meant to 
sound like Teh-O from Southeast Asia (Singapore Specifically).

## `no_std` + `alloc` (gh:#1600)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p teh-o-prke --lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). Every model builds without it: the
zero-power and n-group PRKEs, the six-group solvers (`ndarray` without its
`std` feature), the feedback mechanisms, decay heat, the point reactor, the
Nordheim-Fuchs stepper (including its first-order actuator lag, through
`chem-eng-real-time-process-control-simulator`, itself `no_std` since
gh:#1612) and the delayed-neutron layer. `std` adds only what the crate's
dependencies carry under theirs (chem-eng's CSV helpers); the library itself
prints and reads clocks only in tests. With `std` on (the default) nothing
changes.

## FHR Educational Simulator

To showcase teh-o-prke, an FHR educational simulator was constructed.
This includes PRKE with feedback for:

1. delayed neutron precursor
2. two control rod banks
3. fuel temperature feedback

This also includes accounting for:
1. decay heat

More features to be added in future...


```sh
cargo run --example fhr_sim_v1 --release
```



Please remember to run the client AFTER the server.

# prerequisites

You'll need openblas to run this on linux.

# licensing 

The point reactor kinetics code here copies some of the time stepping 
algorithm source files available in OpenFOAM. These are licensed files  
are available under GPLv3. The source files in Rust directly translte 
these source files. To respect OpenFOAM copyright, the PRKE files here 
are also released under GPLv3 (~~GPLv3~~ the crate is **AGPL-3.0-only** as a combined work, ported files keep GPL-3.0; relicensed 2026-10-20, gh:#973).

**The dense LU matrix solver (`src/matrix.rs`) is also from OpenFOAM** —
specifically, it is an inlined copy of `outram-foam-basic-lib`'s
`matrix::SquareMatrix` (itself an OpenFOAM translation), copied in directly
rather than kept as a path dependency so that the inter-crate dependency
graph stays acyclic (a future `tampines`/`nee_soon` composition can pull in
`teh-o-prke` and `tuas_boussinesq_solver` together without a dependency
loop). See the header of `src/matrix.rs` for the full attribution.


## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
