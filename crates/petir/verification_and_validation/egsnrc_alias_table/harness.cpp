// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// Differential-test harness for PETIR's port of EGSnrc's alias tables
// (crates/petir/src/rng/alias_table.rs, gh:#1289).
//
// It links UPSTREAM'S OWN egs_alias_table.cpp (compiled from the vendored,
// gitignored EGSnrc tree, commit bd49a47aae3938f49f4100b8e018091861408b19)
// and drives it with a REPLAY random generator: a stub EGS_RandomGenerator
// whose fillArray() copies from a fixed list of uniforms k * 2^-53. The Rust
// side replays the same list through PETIR's `Rng` trait
// (next_u64 = k << 11, so random::<f64>() = k * 2^-53 exactly).
//
// Output, into the directory given as argv[1]:
//   <case>.txt   every table entry and every sample, as exact hex bits;
//   summary.txt  per case: FNV-1a digests, first samples, small tables.
//
// The case list and every input recipe are mirrored in
// crates/petir/tests/alias_table_vs_egsnrc.rs. Inputs use only + - * / and
// ldexp, so both languages produce the same input bits.

#include "egs_alias_table.h"
#include "egs_functions.h"

#include <cmath>
#include <cstdarg>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

// ---------------------------------------------------------------- stubs ----
// egs_functions.cpp's three message hooks. A warning is counted (so the
// harness can report it) and printed; fatal exits.
static int g_warnings = 0;
static void h_info(const char *fmt, ...) {
    va_list a;
    va_start(a, fmt);
    vfprintf(stderr, fmt, a);
    va_end(a);
}
static void h_warn(const char *fmt, ...) {
    ++g_warnings;
    va_list a;
    va_start(a, fmt);
    vfprintf(stderr, fmt, a);
    va_end(a);
    fprintf(stderr, "\n");
}
static void h_fatal(const char *fmt, ...) {
    va_list a;
    va_start(a, fmt);
    vfprintf(stderr, fmt, a);
    va_end(a);
    exit(2);
}
EGS_InfoFunction egsInformation = h_info;
EGS_InfoFunction egsWarning = h_warn;
EGS_InfoFunction egsFatal = h_fatal;

// egs_rndm.cpp:49-81, the two base-class members the header leaves
// out of line (copied in behaviour: allocate n, ip = np, zero the array).
void EGS_RandomGenerator::allocate(int n) {
    if (n < 1) {
        egsFatal("Attempt to construct a RNG with n < 1\n");
    }
    rarray = new EGS_Float[n];
    np = n;
    ip = np;
}
EGS_RandomGenerator::EGS_RandomGenerator(int n) : count(0), np(0) {
    allocate(n);
    have_x = false;
    for (int j = 0; j < np; j++) {
        rarray[j] = 0;
    }
}

// --------------------------------------------------------- replay RNG ----
class ReplayRng : public EGS_RandomGenerator {
public:
    ReplayRng(const std::vector<uint64_t> &k) : EGS_RandomGenerator(128), ks(k), pos(0) {}
    void fillArray(int n, EGS_Float *array) override {
        for (int i = 0; i < n; i++) {
            array[i] = ldexp((double)ks[pos % ks.size()], -53);
            ++pos;
        }
        count += n;
    }
    EGS_RandomGenerator *getCopy() override { return 0; }
    void setState(EGS_RandomGenerator *) override {}
    void saveState() override {}
    void resetState() override {}
    int rngSize() const override { return 0; }
    bool storePrivateState(ostream &) override { return true; }
    bool setPrivateState(istream &) override { return true; }

private:
    std::vector<uint64_t> ks;
    size_t pos;
};

// ----------------------------------------------------------- recipes ----
static uint64_t splitmix64(uint64_t &s) {
    s += 0x9E3779B97F4A7C15ULL;
    uint64_t z = s;
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
    z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
    return z ^ (z >> 31);
}

static const size_t STREAM_LEN = 250000;
static const int NSAMPLE = 100000;

// The uniform stream: five edge values first (0, the largest uniform below 1,
// 1/2, the smallest positive, the second largest), then splitmix64 >> 11.
static std::vector<uint64_t> uniform_stream() {
    std::vector<uint64_t> k;
    const uint64_t top = (1ULL << 53) - 1;
    k.push_back(0);
    k.push_back(top);
    k.push_back(1ULL << 52);
    k.push_back(1);
    k.push_back(top - 1);
    uint64_t s = 1289;
    while (k.size() < STREAM_LEN) k.push_back(splitmix64(s) >> 11);
    return k;
}

// u^3 with u = (splitmix64 >> 11) * 2^-53: weights spanning decades.
static std::vector<double> cubed(uint64_t seed, int n) {
    std::vector<double> f(n);
    uint64_t s = seed;
    for (int i = 0; i < n; i++) {
        double u = ldexp((double)(splitmix64(s) >> 11), -53);
        f[i] = u * u * u;
    }
    return f;
}

static const double ZEROS[13] = {0, 3, 0, 0, 1, 0, 7, 0, 2, 0, 0, 5, 0};

// ------------------------------------------------------------- output ----
static uint64_t bits(double x) {
    uint64_t b;
    memcpy(&b, &x, 8);
    return b;
}
struct Fnv {
    uint64_t h = 0xcbf29ce484222325ULL;
    void add(uint64_t v) {
        for (int i = 0; i < 8; i++) {
            h ^= (v >> (8 * i)) & 0xff;
            h *= 0x100000001b3ULL;
        }
    }
};

// The private table arrays, read by a layout-identical mirror (the class has
// no accessors for them). Static-asserted against the real layout below.
struct AtView {
    int n;
    int np;
    EGS_Float *fi;
    EGS_Float *xi;
    EGS_Float *wi;
    EGS_Float average;
    int *bin;
    int type;
};
static_assert(sizeof(AtView) == sizeof(EGS_AliasTable), "EGS_AliasTable layout changed");
struct SatView {
    int n;
    EGS_Float *wi;
    int *bins;
};
static_assert(sizeof(SatView) == sizeof(EGS_SimpleAliasTable), "EGS_SimpleAliasTable layout");

static FILE *g_sum;

static void write_alias(const std::string &dir, const std::string &name, const EGS_AliasTable &t,
                        int func_rc) {
    AtView v;
    memcpy(&v, &t, sizeof v);
    int nfi = v.type == 1 ? v.np : v.n;
    std::string path = dir + "/" + name + ".txt";
    FILE *o = fopen(path.c_str(), "w");
    Fnv td;
    td.add(v.n);
    td.add(v.np);
    td.add(bits(v.average));
    fprintf(o, "n %d\nnp %d\ntype %d\naverage %016llx\nwarnings %d\nfunc_rc %d\n", v.n, v.np,
            v.type, (unsigned long long)bits(v.average), g_warnings, func_rc);
    for (int i = 0; i < v.n; i++) {
        td.add(bits(v.xi[i]));
        fprintf(o, "xi %d %016llx\n", i, (unsigned long long)bits(v.xi[i]));
    }
    for (int i = 0; i < nfi; i++) {
        td.add(bits(v.fi[i]));
        fprintf(o, "fi %d %016llx\n", i, (unsigned long long)bits(v.fi[i]));
    }
    for (int i = 0; i < v.np; i++) {
        td.add(bits(v.wi[i]));
        fprintf(o, "wi %d %016llx\n", i, (unsigned long long)bits(v.wi[i]));
    }
    for (int i = 0; i < v.np; i++) {
        td.add((uint64_t)v.bin[i]);
        fprintf(o, "bin %d %d\n", i, v.bin[i]);
    }
    std::vector<uint64_t> ks = uniform_stream();
    Fnv sd, bd;
    std::vector<uint64_t> first;
    {
        ReplayRng r(ks);
        for (int i = 0; i < NSAMPLE; i++) {
            double x = t.sample(&r);
            sd.add(bits(x));
            if (i < 16) first.push_back(bits(x));
            fprintf(o, "s %d %016llx\n", i, (unsigned long long)bits(x));
        }
    }
    {
        ReplayRng r(ks);
        for (int i = 0; i < NSAMPLE; i++) {
            int b = t.sampleBin(&r);
            bd.add((uint64_t)b);
            fprintf(o, "b %d %d\n", i, b);
        }
    }
    fclose(o);
    fprintf(g_sum, "case %s\nkind alias\nn %d\ntype %d\nwarnings %d\nfunc_rc %d\n", name.c_str(),
            v.n, v.type, g_warnings, func_rc);
    fprintf(g_sum, "average %016llx\ntable_digest %016llx\nsample_digest %016llx\nbin_digest %016llx\n",
            (unsigned long long)bits(v.average), (unsigned long long)td.h,
            (unsigned long long)sd.h, (unsigned long long)bd.h);
    fprintf(g_sum, "first");
    for (uint64_t b : first) fprintf(g_sum, " %016llx", (unsigned long long)b);
    fprintf(g_sum, "\n");
    if (v.np <= 40) {
        fprintf(g_sum, "wi");
        for (int i = 0; i < v.np; i++) fprintf(g_sum, " %016llx", (unsigned long long)bits(v.wi[i]));
        fprintf(g_sum, "\nbin");
        for (int i = 0; i < v.np; i++) fprintf(g_sum, " %d", v.bin[i]);
        fprintf(g_sum, "\n");
    }
    fprintf(g_sum, "end\n");
}

static void write_simple(const std::string &dir, const std::string &name,
                         const EGS_SimpleAliasTable &t) {
    SatView v;
    memcpy(&v, &t, sizeof v);
    std::string path = dir + "/" + name + ".txt";
    FILE *o = fopen(path.c_str(), "w");
    Fnv td;
    td.add(v.n);
    fprintf(o, "n %d\nwarnings %d\n", v.n, g_warnings);
    for (int i = 0; i < v.n; i++) {
        td.add(bits(v.wi[i]));
        fprintf(o, "wi %d %016llx\n", i, (unsigned long long)bits(v.wi[i]));
    }
    for (int i = 0; i < v.n; i++) {
        td.add((uint64_t)v.bins[i]);
        fprintf(o, "bin %d %d\n", i, v.bins[i]);
    }
    std::vector<uint64_t> ks = uniform_stream();
    Fnv bd;
    std::vector<int> first;
    ReplayRng r(ks);
    for (int i = 0; i < NSAMPLE; i++) {
        int b = t.sample(&r);
        bd.add((uint64_t)b);
        if (i < 16) first.push_back(b);
        fprintf(o, "b %d %d\n", i, b);
    }
    fclose(o);
    fprintf(g_sum, "case %s\nkind simple\nn %d\nwarnings %d\n", name.c_str(), v.n, g_warnings);
    fprintf(g_sum, "table_digest %016llx\nbin_digest %016llx\nfirst", (unsigned long long)td.h,
            (unsigned long long)bd.h);
    for (int b : first) fprintf(g_sum, " %d", b);
    fprintf(g_sum, "\n");
    if (v.n <= 40) {
        fprintf(g_sum, "wi");
        for (int i = 0; i < v.n; i++) fprintf(g_sum, " %016llx", (unsigned long long)bits(v.wi[i]));
        fprintf(g_sum, "\nbin");
        for (int i = 0; i < v.n; i++) fprintf(g_sum, " %d", v.bins[i]);
        fprintf(g_sum, "\n");
    }
    fprintf(g_sum, "end\n");
}

// ------------------------------------------------------------- cases ----
static void tab(const std::string &dir, const std::string &name, int type,
                const std::vector<double> &x, const std::vector<double> &f) {
    g_warnings = 0;
    EGS_AliasTable t((int)x.size(), x.data(), f.data(), type);
    write_alias(dir, name, t, -1);
}

static double lorentz(double x, void *) { return 1.0 / (1.0 + x * x); }
static double cubic(double x, void *) { return x * x * x + 0.01; }

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: %s OUTDIR\n", argv[0]);
        return 1;
    }
    std::string dir = argv[1];
    g_sum = fopen((dir + "/summary.txt").c_str(), "w");

    auto seq = [](int n, double scale) {
        std::vector<double> v(n);
        for (int i = 0; i < n; i++) v[i] = scale * (double)i;
        return v;
    };
    auto sq = [](int n) {
        std::vector<double> v(n);
        for (int i = 0; i < n; i++) v[i] = 0.01 * (double)(i * i);
        return v;
    };
    auto ones = [](int n) { return std::vector<double>(n, 1.0); };
    auto steep = [](int n) {
        std::vector<double> v(n);
        for (int i = 0; i < n; i++) v[i] = ldexp(1.0, -i);
        return v;
    };
    auto zeros = [](int n) { return std::vector<double>(ZEROS, ZEROS + n); };

    tab(dir, "discrete_uniform", 0, seq(10, 1.0), ones(10));
    tab(dir, "discrete_steep", 0, seq(40, 0.25), steep(40));
    tab(dir, "discrete_zeros", 0, seq(12, 1.0), zeros(12));
    tab(dir, "discrete_single", 0, {2.5}, {4.0});
    tab(dir, "discrete_many", 0, seq(10000, 0.5), cubed(101, 10000));
    tab(dir, "histogram_uniform", 1, seq(11, 0.1), ones(10));
    tab(dir, "histogram_steep", 1, sq(33), steep(32));
    tab(dir, "histogram_zeros", 1, seq(13, 1.0), zeros(12));
    tab(dir, "histogram_single", 1, {1.0, 3.0}, {0.5});
    tab(dir, "histogram_many", 1, seq(10001, 1e-3), cubed(202, 10000));
    tab(dir, "linear_uniform", 2, seq(11, 0.1), ones(11));
    tab(dir, "linear_steep", 2, sq(33), steep(33));
    tab(dir, "linear_zeros", 2, seq(13, 1.0), zeros(13));
    tab(dir, "linear_single", 2, {0.0, 1.0}, {1.0, 1.1});
    tab(dir, "linear_many", 2, seq(10001, 1e-3), cubed(303, 10001));
    tab(dir, "linear_type3", 3, sq(33), steep(33));
    {
        g_warnings = 0;
        EGS_AliasTable t;
        int rc = t.initialize(0.0, 4.0, 1e-3, 1000, lorentz, 0);
        write_alias(dir, "func_lorentz", t, rc);
    }
    {
        g_warnings = 0;
        EGS_AliasTable t;
        int rc = t.initialize(0.0, 2.0, 1e-8, 50, cubic, 0);
        write_alias(dir, "func_nmax", t, rc);
    }
    auto simple = [&](const std::string &name, const std::vector<double> &f) {
        g_warnings = 0;
        EGS_SimpleAliasTable t((int)f.size(), f.data());
        write_simple(dir, name, t);
    };
    simple("simple_uniform", ones(10));
    simple("simple_steep", steep(40));
    simple("simple_zeros", zeros(12));
    simple("simple_single", {4.0});
    simple("simple_many", cubed(404, 10000));
    fclose(g_sum);
    return 0;
}
