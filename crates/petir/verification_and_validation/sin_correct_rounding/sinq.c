/* Reference for gh:#1455: glibc sin() and the correctly rounded sin (binary128
 * libquadmath `sinq`, rounded once to double) on a deterministic argument set.
 * Prints: x bits, glibc sin bits, correctly rounded bits (hex). */
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <quadmath.h>
static uint64_t s = 0x9E3779B97F4A7C15ULL;
static uint64_t nx(void) { s ^= s << 13; s ^= s >> 7; s ^= s << 17; return s; }
static double u(void) { return (nx() >> 11) * (1.0 / 9007199254740992.0); }
static uint64_t b(double x) { uint64_t k; memcpy(&k, &x, 8); return k; }
int main(void) {
  for (int i = 0; i < 8000; i++) {
    double x;
    int r = i % 4;
    if (r == 0) x = u() * 6.283185307179586;
    else if (r == 1) x = u() * 100.0;
    else if (r == 2) x = u() * 1000.0 - 500.0;
    else x = u() * 1.0e-1;
    double g = sin(x);
    double c = (double)sinq((__float128)x);
    printf("%016llx %016llx %016llx\n", (unsigned long long)b(x), (unsigned long long)b(g), (unsigned long long)b(c));
  }
  return 0;
}
