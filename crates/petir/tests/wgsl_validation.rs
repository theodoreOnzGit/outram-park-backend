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

use petir::wgsl::{
    test_kernel, AIRY, ALL, ALL_NAMES, ATANINT, BESSEL, CHEB, CLAUSEN, DAWSON, DEBYE, DILOG, ERF,
    EXPINT3, FERMI_DIRAC, GAMMA, LAMBERT, LEGENDRE, MATRIX, POLY, PSI_ZETA, SININT, SYNCHROTRON,
    TRANSPORT,
};

/// The sources a shader needs concatenated ahead of it, and a call that
/// exercises it.
///
/// Only one source has a dependency today: `psi_zeta.wgsl`'s `petir_zeta`
/// calls `petir_gamma` rather than duplicating the Lanczos table. Returning a
/// list rather than a single source is what lets that stay true — the
/// alternative was a second copy of nine coefficients, which is exactly the
/// drift this crate's audits exist to prevent.
fn kernel_for(name: &str) -> (Vec<&'static str>, &'static str) {
    match name {
        "poly" => (vec![POLY], "petir_poly_eval(0u, params.n, x)"),
        "cheb" => (
            vec![CHEB],
            "petir_cheb_eval(0u, params.n, params.a, params.b, x)",
        ),
        "legendre" => (vec![LEGENDRE], "petir_legendre_p(params.k, x)"),
        "erf" => (vec![ERF], "petir_erfc(x)"),
        "matrix" => (vec![MATRIX], "petir_blas_dot(0u, 0u, params.n)"),
        "gamma" => (vec![GAMMA], "petir_lngamma(x)"),
        "bessel" => (vec![BESSEL], "petir_bessel_j0(x)"),
        "psi_zeta" => (vec![GAMMA, PSI_ZETA], "petir_psi(x) + petir_zeta(x)"),
        "debye" => (vec![DEBYE], "petir_debye(3u, x)"),
        "dilog" => (vec![DILOG], "petir_dilog(x)"),
        "airy" => (vec![AIRY], "petir_airy_ai(x) + petir_airy_bi_scaled(x)"),
        "lambert" => (vec![LAMBERT], "petir_lambert_w0(x)"),
        "clausen" => (vec![CLAUSEN], "petir_clausen(x)"),
        "transport" => (vec![TRANSPORT], "petir_transport(4u, x)"),
        "atanint" => (vec![ATANINT], "petir_atanint(x)"),
        "synchrotron" => (
            vec![SYNCHROTRON],
            "petir_synchrotron_1(x) + petir_synchrotron_2(x)",
        ),
        "fermi_dirac" => (vec![FERMI_DIRAC], "petir_fermi_dirac(params.k, x)"),
        "dawson" => (vec![DAWSON], "petir_dawson(x)"),
        "expint3" => (vec![EXPINT3], "petir_expint_3(x)"),
        "sinint" => (vec![SININT], "petir_si(x) + petir_ci(abs(x) + 1.0)"),
        other => panic!("no validation call registered for {other}.wgsl"),
    }
}

/// [`ALL`] and [`ALL_NAMES`] are the same length.
///
/// # Why this is its own test
///
/// Every sweep below is `ALL_NAMES.iter().zip(ALL.iter())`, and `zip` stops
/// at the shorter operand. A shader added to `ALL` without a name in
/// `ALL_NAMES` is therefore not a failure — it is silently dropped from naga
/// validation, from the baseline-capability check and from the ledger check,
/// all of which stay green while covering one fewer shader.
///
/// This is not hypothetical: `psi_zeta.wgsl` shipped that way for exactly one
/// test run. The lengths are in the types, so this could be a compile-time
/// check — but this crate's `no_panic_gate` forbids `assert!` in library
/// code, and a const assertion there is what it would take.
#[test]
fn all_and_all_names_are_the_same_length() {
    assert_eq!(
        ALL.len(),
        ALL_NAMES.len(),
        "ALL has {} sources and ALL_NAMES has {} names; the zip in every \
         other test here would silently skip the difference",
        ALL.len(),
        ALL_NAMES.len()
    );
}

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
        let (sources, call) = kernel_for(name);
        assert!(
            sources.contains(src),
            "kernel_for({name}) does not include its own source"
        );
        let kernel = test_kernel(&sources, call);
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
    let expected: [(&str, &[&str]); 20] = [
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
        (
            GAMMA,
            &[
                "petir_lngamma",
                "petir_gamma",
                "petir_lnbeta",
                "petir_beta",
                "petir_lngamma_lanczos",
                "petir_lngamma_1_pade",
                "petir_lngamma_2_pade",
            ],
        ),
        (
            BESSEL,
            &[
                "petir_bessel_j0",
                "petir_bessel_j1",
                "petir_bessel_y0",
                "petir_bessel_y1",
                "petir_bessel_i0",
                "petir_bessel_i1",
                "petir_bessel_k0",
                "petir_bessel_k1",
                "petir_bessel_i0_scaled",
                "petir_bessel_i1_scaled",
                "petir_bessel_k0_scaled",
                "petir_bessel_k1_scaled",
                "petir_bessel_cos_pi4",
                "petir_bessel_sin_pi4",
                "petir_bessel_sin_cos_eps",
            ],
        ),
        (
            PSI_ZETA,
            &[
                "petir_psi",
                "petir_psi_1",
                "petir_psi_1piy",
                "petir_hzeta",
                "petir_zeta",
                "petir_zetam1",
                "petir_eta",
            ],
        ),
        (
            DEBYE,
            &[
                "petir_debye",
                "petir_debye_cheb",
                "petir_debye_fall",
                "petir_debye_fall_x",
                "petir_debye_pow_n",
            ],
        ),
        (
            DILOG,
            &[
                "petir_dilog",
                "petir_dilog_xge0",
                "petir_dilog_series_1",
                "petir_dilog_series_2",
                "petir_dilog_series_2_raw",
            ],
        ),
        (
            AIRY,
            &[
                "petir_airy_ai",
                "petir_airy_ai_scaled",
                "petir_airy_bi",
                "petir_airy_bi_scaled",
                "petir_airy_mod_phase",
                "petir_airy_aie",
                "petir_airy_bie",
            ],
        ),
        (
            LAMBERT,
            &[
                "petir_lambert_w0",
                "petir_lambert_wm1",
                "petir_lambert_halley",
                "petir_lambert_series",
            ],
        ),
        (
            CLAUSEN,
            &[
                "petir_clausen",
                "petir_clausen_reduce",
                "petir_clausen_cheb",
            ],
        ),
        (
            TRANSPORT,
            &[
                "petir_transport",
                "petir_transport_sumexp",
                "petir_transport_cheb",
                "petir_transport_vinf",
            ],
        ),
        (ATANINT, &["petir_atanint", "petir_atanint_cheb"]),
        (
            SYNCHROTRON,
            &[
                "petir_synchrotron_1",
                "petir_synchrotron_2",
                "petir_synch_pow_int",
                "petir_synch_cheb_synch1",
                "petir_synch_cheb_synch2",
                "petir_synch_cheb_synch1a",
                "petir_synch_cheb_synch21",
                "petir_synch_cheb_synch22",
                "petir_synch_cheb_synch2a",
            ],
        ),
        (
            FERMI_DIRAC,
            &[
                "petir_fermi_dirac",
                "petir_fd_m1",
                "petir_fd_0",
                "petir_fd_1",
                "petir_fd_2",
                "petir_fd_mhalf",
                "petir_fd_half",
                "petir_fd_3half",
                "petir_fd_asymp",
                "petir_fd_series",
                "petir_fd_series_half",
                "petir_fd_eta",
            ],
        ),
        (
            DAWSON,
            &[
                "petir_dawson",
                "petir_dawson_cheb_daw",
                "petir_dawson_cheb_daw2",
                "petir_dawson_cheb_dawa",
            ],
        ),
        (
            EXPINT3,
            &[
                "petir_expint_3",
                "petir_expint3_cheb_expint3",
                "petir_expint3_cheb_expint3a",
            ],
        ),
        (
            SININT,
            &[
                "petir_si",
                "petir_ci",
                "petir_si_fg_asymp",
                "petir_si_sin_cos",
                "petir_sinint_cheb_f1",
                "petir_sinint_cheb_f2",
                "petir_sinint_cheb_g1",
                "petir_sinint_cheb_g2",
                "petir_sinint_cheb_si",
                "petir_sinint_cheb_ci",
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
    use petir::wgsl::{mirror, mirror_bessel, mirror_erf, mirror_psi_zeta};

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
    // Digamma: the recurrence psi(x+1) - psi(x) = 1/x, which crosses the
    // module's branch cuts.
    for k in 1..=40 {
        let x = 0.25 * k as f32;
        let d = mirror_psi_zeta::psi(x + 1.0) - mirror_psi_zeta::psi(x);
        assert!(
            ((d - 1.0 / x) * x).abs() < 1e-4,
            "psi recurrence at {x} is {d}, expected {}",
            1.0 / x
        );
    }
    // Bessel family: the J/Y Wronskian, which no single table can satisfy on
    // its own.
    for k in 1..=40 {
        let x = 0.25 * k as f32;
        let w = mirror_bessel::j0(x) * mirror_bessel::y1(x)
            - mirror_bessel::j1(x) * mirror_bessel::y0(x);
        let exact = -2.0 / (core::f32::consts::PI * x);
        assert!(
            ((w - exact) / exact).abs() < 1e-4,
            "J/Y Wronskian at {x} is {w}, expected {exact}"
        );
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
        let (sources, call) = kernel_for(name);
        let kernel = test_kernel(&sources, call);
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
            "gamma" => LEDGER.contains("gamma family"),
            "bessel" => LEDGER.contains("Bessel family"),
            "psi_zeta" => LEDGER.contains("psi/zeta family"),
            "debye" => LEDGER.contains("Debye family"),
            "dilog" => LEDGER.contains("dilogarithm"),
            "airy" => LEDGER.contains("Airy family"),
            "lambert" => LEDGER.contains("Lambert"),
            "clausen" => LEDGER.contains("Clausen"),
            "transport" => LEDGER.contains("transport integrals"),
            "atanint" => LEDGER.contains("inverse-tangent integral"),
            "synchrotron" => LEDGER.contains("synchrotron"),
            // NOT just "Fermi-Dirac": that string was already in the
            // PORTABLE row listing it as a future block, so the check passed
            // before the row existed. Match the row itself.
            "fermi_dirac" => LEDGER.contains("(Fermi-Dirac integrals)"),
            "dawson" => LEDGER.contains("Dawson"),
            "expint3" => LEDGER.contains("cubic exponential integral"),
            "sinint" => LEDGER.contains("sine and cosine integrals"),
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

/// Every generated shader holds **identical** coefficients to its mirror.
///
/// # Why this needs a test when both are generated
///
/// Each pair was emitted by one script from one parse of the `f64` module, so
/// they agree today by construction. That guarantee expires the moment anyone
/// hand-edits either file — and a mirror that disagreed with its shader would
/// make GPU-vs-mirror measure the difference between two tables rather than
/// the fidelity of the transcription, which is the one thing that comparison
/// exists to establish. The failure would be silent and would look like a
/// device problem.
///
/// # What it covers
///
/// **Every generated pair**, 96 tables and 1750 coefficients as of
/// 2026-09-19:
///
/// | shader | tables | coefficients |
/// |---|---|---|
/// | `fermi_dirac` | 23 | 492 |
/// | `bessel` | 22 | 358 |
/// | `airy` | 13 | 281 |
/// | `psi_zeta` | 8 | 155 |
/// | `debye` | 6 | 103 |
/// | `synchrotron` | 6 | 91 |
/// | `transport` | 4 | 72 |
/// | `atanint` | 1 | 21 |
/// | `clausen` | 1 | 15 |
/// | `gamma` | 1 | 9 |
/// | `dawson` | 3 | 45 |
/// | `expint3` | 2 | 27 |
/// | `sinint` | 6 | 81 |
///
/// ~~Covers `bessel.wgsl` and `psi_zeta.wgsl`.~~ **CORRECTED 2026-09-19** —
/// the doc comment claimed `psi_zeta` was covered and the body compared
/// `bessel` alone, so seven of the nine pairs above were unchecked. The
/// sweep below is now driven by a list, so adding a shader to it is the
/// whole change.
///
/// **`erf` and `lambert` are deliberately absent.** `mirror_erf` predates the
/// generator and holds its tables in a different order, and `lambert.wgsl`
/// declares no `array<f32, N>` at all — its two series are unrolled. Neither
/// is a generated pair, so there is nothing here for this test to protect.
///
/// Only the *array bodies* are compared — `array<f32, N>(...)` on one side
/// and `const NAME: [f32; N] = [...]` on the other — because the surrounding
/// code is full of literals (`0.0`, `0.5`, `2.75`) that legitimately appear
/// in different places on the two sides. **Comments are stripped from both
/// first**: several shader headers discuss `array<f32, N>` in prose, and
/// without stripping, each such sentence was parsed as a one-element table
/// and shifted every comparison after it by one.
#[test]
fn every_generated_shader_and_its_mirror_hold_the_same_constants() {
    /// Every `f32` in `body`, which is assumed to be nothing but a comma-list
    /// of literals.
    fn parse(body: &str) -> Vec<f32> {
        body.split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| {
                // Rust digit separators are legal on the mirror side and
                // never appear on the shader side, so strip them first.
                let t = t.replace('_', "");
                t.parse::<f32>()
                    .unwrap_or_else(|_| panic!("not a literal in a coefficient table: {t:?}"))
            })
            .collect()
    }

    /// `src` with every `//` line comment removed, so prose that happens to
    /// mention a table declaration is not parsed as one.
    fn code_only(src: &str) -> String {
        src.lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Bodies of every `array<f32, N>( ... )` in the shader, in order.
    fn shader_tables(src: &str) -> Vec<Vec<f32>> {
        let code = code_only(src);
        let mut out = Vec::new();
        let mut rest = code.as_str();
        while let Some(at) = rest.find("array<f32, ") {
            rest = &rest[at..];
            let Some(open) = rest.find('(') else { break };
            let Some(close) = rest[open..].find(')') else {
                break;
            };
            out.push(parse(&rest[open + 1..open + close]));
            rest = &rest[open + close..];
        }
        out
    }

    /// Bodies of every `const NAME: [f32; N] = [ ... ];` in the mirror, in
    /// order.
    fn mirror_tables(src: &str) -> Vec<Vec<f32>> {
        let code = code_only(src);
        let mut out = Vec::new();
        let mut rest = code.as_str();
        while let Some(at) = rest.find(": [f32; ") {
            rest = &rest[at..];
            let Some(eq) = rest.find("] = [") else { break };
            let start = eq + 5;
            let Some(close) = rest[start..].find(']') else {
                break;
            };
            out.push(parse(&rest[start..start + close]));
            rest = &rest[start + close..];
        }
        out
    }

    // (name, shader source, mirror source, tables, coefficients). The counts
    // are asserted so that a table silently lost from either side fails here
    // rather than shrinking the comparison.
    let pairs: &[(&str, &str, &str, usize, usize)] = &[
        (
            "sinint",
            petir::wgsl::SININT,
            include_str!("../src/wgsl/mirror_sinint.rs"),
            6,
            81,
        ),
        (
            "expint3",
            petir::wgsl::EXPINT3,
            include_str!("../src/wgsl/mirror_expint3.rs"),
            2,
            27,
        ),
        (
            "dawson",
            petir::wgsl::DAWSON,
            include_str!("../src/wgsl/mirror_dawson.rs"),
            3,
            45,
        ),
        (
            "fermi_dirac",
            petir::wgsl::FERMI_DIRAC,
            include_str!("../src/wgsl/mirror_fermi_dirac.rs"),
            23,
            492,
        ),
        (
            "bessel",
            petir::wgsl::BESSEL,
            include_str!("../src/wgsl/mirror_bessel.rs"),
            22,
            358,
        ),
        (
            "airy",
            petir::wgsl::AIRY,
            include_str!("../src/wgsl/mirror_airy.rs"),
            13,
            281,
        ),
        (
            "psi_zeta",
            petir::wgsl::PSI_ZETA,
            include_str!("../src/wgsl/mirror_psi_zeta.rs"),
            8,
            155,
        ),
        (
            "debye",
            petir::wgsl::DEBYE,
            include_str!("../src/wgsl/mirror_debye.rs"),
            6,
            103,
        ),
        (
            "synchrotron",
            petir::wgsl::SYNCHROTRON,
            include_str!("../src/wgsl/mirror_synchrotron.rs"),
            6,
            91,
        ),
        (
            "transport",
            petir::wgsl::TRANSPORT,
            include_str!("../src/wgsl/mirror_transport.rs"),
            4,
            72,
        ),
        (
            "atanint",
            petir::wgsl::ATANINT,
            include_str!("../src/wgsl/mirror_atanint.rs"),
            1,
            21,
        ),
        (
            "clausen",
            petir::wgsl::CLAUSEN,
            include_str!("../src/wgsl/mirror_clausen.rs"),
            1,
            15,
        ),
        (
            "gamma",
            petir::wgsl::GAMMA,
            include_str!("../src/wgsl/mirror_gamma.rs"),
            1,
            9,
        ),
    ];

    let mut grand = 0usize;
    for (name, shader_src, mirror_src, tables, coeffs) in pairs {
        let shader = shader_tables(shader_src);
        let mirror = mirror_tables(mirror_src);

        assert_eq!(
            shader.len(),
            *tables,
            "{name}.wgsl declares {} coefficient arrays, expected {tables}",
            shader.len()
        );
        assert_eq!(
            mirror.len(),
            *tables,
            "mirror_{name}.rs declares {} coefficient arrays, expected {tables}",
            mirror.len()
        );
        let total: usize = shader.iter().map(Vec::len).sum();
        assert_eq!(
            total, *coeffs,
            "{name}: expected {coeffs} coefficients, found {total}"
        );
        grand += total;

        for (t, (a, b)) in shader.iter().zip(mirror.iter()).enumerate() {
            assert_eq!(
                a.len(),
                b.len(),
                "{name} table {t} has {} vs {} values",
                a.len(),
                b.len()
            );
            for (k, (x, y)) in a.iter().zip(b.iter()).enumerate() {
                assert_eq!(
                    x.to_bits(),
                    y.to_bits(),
                    "{name} table {t} coefficient {k}: {name}.wgsl has {x:e}, \
                     mirror_{name}.rs has {y:e}"
                );
            }
        }
    }
    assert_eq!(
        grand, 1750,
        "the generated pairs are documented as holding 1750 coefficients in \
         total; this run compared {grand}"
    );
}
