# GPU pour against CPU pours: packing fraction and g(r) (gh:#1250, 2026-11-05)

> Research, education and V&V only. AI-assisted change (Claude Code), not yet reviewed by the maintainer.

**Criterion** (maintainer: "the key thing for the WGSL LIGGGHTS to emulate is the packing fraction and rdf"). Decided before the run: the GPU pour must lie inside the CPU pours' 95 % prediction interval, quantity by quantity (`examples/gpu_pour_compare.rs`).

**Run.**
- `gpu_pour_compare --n 12000 --seeds 1,2 --threads 4 --csv gr_n12000.csv`.
- The GPU ran on lavapipe (Mesa's software Vulkan); the CPU on 4 threads.
- A quick check: two CPU seeds only, at the maintainer's request.

**Results** (`gpu_pour_compare_n12000.log`):

| quantity | CPU seed 1 | CPU seed 2 | GPU seed 1 | verdict |
|---|---|---|---|---|
| whole-core φ | 0.5843 | 0.5858 | 0.5841 | inside |
| surface (m) | 0.7242 | 0.7218 | 0.7254 | inside |
| g contact peak | 19.12 | 19.20 | 19.02 | inside |
| g(√2 d) | 0.625 | 0.616 | 0.641 | inside |
| g(√3 d) | 1.293 | 1.270 | 1.256 | inside |
| g(2 d) | 1.186 | 1.178 | 1.273 | **OUTSIDE** (interval ±0.084) |

There were no table overflows.

**Reading.**
- **Packing fraction and surface:** the GPU matches the CPU to within the CPU's own seed scatter.
- **g(r):** the GPU's has the same random-packing shape: a contact peak near 19, a trough at √2 d, and the split second peak at √3 d and 2 d.
- **The miss at 2 d:**
  - The sharp 2 d spike is higher on the GPU (bins at 1.99 d and 2.01 d: GPU 1.78 / 1.10, CPU 1.65 / 0.98 and 1.63 / 0.97).
  - Two CPU seeds give a poor spread estimate. Single bins near 2 d differ between the CPU seeds by up to 0.2 (at 1.91 d: 1.23 against 1.44).
  - So the miss may be scatter or a real small excess of collinear triples on the GPU.
  - It stands as recorded: settling it needs more CPU seeds and GPU seeds, which was not run.

**The curves:** `gr_n12000.csv` (r/d, then g for each pour, the same bins).
