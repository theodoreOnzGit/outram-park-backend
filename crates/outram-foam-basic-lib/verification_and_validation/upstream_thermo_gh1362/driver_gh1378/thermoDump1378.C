// gh:#1378 family 1: thermophysics dump against the UNMODIFIED OpenFOAM-v2506
// (ESI) specie layer, for the equations of state, thermo and transport models
// the gh:#1362 row 4 driver does not cover. Output format as thermoDump.C: 12
// hex bit patterns per (T, p): Cp Ha Hs Hc S rho psi Z CpMCv mu kappa alphah.
// eConst cannot give Cp/Ha/Hs/S in v2506 (they are undefined upstream), so its
// rows carry Cv Ea Es Hc 0 rho psi Z CpMCv 0 0 0.
#include "specie.H"
#include "incompressiblePerfectGas.H"
#include "Boussinesq.H"
#include "adiabaticPerfectFluid.H"
#include "perfectFluid.H"
#include "rPolynomial.H"
#include "linear.H"
#include "perfectGas.H"
#include "rhoConst.H"
#include "hConstThermo.H"
#include "hPowerThermo.H"
#include "eConstThermo.H"
#include "sensibleEnthalpy.H"
#include "sensibleInternalEnergy.H"
#include "thermo.H"
#include "constTransport.H"
#include "WLFTransport.H"
#include "logPolynomialTransport.H"
#include "IStringStream.H"
#include "dictionary.H"
#include <cstdint>
#include <cstring>
#include <cstdio>
#include <string>

using namespace Foam;

static const double Ts[] = {250, 280, 300, 350, 400, 600, 900, 1100, 1500, 1900};
static const double Ps[] = {1e5, 1e6, 3e6, 8e6};

static void hex(const char* tag, int it, int ip, const double* v, int n)
{
    std::printf("%s %d %d", tag, it, ip);
    for (int i = 0; i < n; i++)
    {
        std::uint64_t u;
        std::memcpy(&u, &v[i], 8);
        std::printf(" %016llx", (unsigned long long)u);
    }
    std::printf("\n");
}

template<class Mixture>
static void dumpMixture(const char* tag, const std::string& text)
{
    IStringStream is(text);
    dictionary dict(is);
    Mixture m(dict);
    for (int it = 0; it < 10; it++)
    {
        for (int ip = 0; ip < 4; ip++)
        {
            const scalar T = Ts[it];
            const scalar p = Ps[ip];
            double v[12] =
            {
                m.Cp(p, T), m.Ha(p, T), m.Hs(p, T), m.Hc(), m.S(p, T),
                m.rho(p, T), m.psi(p, T), m.Z(p, T), m.CpMCv(p, T),
                m.mu(p, T), m.kappa(p, T), m.alphah(p, T)
            };
            hex(tag, it, ip, v, 12);
        }
    }
}

template<class Mixture>
static void dumpEMixture(const char* tag, const std::string& text)
{
    IStringStream is(text);
    dictionary dict(is);
    Mixture m(dict);
    for (int it = 0; it < 10; it++)
    {
        for (int ip = 0; ip < 4; ip++)
        {
            const scalar T = Ts[it];
            const scalar p = Ps[ip];
            double v[12] =
            {
                m.Cv(p, T), m.Ea(p, T), m.Es(p, T), m.Hc(), 0.0,
                m.rho(p, T), m.psi(p, T), m.Z(p, T), m.CpMCv(p, T),
                0.0, 0.0, 0.0
            };
            hex(tag, it, ip, v, 12);
        }
    }
}

struct EosPG   { static const char* name() { return "perfectGas"; }
                 template<class S> using type = perfectGas<S>;
                 static const char* text() { return ""; } };
struct EosRC   { static const char* name() { return "rhoConst"; }
                 template<class S> using type = rhoConst<S>;
                 static const char* text() { return "equationOfState { rho 998.2; }\n"; } };
struct EosIPG  { static const char* name() { return "incompressiblePerfectGas"; }
                 template<class S> using type = incompressiblePerfectGas<S>;
                 static const char* text() { return "equationOfState { pRef 1e5; }\n"; } };
struct EosBou  { static const char* name() { return "Boussinesq"; }
                 template<class S> using type = Boussinesq<S>;
                 static const char* text() { return "equationOfState { rho0 1000; T0 300; beta 2e-4; }\n"; } };
struct EosAPF  { static const char* name() { return "adiabaticPerfectFluid"; }
                 template<class S> using type = adiabaticPerfectFluid<S>;
                 static const char* text() { return "equationOfState { p0 1e5; rho0 1000; gamma 7.15; B 3e8; }\n"; } };
struct EosPF   { static const char* name() { return "perfectFluid"; }
                 template<class S> using type = perfectFluid<S>;
                 static const char* text() { return "equationOfState { R 3000; rho0 900; }\n"; } };
struct EosRP   { static const char* name() { return "rPolynomial"; }
                 template<class S> using type = rPolynomial<S>;
                 static const char* text() { return "equationOfState { C (1e-3 2e-7 1e-9 4e-13 1e-16); }\n"; } };
struct EosLin  { static const char* name() { return "linear"; }
                 template<class S> using type = linear<S>;
                 static const char* text() { return "equationOfState { psi 4.5e-7; rho0 998; }\n"; } };

struct ThConst { static const char* name() { return "hConst"; }
                 template<class E> using type = hConstThermo<E>;
                 static const char* text() { return "thermodynamics { Cp 1004.5; Hf 2.5e5; }\n"; } };
struct ThPower { static const char* name() { return "hPower"; }
                 template<class E> using type = hPowerThermo<E>;
                 static const char* text() { return "thermodynamics { C0 1000; n0 0.7; Tref 300; Hf 1e5; }\n"; } };
struct ThEConst{ static const char* name() { return "eConst"; }
                 template<class E> using type = eConstThermo<E>;
                 static const char* text() { return "thermodynamics { Cv 718; Hf 2.5e5; }\n"; } };

struct TrConst { static const char* name() { return "const"; }
                 template<class T> using type = constTransport<T>;
                 static const char* text() { return "transport { mu 1.8e-5; Pr 0.7; }\n"; } };
struct TrWLF   { static const char* name() { return "WLF"; }
                 template<class T> using type = WLFTransport<T>;
                 static const char* text() { return "transport { mu0 1e3; Tr 300; C1 17.44; C2 51.6; Pr 10; }\n"; } };
struct TrLogP  { static const char* name() { return "logPolynomial"; }
                 template<class T> using type = logPolynomialTransport<T, 8>;
                 static const char* text() { return "transport { muLogCoeffs<8> (-13.1 0.7 0 0 0 0 0 0); kappaLogCoeffs<8> (-9.2 0.8 0 0 0 0 0 0); }\n"; } };

static std::string head()
{
    char buf[64];
    std::snprintf(buf, sizeof buf, "specie { molWeight %.17g; }\n", 28.96);
    return std::string(buf);
}

template<class Eos, class Th, class Tr>
static void oneCombo()
{
    typedef typename Tr::template type
    <
        species::thermo<typename Th::template type<typename Eos::template type<specie>>, sensibleEnthalpy>
    > Mixture;
    const std::string tag = std::string(Th::name()) + "/" + Eos::name() + "/" + Tr::name();
    dumpMixture<Mixture>(tag.c_str(), head() + Eos::text() + Th::text() + Tr::text());
}

template<class Eos, class Th>
static void transports()
{
    oneCombo<Eos, Th, TrConst>();
    oneCombo<Eos, Th, TrWLF>();
    oneCombo<Eos, Th, TrLogP>();
}

template<class Eos>
static void eConst()
{
    typedef species::thermo<eConstThermo<typename Eos::template type<specie>>, sensibleInternalEnergy> Mixture;
    const std::string tag = std::string("eConst/") + Eos::name();
    dumpEMixture<Mixture>(tag.c_str(), head() + Eos::text() + ThEConst::text());
}

template<class Eos>
static void forEos()
{
    transports<Eos, ThConst>();
    transports<Eos, ThPower>();
    eConst<Eos>();
}

int main()
{
    std::printf("# thermoDump1378 gh:#1378 family 1, ESI v2506; T grid 10 x p grid 4; fields: Cp Ha Hs Hc S rho psi Z CpMCv mu kappa alphah (eConst rows: Cv Ea Es Hc 0 rho psi Z CpMCv 0 0 0)\n");
    forEos<EosPG>();
    forEos<EosRC>();
    forEos<EosIPG>();
    forEos<EosBou>();
    forEos<EosAPF>();
    forEos<EosPF>();
    forEos<EosRP>();
    forEos<EosLin>();
    return 0;
}
