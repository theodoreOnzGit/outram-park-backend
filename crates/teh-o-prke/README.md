# teh-o-prke

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

Point Reactor Kinetics Equations Module for the Teh-O package

Teh-O is the Transport, Eigenvalue and Hybrid Open Source Solver. It is meant to 
sound like Teh-O from Southeast Asia (Singapore Specifically).

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
are also released under GPLv3.

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
