// gh:#1934 instrumentation of a COPY of ESI v2506 GAMGSolver (not the shipped sources). Writes, for the solve whose index
// equals $GAMGDUMP_TARGET (default 0), the fine matrix, the source, the initial guess, the whole agglomeration and the coarse
// matrices; for every solve (up to $GAMGDUMP_MAXSOLVES, default 400) one `VC` line per V-cycle with the normalised residual.
#ifndef gamgDump_H
#define gamgDump_H
#include <cstdio>
#include <cstdlib>
#include <string>
namespace gamgDump
{
    inline int& solveIndex() { static int i = -1; return i; }
    inline int target() { const char* e = std::getenv("GAMGDUMP_TARGET"); return e ? std::atoi(e) : 0; }
    inline int maxSolves() { const char* e = std::getenv("GAMGDUMP_MAXSOLVES"); return e ? std::atoi(e) : 400; }
    inline std::string dir() { const char* e = std::getenv("GAMGDUMP_DIR"); return e ? std::string(e) : std::string(); }
    inline FILE* open(const char* name, const char* mode)
    {
        std::string d = dir();
        if (d.empty()) return nullptr;
        return std::fopen((d + "/" + name).c_str(), mode);
    }
    template<class L> void ints(FILE* f, const char* key, const L& l)
    {
        std::fprintf(f, "%s %d", key, int(l.size()));
        for (int i = 0; i < int(l.size()); i++) std::fprintf(f, " %d", int(l[i]));
        std::fprintf(f, "\n");
    }
    template<class L> void bools(FILE* f, const char* key, const L& l)
    {
        std::fprintf(f, "%s %d", key, int(l.size()));
        for (int i = 0; i < int(l.size()); i++) std::fprintf(f, " %d", l[i] ? 1 : 0);
        std::fprintf(f, "\n");
    }
    template<class L> void reals(FILE* f, const char* key, const L& l)
    {
        std::fprintf(f, "%s %d", key, int(l.size()));
        for (int i = 0; i < int(l.size()); i++) std::fprintf(f, " %.17g", double(l[i]));
        std::fprintf(f, "\n");
    }
}
#endif
