// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// Differential-testing harness for PETIR's port of EGSnrc's random number
// generators (gh:#1289). OUR code, not upstream: it compiles UPSTREAM's
// sources unmodified and writes their output streams as exact bits.
//
//   RANMAR: HEN_HOUSE/egs++/egs_rndm.cpp, #included below so the file-local
//           class EGS_Ranmar can be constructed with any seeds, array size and
//           resolution (upstream only exposes it through createRNG/defaultRNG).
//   RANLUX: HEN_HOUSE/src/ranlux.mortran, put through upstream's Mortran3
//           compiler and gfortran (see build.sh), called through its Fortran
//           entry points. The $RANDOMSET / $INITIALIZE RNG macros of
//           HEN_HOUSE/src/ranlux.macros are a 24-element buffer with a
//           pointer; `LuxBuffer` below reproduces them.
//
// EGSnrc commit bd49a47aae3938f49f4100b8e018091861408b19 (2026-09-28).
// Output: reference.txt (text, the source of the committed test vectors) and
// dump_<case>.bin (every draw, f64 little-endian) for the full comparison.

#include "egs_rndm.cpp"

#include <cstdarg>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <sstream>
#include <string>
#include <vector>

// ---------------------------------------------------------------------------
// Link stand-ins for egs++ functions egs_rndm.cpp references. These are the
// harness's own minimal versions (upstream's live in egs_functions.cpp, which
// drags in EGS_Application). None of them is on a path whose numbers are
// compared, except egsStoreI64/egsGetI64 in the text round-trip observation,
// where any exact integer encoding serves.
// ---------------------------------------------------------------------------
static void harness_info(const char *msg, ...) {
    va_list ap;
    va_start(ap, msg);
    vfprintf(stderr, msg, ap);
    va_end(ap);
}
static void harness_fatal(const char *msg, ...) {
    va_list ap;
    va_start(ap, msg);
    vfprintf(stderr, msg, ap);
    va_end(ap);
    exit(1);
}
EGS_InfoFunction egsInformation = harness_info;
EGS_InfoFunction egsWarning = harness_info;
EGS_InfoFunction egsFatal = harness_fatal;
bool egsStoreI64(ostream &data, EGS_I64 n) {
    data << " " << n;
    return !data.fail();
}
bool egsGetI64(istream &data, EGS_I64 &n) {
    data >> n;
    return data.good();
}

// ---------------------------------------------------------------------------
// RANLUX: the Fortran entry points of ranlux.mortran.
// ---------------------------------------------------------------------------
extern "C" {
void ranlux_(double *rng_array);
void init_ranlux_(int *luxury_level, int *seedin);
void get_ranlux_state_(int *state);
void set_ranlux_state_(int *state);
}

// The COMIN/RANDOM buffer and $RANDOMSET of ranlux.macros.
struct LuxBuffer {
    double rng_array[24];
    int rng_seed;
    // $INITIALIZE RNG USING lux AND seed;
    void init(int lux, int seed) {
        init_ranlux_(&lux, &seed);
        ranlux_(rng_array);
        rng_seed = 1;
    }
    // The first fill without any init_ranlux call (the not_initialized path).
    void fill_uninitialised() {
        ranlux_(rng_array);
        rng_seed = 1;
    }
    // $RANDOMSET x;
    double next() {
        if (rng_seed > 24) {
            ranlux_(rng_array);
            rng_seed = 1;
        }
        double x = rng_array[rng_seed - 1];
        rng_seed = rng_seed + 1;
        return x;
    }
};

// ---------------------------------------------------------------------------
// Output helpers.
// ---------------------------------------------------------------------------
static FILE *ref_out = nullptr;
static std::string dump_dir;

static uint64_t bits(double x) {
    uint64_t b;
    memcpy(&b, &x, 8);
    return b;
}

static uint64_t fnv1a(uint64_t h, double x) {
    uint64_t b = bits(x);
    for (int k = 0; k < 8; k++) {
        h ^= (b >> (8 * k)) & 0xff;
        h *= 0x100000001b3ULL;
    }
    return h;
}

static const uint64_t FNV_OFFSET = 0xcbf29ce484222325ULL;

static bool recorded_index(long i, long n) {
    if (i < 48) return true;
    if (i >= 120 && i <= 136) return true;
    if (i >= 990 && i <= 1010) return true;
    if (i >= n - 10) return true;
    return false;
}

// Write the stream `xs` as one case: recorded values, digests, binary dump.
static void write_stream(const std::string &name, const std::vector<double> &xs) {
    long n = (long)xs.size();
    uint64_t h = FNV_OFFSET;
    for (long i = 0; i < n; i++) {
        if (recorded_index(i, n)) {
            fprintf(ref_out, "V %ld %016llx\n", i, (unsigned long long)bits(xs[i]));
        }
        h = fnv1a(h, xs[i]);
        long c = i + 1;
        if (c == 1000 || c == 100000 || c == n) {
            fprintf(ref_out, "FNV %ld %016llx\n", c, (unsigned long long)h);
        }
    }
    std::string path = dump_dir + "/dump_" + name + ".bin";
    FILE *f = fopen(path.c_str(), "wb");
    fwrite(xs.data(), sizeof(double), xs.size(), f);
    fclose(f);
}

static const long N = 1000000;

static void ranmar_case(const std::string &name, EGS_RandomGenerator *r,
                        const char *params) {
    fprintf(ref_out, "CASE %s ranmar %s\n", name.c_str(), params);
    std::vector<double> xs(N);
    for (long i = 0; i < N; i++) xs[i] = r->getUniform();
    write_stream(name, xs);
    fprintf(ref_out, "COUNT %lld %lld\n", (long long)r->numbersGenerated(),
            (long long)r->numbersUsed());
    fprintf(ref_out, "END\n");
}

static void ranmar_plain(const std::string &name, int ixx, int jxx, int n, bool hr) {
    EGS_Ranmar r(ixx, jxx, n);
    r.setHighResolution(hr);
    char p[128];
    snprintf(p, sizeof p, "ixx=%d jxx=%d n=%d hr=%d", ixx, jxx, n, hr ? 1 : 0);
    ranmar_case(name, &r, p);
}

static void ranlux_case(const std::string &name, int lux, int seed, bool uninit) {
    LuxBuffer b;
    if (uninit) {
        b.fill_uninitialised();
    } else {
        b.init(lux, seed);
    }
    fprintf(ref_out, "CASE %s ranlux lux=%d seed=%d uninit=%d\n", name.c_str(), lux,
            seed, uninit ? 1 : 0);
    std::vector<double> xs(N);
    for (long i = 0; i < N; i++) xs[i] = b.next();
    write_stream(name, xs);
    int st[25];
    get_ranlux_state_(st);
    fprintf(ref_out, "STATE");
    for (int k = 0; k < 25; k++) fprintf(ref_out, " %d", st[k]);
    fprintf(ref_out, " %d\n", b.rng_seed);
    fprintf(ref_out, "END\n");
}

int main(int argc, char **argv) {
    dump_dir = argc > 1 ? argv[1] : ".";
    std::string ref_path = dump_dir + "/reference.txt";
    ref_out = fopen(ref_path.c_str(), "w");
    fprintf(ref_out, "# EGSnrc egs_rndm / ranlux reference streams for PETIR (gh:#1289)\n");
    fprintf(ref_out, "# upstream: EGSnrc commit bd49a47aae3938f49f4100b8e018091861408b19\n");
    fprintf(ref_out, "# made by crates/petir/verification_and_validation/egsnrc_rndm/harness/\n");
    fprintf(ref_out, "# run: %s\n", argc > 2 ? argv[2] : "unlabelled");
    fprintf(ref_out, "# V <index> <f64 bits hex> | FNV <count> <fnv1a-64 of LE bits> \n");

    // RANLUX's not-initialised path must run first: the Fortran SAVE state
    // is process-global and init_ranlux clears the flag for good.
    ranlux_case("ranlux_uninit", 1, 0, true);
    int seeds[] = {0, 1, 12345, 314159265, 2147483647};
    for (int lux = 0; lux <= 4; lux++) {
        for (int s : seeds) {
            ranlux_case("ranlux_l" + std::to_string(lux) + "_s" + std::to_string(s), lux,
                        s, false);
        }
    }
    ranlux_case("ranlux_l7_s1", 7, 1, false);     // out-of-range luxury -> 1
    ranlux_case("ranlux_l2_sneg5", 2, -5, false); // seed <= 0 -> default

    // RANLUX state round trip (get_ranlux_state / set_ranlux_state plus the
    // COMIN/RANDOM buffer, as $STORE / $RETRIEVE RNG STATE do).
    {
        LuxBuffer b;
        b.init(2, 12345);
        for (int i = 0; i < 1000; i++) b.next();
        int st[25];
        get_ranlux_state_(st);
        LuxBuffer saved = b;
        double a[300], c[300];
        for (int i = 0; i < 300; i++) a[i] = b.next();
        set_ranlux_state_(st);
        b = saved;
        for (int i = 0; i < 300; i++) c[i] = b.next();
        int same = 0;
        for (int i = 0; i < 300; i++) same += bits(a[i]) == bits(c[i]);
        fprintf(ref_out, "CASE ranlux_state_roundtrip ranlux lux=2 seed=12345 skip=1000\n");
        fprintf(ref_out, "STATE");
        for (int k = 0; k < 25; k++) fprintf(ref_out, " %d", st[k]);
        fprintf(ref_out, " %d\n", saved.rng_seed);
        for (int i = 0; i < 300; i++)
            fprintf(ref_out, "V %d %016llx\n", i, (unsigned long long)bits(a[i]));
        fprintf(ref_out, "SAME %d 300\n", same);
        fprintf(ref_out, "END\n");
    }

    // RANMAR.
    ranmar_plain("ranmar_default", 1802, 9373, 128, false);
    ranmar_plain("ranmar_default_hr", 1802, 9373, 128, true);
    ranmar_plain("ranmar_1_1", 1, 1, 128, false);
    ranmar_plain("ranmar_1_1_hr", 1, 1, 128, true);
    ranmar_plain("ranmar_max", 31327, 30080, 128, false);
    ranmar_plain("ranmar_max_hr", 31327, 30080, 128, true);
    ranmar_plain("ranmar_clamp", 0, 40000, 128, false);
    ranmar_plain("ranmar_12345_6789_n97", 12345, 6789, 97, false);
    ranmar_plain("ranmar_12345_6789_n1000_hr", 12345, 6789, 1000, true);
    ranmar_plain("ranmar_n1_hr", 1802, 9373, 1, true);
    {
        EGS_RandomGenerator *r = EGS_RandomGenerator::defaultRNG(0);
        ranmar_case("ranmar_defaultrng_0", r, "sequence=0");
        delete r;
    }
    {
        EGS_RandomGenerator *r = EGS_RandomGenerator::defaultRNG(100000);
        ranmar_case("ranmar_defaultrng_100000", r, "sequence=100000");
        delete r;
    }

    // getGaussian and getAzimuth on the default stream.
    {
        EGS_Ranmar r(1802, 9373, 128);
        long m = 200000;
        std::vector<double> xs(m);
        for (long i = 0; i < m; i++) xs[i] = r.getGaussian();
        fprintf(ref_out, "CASE ranmar_gaussian ranmar_gaussian ixx=1802 jxx=9373 n=128 hr=0\n");
        write_stream("ranmar_gaussian", xs);
        fprintf(ref_out, "END\n");
    }
    {
        EGS_Ranmar r(1802, 9373, 128);
        long m = 100000;
        std::vector<double> xs(2 * m);
        for (long i = 0; i < m; i++) {
            double c, s;
            r.getAzimuth(c, s);
            xs[2 * i] = c;
            xs[2 * i + 1] = s;
        }
        fprintf(ref_out, "CASE ranmar_azimuth ranmar_azimuth ixx=1802 jxx=9373 n=128 hr=0\n");
        write_stream("ranmar_azimuth", xs);
        fprintf(ref_out, "END\n");
    }

    // saveState / resetState, and getCopy.
    {
        EGS_Ranmar r(1802, 9373, 128);
        for (int i = 0; i < 1000; i++) r.getUniform();
        r.saveState();
        double a[300], c[300], d[300];
        for (int i = 0; i < 300; i++) a[i] = r.getUniform();
        r.resetState();
        EGS_RandomGenerator *cp = r.getCopy();
        for (int i = 0; i < 300; i++) c[i] = r.getUniform();
        for (int i = 0; i < 300; i++) d[i] = cp->getUniform();
        int same_reset = 0, same_copy = 0;
        for (int i = 0; i < 300; i++) {
            same_reset += bits(a[i]) == bits(c[i]);
            same_copy += bits(a[i]) == bits(d[i]);
        }
        fprintf(ref_out, "CASE ranmar_save_reset ranmar ixx=1802 jxx=9373 n=128 hr=0 skip=1000\n");
        for (int i = 0; i < 300; i++)
            fprintf(ref_out, "V %d %016llx\n", i, (unsigned long long)bits(a[i]));
        fprintf(ref_out, "SAME_RESET %d 300\nSAME_COPY %d 300\n", same_reset, same_copy);
        fprintf(ref_out, "END\n");
        delete cp;
    }

    // OBSERVATION, not ported: the text storeState/setState round trip.
    // storeState writes rarray with the stream's default precision (6
    // significant digits), so the not-yet-used numbers come back rounded.
    {
        EGS_Ranmar r(1802, 9373, 128);
        for (int i = 0; i < 1000; i++) r.getUniform();
        std::stringstream ss;
        EGS_RandomGenerator *base = &r;
        base->storeState(ss);
        EGS_RandomGenerator *r2 = EGS_RandomGenerator::defaultRNG(0);
        bool ok = r2->setState(ss);
        int same = 0, first_diff = -1;
        double worst = 0;
        for (int i = 0; i < 300; i++) {
            double x = r.getUniform(), y = r2->getUniform();
            if (bits(x) == bits(y)) {
                same++;
            } else {
                if (first_diff < 0) first_diff = i;
                double d = fabs(x - y) / (x > 0 ? x : 1);
                if (d > worst) worst = d;
            }
        }
        fprintf(stderr,
                "text round trip: setState ok=%d, %d of 300 identical, first diff at %d, "
                "worst rel diff %.3e\n",
                ok ? 1 : 0, same, first_diff, worst);
        fprintf(ref_out, "# OBSERVATION text storeState/setState round trip: ok=%d same=%d/300 "
                     "first_diff=%d worst_rel=%.3e\n",
                ok ? 1 : 0, same, first_diff, worst);
        delete r2;
    }

    fclose(ref_out);
    return 0;
}
