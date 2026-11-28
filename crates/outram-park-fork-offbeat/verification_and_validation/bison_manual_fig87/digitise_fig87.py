# Digitise BISON Theory Manual (INL/EXT-13-29930, 2013) Fig. 8.7, the red
# 10 MWd/kgU curves, from a 400 dpi render of PDF page 24.
# Usage: python3 -I digitise_fig87.py vhi-24.png
import sys
from PIL import Image
import numpy as np
im=np.array(Image.open(sys.argv[1]).convert('RGB')).astype(int)
R,G,B=im[...,0],im[...,1],im[...,2]
red=(R>170)&(G<110)&(B<110)
# Axis calibration from detected tick marks: x ticks 500..2500 K at
# px 1376, 1569, 1761.5, 1954, 2147 (192.75 px / 500 K); y ticks 0..0.2 at
# py 2858, 2640, 2423, 2205, 1987.5 (217.6 px / 0.05).
T=lambda px: 500+(px-1376)*500/192.75
V=lambda py: (2858-py)*0.05/217.6
markersT=[300,900,1500,2100,2700]
gas={};tot={}
for x in range(1300,3200):
    t=T(x)
    if any(abs(t-m)<45 for m in markersT): continue
    ys=np.where(red[1950:2905,x])[0]+1950
    ys=[y for y in ys if not (t<1400 and V(y)>0.05)]  # legend box
    if len(ys)==0: continue
    cl=[];cur=[ys[0]]
    for y in ys[1:]:
        if y-cur[-1]<=3: cur.append(y)
        else: cl.append(cur);cur=[y]
    cl.append(cur)
    cen=[np.mean(c) for c in cl]
    tot[t]=V(min(cen))
    if len(cen)>=2: gas[t]=V(max(cen))
pl=[v for t,v in tot.items() if 400<t<1200]
print('10-Total plateau (400-1200 K): mean %.5f sd %.5f n %d'%(np.mean(pl),np.std(pl),len(pl)))
pts=sorted((t,v) for t,v in gas.items() if 1900<t<2260)
t_=np.array([p[0] for p in pts]);v_=np.array([p[1] for p in pts])
c=np.polyfit(t_,v_,2); tp=-c[1]/(2*c[0])
print('10-Gaseous: parabola through %d points 1900-2260 K: peak at %.1f K, value %.4f'%(len(pts),tp,np.polyval(c,tp)))
for tq in [1600,1700,1800,2300,2400]:
    near=sorted((abs(t-tq),t,v) for t,v in gas.items())[0]
    print('10-Gaseous at %.1f K: %.4f'%(near[1],near[2]))
