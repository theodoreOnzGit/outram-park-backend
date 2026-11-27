#!/usr/bin/env python3
"""gh:#1934 round 67: adds the dump calls to the COPY `GAMGdumpSolverSolve.C` made by build.sh (never the shipped tree)."""
import sys
p = sys.argv[1]
s = open(p).read()
s = s.replace('#include "PrecisionAdaptor.H"\n', '#include "PrecisionAdaptor.H"\n#include "gamgDump.H"\n', 1)
marker = '    solverPerf.finalResidual() = solverPerf.initialResidual();\n'
ins = marker + r'''
    const int gdIdx = ++gamgDump::solveIndex();
    FILE* gdCyc = (gdIdx < gamgDump::maxSolves()) ? gamgDump::open("cycles.txt", "a") : nullptr;
    if (gdCyc)
    {
        std::fprintf(gdCyc, "SOLVE %d %s init %.17g norm %.17g\n", gdIdx, fieldName_.c_str(), solverPerf.initialResidual(), normFactor);
    }
    if (gdIdx == gamgDump::target())
    {
        FILE* f = gamgDump::open("matrix.txt", "w");
        if (f)
        {
            std::fprintf(f, "NCELLS %d NFACES %d SYM %d NLEVELS %d\n", int(matrix_.diag().size()), int(matrix_.upper().size()), int(matrix_.symmetric()), int(matrixLevels_.size()));
            gamgDump::ints(f, "LOWER", matrix_.lduAddr().lowerAddr());
            gamgDump::ints(f, "UPPER", matrix_.lduAddr().upperAddr());
            gamgDump::reals(f, "DIAG", matrix_.diag());
            gamgDump::reals(f, "UPPERC", matrix_.upper());
            if (matrix_.hasLower()) gamgDump::reals(f, "LOWERC", matrix_.lower());
            gamgDump::reals(f, "SOURCE", source);
            gamgDump::reals(f, "PSI0", psi);
            std::fprintf(f, "TOL %.17g RELTOL %.17g MAXITER %d MINITER %d\n", tolerance_, relTol_, int(maxIter_), int(minIter_));
            std::fclose(f);
        }
        f = gamgDump::open("agglom.txt", "w");
        if (f)
        {
            std::fprintf(f, "NLEVELS %d\n", int(matrixLevels_.size()));
            for (label l = 0; l < label(matrixLevels_.size()); l++)
            {
                std::fprintf(f, "LEVEL %d NCOARSE %d NCFACES %d\n", int(l), int(agglomeration_.nCells(l)), int(agglomeration_.nFaces(l)));
                gamgDump::ints(f, "RESTRICT", agglomeration_.restrictAddressing(l));
                gamgDump::ints(f, "FACERESTRICT", agglomeration_.faceRestrictAddressing(l));
                gamgDump::bools(f, "FACEFLIP", agglomeration_.faceFlipMap(l));
                const lduMatrix& cm = matrixLevels_[l];
                gamgDump::ints(f, "CLOWER", cm.lduAddr().lowerAddr());
                gamgDump::ints(f, "CUPPER", cm.lduAddr().upperAddr());
                gamgDump::reals(f, "CDIAG", cm.diag());
                gamgDump::reals(f, "CUPPERC", cm.upper());
                if (cm.hasLower()) gamgDump::reals(f, "CLOWERC", cm.lower());
            }
            std::fclose(f);
        }
    }
'''
assert marker in s
s = s.replace(marker, ins, 1)
m2 = '                solverPerf.print(Info.masterStream(matrix().mesh().comm()));\n            }\n'
assert m2 in s
s = s.replace(m2, m2 + '''            if (gdCyc)
            {
                std::fprintf(gdCyc, "VC %d %d %.17g\\n", gdIdx, int(solverPerf.nIterations()) + 1, solverPerf.finalResidual());
            }
''', 1)
m3 = '    return solverPerf;\n'
s = s.replace(m3, '''    if (gdCyc)
    {
        std::fprintf(gdCyc, "END %d iters %d final %.17g\\n", gdIdx, int(solverPerf.nIterations()), solverPerf.finalResidual());
        std::fclose(gdCyc);
    }
    if (gdIdx == gamgDump::target())
    {
        FILE* f = gamgDump::open("solution.txt", "w");
        if (f)
        {
            gamgDump::reals(f, "PSI", psi);
            std::fclose(f);
        }
    }
''' + m3, 1)
open(p, 'w').write(s)
