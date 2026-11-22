// Differential-comparison driver for gh:#1349: generates the input grids AND the
// CoolProp outputs for outram-park-fork-coolprop.
//
// Upstream:  CoolProp  commit 0e67fe74b30a2fe9526af9bc64ea026a96f56ebf (2026-07-05), MIT
// Build:     static library from CMake (Release, -O3 -DNDEBUG, plus -ffp-contract=off; NOT
//            -ffast-math, NOT -march=native, so no FMA), g++ 13.3, glibc 2.39, x86-64.
//            See README.md here for the exact commands.
// Run:       ./drv fluids.txt > coolprop_reference.csv      (fluids.txt: one CoolProp name per line)
//
// Every number is printed with %.17g (exact binary64 round trip); failures print "nan".
// Record layout (tag first):
//   E    fluid idx T rho_mass delta tau  ar ar_d ar_t ar_dd ar_dt ar_tt  a0 a0_d a0_t a0_dd a0_dt a0_tt
//        p u h s cv cp w  eta lambda
//   SATA fluid T  p_anc rhoL_anc rhoV_anc          (ancillary fits, superancillaries OFF)
//   SAT  fluid T  p rhoL rhoV                      (full VLE solve, superancillaries OFF)
//   SATS fluid T  p rhoL rhoV                      (full VLE solve, superancillaries ON: CoolProp default)
//   PT   fluid T p rho h s phase                   (phase: CoolProp phase index; psat in the row after)
//   META fluid M R Tr rhor Tc pc Ttriple Tmax pmax rhoc_molar
#include "CoolProp.h"
#include "AbstractState.h"
#include "Configuration.h"
#include <cmath>
#include <cstdio>
#include <fstream>
#include <functional>
#include <iostream>
#include <memory>
#include <string>
#include <vector>

using namespace CoolProp;

static const double NaN = std::nan("");

static double guard(const std::function<double()> &f) {
    try { return f(); } catch (...) { return NaN; }
}

static void pr(double x) {
    if (std::isnan(x) || std::isinf(x)) std::printf(",nan");
    else std::printf(",%.17g", x);
}

static std::vector<double> logspace(double a, double b, int n) {
    std::vector<double> v;
    for (int i = 0; i < n; ++i) v.push_back(a * std::pow(b / a, double(i) / (n - 1)));
    return v;
}

static void emit_e(const std::string &name, int idx, AbstractState &as, double T, double rho) {
    // Impose the gas phase so DmassT evaluates the raw EOS (no phase determination); the
    // properties are then the single-phase Helmholtz algebra, also inside the dome.
    bool ok = true;
    try {
        as.specify_phase(iphase_gas);
        as.update(DmassT_INPUTS, rho, T);
    } catch (...) { ok = false; }
    std::printf("E,%s,%d", name.c_str(), idx);
    pr(T); pr(rho);
    auto g = [&](const std::function<double()> &f) { pr(ok ? guard(f) : NaN); };
    g([&] { return as.delta(); });
    g([&] { return as.tau(); });
    g([&] { return as.alphar(); });
    g([&] { return as.dalphar_dDelta(); });
    g([&] { return as.dalphar_dTau(); });
    g([&] { return as.d2alphar_dDelta2(); });
    g([&] { return as.d2alphar_dDelta_dTau(); });
    g([&] { return as.d2alphar_dTau2(); });
    g([&] { return as.alpha0(); });
    g([&] { return as.dalpha0_dDelta(); });
    g([&] { return as.dalpha0_dTau(); });
    g([&] { return as.d2alpha0_dDelta2(); });
    g([&] { return as.d2alpha0_dDelta_dTau(); });
    g([&] { return as.d2alpha0_dTau2(); });
    g([&] { return as.p(); });
    g([&] { return as.umass(); });
    g([&] { return as.hmass(); });
    g([&] { return as.smass(); });
    g([&] { return as.cvmass(); });
    g([&] { return as.cpmass(); });
    g([&] { return as.speed_sound(); });
    g([&] { return as.viscosity(); });
    g([&] { return as.conductivity(); });
    std::printf("\n");
}

static void emit_sat(const char *tag, const std::string &name, AbstractState &as, double T) {
    std::printf("%s,%s", tag, name.c_str());
    pr(T);
    try {
        as.update(QT_INPUTS, 0.0, T);
        pr(as.p());
        pr(as.saturated_liquid_keyed_output(iDmass));
        pr(as.saturated_vapor_keyed_output(iDmass));
    } catch (...) { pr(NaN); pr(NaN); pr(NaN); }
    std::printf("\n");
}

int main(int argc, char **argv) {
    if (argc < 2) { std::fprintf(stderr, "usage: drv fluids.txt\n"); return 1; }
    std::ifstream in(argv[1]);
    std::vector<std::string> names;
    for (std::string l; std::getline(in, l);) if (!l.empty()) names.push_back(l);

    const double delta_grid[12] = {1e-4, 0.01, 0.1, 0.3, 0.6, 0.9, 1.0, 1.1, 1.5, 2.0, 2.5, 3.0};
    const double nc_tau[7] = {0.99, 0.995, 0.999, 1.0, 1.001, 1.005, 1.01};
    const double nc_delta[7] = {0.9, 0.95, 0.99, 1.0, 1.01, 1.05, 1.1};
    const double pfac[8] = {0.5, 0.9, 0.99, 1.01, 1.1, 2.0, 10.0, 100.0};

    for (const auto &name : names) {
        std::shared_ptr<AbstractState> as;
        // This crate names a fluid after its JSON file stem; CoolProp's canonical NAME for
        // dev/fluids/R1224yd(Z).json is "R1224YDZ", and its factory rejects the stem.
        const std::string cpname = (name == "R1224yd(Z)") ? std::string("R1224YDZ") : name;
        try { as.reset(AbstractState::factory("HEOS", cpname)); }
        catch (...) { std::printf("FAIL,%s,factory\n", name.c_str()); continue; }

        double M = as->molar_mass(), R = as->gas_constant();
        double Tr = as->T_reducing(), rhor = as->rhomolar_reducing();
        double Tc = as->T_critical(), pc = as->p_critical();
        double Tt = guard([&] { return as->Ttriple(); });
        double Tmax = as->Tmax(), pmax = as->pmax();
        double rhoc = as->rhomolar_critical();
        std::printf("META,%s", name.c_str());
        pr(M); pr(R); pr(Tr); pr(rhor); pr(Tc); pr(pc); pr(Tt); pr(Tmax); pr(pmax); pr(rhoc);
        std::printf("\n");

        // --- E grid ---
        int idx = 0;
        double tau_lo = Tr / Tmax, tau_hi = Tr / Tt;
        std::vector<double> taus = logspace(tau_lo, tau_hi, 11);
        for (double tau : taus)
            for (double d : delta_grid)
                emit_e(name, idx++, *as, Tr / tau, d * rhor * M);
        for (double tau : nc_tau)
            for (double d : nc_delta)
                emit_e(name, idx++, *as, Tr / tau, d * rhor * M);

        // --- saturation ---
        if (Tc > Tt) {
            for (int i = 0; i < 30; ++i) {
                double u = double(i) / 29.0;
                double T = Tt + (Tc - 1e-3 - Tt) * (1.0 - std::pow(1.0 - u, 3.0));
                // ancillary fits only (superancillaries off)
                set_config_bool(ENABLE_SUPERANCILLARIES, false);
                std::printf("SATA,%s", name.c_str());
                pr(T);
                pr(guard([&] { return as->saturation_ancillary(iP, 0, iT, T); }));
                // molar density, converted exactly as the Rust side does (rho_molar * M)
                pr(guard([&] { return as->saturation_ancillary(iDmolar, 0, iT, T) * M; }));
                pr(guard([&] { return as->saturation_ancillary(iDmolar, 1, iT, T) * M; }));
                std::printf("\n");
                emit_sat("SAT", name, *as, T);
                set_config_bool(ENABLE_SUPERANCILLARIES, true);
                emit_sat("SATS", name, *as, T);
            }
        }

        // --- PT flash ---
        auto emit_pt = [&](double T, double p) {
            std::printf("PT,%s", name.c_str());
            pr(T); pr(p);
            try {
                as->unspecify_phase();
                as->update(PT_INPUTS, p, T);
                pr(as->rhomass());
                pr(as->hmass());
                pr(as->smass());
                std::printf(",%d", (int)as->phase());
            } catch (...) { pr(NaN); pr(NaN); pr(NaN); std::printf(",-1"); }
            std::printf("\n");
        };
        set_config_bool(ENABLE_SUPERANCILLARIES, true);
        for (int i = 0; i < 8; ++i) {
            double T = Tt * 1.001 + (Tc - 0.01 - Tt * 1.001) * double(i) / 7.0;
            double ps = guard([&] { as->unspecify_phase(); as->update(QT_INPUTS, 0.0, T); return as->p(); });
            if (std::isnan(ps)) continue;
            for (double f : pfac) {
                double p = ps * f;
                if (p > 0.9 * pmax) p = 0.9 * pmax;
                emit_pt(T, p);
            }
        }
        std::vector<double> ps6 = logspace(0.01 * pc, 0.9 * pmax, 6);
        for (int i = 0; i < 6; ++i) {
            double T = 1.01 * Tc + (0.95 * Tmax - 1.01 * Tc) * double(i) / 5.0;
            for (double p : ps6) emit_pt(T, p);
        }
    }
    return 0;
}
