import math
def ign(xh2,xco,xo2,xd,L=(0.10,0.167),o2=0.05,dil=0.55):
    y=xh2+xco*L[0]/L[1]; return y, (y>=L[0] and xo2>=o2 and xd<dil)
print("BUR-01")
for c in [(0.10,0,0.2,0),(0.099,0,0.2,0),(0.06,0.0668,0.2,0),(0.06,0.0669,0.2,0),(0.2,0,0.049,0),(0.2,0,0.2,0.55)]:
    print(c, ign(*c))
print("igniters", ign(0.05,0.05,0.2,0,(0.07,0.129)), ign(0.03,0.07,0.2,0,(0.07,0.129)))
print("CO-only threshold no igniter", 0.167, "with igniter",0.129)
def cc(y): return min(1,max(0,23.4116*(y-0.03746)))
print("BUR-02")
for y in [0.03,0.03746,0.05,0.06,0.08,0.0801588]: print(y, repr(cc(y)), (1-cc(y))*y)
print("CC=1 at y=",0.03746+1/23.4116)
y=0.05+0.03*0.08/0.148; print("H2 5% CO 3%: Y=",repr(y),"CC=",repr(cc(y)),"Ymin=",repr((1-cc(y))*y))
def vb(y):
    if y<=0.1: return 59.2*y+1.792
    if y<=0.2: return 172.88*y-9.576
    if y<=0.3: return 50*y+15
    if y<=0.4: return -50*y+45
    if y<=0.6: return -75*y+55
    return -64.3*y+48.58
def cd(y,xd):
    lean=max(0.05,1-4.53*xd+5.37*xd*xd); rich=max(0.0,1-1.29*xd)
    if y<=0.2: return lean
    if y<=0.3: return lean*(0.3-y)/0.1+rich*(y-0.2)/0.1
    return rich
print("BUR-03")
for y in [0,0.05,0.1,0.15,0.2,0.25,0.3,0.35,0.4,0.5,0.6,0.7,0.75,1.0]: print(y, repr(vb(y)))
print("zero at", 48.58/64.3)
C=[59.2,1.792,-9.576,15,45,55,48.58]; Y=[0.1,0.2,0.3,0.4,0.6]
s=C[0]; print("slopes",s)
for i in range(5):
    s+= (C[i+1]-C[i+2])/Y[i]; print(s)
print("BUR-04")
for y,xd in [(0.1,0),(0.1,0.2),(0.1,0.4),(0.1,0.5),(0.25,0.2),(0.35,0.2),(0.35,0.8),(0.1,0.42)]: print(y,xd,repr(cd(y,xd)), repr(vb(y)*cd(y,xd)))
print("lean min at xd=",4.53/(2*5.37), 1-4.53**2/(4*5.37))
print("BUR-05")
for V in [26000.0,1000.0]:
    r=(3*V/(4*math.pi))**(1/3); print(V, repr(r))
r=(3*26000/(4*math.pi))**(1/3); y=0.12; v=vb(y)*cd(y,0); print("tcomb", repr(r/v), "tprp TFRAC=0.5", r/v*0.5)
y=0.06;print("6%: V",repr(vb(y)),"tcomb",repr(r/vb(y)))
print("BUR-06 rate")
# n=1000 mol, xh2=0.10,xco=0.02, Y with cc ratio
n=1000.0; h2=100.0; co=20.0; ratio=0.08/0.148
Y=(h2+co*ratio)/n; Ymin=(1-cc(Y))*Y; t=r/(vb(Y)*cd(Y,0.1))
print("Y",repr(Y),"CC",cc(Y),"Ymin",Ymin,"V",repr(vb(Y)*cd(Y,0.1)), "tcomb(26000)",repr(t))
# single step DT=0.5 from t0
DT=0.5; yrate=(Y-Ymin)/t; dh2=h2*yrate*DT/Y; dco=co*yrate*DT/Y
print("YRATE",repr(yrate),"DELH2",repr(dh2),"DELCO",repr(dco),"O2",repr(0.5*(dh2+dco)))
# exact final after many steps no flow
h,c,tl=h2,co,t
while tl>1e-12:
    d=min(0.5,tl); Yc=(h+c*ratio)/n; f=(Yc-Ymin)/tl*d/Yc; h-=h*f;c-=c*f; tl-=d
print("final Y",repr((h+c*ratio)/n),"vs Ymin",repr(Ymin), h, c)
print("BUR-07 prop")
for d,(lh,lc) in {"up":(0.041,0.125),"hor":(0.06,0.138),"down":(0.09,0.15)}.items():
    xh,xc=0.05,0.0;print(d, xh+xc*lh/lc>=lh)
    print(d, "H2 4% + CO 3%:", repr(0.04+0.03*lh/lc), 0.04+0.03*lh/lc>=lh)
print("BUR-08 det", (0.15>0.14 and 0.10>0.09 and 0.29<0.30),(0.14>0.14))
