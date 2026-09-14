#include <stdio.h>
#include <math.h>
int main(void) {
  for (int k = -400; k <= 400; k++) {
    double x = k * 0.0137 * 3.0;
    printf("erf %.17e %.17e\n", x, erf(x));
    printf("erfc %.17e %.17e\n", x, erfc(x));
    if (x > 0.0) {
      printf("tgamma %.17e %.17e\n", x, tgamma(x));
      printf("lgamma %.17e %.17e\n", x, lgamma(x));
    }
  }
  return 0;
}
