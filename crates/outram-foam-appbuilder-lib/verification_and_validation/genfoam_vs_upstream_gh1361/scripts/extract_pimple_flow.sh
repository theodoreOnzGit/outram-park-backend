#!/bin/bash
# Per-step PIMPLE control flow of a GeN-Foam log (gh:#1361 round 5).
# usage: extract_pimple_flow.sh <log.GeN-Foam>
# One line per time step:  STEP <n> <time> <iterations run> <ending> <p_rgh res list> <h.liquid res list>
#   ending = conv<N>     "PIMPLE: converged in N iterations" (N = 0 is a SKIPPED step: converged_ left set by the previous step)
#            notconv     "PIMPLE: not converged within K iterations" (and, if the criteria were met in the last iteration, flagged)
#            end         neither text (nOuterCorrectors 1 style)
# The residual lists are the "abs tol = " values `customPimpleControl::criteriaSatisfied` prints at the START of
# outer iteration k >= 2 (the first-PISO p_rgh residual of iteration k-1, and the last h.liquid initial residual).
awk '
/^Time = /{ if (n>0) out(); n++; t=$3; it=0; end="end"; pr=""; hr="" }
/^PIMPLE: iteration /{ it++ }
/^PIMPLE: converged in /{ end="conv" $4 }
/^PIMPLE: not converged within /{ end="notconv" }
/p_rgh PIMPLE iter/{ match($0,/abs tol = [^ ]*/); v=substr($0,RSTART+10,RLENGTH-10); pr=pr (pr==""?"":",") v }
/h.liquid PIMPLE iter/{ match($0,/abs tol = [^ ]*/); v=substr($0,RSTART+10,RLENGTH-10); hr=hr (hr==""?"":",") v }
function out(){ print "STEP", n, t, it, end, (pr==""?"-":pr), (hr==""?"-":hr) }
END{ if (n>0) out() }
' "$1"
