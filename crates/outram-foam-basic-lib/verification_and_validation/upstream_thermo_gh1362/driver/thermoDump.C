// gh:#1362 row 4: thermophysics dump against the UNMODIFIED OpenFOAM-v2506
// (ESI) specie layer. Every combination thermo x equationOfState x transport
// is built from a dictionary exactly as a case file would, then Cp, Ha, Hs,
// Hc, S (0 where upstream cannot compile it), rho, psi, Z, CpMCv, mu, kappa, alphah are printed as the hex bit
// pattern of each double on a (T, p) grid.
#include "specie.H"
#include "perfectGas.H"
#include "rhoConst.H"
#include "icoPolynomial.H"
#include "PengRobinsonGas.H"
#include "hConstThermo.H"
#include "janafThermo.H"
#include "hPolynomialThermo.H"
#include "hTabulatedThermo.H"
#include "sensibleEnthalpy.H"
#include "thermo.H"
#include "constTransport.H"
#include "sutherlandTransport.H"
#include "polynomialTransport.H"
#include "tabulatedTransport.H"
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

template<class Mixture, bool HasS>
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
                m.Cp(p, T), m.Ha(p, T), m.Hs(p, T), m.Hc(), 0.0,
                m.rho(p, T), m.psi(p, T), m.Z(p, T), m.CpMCv(p, T),
                m.mu(p, T), m.kappa(p, T), m.alphah(p, T)
            };
            if constexpr (HasS) { v[4] = m.S(p, T); }
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
struct EosIco  { static const char* name() { return "icoPolynomial"; }
                 template<class S> using type = icoPolynomial<S, 8>;
                 static const char* text() { return "equationOfState { rhoCoeffs<8> (1010 -0.4 -2.0e-3 1.0e-6 0 0 0 0); }\n"; } };
struct EosPR   { static const char* name() { return "PengRobinson"; }
                 template<class S> using type = PengRobinsonGas<S>;
                 static const char* text() { return "equationOfState { Tc 304.12; Vc 0.0940; Pc 7.374e6; omega 0.2239; }\n"; } };

struct ThConst { static constexpr bool hasS = true; static const char* name() { return "hConst"; }
                 template<class E> using type = hConstThermo<E>;
                 static const char* text() { return "thermodynamics { Cp 1004.5; Hf 2.5e5; }\n"; } };
struct ThJanaf { static constexpr bool hasS = true; static const char* name() { return "janaf"; }
                 template<class E> using type = janafThermo<E>;
                 static const char* text() { return "thermodynamics { Tlow 200; Thigh 6000; Tcommon 1000;\n"
                   " highCpCoeffs (2.95257637 1.3969004e-3 -4.92631603e-7 7.8600091e-11 -4.60755204e-15 -923.948688 5.87188762);\n"
                   " lowCpCoeffs (3.53100528 -1.23660988e-4 -5.02999433e-7 2.43530612e-9 -1.40881235e-12 -1046.97628 2.96747038); }\n"; } };
struct ThPoly  { static constexpr bool hasS = true; static const char* name() { return "hPolynomial"; }
                 template<class E> using type = hPolynomialThermo<E, 8>;
                 static const char* text() { return "thermodynamics { Hf 1.0e5; Sf 200; CpCoeffs<8> (1000 0.1 -2.0e-5 3.0e-9 0 0 0 0); }\n"; } };
struct ThTab   { static constexpr bool hasS = false; static const char* name() { return "hTabulated"; }
                 template<class E> using type = hTabulatedThermo<E>;
                 static const char* text() { return "thermodynamics { Hf 1.0e5; Sf 200; Cp ((200 1000) (400 1020) (800 1100) (1200 1180) (2000 1250)); }\n"; } };

struct TrConst { static const char* name() { return "const"; }
                 template<class T> using type = constTransport<T>;
                 static const char* text() { return "transport { mu 1.8e-5; Pr 0.7; }\n"; } };
struct TrSuth  { static const char* name() { return "sutherland"; }
                 template<class T> using type = sutherlandTransport<T>;
                 static const char* text() { return "transport { As 1.458e-6; Ts 110.4; }\n"; } };
struct TrPoly  { static const char* name() { return "polynomial"; }
                 template<class T> using type = polynomialTransport<T, 8>;
                 static const char* text() { return "transport { muCoeffs<8> (1.0e-5 4.0e-8 -1.0e-11 0 0 0 0 0); kappaCoeffs<8> (0.01 6.0e-5 -1.0e-8 0 0 0 0 0); }\n"; } };
struct TrTab   { static const char* name() { return "tabulated"; }
                 template<class T> using type = tabulatedTransport<T>;
                 static const char* text() { return "transport { mu ((200 1.0e-5) (500 2.4e-5) (1000 4.0e-5) (2000 6.5e-5)); kappa ((200 0.02) (600 0.045) (1200 0.08) (2000 0.12)); }\n"; } };

template<class Eos, class Th, class Tr>
static void oneCombo()
{
    typedef typename Tr::template type
    <
        species::thermo<typename Th::template type<typename Eos::template type<specie>>, sensibleEnthalpy>
    > Mixture;
    const std::string tag = std::string(Th::name()) + "/" + Eos::name() + "/" + Tr::name();
    const double W = (std::string(Eos::name()) == "PengRobinson") ? 44.01 : 28.96;
    char buf[64];
    std::snprintf(buf, sizeof buf, "specie { molWeight %.17g; }\n", W);
    dumpMixture<Mixture, Th::hasS>(tag.c_str(), std::string(buf) + Eos::text() + Th::text() + Tr::text());
}

template<class Eos, class Th>
static void transports()
{
    oneCombo<Eos, Th, TrConst>();
    oneCombo<Eos, Th, TrSuth>();
    oneCombo<Eos, Th, TrPoly>();
    oneCombo<Eos, Th, TrTab>();
}

template<class Eos>
static void thermos()
{
    transports<Eos, ThConst>();
    transports<Eos, ThJanaf>();
    transports<Eos, ThPoly>();
    transports<Eos, ThTab>();
}

int main()
{
    std::printf("# thermoDump gh:#1362 row 4, ESI v2506; T grid 10 x p grid 4; fields: Cp Ha Hs Hc S rho psi Z CpMCv mu kappa alphah\n");
    thermos<EosPG>();
    thermos<EosRC>();
    thermos<EosIco>();
    thermos<EosPR>();
    return 0;
}
