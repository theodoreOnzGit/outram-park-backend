import math
v0=0.3; cap=0.4; A=20.0; Vl=40.0
def ours(sources):  # sources: list of (z, m3/s); cylinder; port of swollen_level_from_sources' integral
    h=0.01; z=0.0; held=0.0
    while z<100:
        flow=sum(s for zz,s in sources if zz<=z+0.5*h)
        a=min(flow/A/v0,cap)
        slab=A*h*(1-a)
        if held+slab>=Vl: return z+(Vl-held)/slab*h
        held+=slab; z+=h
for G in [1.5,3.0]:
    melcor_a=min(G/(2*v0*A),cap)
    print("bottom G",G,"ours",repr(ours([(0.0,G)])),"MELCOR",repr(2.0/(1-melcor_a)))
    n=200; src=[((j+0.5)*2.0/n, G/n) for j in range(n)]
    print("uniform-over-collapsed G",G,"ours",repr(ours(src)),"MELCOR",repr(2.0/(1-melcor_a)))
PA=2.0e5; PwA=3.0e4; C=0.01; m=-0.0005
Pwl=PA-(PA-PwA)*math.exp(-m/C); print("CVH04 Pwl", repr(Pwl), repr(C*math.log((PA-PwA)/(PA-Pwl))))
# CVH-05 interface h, C4407 defaults
sig=5.670374e-8; g=9.80665
def hfreeA(GrPr,k,X): return max(0.54*GrPr**0.25,0.14*GrPr**(1/3))*k/X
def hfreeP(GrPr,k,X): return max(0.27*GrPr**0.25,0.27*GrPr**0.25)*k/X
# atmosphere: air-like, T_A=350 K, T_l=330K, beta=1/350, rho=1.0, mu=2.0e-5, k=0.03, cp=1010, Ds=10, L=5 -> X=5
rho=1.0; mu=2.0e-5; k=0.03; cp=1010.0; X=min(10.0,5.0)
Gr=g*(1/350)*20*X**3*(rho/mu)**2; Pr=mu*cp/k
print("CVH05 atm Gr",repr(Gr),"Pr",repr(Pr),"GrPr",repr(Gr*Pr),"h_free",repr(hfreeA(Gr*Pr,k,X)))
print("  laminar",repr(0.54*(Gr*Pr)**0.25*k/X),"turb",repr(0.14*(Gr*Pr)**(1/3)*k/X))
# crossover GrPr where 0.54 x^1/4 = 0.14 x^1/3 -> x = (0.54/0.14)^12
print("  crossover GrPr", repr((0.54/0.14)**12))
hf=0.02*rho*cp*0.5; print("  h_forced v=0.5", repr(hf))
hstar_rad=sig*(350**2+330**2)*(350+330); print("  rad part of h*", repr(hstar_rad))
print("  h* = ", repr(max(hfreeA(Gr*Pr,k,X),hf,k/5.0)+hstar_rad))
# pool: water 360K, T_l 350, beta 7.0e-4, rho 967, mu 3.26e-4, k 0.674, cp 4200, X=min(10,2)=2
rho=967.0; mu=3.26e-4; k=0.674; cp=4200.0; X=2.0
Gr=g*7.0e-4*10*X**3*(rho/mu)**2; Pr=mu*cp/k
print("CVH05 pool GrPr",repr(Gr*Pr),"h_free",repr(hfreeP(Gr*Pr,k,X)),"k/L",repr(k/2.0))
