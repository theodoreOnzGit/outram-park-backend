import math
p=lambda *a: print(*a)
# CVH-01 retention fraction Eq 5-29
v0=0.3
for dt,Z in [(0.1,2.0),(1.0,2.0),(1e-6,2.0),(10.0,0.5)]:
    p("CVH01 f", dt, Z, repr(1/(1+2*v0*dt/Z)))
# CVH-02 steady state of the dynamic scheme -> Eq 5-26
A=20.0; Vl=40.0
for G in [3.0, 10.0]:
    # steady from 5-29: VB = f(VB + G dt) -> VB = G Z/(2 v0)  (Z swollen); iterate
    for dt in [0.01,0.1,1.0]:
        VB=0.0
        for i in range(20000):
            Z=(Vl+VB)/A
            f=1/(1+2*v0*dt/Z)
            VB=f*(VB+G*dt)
            a=VB/(Vl+VB)
            if a>0.4:
                VB=0.4/(0.6)*Vl; a=0.4
        p("CVH02 G",G,"dt",dt,"alpha",repr(a),"Zswollen",repr((Vl+VB)/A))
    a=G/(2*v0*A); p("CVH02 closed alpha",repr(a), "Z", repr(2.0/(1-min(a,0.4))))
# from q: 50 MW at 7 MPa, hfg=1.5052e6, vg=0.027378 (inputs)
q=50e6; hfg=2772.6e3-1267.4e3; vg=0.027378
G=q/hfg*vg; a=G/(2*v0*A); p("CVH02b G",repr(G),"alpha",repr(a),"Z",repr(2.0/(1-a)))
# CVH-03 Eq 5-30
MP=1.0e5; hl=1267.4e3; hv=2772.6e3
for hP in [1.30e6, 1.2674e6, 1.25e6, 3.0e6]:
    Mv=(hP-hl)/(hv-hl)*MP; p("CVH03 hP",hP,"Mv",repr(min(max(Mv,0) if False else Mv, MP)))
# CVH-04 Eq 5-1 / 5-6
PA=2.0e5; PwA=3.0e4; C=0.01
for m in [-0.002, 0.0, 0.002]:
    Pwl=PA-(PA-PwA)*math.exp(-m/C); back=C*math.log((PA-PwA)/(PA-Pwl)); p("CVH04 m",m,"Pwl",repr(Pwl),"roundtrip",repr(back))
# Eq 5-2: C = rho_v D h/k (Sc/Pr)^(1/3)
rho_v=1.1291; D=3.0e-5; h=5.0; k=0.03; Sc=0.6; Pr=0.7
p("CVH04 C",repr(rho_v*D*h/k*(Sc/Pr)**(1/3)))
