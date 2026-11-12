# PRE_RUN hook for the one-step Godiva_SN comparison (gh:#1474): endTime 1, writePrecision 17, angularFlux solver 1e-10 / relTol 0.
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
sed -i 's/^endTime .*/endTime         1;/' system/controlDict
# only the angularFlux solver is tightened (BiCGStab to 1e-10, relTol 0); the rest stays as shipped
python3 - <<'EOF'
import re
p='system/neutroRegion/fvSolution'
s=open(p).read()
i=s.index('"angularFlux.*"')
j=s.index('}',i)
blk=s[i:j]
blk=re.sub(r'tolerance\s+[0-9.e+-]+;','tolerance       1e-10;',blk)
blk=re.sub(r'relTol\s+[0-9.e+-]+;','relTol          0;',blk)
s=s[:i]+blk+s[j:]
open(p,'w').write(s)
EOF
