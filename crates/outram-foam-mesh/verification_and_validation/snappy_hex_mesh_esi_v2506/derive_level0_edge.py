#!/usr/bin/env python3
"""gh:#1467 round 8: the exact double of ESI's level0Edge (hexRef8::getLevel0EdgeLength on the
blockMesh points as snappyHexMesh READS them, 16-digit ASCII): sqrt(min edge magSqr), level 0.
The log/file print only 16 digits (0.002999999999999978); the double is 0.0029999999999999784.
usage: python3 -I derive_level0_edge.py RUN_DIR"""
import sys
import re,math
t=open(sys.argv[1] + '/constant/polyMesh/points').read()
body=t[t.index('(\n',t.index('\n3')) if False else 0:]
pts=[tuple(float(x) for x in m.split()) for m in re.findall(r'\(([^()]+)\)',t[t.index('\n('):])]
print(len(pts))
nx,ny,nz=20,16,12
def idx(i,j,k): return i+j*(nx+1)+k*(nx+1)*(ny+1)
best=1e300
for k in range(nz+1):
  for j in range(ny+1):
    for i in range(nx+1):
      a=pts[idx(i,j,k)]
      for d in ((1,0,0),(0,1,0),(0,0,1)):
        ii,jj,kk=i+d[0],j+d[1],k+d[2]
        if ii<=nx and jj<=ny and kk<=nz:
          b=pts[idx(ii,jj,kk)]
          dx=(b[0]-a[0],b[1]-a[1],b[2]-a[2])
          ms=dx[0]*dx[0]+dx[1]*dx[1]+dx[2]*dx[2]
          best=min(best,ms)
l=math.sqrt(best)
print(repr(l), l.hex())
print(repr(0.002999999999999978))
