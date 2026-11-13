// gh:#1363 row 5: text dump of intermediates (scratch copy only; see
// driver/twoPhaseEulerDump.diff). One line per (step, name): the values at
// 17 significant digits, vectors flattened x y z, face fields = internal faces.
#include <cstdio>
static FILE* dumpFile = nullptr;
static void dumpS(int step, const char* name, const Foam::UList<Foam::scalar>& f, Foam::label n)
{
    if (!dumpFile) dumpFile = fopen("dump.txt", "w");
    fprintf(dumpFile, "%d %s", step, name);
    for (Foam::label i = 0; i < n; ++i) fprintf(dumpFile, " %.17g", f[i]);
    fprintf(dumpFile, "\n");
}
static void dumpV(int step, const char* name, const Foam::UList<Foam::vector>& f, Foam::label n)
{
    if (!dumpFile) dumpFile = fopen("dump.txt", "w");
    fprintf(dumpFile, "%d %s", step, name);
    for (Foam::label i = 0; i < n; ++i) fprintf(dumpFile, " %.17g %.17g %.17g", f[i].x(), f[i].y(), f[i].z());
    fprintf(dumpFile, "\n");
}
