// Differential-comparison driver for gh:#1346: generates the input grids AND the
// CoolProp IF97.h outputs for tampines-steam-tables.
//
// Upstream:  CoolProp/IF97  v2.4.0  (header-only, MIT)
//            commit 138ec05fd64c60167515f6a4f5448cdd19466d27
// Build:     g++ -O2 -std=c++17 -I<path to IF97 checkout> coolprop_if97_driver.cpp -o drv
//            (glibc 2.39, g++ 13.3, x86-64; NOT -ffast-math, NOT -march=native, so no FMA)
// Run:       ./drv > coolprop_if97_reference.csv
//
// Every number is printed with %.17g (exact binary64 round trip); failures print "nan".
// Record layout (tag first):
//   TP   T p region rho h s u cp cv w mu lambda
//   SATT T psat sigma
//   SATP p Tsat
//   PH   p h T rho Q s region
//   PS   p s T rho Q h region
//   HS   h s T p Q region
#include "IF97.h"
#include <cmath>
#include <cstdio>
#include <functional>
#include <vector>

static const double NaN = std::nan("");

static double guard(const std::function<double()> &f) {
    try { return f(); } catch (...) { return NaN; }
}

static void pr(double x) {
    if (std::isnan(x)) std::printf(",nan");
    else std::printf(",%.17g", x);
}

static std::vector<double> logspace(double a, double b, int n) {
    std::vector<double> v;
    for (int i = 0; i < n; ++i) v.push_back(a * std::pow(b / a, double(i) / (n - 1)));
    return v;
}

static void emit_tp(double T, double p) {
    int region = -1;
    try { region = IF97::RegionDetermination_TP(T, p) + 1; } catch (...) {}
    std::printf("TP,%.17g,%.17g,%d", T, p, region);
    pr(guard([&] { return IF97::rhomass_Tp(T, p); }));
    pr(guard([&] { return IF97::hmass_Tp(T, p); }));
    pr(guard([&] { return IF97::smass_Tp(T, p); }));
    pr(guard([&] { return IF97::umass_Tp(T, p); }));
    pr(guard([&] { return IF97::cpmass_Tp(T, p); }));
    pr(guard([&] { return IF97::cvmass_Tp(T, p); }));
    pr(guard([&] { return IF97::speed_sound_Tp(T, p); }));
    pr(guard([&] { return IF97::visc_Tp(T, p); }));
    pr(guard([&] { return IF97::tcond_Tp(T, p); }));
    std::printf("\n");
}

int main() {
    const double up = INFINITY;
    // ---------------- (T,p) grid ----------------
    std::vector<double> Ts;
    for (double T = 273.15; T <= 1073.15 + 1e-9; T += 30.0) Ts.push_back(T);
    for (double T = 1173.15; T <= 2273.15 + 1e-9; T += 100.0) Ts.push_back(T);
    const double Tspecial[] = {273.15, 623.15, 647.096, 863.15, 1073.15, 2273.15};
    for (double T : Tspecial) {
        Ts.push_back(T);
        Ts.push_back(std::nextafter(T, -up));
        Ts.push_back(std::nextafter(T, up));
    }
    // dense near the critical point
    const double Tcrit_dense[] = {640, 643, 645, 646, 646.5, 647, 647.5, 648, 650, 655, 660, 700};
    for (double T : Tcrit_dense) Ts.push_back(T);
    std::vector<double> pbase = logspace(611.213, 100.0e6, 36);
    const double pspecial[] = {611.213, 1.0e5, 1.0e6, 16.529164e6, 20.0e6, 21.0e6, 22.0e6, 22.064e6,
                               22.5e6, 23.0e6, 25.0e6, 30.0e6, 40.0e6, 50.0e6, 100.0e6};
    for (double T : Ts) {
        std::vector<double> ps = pbase;
        for (double p : pspecial) ps.push_back(p);
        if (T >= 273.15 && T <= 647.096) {
            double psat = IF97::psat97(T);
            ps.push_back(psat);
            ps.push_back(std::nextafter(psat, -up));
            ps.push_back(std::nextafter(psat, up));
            ps.push_back(psat * (1 - 1e-9));
            ps.push_back(psat * (1 + 1e-9));
        }
        if (T >= 623.15 && T <= 863.15) {
            double pb = IF97::Region23_T(T);
            ps.push_back(pb);
            ps.push_back(std::nextafter(pb, -up));
            ps.push_back(std::nextafter(pb, up));
        }
        for (double p : ps) {
            if (T > 1073.15 && p > 50.0e6) continue;  // outside IF97 for T>1073.15 K
            emit_tp(T, p);
        }
    }
    // ---------------- saturation ----------------
    for (double T = 273.16; T <= 647.0 + 1e-9; T += 1.0) {
        std::printf("SATT,%.17g", T);
        pr(guard([&] { return IF97::psat97(T); }));
        pr(guard([&] { return IF97::sigma97(T); }));
        std::printf("\n");
    }
    for (double p : logspace(611.213, 22.0e6, 120)) {
        std::printf("SATP,%.17g", p);
        pr(guard([&] { return IF97::Tsat97(p); }));
        std::printf("\n");
    }
    // ---------------- (p,h) ----------------
    std::vector<double> pq = logspace(611.213, 100.0e6, 22);
    const double pq_extra[] = {1.0e5, 1.0e6, 22.064e6, 25.0e6, 50.0e6, 100.0e6};
    for (double p : pq_extra) pq.push_back(p);
    for (double p : pq) {
        std::vector<double> hs;
        for (double h = 20.0e3; h <= 4800.0e3 + 1; h += 100.0e3) hs.push_back(h);
        if (p < 22.064e6) {
            double Ts_ = IF97::Tsat97(p);
            double hl = IF97::hliq_p(p), hv = IF97::hvap_p(p);
            const double ds[] = {0.0, 1e-9, -1e-9, 1e-6, -1e-6, 1e-4, -1e-4};
            for (double d : ds) { hs.push_back(hl * (1 + d)); hs.push_back(hv * (1 + d)); }
            (void)Ts_;
        }
        for (double h : hs) {
            std::printf("PH,%.17g,%.17g", p, h);
            pr(guard([&] { return IF97::T_phmass(p, h); }));
            pr(guard([&] { return IF97::rhomass_phmass(p, h); }));
            pr(guard([&] { return IF97::Q_phmass(p, h); }));
            pr(guard([&] { return IF97::smass_phmass(p, h); }));
            pr(guard([&] { return double(IF97::Region_ph(p, h)); }));
            std::printf("\n");
        }
    }
    // ---------------- (p,s) ----------------
    for (double p : pq) {
        std::vector<double> ss;
        for (double s = 100.0; s <= 10500.0 + 1; s += 250.0) ss.push_back(s);
        if (p < 22.064e6) {
            double sl = IF97::sliq_p(p), sv = IF97::svap_p(p);
            const double ds[] = {0.0, 1e-9, -1e-9, 1e-6, -1e-6, 1e-4, -1e-4};
            for (double d : ds) { ss.push_back(sl * (1 + d)); ss.push_back(sv * (1 + d)); }
        }
        for (double s : ss) {
            std::printf("PS,%.17g,%.17g", p, s);
            pr(guard([&] { return IF97::T_psmass(p, s); }));
            pr(guard([&] { return IF97::rhomass_psmass(p, s); }));
            pr(guard([&] { return IF97::Q_psmass(p, s); }));
            pr(guard([&] { return IF97::hmass_psmass(p, s); }));
            pr(guard([&] { return double(IF97::Region_ps(p, s)); }));
            std::printf("\n");
        }
    }
    // ---------------- (h,s) ----------------
    for (double h = 100.0e3; h <= 4700.0e3 + 1; h += 200.0e3) {
        for (double s = 300.0; s <= 10300.0 + 1; s += 500.0) {
            std::printf("HS,%.17g,%.17g", h, s);
            pr(guard([&] { return IF97::T_hsmass(h, s); }));
            pr(guard([&] { return IF97::p_hsmass(h, s); }));
            pr(guard([&] { return IF97::Q_hsmass(h, s); }));
            pr(guard([&] { return double(IF97::Region_hs(h, s)); }));
            std::printf("\n");
        }
    }
    // ---------------- (h,s) from forward states: in-domain points in every backward region ----
    // (h, s) taken from IF97.h forward equations at (T,p), so every row is a valid state; the
    // backward 2a/2b/2c/3a/3b (h,s) equations are then exercised, not just the coarse rectangle.
    const double Th[] = {280, 300, 350, 400, 450, 500, 550, 600, 620, 640, 660, 700, 750, 800, 900, 1000};
    const double Ph[] = {1.0e4, 1.0e5, 5.0e5, 1.0e6, 2.0e6, 4.0e6, 6.0e6, 1.0e7, 1.5e7,
                         2.0e7, 2.5e7, 3.0e7, 4.0e7, 6.0e7, 8.0e7, 1.0e8};
    for (double T : Th) {
        for (double p : Ph) {
            double h = guard([&] { return IF97::hmass_Tp(T, p); });
            double sv = guard([&] { return IF97::smass_Tp(T, p); });
            if (std::isnan(h) || std::isnan(sv)) continue;
            std::printf("HS,%.17g,%.17g", h, sv);
            pr(guard([&] { return IF97::T_hsmass(h, sv); }));
            pr(guard([&] { return IF97::p_hsmass(h, sv); }));
            pr(guard([&] { return IF97::Q_hsmass(h, sv); }));
            pr(guard([&] { return double(IF97::Region_hs(h, sv)); }));
            std::printf("\n");
        }
    }
    // two-phase (h,s): saturated mixtures at quality 0.1, 0.5, 0.9
    const double Pd[] = {1.0e3, 1.0e4, 1.0e5, 1.0e6, 5.0e6, 1.0e7, 1.5e7, 2.0e7};
    for (double p : Pd) {
        for (double x : {0.1, 0.5, 0.9}) {
            double h = IF97::hmass_pQ(p, x), sv = IF97::smass_pQ(p, x);
            std::printf("HS,%.17g,%.17g", h, sv);
            pr(guard([&] { return IF97::T_hsmass(h, sv); }));
            pr(guard([&] { return IF97::p_hsmass(h, sv); }));
            pr(guard([&] { return IF97::Q_hsmass(h, sv); }));
            pr(guard([&] { return double(IF97::Region_hs(h, sv)); }));
            std::printf("\n");
        }
    }
    return 0;
}
