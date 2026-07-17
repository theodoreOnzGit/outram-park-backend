# Blender's Third-Party Dependency Landscape — Audited for a Rust Mesh-Authoring Frontend

`outram-blender` takes Blender's mesh/geometry **architecture** — the concepts
behind its `BMesh` half-edge topology, its modifier stack, subdivision, and
boolean/CSG operations — and re-expresses them as a small, pure-Rust,
Android-buildable **mesh-authoring frontend** whose job is to produce clean
meshes for the solvers in this workspace. It is **not** a code port of Blender,
and it is **not** a renderer, media tool, color pipeline, physics engine, or GPU
application. This document audits Blender's full third-party dependency stack and
judges, library by library, what a Rust mesh-authoring frontend actually needs
versus what is out of scope. The overwhelming majority of Blender's dependencies
serve rendering, media, color, audio, GPU compute, and text layout — none of
which a solver-mesh frontend requires.

## Source and version provenance

- **File read:** `build_files/build_environment/cmake/versions.cmake`
- **Raw URL:** <https://raw.githubusercontent.com/blender/blender/main/build_files/build_environment/cmake/versions.cmake>
- **Latest commit touching that path:** `9f5c3edcf34bea02589fa09fc2ce6830ffe4acdf`
  — "Merge branch 'blender-v5.2-release'" (Jonas Holzman), dated **2026-07-02**,
  obtained from the GitHub commits API filtered by path.

All versions below are transcribed as they appeared in that read. Where a library
was not clearly present in the read, the version column says **not confirmed in
the read** rather than a guessed value. This is scaffold-stage analysis, not a
finished dependency plan.

> **License note.** Blender itself is **GPLv2-or-later**, which is compatible
> with this workspace's GPLv3-only crates. The dependency licenses listed by
> Blender's build environment are **not inherited by merely reimplementing a
> concept** — designing a Rust half-edge structure inspired by BMesh, or a
> Catmull-Clark subdivision routine inspired by OpenSubdiv, creates no license
> obligation to those upstreams. However, **any future *literal* port** of a
> dependency's algorithm or source (as opposed to a clean-room reimplementation
> of the concept) must re-check *that dependency's own* license for GPLv3
> compatibility, and must carry the upstream attribution header block per this
> repo's provenance rules. When in doubt: reimplement the concept, cite the
> reference, and keep it clean-room.

### Legend for the "Rust-ecosystem path" column

- A **named crate** (e.g. `glam`, `parry3d`, `gltf`) means: use this instead.
- **Reimplement (concept)** means: we write it ourselves in Rust, inspired by the
  Blender/dependency concept, no code port.
- **Skip — `<reason>`** means: out of scope for authoring solver meshes
  (rendering, media, color, GPU, audio, physics-solver, text layout, etc.).

**Android-hostility** is flagged inline. This crate is pure-Rust and must build
for `aarch64-linux-android`, so any dependency requiring a C/C++/Fortran
toolchain, system BLAS/LAPACK, GPU drivers, or a heavy native build is treated as
Android-hostile and avoided by construction — the Rust-ecosystem path is always a
pure-Rust crate or a reimplemented concept.

---

## 1. Language / scripting

Blender embeds CPython for its add-on and tooling ecosystem. A mesh-authoring
frontend needs none of the embedded-interpreter machinery.

| Library | Version | Purpose in Blender | Relevant? | Rust-ecosystem path |
|---|---|---|---|---|
| Python | 3.13.13 | Embedded interpreter for add-ons, scripting, tooling | No | Skip — no embedded scripting in a Rust frontend |
| NumPy | 2.3.4 | Array math exposed to Python API | No | Skip — Rust uses `ndarray`/`nalgebra` natively |
| Cython | 3.0.11 | Build-time Python/C glue | No | Skip — build tooling, not needed |
| NanoBind | v2.1.0 | C++↔Python bindings | No | Skip — no Python bindings |
| pybind11 | 3.0.1 | C++↔Python bindings | No | Skip — no Python bindings |

---

## 2. Geometry & subdivision (the core of interest)

This is the only group where several entries are genuinely relevant — and even
here the relevance is **conceptual**: we reimplement the geometry operations in
Rust rather than porting the C++ libraries (which are Android-hostile native
builds).

| Library | Version | Purpose in Blender | Relevant? | Rust-ecosystem path |
|---|---|---|---|---|
| OpenSubdiv | v3_7_0 | Catmull-Clark / Loop subdivision surfaces | **Yes (concept)** | Reimplement (concept) — Catmull-Clark on our half-edge mesh |
| OpenVDB | 13.0.0 | Sparse volumetric grids, volume meshing, remesh | Partial | Reimplement narrowly if voxel-remesh needed; otherwise Skip. Android-hostile native build; avoid the C++ lib |
| Manifold | v3.5.2 | Robust mesh boolean / CSG | **Yes (concept)** | Reimplement (concept) — boolean/CSG; or evaluate `csgrs`. Manifold C++ is Android-hostile |
| meshoptimizer | 1.1 | Vertex cache / overdraw / simplification | **Yes** | `meshopt` crate (Rust bindings) or Reimplement decimation concept |
| Draco | 1.5.7 | Mesh compression (glTF payloads) | Maybe | `gltf` ecosystem / skip unless compressed glTF needed |
| Embree | 4.4.1 | CPU ray-tracing BVH acceleration | No | `parry3d`/`embree`-free BVH if raycast picking needed; else Skip — rendering/query accel |
| GMP | 6.3.0 | Arbitrary-precision arithmetic (exact boolean predicates) | **Yes (concept)** | `rug`/`num-bigint` or robust-predicate crate (`robust`) for exact CSG; GMP C is Android-hostile |
| Eigen | 8a1083e (header-only) | Dense linear algebra (used across geometry/solver code) | **Yes** | `nalgebra` / `glam` — pure Rust, Android-friendly |
| Ceres | 0c70ed3 | Non-linear least-squares solver | No | Skip — optimization solver, not mesh authoring (`argmin` exists if ever needed) |

---

## 3. Interchange / USD (mesh import-export)

Import/export of mesh formats is directly in scope — but via Rust format crates,
not the heavyweight C++ interchange libraries.

| Library | Version | Purpose in Blender | Relevant? | Rust-ecosystem path |
|---|---|---|---|---|
| USD | 26.03 | Universal Scene Description interchange | Maybe | Skip for v1 (huge Android-hostile C++ stack); revisit with a Rust USD reader if scene interchange needed |
| Alembic | 1.8.3 | Baked geometry/animation interchange | No | Skip — animation-cache interchange, out of scope |
| MaterialX | 1.39.4 | Material graph interchange | No | Skip — materials/shading, not geometry |

> Mesh interchange the frontend *does* want (OBJ, PLY, STL, glTF) is not a
> Blender build dependency — Blender implements those importers in-tree. Rust
> path: `obj-rs`/`tobj`, `ply-rs`, `stl_io`, `gltf`. These are pure-Rust and
> Android-buildable.

---

## 4. Image / color / media (rendering & texture pipeline)

Entirely out of scope for a solver-mesh frontend. Nearly all are Android-hostile
native C/C++ builds.

| Library | Version | Purpose in Blender | Relevant? | Rust-ecosystem path |
|---|---|---|---|---|
| OpenEXR | 3.4.10 | HDR image I/O | No | Skip — image/rendering |
| Imath | 3.2.2 | Half-float / vector math for EXR | No | Skip — image math (`half` crate exists if ever needed) |
| OpenColorIO | 2.5.0 | Color management transforms | No | Skip — color management |
| OpenImageIO | v3.1.13.1 | Unified image I/O | No | Skip — image pipeline |
| OpenImageDenoise | 2.5.0 | AI render denoiser | No | Skip — rendering |
| Open Shading Language | 1.15.3.0 | Programmable shading | No | Skip — shading/rendering |
| Open PGL | v0.7.1 | Path-guiding for rendering | No | Skip — rendering |
| libpng | 1.6.58 | PNG I/O | No | Skip — `image` crate if any raster ever needed |
| libjpeg-turbo | 2.1.3 | JPEG I/O | No | Skip — image |
| LibTIFF | 4.7.1 | TIFF I/O | No | Skip — image |
| OpenJPEG | 2.5.3 | JPEG 2000 I/O | No | Skip — image |
| OpenJPH | 0.25.2 | HTJ2K (JPEG 2000 HT) I/O | No | Skip — image |
| libwebp | 1.6.0 | WebP I/O | No | Skip — image |
| libheif | 1.20.2 | HEIF/AVIF container I/O | No | Skip — image |
| Potrace | 1.16 | Raster-to-vector tracing | No | Skip — 2D vector, not 3D mesh |
| libharu | 2.4.5 | PDF generation | No | Skip — document export |
| ThorVG | v1.0.3 | Vector graphics engine | No | Skip — 2D vector rendering |
| FFmpeg | 8.1 | Video/audio codecs | No | Skip — media |
| libvpx | 1.15.2 | VP8/VP9 video codec | No | Skip — media |
| libaom | 3.13.1 | AV1 video codec | No | Skip — media |
| x264 | 35fe20d (commit) | H.264 encoder | No | Skip — media |
| x265 | 4.1 | H.265 encoder | No | Skip — media |
| libtheora | 1.1.1 | Theora video codec | No | Skip — media |

---

## 5. Audio

Out of scope — no audio in a mesh frontend.

| Library | Version | Purpose in Blender | Relevant? | Rust-ecosystem path |
|---|---|---|---|---|
| OpenAL | 1.23.1 | 3D audio playback | No | Skip — audio |
| libsndfile | 1.2.2 | Audio file I/O | No | Skip — audio |
| LAME | 3.100 | MP3 encoder | No | Skip — audio |
| libogg | 1.3.5 | Ogg container | No | Skip — audio |
| libvorbis | 1.3.7 | Vorbis audio codec | No | Skip — audio |
| FLAC | 1.4.2 | Lossless audio codec | No | Skip — audio |
| Opus | 1.3.1 | Opus audio codec | No | Skip — audio |
| Rubber Band Library | 4.0.0 | Audio time-stretch/pitch | No | Skip — audio |
| FFTW | 3.3.10 | FFT (audio/sim); Android-hostile native build | No | Skip — `rustfft` if any FFT ever needed |

---

## 6. GPU / windowing / compute

Blender is a GPU application; this crate is a headless-capable, pure-Rust library.
All GPU/windowing/compiler-toolchain dependencies are out of scope and are the
most Android-hostile of the stack (drivers, native compilers, SPIR-V toolchains).

| Library | Version | Purpose in Blender | Relevant? | Rust-ecosystem path |
|---|---|---|---|---|
| Vulkan-Headers | 1.4.341 | Vulkan API headers | No | Skip — GPU. `ash`/`wgpu` exist if a viewer is ever built |
| Vulkan-Loader | 1.4.341 | Vulkan ICD loader | No | Skip — GPU |
| Vulkan-Utility-Libraries | 1.4.341 | Vulkan helpers | No | Skip — GPU |
| Vulkan-Memory-Allocator | 3.2.1 | GPU memory allocator | No | Skip — GPU |
| SPIR-V Headers | 1.4.341 | SPIR-V definitions | No | Skip — GPU |
| SPIRV-Reflect | 1.4.341 | Shader reflection | No | Skip — GPU |
| SPIR-V Tools | v2026.1 | SPIR-V assembler/validator | No | Skip — GPU |
| ShaderC | v2025.4 | GLSL→SPIR-V compiler | No | Skip — GPU |
| glslang | d213562 (commit) | GLSL front-end | No | Skip — GPU |
| libepoxy | 1.5.10 | GL/GLX function loading | No | Skip — GPU |
| SDL | 3.4.2 | Windowing / input | No | Skip — windowing (`winit` if a viewer is built) |
| Wayland | 1.24.0 | Linux display protocol | No | Skip — windowing |
| Wayland-Protocols | 1.44 | Wayland protocol defs | No | Skip — windowing |
| Weston | 14.0.2 | Reference Wayland compositor (headless test) | No | Skip — windowing |
| OpenXR | 1.1.53 | VR/AR runtime | No | Skip — XR |
| FreeSpacenav | 1.1 | 3D-mouse (SpaceNavigator) input | No | Skip — input device |
| OpenCL-Headers | 6eabe90 (commit) | OpenCL API headers | No | Skip — GPU compute |
| OpenCL-ICD-Loader | ddf6c70 (commit) | OpenCL loader | No | Skip — GPU compute |
| oneAPI Level Zero | 35c037c (commit) | Intel GPU compute API | No | Skip — GPU compute |
| Unified Memory Framework | v1.0.0-rc2 | Intel oneAPI memory | No | Skip — GPU compute |
| HIPRT | 606b488 (commit) | AMD HIP ray-tracing | No | Skip — GPU |
| DPC++ | v6.3.0 | Intel SYCL compiler (GPU kernels) | No | Skip — GPU toolchain |
| ISPC | v1.30.0 | SPMD CPU/GPU compiler | No | Skip — compiler toolchain |
| IGC | 2.30.1 | Intel Graphics Compiler | No | Skip — GPU toolchain |
| IGC LLVM | llvmorg-16.0.6 | LLVM for IGC | No | Skip — GPU toolchain |
| opencl-clang | v16.0.10 | OpenCL C front-end | No | Skip — GPU toolchain |
| VC Intrinsics (IGC) | 0.25.0 | Vector-compute intrinsics | No | Skip — GPU toolchain |
| vc-intrinsics | 60cea75 (commit) | Vector-compute intrinsics (DPC++) | No | Skip — GPU toolchain |
| SPIR-V Headers (DPCPP) | c9aad99 (commit) | SPIR-V for DPC++ | No | Skip — GPU toolchain |
| SPIR-V Headers (IGC) | 9268f30 (commit) | SPIR-V for IGC | No | Skip — GPU toolchain |
| SPIR-V Tools (IGC) | 28a883b (commit) | SPIR-V tools for IGC | No | Skip — GPU toolchain |
| SPIR-V Translator | v16.0.10 | LLVM↔SPIR-V | No | Skip — GPU toolchain |
| gmmlib | intel-gmmlib-22.8.1 | Intel GPU memory mgmt | No | Skip — GPU driver |
| ocloc | 25.31.34666.3 | Intel offline GPU compiler | No | Skip — GPU toolchain |
| LLVM | 20.1.8 | Compiler infra (OSL/shading, GPU) | No | Skip — compiler toolchain |
| sse2neon | 227cc41 (commit) | SSE→NEON intrinsics shim | No | Skip — Rust `std::arch`/portable SIMD handle this |

---

## 7. Text / font shaping

Blender lays out UI and 3D text; a solver-mesh frontend does not do glyph-to-mesh
in v1 (and if it ever did, Rust has native crates).

| Library | Version | Purpose in Blender | Relevant? | Rust-ecosystem path |
|---|---|---|---|---|
| FreeType | 2.13.3 | Font rasterization / outlines | No | Skip — text (`ttf-parser`/`ab_glyph` if text-to-mesh ever needed) |
| harfbuzz | 10.0.1 | Text shaping | No | Skip — text (`rustybuzz` is a pure-Rust port) |
| fribidi | v1.0.12 | Bidirectional text | No | Skip — text |

---

## 8. Compression / IO / concurrency / utility

Infrastructure Blender links broadly. A Rust frontend gets equivalents from
`std` and small pure-Rust crates; none of the native C libraries are needed.

| Library | Version | Purpose in Blender | Relevant? | Rust-ecosystem path |
|---|---|---|---|---|
| ZLIB | 1.3.1 | DEFLATE compression | Maybe | `flate2`/`miniz_oxide` (pure Rust) if compressed I/O needed |
| Zstandard | 1.5.7 | Zstd compression (C lib) | Maybe | `zstd`/`ruzstd` if needed |
| zstandard | 0.25.0 | Python zstd bindings | No | Skip — Python |
| libdeflate | 1.18 | Fast DEFLATE | No | Skip — `miniz_oxide` covers it |
| c-blosc | 1.21.1 | Chunked compression (OpenVDB) | No | Skip — tied to OpenVDB |
| minizip-ng | 4.0.10 | Zip archive I/O | No | Skip — `zip` crate if needed |
| bzip2 | 1.0.8 | bzip2 compression | No | Skip — media/archive |
| LZMA | 5.2.5 | xz/LZMA compression | No | Skip — `xz2`/`lzma-rs` if ever needed |
| Brotli | 1.0.9 | Brotli compression | No | Skip — `brotli` crate if ever needed |
| libffi | 3.5.2 | Foreign-function calls (CPython) | No | Skip — no FFI |
| OpenSSL | 3.5.6 | TLS/crypto | No | Skip — no networking; `rustls` if ever needed |
| SQLite | 3.51.3 | Embedded database (OCIO cache etc.) | No | Skip — no DB |
| libxml2 | 2.14.6 | XML parsing | No | Skip — `quick-xml` if any XML needed |
| libexpat | 2.7.5 | XML parsing | No | Skip — `quick-xml` |
| yaml-cpp | 0.8.0 | YAML (OCIO configs) | No | Skip — `serde_yaml` if config needed |
| pugixml | 1.10 | XML (OIIO) | No | Skip — image-pipeline XML |
| pystring | v1.1.3 | Python-like string ops (OCIO) | No | Skip — Rust `std` strings |
| fmt | 12.1.0 | C++ formatting | No | Skip — Rust `std::fmt`/`format!` |
| oneTBB | v2022.3.0 | Task/parallel scheduling | No | Reimplement via `rayon` (pure Rust) where parallelism is needed |
| robin-map | v1.3.0 | Fast hash map (Tessil) | No | Skip — `hashbrown`/`std::HashMap` |
| parallel-hashmap | 8a889d3 (commit) | Concurrent hash map | No | Skip — `dashmap`/`hashbrown` |
| emhash | 3ba9abd (commit) | Fast hash map | No | Skip — `hashbrown` |
| pthreads4w | 3.0.0 | POSIX threads on Windows | No | Skip — Rust `std::thread` |
| Tracy | a64b9a2 (commit) | Frame/CPU profiler | No | Skip — `tracing`/`puffin` if profiling wanted |
| Abseil | 20250814.1 | C++ base utilities (USD/others) | No | Skip — Rust `std` |
| libepoxy | 1.5.10 | (see GPU) | No | Skip — GPU |

---

## 9. Build tooling (not runtime libraries)

Listed in `versions.cmake` but they are build-time code generators / assemblers,
irrelevant to a Cargo-built Rust crate.

| Library | Version | Purpose in Blender | Relevant? | Rust-ecosystem path |
|---|---|---|---|---|
| NASM | 2.15.02 | x86 assembler (codec builds) | No | Skip — Cargo/rustc build |
| flex | 2.6.4 | Lexer generator | No | Skip — build tool |
| win_flex_bison | 2.5.24 | flex/bison for Windows | No | Skip — build tool |

---

## Summary — what a Rust mesh-authoring frontend actually needs

Out of the **~120 dependency entries** catalogued from `versions.cmake`
(commit `9f5c3edcf34bea02589fa09fc2ce6830ffe4acdf`, 2026-07-02), only a **small
handful** map to anything this crate needs, and they map as **concepts to
reimplement** or **small pure-Rust crates**, never as native library ports:

- **Geometry math** — `glam` / `nalgebra` (replacing Eigen/Imath); exact
  predicates via `robust` / `num-bigint` where Blender used GMP.
- **Half-edge topology** — **no external dependency**: Blender's own `BMesh`
  half-edge design is the architecture we reimplement in Rust from scratch.
- **Subdivision** — reimplement Catmull-Clark / Loop as a concept (inspired by
  OpenSubdiv), operating on our half-edge mesh.
- **Boolean / CSG** — reimplement robust mesh booleans as a concept (inspired by
  Manifold), or evaluate a pure-Rust CSG crate.
- **Mesh decimation / optimization** — `meshopt` bindings or a reimplemented
  quadric-decimation concept (inspired by meshoptimizer).
- **Mesh import/export** — pure-Rust format crates (`obj-rs`/`tobj`, `ply-rs`,
  `stl_io`, `gltf`); these are not even Blender build dependencies.
- **Parallelism (if needed)** — `rayon` in place of oneTBB.

**Everything else is out of scope.** The vast majority of Blender's dependency
stack — rendering (Embree, OSL, OpenImageDenoise, Open PGL), color management
(OpenColorIO), image/media I/O (OpenEXR, OpenImageIO, FFmpeg, the codec zoo),
audio (OpenAL, the Ogg/Vorbis/FLAC/Opus family), GPU and compute toolchains (the
Vulkan/SPIR-V/OpenCL/oneAPI/HIP/IGC/LLVM cluster), windowing (SDL, Wayland,
Weston), XR (OpenXR), text shaping (FreeType, HarfBuzz, FriBidi), embedded
scripting (Python, NumPy, the binding libraries), USD/Alembic/MaterialX scene
interchange, and general C/C++ infrastructure (Boost-style utilities, hash-map
libraries, compression libraries) — is **not needed** to author meshes for a
solver.

That exclusion is also what keeps this crate **pure-Rust and Android-buildable**:
essentially every skipped library above is Android-hostile (system BLAS/LAPACK,
GPU drivers, C/C++/Fortran toolchains, or heavy native builds). By taking the
*architecture* and reimplementing the few geometry concepts in Rust, the frontend
inherits none of that native-build burden.

> Scaffold-stage note: this audit reflects the dependency manifest as read on the
> cited commit and the crate's *intended* scope. Specific Rust crate choices
> (e.g. which CSG or decimation crate) are provisional and to be confirmed during
> implementation, with license-provenance review per this repo's compliance docs
> before any code is adopted or ported.
