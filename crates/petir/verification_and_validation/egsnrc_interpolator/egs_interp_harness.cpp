// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// Differential-test harness for petir::interp_egs (gh:#1289).
//
// Compiles UNMODIFIED EGSnrc egs_interpolator.cpp (commit
// bd49a47aae3938f49f4100b8e018091861408b19, read from the gitignored clone at
// vendor/egsnrc) and records, as exact IEEE-754 bits, everything the Rust
// test replays: the coefficients a[i], b[i], the sequence of x at which the
// function was called, and interpolate / getIndex / interpolateFast /
// getIndexFast / interpolateFast(i, x) on a fixed query set. See
// build_and_run.sh and README.md beside this file.
//
// The query generator (splitmix64) and digest (FNV-1a over 64-bit words) are
// mirrored exactly in crates/petir/tests/egs_interpolator_vs_egsnrc.rs.
//
// Output (stdout): the committed reference file. With a second argument, a
// full per-value dump is written there too.

#include <cmath>
#include <cstdarg>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <limits>
#include <stdexcept>
#include <string>
#include <vector>

#include "egs_functions.h"
#include "egs_interpolator.h"

// ---- egsFatal: throw instead of exit, so the guards can be observed ----
static void harness_fatal(const char *fmt, ...) {
    char buf[512];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof buf, fmt, ap);
    va_end(ap);
    for (char *p = buf; *p; ++p)
        if (*p == '\n') *p = ' ';
    throw std::runtime_error(buf);
}
EGS_InfoFunction egsFatal = harness_fatal;

// ---- bits, generator, digest (mirrored in the Rust test) ----
static uint64_t bits(double x) {
    uint64_t u;
    std::memcpy(&u, &x, 8);
    return u;
}
static double from_bits(uint64_t u) {
    double x;
    std::memcpy(&x, &u, 8);
    return x;
}
struct SplitMix {
    uint64_t s;
    uint64_t next() {
        s += 0x9E3779B97F4A7C15ULL;
        uint64_t z = s;
        z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
        z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
        return z ^ (z >> 31);
    }
    double uniform() { return (double)(next() >> 11) * 0x1.0p-53; }
};
struct Digest {
    uint64_t h = 0xcbf29ce484222325ULL;
    uint64_t count = 0;
    void word(uint64_t w) {
        for (int k = 0; k < 8; ++k) {
            h ^= (w >> (8 * k)) & 0xff;
            h *= 0x100000001b3ULL;
        }
        ++count;
    }
};

// ---- the functions (identical expressions on the Rust side) ----
struct Recorder {
    int which;
    Digest calls;
};
static EGS_Float the_function(EGS_Float x, void *data) {
    Recorder *r = (Recorder *)data;
    r->calls.word(bits(x));
    switch (r->which) {
    case 0: return std::exp(x);                    // exp
    case 1: return std::pow(x, -1.5);              // power law
    case 2: return 1.0 / (1.0 + x * x);            // rational (no libm)
    case 3: return std::exp(-0.8 * x) + 2.0;       // sigma(E) in u = ln E
    }
    return 0.0;
}

static FILE *dump = nullptr;
static const char *case_name = "";

static void out_value(Digest &d, const char *kind, double x, uint64_t y) {
    d.word(y);
    if (dump) std::fprintf(dump, "%s %s %016llx %016llx\n", case_name, kind,
                           (unsigned long long)bits(x), (unsigned long long)y);
}

static void run_case(int case_index, const char *name, EGS_Interpolator &f, Recorder &rec) {
    case_name = name;
    double xmin = f.getXmin(), xmax = f.getXmax();
    int n = f.getIndex(xmax) + 1;  // getIndex(x >= xmax) returns n - 1
    Digest coef, eval;
    coef.word(bits(xmin));
    coef.word(bits(xmax));
    for (int i = 0; i <= n; ++i) {
        out_value(coef, "a", (double)i, bits(f.get_a(i)));
        out_value(coef, "b", (double)i, bits(f.get_b(i)));
    }
    std::vector<double> q;
    double dx = (xmax - xmin) / n;
    for (int j = 0; j <= n; ++j) q.push_back(xmin + dx * j);
    for (int j = 0; j < n; ++j) q.push_back(xmin + dx * (j + 0.5));
    double L = xmax - xmin;
    double inf = std::numeric_limits<double>::infinity();
    double specials[] = {xmin, xmax,
                         std::nextafter(xmin, -inf), std::nextafter(xmin, inf),
                         std::nextafter(xmax, -inf), std::nextafter(xmax, inf),
                         xmin - 1.0, xmax + 1.0, xmin - L, xmax + L,
                         -inf, inf, std::numeric_limits<double>::quiet_NaN(),
                         0.0, -0.0};
    size_t n_special = sizeof specials / sizeof specials[0];
    size_t special_start = q.size();
    for (size_t k = 0; k < n_special; ++k) q.push_back(specials[k]);
    SplitMix g{12345ULL + (uint64_t)case_index};
    double lo = xmin - 0.1 * L, hi = xmax + 0.1 * L;
    size_t random_start = q.size();
    for (int k = 0; k < 100000; ++k) q.push_back(lo + (hi - lo) * g.uniform());

    for (size_t k = 0; k < q.size(); ++k) {
        double x = q[k];
        double y = f.interpolate(x);
        int i = f.getIndex(x);
        out_value(eval, "interpolate", x, bits(y));
        out_value(eval, "getIndex", x, (uint64_t)(int64_t)i);
        if (x >= xmin && x <= xmax) {
            out_value(eval, "interpolateFast", x, bits(f.interpolateFast(x)));
            out_value(eval, "getIndexFast", x, (uint64_t)(int64_t)f.getIndexFast(x));
            out_value(eval, "interpolateFastAt", x, bits(f.interpolateFast(i, x)));
        }
        bool vec = (k >= special_start && k < special_start + n_special) ||
                   (k >= random_start && k < random_start + 16);
        if (vec)
            std::printf("vec %s %016llx %016llx %d\n", name, (unsigned long long)bits(x),
                        (unsigned long long)bits(y), i);
    }
    for (int i = 0; i <= n && i < 8; ++i)
        std::printf("coef %s %d %016llx %016llx\n", name, i,
                    (unsigned long long)bits(f.get_a(i)), (unsigned long long)bits(f.get_b(i)));
    std::printf("digest %s bins %d\n", name, n);
    std::printf("digest %s calls %llu %016llx\n", name, (unsigned long long)rec.calls.count,
                (unsigned long long)rec.calls.h);
    std::printf("digest %s coef %llu %016llx\n", name, (unsigned long long)coef.count,
                (unsigned long long)coef.h);
    std::printf("digest %s eval %llu %016llx\n", name, (unsigned long long)eval.count,
                (unsigned long long)eval.h);
}

static std::vector<double> random_table(uint64_t seed, int len, double scale, double shift) {
    SplitMix g{seed};
    std::vector<double> v;
    for (int k = 0; k < len; ++k) v.push_back(g.uniform() * scale + shift);
    return v;
}

static void fatal_case(const char *name, void (*fn)()) {
    try {
        fn();
        std::printf("fatal %s none\n", name);
    } catch (const std::exception &e) {
        std::printf("fatal %s raised: %s\n", name, e.what());
    }
}

int main(int argc, char **argv) {
    if (argc > 1) dump = std::fopen(argv[1], "w");
    std::printf("# EGSnrc egs_interpolator reference, commit bd49a47aae3938f49f4100b8e018091861408b19\n");
    std::printf("# written by verification_and_validation/egsnrc_interpolator/egs_interp_harness.cpp\n");
    int ci = 0;
    {   // T1: random table, passed identically to both sides
        Recorder r{-1, {}};
        std::vector<double> v = random_table(1, 33, 10.0, -5.0);
        EGS_Interpolator f;
        f.initialize(33, 0.25, 7.0, v.data());
        run_case(ci++, "values_random", f, r);
    }
    {   // T2: a step table with a jump and sign changes
        Recorder r{-1, {}};
        double v[] = {0, 0, 1, 1, 1, -2, -2, 5, 5};
        EGS_Interpolator f;
        f.initialize(9, -1.0, 1.0, v);
        run_case(ci++, "values_steps", f, r);
    }
    {   Recorder r{0, {}}; EGS_Interpolator f;
        f.initialize(65, 0.0, 5.0, the_function, &r);
        run_case(ci++, "func_exp", f, r); }
    {   Recorder r{1, {}}; EGS_Interpolator f;
        f.initialize(200, 0.1, 10.0, the_function, &r);
        run_case(ci++, "func_powlaw", f, r); }
    {   Recorder r{2, {}}; EGS_Interpolator f;
        f.initialize(101, -3.0, 3.0, the_function, &r);
        run_case(ci++, "func_rational", f, r); }
    {   Recorder r{3, {}}; EGS_Interpolator f;
        f.initialize(128, -6.907755278982137, 2.995732273553991, the_function, &r);
        run_case(ci++, "func_logvar", f, r); }
    {   Recorder r{0, {}}; EGS_Interpolator f;
        f.initialize(0.0, 5.0, the_function, &r, 1024, 1e-4);
        run_case(ci++, "adaptive_exp", f, r); }
    {   Recorder r{1, {}}; EGS_Interpolator f;
        f.initialize(0.1, 10.0, the_function, &r, 4096, 1e-6);
        run_case(ci++, "adaptive_powlaw", f, r); }
    {   Recorder r{2, {}}; EGS_Interpolator f;
        f.initialize(-3.0, 3.0, the_function, &r, 8, 1e-4);
        run_case(ci++, "adaptive_rational_small", f, r); }
    {   Recorder r{0, {}}; EGS_Interpolator f;
        f.initialize(0.0, 30.0, the_function, &r, 100, 1e-8);
        run_case(ci++, "adaptive_cap", f, r); }
    {   Recorder r{2, {}}; EGS_Interpolator f;
        f.initialize(-3.0, 3.0, the_function, &r, 1024, 1e-5);
        run_case(ci++, "adaptive_rational", f, r); }
    {   Recorder r{3, {}}; EGS_Interpolator f;
        f.initialize(-6.907755278982137, 2.995732273553991, the_function, &r, 1024, 1e-4);
        run_case(ci++, "adaptive_logvar", f, r); }
    {   // C1: existing coefficients (the caller keeps them alive here)
        Recorder r{-1, {}};
        static std::vector<double> a = random_table(2, 21, 4.0, -2.0);
        static std::vector<double> b = random_table(3, 21, 2.0, -1.0);
        EGS_Interpolator f;
        f.initialize(21, 1.0, 4.0, a.data(), b.data());
        run_case(ci++, "coefficients", f, r);
    }

    // Guards, and the constructor defect (Xmax passed for Xmin).
    fatal_case("values_nbin1", [] { double v[] = {1}; EGS_Interpolator f(1, 0.0, 1.0, (const EGS_Float *)v); });
    fatal_case("values_equal_ends", [] { double v[] = {1, 2}; EGS_Interpolator f(2, 1.0, 1.0, (const EGS_Float *)v); });
    fatal_case("values_reversed", [] { double v[] = {1, 2}; EGS_Interpolator f(2, 2.0, 1.0, (const EGS_Float *)v); });
    fatal_case("values_nan_xmin", [] { double v[] = {1, 2}; EGS_Interpolator f(2, std::nan(""), 1.0, (const EGS_Float *)v); });
    fatal_case("adaptive_nmax1", [] { Recorder r{2, {}}; EGS_Interpolator f(0.0, 1.0, the_function, &r, 1, 1e-4); });
    fatal_case("ctor_function_defect", [] { Recorder r{2, {}}; EGS_Interpolator f(5, 0.0, 1.0, the_function, &r); });
    fatal_case("ctor_coefficients_defect", [] {
        static double a[] = {1, 2, 3}, b[] = {1, 2, 3};
        EGS_Interpolator f(3, 0.0, 1.0, a, b); });
    fatal_case("initialize_function_ok", [] { Recorder r{2, {}}; EGS_Interpolator f; f.initialize(5, 0.0, 1.0, the_function, &r); });
    if (dump) std::fclose(dump);
    return 0;
}
