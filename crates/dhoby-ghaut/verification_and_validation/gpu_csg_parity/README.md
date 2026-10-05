# GPU ray tracing of the workbench views (gh:#587)

Pictures from 2026-10-05, NVIDIA RTX A5000 (Vulkan). Research, education
and V&V only.

Methodology and the numbers are in the doc comment of
`crates/dhoby-ghaut/tests/gpu_csg_parity.rs` (HTR-10) and
`crates/outram-blender/tests/gpu_csg_parity.rs` (a small nested model).
In short: each view is drawn by the CPU plotter (`f64`, the reference) and
by the GPU tracer (`f32`) from the same plot description, and the material
each pixel shows is compared.

| file | what it is | what was checked by eye |
|---|---|---|
| `htr10_bed_half_section_cpu.png` | Step 1's view, CPU | reference |
| `htr10_bed_half_section_gpu.png` | the same view, GPU | same picture: reflector zones, borings, cavity, cut bed with TRISO in the pebble cut faces |
| `htr10_bed_half_section_diff.png` | white = pixels whose colour differs by more than 8 levels | the differences lie on columns of pebbles cut by the plane y = 0 and on the reflector's right silhouette edge |
| `workbench_step3_reflector_half_section_gpu.png` | the running workbench, Step 3, GPU path ("GPU ray-traced in 643 ms", wall time under Xvfb) | the view draws; zones, boronated bricks (purple), chute |
| `workbench_step3_xray_gpu.png` | Step 3 in X-ray, GPU (2.2 s) | outlines of the borings and zones; attenuation shading |
| `workbench_step2_pebble_section_gpu.png` | Step 2, one pebble cut through a particle row, GPU (71 ms) | TRISO particles with their coating colours in the cut faces |
| `workbench_step2_xy_slice_gpu.png` | Step 2's 2D slice, GPU | pebbles, helium, TRISO lattice; the legend (7 materials) comes from the GPU's per-pixel ids |

Not checked here: the DEM-poured bed (Step 1 with a finished pour), and
pictures on any other GPU or driver.
