# GPU ray tracing of the workbench views (gh:#587)

Pictures from 2026-10-05, NVIDIA RTX A5000 (Vulkan). Research, education
and V&V only.

Methodology and the numbers are in the doc comment of
`crates/dhoby-ghaut/tests/gpu_csg_parity.rs` (HTR-10, the Şeker lattice bed
and a DEM-poured bed) and `crates/outram-blender/tests/gpu_csg_parity.rs`
(small nested models). In short: each view is drawn by the CPU plotter
(`f64`, the reference) and by the GPU tracer (`f32`) from the same plot
description, and the material each pixel shows is compared. Since the grid
index (second run, 2026-10-05) the GPU draws every view twice, with and
without the index.

| file | what it is | what was checked by eye |
|---|---|---|
| `htr10_bed_half_section_cpu.png` | Step 1's view, Şeker bed, CPU (redrawn in the second run, after the plotter's complement fix) | reference |
| `htr10_bed_half_section_gpu.png` | the same view, GPU with the grid index | same picture: reflector zones, borings, cavity, cut bed with TRISO in the pebble cut faces |
| `htr10_bed_half_section_diff.png` | white = pixels whose colour differs by more than 8 levels (4 495 of 504 000) | as in the first run: columns of pebbles cut by the plane y = 0, and the reflector's right silhouette edge |
| `dem_bed_half_section_cpu.png` | Step 1's view of a DEM-poured bed (27 554 pebbles of `reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`), CPU | reference |
| `dem_bed_half_section_gpu.png` | the same, GPU with the grid index | the random bed fills the core, conus and tube; reflector unchanged |
| `dem_bed_half_section_diff.png` | as above (4 228 pixels) | scattered along pebbles cut by y = 0 and the silhouette edge, no region |
| `dem_bed_xy_slice_mid_bed_gpu.png` | x-y slice at mid-bed of the DEM bed, GPU | random pebbles of varied cut size, TRISO in the fuel ones, helium between |
| `workbench_step1_dem_pour_half_gpu.png` | the running workbench, Step 1, a 27 000-pebble pour streaming (step 2 000), drawn as GPU sphere impostors ("1.2 ms to submit") | pebbles, half cutaway, vessel outline |
| `workbench_step1_dem_pour_whole_gpu.png` | the same pour, whole bed, after an orbit drag and the Whole button, during the pour (step 3 000) | the view followed the drag and the button while positions streamed |
| `workbench_step1_dem_pour_zoom_gpu.png` | zoomed six steps | spheres shaded and occluding each other correctly (depth buffer), dark rims |
| `workbench_step3_half_section_gpu_indexed.png` | Step 3, GPU with the grid index: status line "GPU ray-traced in 27 ms" (first run: 643 ms) | the same view as the first run's picture below |
| `workbench_step3_xray_gpu_indexed.png` | Step 3 in X-ray, GPU with the grid index: 117 ms (first run: 2.2 s) | outlines of the zones, borings and slots; attenuation shading |
| `workbench_step3_reflector_half_section_gpu.png` | first run: Step 3, GPU path ("GPU ray-traced in 643 ms", wall time under Xvfb) | the view draws; zones, boronated bricks (purple), chute |
| `workbench_step3_xray_gpu.png` | first run: Step 3 in X-ray, GPU (2.2 s) | outlines of the borings and zones; attenuation shading |
| `workbench_step2_pebble_section_gpu.png` | first run: Step 2, one pebble cut through a particle row, GPU (71 ms) | TRISO particles with their coating colours in the cut faces |
| `workbench_step2_xy_slice_gpu.png` | first run: Step 2's 2D slice, GPU | pebbles, helium, TRISO lattice; the legend (7 materials) comes from the GPU's per-pixel ids |

~~Not checked here: the DEM-poured bed (Step 1 with a finished pour), and
pictures on any other GPU or driver.~~ **UPDATED 2026-10-05 (second run):**
the DEM-poured bed is checked, by the parity test on a committed pour CSV
assembled exactly as the workbench's `Req::AssembleFromCentres` does; the
running workbench was not taken through a finished pour (it takes minutes),
but its assembled core goes to the GPU by the same `Gpu::set_core` call as
the lattice bed's. Still not checked: pictures on any other GPU or driver.
The workbench screenshots were taken under Xvfb on the same GPU.
