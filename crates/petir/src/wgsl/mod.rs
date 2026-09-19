//! **WGSL kernels** — PETIR's numerics as GPU shader source, in `f32`.
//!
//! This module is the GPU half of the crate. It ships the WGSL text for
//! PETIR's pointwise evaluators, an `f32` CPU mirror of each one, and — behind
//! the off-by-default `wgpu` feature — a headless runner that dispatches them
//! so the two can be compared on real hardware.
//!
//! # The `no_std` contract is not relaxed here
//!
//! This module is **`no_std` and dependency-free**. WGSL is just `&'static
//! str`, and the mirrors are arithmetic over `f32` through
//! [`crate::real::Real`]. Everything in this module compiles for
//! `thumbv7em-none-eabihf` exactly as the rest of the crate does.
//!
//! Only [`gpu`] needs `std`, and it is behind the **off-by-default `wgpu`
//! feature**, which nothing else enables. The five checks in this crate's
//! `CLAUDE.md` all run with default features and all still pass.
//!
//! That feature is the one place `std` enters the crate, and it is gated
//! carefully because the last attempt was not: the removed `platform-libm`
//! feature declared a `std` feature while `lib.rs` gated `extern crate std;`
//! on `cfg(test)` **alone**, so `std` never arrived and the feature had been
//! dead since the crate went `no_std` (`bn:op-7kwt`). `lib.rs` now reads
//! `#[cfg(any(test, feature = "wgpu"))] extern crate std;`, and
//! `cargo check -p petir --all-features --all-targets` — the sweep that caught
//! the last one — is what keeps it honest.
//!
//! If you are adding to this module, keep the split: **shader text and
//! numerics on the `no_std` side, device handling behind the feature.**
//!
//! # `f32` is the whole point, and it changes what "correct" means
//!
//! A baseline WebGPU device is guaranteed `f32` and nothing wider. `f64` in a
//! shader is an extension that most targets — every browser, most mobile — do
//! not have. So a GPU port of an `f64` library is not a recompilation, it is a
//! **different numerical object**, and it must be judged against an `f32`
//! error budget rather than the crate's usual `1e-15`.
//!
//! `f32` carries about 7.2 decimal digits. One operation is good to ~1e-7
//! relative; a Clenshaw sweep over 30 coefficients is not. Every test in this
//! module states the budget it holds to and the number it measured, and
//! [`crate::wgsl::mirror`] exists so that budget can be checked without a GPU
//! at all.
//!
//! # Three layers, and why each is separate
//!
//! | layer | where | what it establishes |
//! |---|---|---|
//! | `f64` reference | the rest of PETIR, verified against compiled GSL | what the right answer is |
//! | `f32` mirror | [`mirror`] | what `f32` costs, on the CPU, deterministically |
//! | WGSL | the `.wgsl` sources here, run by [`gpu`] | that the GPU agrees with the mirror |
//!
//! Splitting the last two matters. If a GPU result disagrees with the `f64`
//! reference, that single comparison cannot tell you whether the shader is
//! wrong or whether `f32` simply cannot do better. The mirror separates the
//! two: **mirror-vs-`f64`** is the precision cost, and **GPU-vs-mirror** is the
//! transcription's correctness. Only the second should ever be near-exact.
//!
//! # The shader library assumes one binding
//!
//! Every function here reads its array data from a single storage binding
//! named `src`:
//!
//! ```wgsl
//! @group(0) @binding(0) var<storage, read> src: array<f32>;
//! ```
//!
//! and takes an `off: u32` offset into it. That convention is deliberate:
//! WGSL's support for storage pointers as function parameters is an optional
//! language extension, so passing the array itself would make these functions
//! refuse to compile on conforming baseline devices. [`BINDING_PRELUDE`] is
//! that declaration, and [`test_kernel`] wraps a call in a complete shader.
//!
//! # Lineage
//!
//! Each `.wgsl` file carries its own header naming what it was transcribed
//! from. They are **transcriptions of PETIR's own ports**, not new numerics:
//! the arithmetic, the loop order and the association are taken from the `f64`
//! code, which was in turn read from GSL. Where a function has no GSL
//! counterpart it says so in its own comment — `petir_poly_eval_comp` is the
//! one such case at present.

pub mod mirror;

/// `f32` mirrors of the error-function shaders, generated from the same parse
/// of GSL's `specfunc/erfc.c` as `shaders/erf.wgsl`.
pub mod mirror_erf;

/// `f32` mirrors of the matrix and CBLAS shaders.
pub mod mirror_matrix;

/// `f32` mirrors of the gamma-family shaders.
pub mod mirror_gamma;

/// Headless GPU execution of these kernels. **Behind the off-by-default
/// `wgpu` feature**, and the only module in the crate that uses `std`.
#[cfg(all(
    feature = "wgpu",
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]
pub mod gpu;

/// The storage binding every function in this module reads from.
///
/// Prepend this (or an equivalent declaration of `src`) to any shader that
/// includes [`POLY`], [`CHEB`] or [`LEGENDRE`].
pub const BINDING_PRELUDE: &str = "@group(0) @binding(0) var<storage, read> src: array<f32>;\n";

/// Horner polynomial evaluation. Transcribed from [`crate::poly::eval`].
///
/// Provides `petir_poly_eval(off, n, x)` and `petir_poly_eval_comp(off, n, x)`.
/// Coefficients are **ascending**, matching GSL and [`crate::poly::eval`], and
/// opposite to [`crate::poly::dense`].
pub const POLY: &str = include_str!("shaders/poly.wgsl");

/// Chebyshev series evaluation by Clenshaw. Transcribed from [`crate::cheb`].
///
/// Provides `petir_cheb_eval(off, n, a, b, x)`,
/// `petir_cheb_eval_n(off, n, eval_order, a, b, x)` and
/// `petir_cheb_scale(v, lo, hi)`.
pub const CHEB: &str = include_str!("shaders/cheb.wgsl");

/// Legendre polynomials by Bonnet's recurrence.
///
/// Provides `petir_legendre_p(n, x)` and `petir_legendre_p_dp(n, x)`.
pub const LEGENDRE: &str = include_str!("shaders/legendre.wgsl");

/// The error-function family, ported from GSL's `specfunc/erfc.c`.
///
/// Provides `petir_erf(x)`, `petir_erfc(x)`, `petir_erfseries(x)`,
/// `petir_erfc8(x)` and the three Chebyshev branch helpers. Truncated at
/// GSL's own `order_sp`, the single-precision order it carries for each
/// series.
pub const ERF: &str = include_str!("shaders/erf.wgsl");

/// GSL's matrix layer and reference CBLAS, ported from `cblas/source_*_r.h`
/// and `matrix/`.
///
/// Provides `petir_mat_get`, `petir_mat_index`, the Level-1 kernels
/// `petir_blas_dot` / `nrm2` / `asum` / `iamax`, the Level-2
/// `petir_blas_gemv_row` and `_trans`, the Level-3
/// `petir_blas_gemm_element` and `_nt`, and the element-wise
/// `petir_mat_add` / `sub` / `mul_elements` / `div_elements` / `scale` /
/// `add_constant` / `transpose`.
///
/// **Row-major with an explicit leading dimension**, matching
/// `CblasRowMajor` and `gsl_matrix.tda`. Column-major is not ported.
///
/// **A per-element GPU kernel cannot reproduce GSL's `k`-outer `gemm`
/// accumulation** — one invocation owns one output element, so it must sum
/// over `k` itself. See the shader header and
/// [`mirror_matrix::gemm_gsl_order`].
pub const MATRIX: &str = include_str!("shaders/matrix.wgsl");

/// GSL's gamma family, ported from `specfunc/gamma.c` by way of
/// [`crate::specfunc::gamma`].
///
/// Provides `petir_lngamma`, `petir_gamma`, `petir_lnbeta`, `petir_beta`, the
/// Lanczos branch and the two Padé branches at `ln Gamma`'s zeros.
///
/// **`Gamma` overflows near `x = 35` in `f32`** — stay in log space above
/// that.
pub const GAMMA: &str = include_str!("shaders/gamma.wgsl");

/// Every shader source in this module, in dependency order.
///
/// They are mutually independent today; the order is fixed so that a
/// concatenation is reproducible.
pub const ALL: [&str; 6] = [POLY, CHEB, LEGENDRE, ERF, MATRIX, GAMMA];

/// Names of the sources in [`ALL`], index for index, for diagnostics.
pub const ALL_NAMES: [&str; 6] = ["poly", "cheb", "legendre", "erf", "matrix", "gamma"];

pub use kernel_builder::test_kernel;

// `test_kernel` needs `alloc` to build a string. `alloc` is unconditional in
// this crate, so this is not feature-gated -- the module is inline rather than
// a file only because it is six lines.
mod kernel_builder {
    use alloc::string::String;

    /// Wrap a call to one of this module's functions in a complete, runnable
    /// compute shader.
    ///
    /// The generated kernel maps over an input array: workgroup `i` reads
    /// `probe[i]`, evaluates `call` with `x` bound to it, and writes
    /// `dst[i]`. `call` is a WGSL expression that may use `x` and `src`.
    ///
    /// This exists so a test can dispatch any function here without
    /// hand-writing a shader per case, which is how a verification suite
    /// quietly stops covering things.
    ///
    /// # Example
    ///
    /// ```
    /// use petir::wgsl::{test_kernel, POLY};
    /// let src = test_kernel(&[POLY], "petir_poly_eval(0u, params.n, x)");
    /// assert!(src.contains("@compute"));
    /// assert!(src.contains("petir_poly_eval"));
    /// ```
    pub fn test_kernel(sources: &[&str], call: &str) -> String {
        let mut s = String::new();
        // Eight slots, 32 bytes. The first four are the scalar kernels'
        // original layout and are unchanged; the last four were appended for
        // the matrix kernels, which need a second matrix offset and leading
        // dimension. Appending keeps every existing call site working through
        // `..Default::default()`.
        s.push_str(
            "struct Params { n: u32, a: f32, b: f32, k: u32,              m: u32, off_b: u32, ld_b: u32, off_c: u32 };\n",
        );
        s.push_str("@group(0) @binding(0) var<storage, read> src: array<f32>;\n");
        s.push_str("@group(0) @binding(1) var<storage, read> probe: array<f32>;\n");
        s.push_str("@group(0) @binding(2) var<storage, read_write> dst: array<f32>;\n");
        s.push_str("@group(0) @binding(3) var<uniform> params: Params;\n\n");
        for src in sources {
            s.push_str(src);
            s.push('\n');
        }
        s.push_str("\n@compute @workgroup_size(64)\n");
        s.push_str("fn main(@builtin(global_invocation_id) gid: vec3<u32>) {\n");
        // Keep EVERY binding live, whatever `call` happens to read.
        //
        // naga strips a binding no code references, and `create_bind_group`
        // then rejects the four entries a caller supplies against a
        // three-entry layout. Both halves of this are real failures already
        // hit here: `petir_legendre_p` reads no array (binding 0), and
        // `petir_erfc` reads neither array nor uniform (bindings 0 and 3).
        //
        // `arrayLength(&src)` covers binding 0. The `params` guard covers
        // binding 3 with a genuine branch on a genuine uniform read, so it
        // cannot be folded away -- `0xffffffff` elements is not a length any
        // real dispatch has.
        s.push_str("    if (arrayLength(&src) == 0u) { return; }\n");
        s.push_str("    let i = gid.x;\n");
        s.push_str("    if (i >= arrayLength(&probe)) { return; }\n");
        s.push_str("    if (params.n == 0xffffffffu) { dst[i] = 0.0; return; }\n");
        s.push_str("    let x = probe[i];\n");
        s.push_str("    dst[i] = ");
        s.push_str(call);
        s.push_str(";\n}\n");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shader source is present, non-empty, and carries its licence and
    /// lineage header.
    ///
    /// A `.wgsl` file that lost its provenance header is exactly the failure
    /// this crate's rules exist to prevent, and `include_str!` would happily
    /// ship it.
    #[test]
    fn every_shader_declares_its_licence_and_lineage() {
        for (name, src) in ALL_NAMES.iter().zip(ALL.iter()) {
            assert!(!src.is_empty(), "{name}.wgsl is empty");
            assert!(
                src.contains("SPDX-License-Identifier: GPL-3.0-only"),
                "{name}.wgsl has no SPDX header"
            );
            assert!(
                src.contains("TRANSCRIBED to WGSL"),
                "{name}.wgsl does not say what it was transcribed from"
            );
        }
    }

    /// The generated test kernel is well-formed enough to compile.
    ///
    /// Real validation is naga's, in the `wgpu`-gated tests; this catches the
    /// cheap mistakes without a device.
    #[test]
    fn the_generated_kernel_has_the_pieces_a_shader_needs() {
        let src = test_kernel(&[POLY], "petir_poly_eval(0u, params.n, x)");
        assert!(src.contains("@compute @workgroup_size(64)"));
        assert!(src.contains("var<storage, read> src: array<f32>"));
        assert!(src.contains("var<storage, read_write> dst: array<f32>"));
        assert!(src.contains("var<uniform> params: Params"));
        assert!(
            src.contains("off_b: u32"),
            "the matrix slots must be present"
        );
        assert!(src.contains("fn petir_poly_eval"));
        // The bounds guard must be there: a dispatch rounds up to whole
        // workgroups, so without it the tail invocations write out of range.
        assert!(src.contains("if (i >= arrayLength(&probe)) { return; }"));
        // And every binding must be referenced, or naga strips it from the
        // auto layout and the bind group no longer matches. Neither of these
        // is hypothetical: `petir_legendre_p` reads no array and
        // `petir_erfc` reads neither array nor uniform, and each failed
        // before its guard was added.
        assert!(src.contains("arrayLength(&src)"));
        assert!(src.contains("params.n == 0xffffffffu"));
    }

    /// The binding prelude matches what the generated kernel declares.
    ///
    /// These are written in two places, and a caller composing shaders by hand
    /// uses the constant while the tests use the generator. If they drift, the
    /// documented convention becomes wrong for exactly the people who followed
    /// it.
    #[test]
    fn the_binding_prelude_matches_the_generated_kernel() {
        let src = test_kernel(&[], "0.0");
        let decl = BINDING_PRELUDE.trim();
        assert!(
            src.contains(decl),
            "BINDING_PRELUDE ({decl}) is not what test_kernel emits"
        );
    }
}
