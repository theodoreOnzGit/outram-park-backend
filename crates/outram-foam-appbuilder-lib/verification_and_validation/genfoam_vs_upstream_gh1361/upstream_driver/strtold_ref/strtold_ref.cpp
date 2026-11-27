// gh:#1897: reference values of strtold-then-narrow for a deterministic family of decimal strings.
// Output lines: <string> <hex bits of the double>.  Same generator as foam_number_vs_strtold.rs.
#include <cstdio>
#include <cstdlib>
#include <cstdint>
#include <cstring>
int main(int argc, char** argv)
{
    const long n = argc > 1 ? atol(argv[1]) : 1000000;
    uint64_t s = 0x9E3779B97F4A7C15ull;
    for (long i = 0; i < n; ++i)
    {
        s = s * 6364136223846793005ull + 1442695040888963407ull;
        const uint64_t r = s >> 11;
        s = s * 6364136223846793005ull + 1442695040888963407ull;
        const uint64_t r2 = s >> 11;
        const int nd = 1 + int(r2 % 19);          // 1..19 significant digits
        const int ex = int((r2 >> 8) % 61) - 30;  // exponent -30..30
        uint64_t m = r;
        char dig[32];
        int k = 0;
        for (int j = 0; j < nd; ++j) { dig[k++] = char('0' + (m % 10)); m /= 10; if (m == 0 && j < nd - 1) m = (s >> (j + 3)) | 1; }
        dig[k] = 0;
        char buf[96];
        snprintf(buf, sizeof buf, "%c%c.%se%d", (r2 & 1) ? '-' : '+', dig[0], dig + 1, ex);
        if (dig[1] == 0) snprintf(buf, sizeof buf, "%c%ce%d", (r2 & 1) ? '-' : '+', dig[0], ex);
        const double v = double(strtold(buf, nullptr));
        uint64_t bits;
        memcpy(&bits, &v, 8);
        printf("%s %016llx\n", buf, (unsigned long long)bits);
    }
    return 0;
}
