//! **WGSL validation that needs no GPU** — parse, type-check and structural
//! checks on every shader in [`petir::wgsl`].
//!
//! # Why this is separate from `wgsl_gpu.rs`
//!
//! Because **CI must not need a GPU**, and must not silently check nothing
//! when it does not have one.
//!
//! `tests/wgsl_gpu.rs` requires the `wgpu` feature and skips every case when no
//! adapter is present. That is the right behaviour for a *dispatch* test, but
//! it would leave a default CI run — no feature, no device — validating none of
//! the shader source at all. These tests therefore live here, need **no
//! feature and no device**, and run on every platform.
//!
//! naga is `wgpu`'s own shader front-end, so passing it is the same check a
//! real device performs when creating the shader module: every syntax error,
//! type error and undefined identifier is caught here, on any host.
//!
//! # What this cannot tell you
//!
//! That the numbers are right. A shader can validate perfectly and compute
//! nonsense. Numerical correctness is `petir::wgsl::mirror`'s job on the CPU
//! (which also runs everywhere) and `tests/wgsl_gpu.rs`'s on a device.

#![cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]

use petir::wgsl::{test_kernel, ALL, ALL_NAMES, CHEB, ERF, LEGENDRE, MATRIX, POLY};

/// Every shader in [`petir::wgsl::ALL`] parses and validates under naga.
///
/// # Why this runs without a GPU
///
/// A skipped dispatch test proves nothing, and most CI has no adapter. naga is
/// `wgpu`'s own front-end, so passing it is the same check a real device
/// performs when creating the shader module — it catches every syntax error,
/// type error and undefined identifier, on any host.
///
/// # Results
///
/// All three sources, and every generated test kernel, validate with **no
/// diagnostics**, measured 2026-09-19.
#[test]
fn every_shader_parses_and_validates_under_naga() {
    for (name, src) in ALL_NAMES.iter().zip(ALL.iter()) {
        // The function libraries reference the `src` binding, so they only
        // validate inside a complete kernel. Wrap each in the harness.
        let call = match *name {
            "poly" => "petir_poly_eval(0u, params.n, x)",
            "cheb" => "petir_cheb_eval(0u, params.n, params.a, params.b, x)",
            "legendre" => "petir_legendre_p(params.k, x)",
            "erf" => "petir_erfc(x)",
            "matrix" => "petir_blas_dot(0u, 0u, params.n)",
            other => panic!("no validation call registered for {other}.wgsl"),
        };
        let kernel = test_kernel(&[src], call);
        let module = naga::front::wgsl::parse_str(&kernel)
            .unwrap_or_else(|e| panic!("{name}.wgsl failed to parse: {e:?}"));
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        );
        validator
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}.wgsl failed validation: {e:?}"));
    }
}

/// Every function the module documents as available is actually defined.
///
/// The doc comments on `POLY`, `CHEB` and `LEGENDRE` list the function names
/// callers are told to use. This checks the text really defines them, so a
/// rename cannot silently make the documentation wrong.
#[test]
fn every_documented_function_is_defined() {
    let expected: [(&str, &[&str]); 5] = [
        (POLY, &["petir_poly_eval", "petir_poly_eval_comp"]),
        (
            CHEB,
            &["petir_cheb_eval", "petir_cheb_eval_n", "petir_cheb_scale"],
        ),
        (LEGENDRE, &["petir_legendre_p", "petir_legendre_p_dp"]),
        (
            ERF,
            &[
                "petir_erf",
                "petir_erfc",
                "petir_erfseries",
                "petir_erfc8",
                "petir_cheb_erfc_xlt1",
                "petir_cheb_erfc_x15",
                "petir_cheb_erfc_x510",
            ],
        ),
        (
            MATRIX,
            &[
                "petir_mat_get",
                "petir_mat_index",
                "petir_blas_dot",
                "petir_blas_nrm2",
                "petir_blas_asum",
                "petir_blas_iamax",
                "petir_blas_gemv_row",
                "petir_blas_gemv_row_trans",
                "petir_blas_gemm_element",
                "petir_blas_gemm_element_nt",
                "petir_mat_add",
                "petir_mat_sub",
                "petir_mat_mul_elements",
                "petir_mat_div_elements",
                "petir_mat_scale",
                "petir_mat_add_constant",
                "petir_mat_transpose",
            ],
        ),
    ];
    for (src, names) in expected {
        for name in names {
            assert!(
                src.contains(&format!("fn {name}(")),
                "documented function {name} is not defined in its shader"
            );
        }
    }
}

/// The CPU mirror answers without any GPU, on every platform.
///
/// # Why this is a test and not an assumption
///
/// The project's standing requirement is that **CI must not need a GPU** and
/// that the CPU/reference path is mandatory everywhere. That is a claim about
/// this crate's behaviour, so it is checked here rather than asserted in a
/// document: this test uses no device, no feature and no adapter, and it
/// exercises one function from each shader family through its mirror.
///
/// If `petir::wgsl::mirror` ever grew a dependency on the `wgpu` feature —
/// the most likely way the guarantee would be lost — this file would stop
/// compiling on a default build, which is exactly the signal wanted.
#[test]
fn the_cpu_mirror_answers_with_no_gpu_and_no_feature() {
    use petir::wgsl::{mirror, mirror_erf};

    // poly: 1 + 2x + 3x^2 at x = 2 is 17.
    assert_eq!(mirror::poly_eval(&[1.0, 2.0, 3.0], 2.0), 17.0);
    // cheb: c[0] enters halved, so [2, 0, 0] is the constant 1.
    assert_eq!(mirror::cheb_eval(&[2.0, 0.0, 0.0], -1.0, 1.0, 0.3), 1.0);
    // legendre: P_n(1) = 1 for every n.
    for n in 0..=12u32 {
        assert!((mirror::legendre_p(n, 1.0) - 1.0).abs() < 1e-6, "P_{n}(1)");
    }
    // erf family: the complementary identity.
    for k in 0..=40 {
        let x = -2.0 + 0.1 * k as f32;
        let s = mirror_erf::erf(x) + mirror_erf::erfc(x);
        assert!((s - 1.0).abs() < 1e-5, "erf + erfc at {x} is {s}");
    }
}

/// Every shader validates under naga's **baseline** capabilities.
///
/// # Why baseline specifically
///
/// `Capabilities::default()` is what a conforming WebGPU device without
/// optional features supports. Validating against it is the check that these
/// kernels run in a browser and on mobile, not merely on the desktop adapter
/// that happens to be present — and it is a check that needs no adapter at
/// all, so CI performs it on every platform.
///
/// In particular this would reject `f64` in a shader, which is an extension
/// most targets do not have. That is the constraint this whole module is
/// built around.
#[test]
fn every_shader_validates_against_baseline_webgpu_capabilities() {
    for (name, src) in ALL_NAMES.iter().zip(ALL.iter()) {
        let call = match *name {
            "poly" => "petir_poly_eval(0u, params.n, x)",
            "cheb" => "petir_cheb_eval(0u, params.n, params.a, params.b, x)",
            "legendre" => "petir_legendre_p(params.k, x)",
            "erf" => "petir_erfc(x)",
            "matrix" => "petir_blas_dot(0u, 0u, params.n)",
            other => panic!("no baseline call registered for {other}.wgsl"),
        };
        let kernel = test_kernel(&[src], call);
        let module = naga::front::wgsl::parse_str(&kernel)
            .unwrap_or_else(|e| panic!("{name}.wgsl failed to parse: {e:?}"));
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        validator.validate(&module).unwrap_or_else(|e| {
            panic!("{name}.wgsl needs a capability a baseline device lacks: {e:?}")
        });
        // Belt and braces on top of naga. Comments are stripped first: every
        // one of these files says in its header that it was transcribed from
        // PETIR's `f64` code, and scanning that text reported a violation
        // that does not exist.
        let code_only: String = src
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code_only.contains("f64"),
            "{name}.wgsl uses f64 in code, which baseline WebGPU does not have"
        );
    }
}

/// The coverage ledger's **PORTED** rows match what the crate actually ships.
///
/// # Why a doc needs a test
///
/// `docs/wgsl-coverage.md` is the answer to "how much of GSL is in WGSL". A
/// coverage document nobody checks drifts within a release or two, and drifts
/// in one direction — claiming more than is there. This pins the direction
/// that matters: every shader the crate ships must appear in the ledger, so a
/// new kernel cannot land without the ledger being updated in the same change.
///
/// It deliberately does **not** try to parse the whole table. The claim being
/// enforced is narrow and mechanical — names present, measured figures
/// present — and a test that tried to validate prose would be worse than none.
#[test]
fn the_coverage_ledger_lists_every_shipped_shader() {
    const LEDGER: &str = include_str!("../docs/wgsl-coverage.md");

    // Every shader source in the crate must be named somewhere in the ledger.
    for name in ALL_NAMES.iter() {
        let mentioned = match *name {
            // The ledger names GSL modules; these are the rows that cover
            // each shader file.
            "poly" => LEDGER.contains("`poly`"),
            "cheb" => LEDGER.contains("`cheb`"),
            "erf" => LEDGER.contains("erf family"),
            "legendre" => LEDGER.contains("Legendre"),
            "matrix" => LEDGER.contains("`matrix`") && LEDGER.contains("`blas`"),
            other => panic!("shader {other}.wgsl has no row in docs/wgsl-coverage.md"),
        };
        assert!(
            mentioned,
            "shader {name}.wgsl is shipped but has no row in docs/wgsl-coverage.md"
        );
    }

    // The ledger must state the verification standard it holds PORTED to,
    // because "ported" without it is an unfalsifiable word.
    for required in [
        "f32` CPU mirror",
        "naga validation",
        "GPU dispatch test",
        "bit-identical",
    ] {
        assert!(
            LEDGER.contains(required),
            "the ledger no longer states '{required}' -- the verification \
             standard is what makes PORTED mean anything"
        );
    }

    // And it must keep the two caveats that stop its numbers being misread.
    assert!(
        LEDGER.contains("ULP bound rather than correct rounding"),
        "the ledger must explain why erf is not bit-identical"
    );
    assert!(
        LEDGER.contains("reassociation"),
        "the ledger must explain the gemm summation-order deviation"
    );
}
