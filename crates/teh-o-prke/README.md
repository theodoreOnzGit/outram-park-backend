# teh-o-prke
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

