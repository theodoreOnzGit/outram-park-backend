import math as m
g=9.80665
# D-5/D-6
psat=108000.0; psurf=3.0e5; rho=957.0; hp=1.2
Xo=1-psat/(psurf+rho*g*hp); print("Xo %.6f"%Xo)
for Xi in (0.2,0.5,Xo,0.9):
    print("Xi",Xi,"DFEC %.6f"%max(Xo/Xi,1.0))
def d7(df,H,r): return 1+min(H*r,1)*(df-1)
print("D-7:",d7(5,100,1e-3),d7(5,0,1e-3),d7(5,1e4,1e-3),"%.6f"%d7(3.2650,16.13,6.6e-4))
# D-1..D-4
def glob(a,b,Do,Q,sig,rho):
    Vo=Q/(m.pi/4*Do*Do); We=rho*Do*Vo*Vo/sig; vn=a*We**b
    return We,vn,(1.5*vn*Do*Do*m.sqrt(sig/(rho*g)))**(1/3)
for nm,(a,b) in (("multi",(3.45,0.46)),("downcomer",(0.0891,0.616)),("horiz",(0.857,0.73))):
    We,vn,D=glob(a,b,0.515,1.0,0.0588,957.0); print(nm,"We %.6e vn %.6e Dgo %.6e m"%(We,vn,D))
# D-8..D-11
def alpha(stk,c7=4.25973e-3):
    a= 1.79182*3.3437e-11**(5.9244e-3**stk) if stk<=0.65868 else 1.13893*1.4173e-6**(c7**stk)
    return min(a,0.99)
for stk in (0.1,0.5,0.65868,0.7,1.0,2.0,5.0):
    a=alpha(stk); print("Stk",stk,"alpha %.6f DF %.6f"%(a,1/(1-a)), "alpha(4.2597e-3) %.8f"%alpha(stk,4.2597e-3))
print("hi at 0.65868 %.6f"%(1.13893*1.4173e-6**(4.25973e-3**0.65868)))
stk=1000*10*(5e-6)**2/(9*2e-5*0.5); print("Stk example %.6e"%stk)
# E.1/E.2/E.3/E.5
for x in (0.0,0.5,1.0):
    d=0.7*10**(-0.2265+m.sqrt(0.0203+0.0313*x)); ab=1 if d<0.15 else (1.675 if d>1.3 else 0.84107+1.13466*d-0.3795*d*d)
    print("xnc",x,"dvm cm %.6f a/b %.6f"%(d,ab))
print("E.2 at 0.15 %.6f at 1.3 %.6f"%(0.84107+1.13466*0.15-0.3795*0.0225,0.84107+1.13466*1.3-0.3795*1.69))
sig=0.0717;rl=996.5
vt=7.876*((sig*1e3)/(rl*1e-3))**0.25; print("Vr,T cm/s %.6f; at 0.7011 cm %.6f; factor at 0.5: %.6f"%(vt,1.40713*0.7011**0.49275*vt,1.40713*0.5**0.49275))
def vs(Q,hp_cm): f=lambda x: m.sqrt((Q+5.33)/3.011e-3)*(1-3.975e-4*x); return min(0.5*(f(0)+f(hp_cm)),170)
print("Vs Q=0 hp=120cm %.6f; Q=10 L/s hp=300 %.6f; Q=1e3 %.6f"%(vs(0,120),vs(10,300),vs(1e3,120)))
# 2-112..2-114
def dfsr(fgl,dfbb): return 1/(fgl+(1-fgl)/dfbb)
print("DFSR fgl0.3 dfbb10 %.6f; fgl1 %.6f fgl0 %.6f"%(dfsr(0.3,10),dfsr(1,10),dfsr(0,10)))
mi=[1.0,2.0,1.0]; dfi=[1.5,10.0,1000.0]
print("DFOV,part %.6f"%(sum(mi)/sum(a/b for a,b in zip(mi,dfi))))
# iodine C7159
def K(T): return [1.3882e-3*m.exp(3279.3/T),7.7606*m.exp(1370/T),1.0423e-2*m.exp(-7148/T),4.2271e-9*m.exp(-1748.5/T),1.56531e-13*m.exp(5462.81/T-1.87376e6/T**2)]
for T in (298.15,350.,375.,400.):
    pc=10**(13.5467-0.0605142*T+7.166e-5*T*T)
    print(T,"K:",["%.6e"%x for x in K(T)],"PC_I2(2-180) %.6e ratio K1/PC %.6f"%(pc,K(T)[0]/pc))
T=298.15;print("PC_I0 %.6e PC_CH3I %.6e"%(0.0238*10**(13.5467-0.0605142*T+7.166e-5*T*T),9.4e-4*m.exp(2641/T)))
# SPR drag
c4=(27/0.271)**(1/(0.217+0.84)); c8=(0.271/2.0)**(-1/0.217)
print("C3000(4) %.6f C3000(8) %.6f"%(c4,c8))
for Re in (1.,77.737154,78.,1000.,10006.443):
    print(Re,"low %.6f mid %.6f"%(27*Re**-0.84,0.271*Re**0.217))
