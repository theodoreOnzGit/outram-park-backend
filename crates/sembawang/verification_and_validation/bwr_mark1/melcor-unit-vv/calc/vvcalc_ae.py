import math as m
kB=1.380649e-23
def med(T=298.15,P=101325.0,M=28.97,mu=1.8e-5,rho_p=1000.0,g=9.8,slip=1.257,chi=1.0,gam=1.0,cs=1.0,eps=0.001,kr=0.05,ct=2.25,delta=1e-5):
    return dict(T=T,P=P,M=M,mu=mu,rp=rho_p,g=g,slip=slip,chi=chi,gam=gam,cs=cs,eps=eps,kr=kr,ct=ct,delta=delta)
def rho(M_): return 1.21e-4*M_['P']*M_['M']/M_['T']
def lam(M_): return M_['mu']/rho(M_)*m.sqrt(1.89e-4*M_['M']/M_['T'])
def Cm(M_,d):
    kn=2*lam(M_)/d; return 1+kn*(M_['slip']+0.4*m.exp(-1.1/kn))
def vgrav(M_,d): return d*d*M_['rp']*M_['g']*Cm(M_,d)/(18*M_['mu']*M_['chi'])
def Dp(M_,d): return kB*M_['T']*Cm(M_,d)/(3*m.pi*d*M_['mu']*M_['chi'])
def vdiff(M_,d): return Dp(M_,d)/M_['delta']
def vtherm(M_,d,gradT,Tw):
    kn=2*lam(M_)/d; ct=M_['ct'];kr=M_['kr']
    return 3*M_['mu']*Cm(M_,d)*(ct*kn+kr)*gradT/(2*M_['chi']*rho(M_)*Tw*(1+3*M_['slip']*kn)*(1+2*ct*kn+kr))
def vdiffusio(W,xs,Ms,Mnc,rb,rs):
    return Ms/(xs*Ms+(1-xs)*Mnc)*W/rb if W>=0 else W/rs
def brown(M_,di,dj):
    def v(d): return m.sqrt(8*kB*M_['T']/(m.pi*M_['rp']*m.pi*d**3/6))
    def gg(d):
        D=Dp(M_,d); l=8*D/(m.pi*v(d)); return ((d+l)**3-(d*d+l*l)**1.5)/(3*d*l)-d
    Di,Dj=Dp(M_,di),Dp(M_,dj); gij=m.hypot(gg(di),gg(dj)); vij=m.hypot(v(di),v(dj)); s=di+dj
    br=s/(s+2*gij)+8*(Di+Dj)/(vij*s*M_['cs'])
    return 2*m.pi*(Di+Dj)*(M_['gam']*di+M_['gam']*dj)/br
def grav(M_,di,dj):
    e=1.5*min(di,dj)**2/(di+dj)**2
    return e*m.pi/4*M_['cs']*(M_['gam']*(di+dj))**2*abs(vgrav(M_,di)-vgrav(M_,dj))
def turb(M_,di,dj):
    r=rho(M_);e=M_['eps'];mu=M_['mu']
    b1=m.sqrt(m.pi*e*r/(120*mu))*(di+dj)**3
    a=lambda d:M_['rp']*Cm(M_,d)*d*d/M_['chi']
    b2=0.04029*r**0.25*e**0.75/mu**1.25*(di+dj)**2*abs(a(di)-a(dj))
    return M_['cs']*m.hypot(b1,b2)
A=med()
print("rho_g %.6e (ideal R=8.314462618: %.6e)"%(rho(A),101325*0.02897/(8.314462618*298.15)))
print("lambda %.6e"%lam(A))
for d in (0.01e-6,0.1e-6,1e-6,10e-6):
    print("d %.0e Cm %.6f vgrav %.6e Dp %.6e vdiff %.6e"%(d,Cm(A,d),vgrav(A,d),Dp(A,d),vdiff(A,d)))
B=med(g=9.80665); print("vgrav 1um g=9.80665 %.6e ratio %.6e"%(vgrav(B,1e-6),vgrav(B,1e-6)/vgrav(A,1e-6)-1))
for d in (0.1e-6,1e-6): print("vtherm d",d,"%.6e"%vtherm(A,d,1000.0,298.15))
print("vdiffusio cond %.6e"%vdiffusio(1e-3,0.3,0.018015,0.02897,1.0,0.1))
print("vdiffusio evap %.6e"%vdiffusio(-1e-3,0.3,0.018015,0.02897,1.0,0.1))
# kernel limits
for d in (1e-9,1e-8,1e-7,1e-6,1e-5):
    b=brown(A,d,d); cont=8*kB*A['T']/(3*A['mu'])*Cm(A,d)
    print("brown equal d %.0e beta %.6e cont*Cm %.6e ratio %.6f"%(d,b,cont,b/cont))
print("brown 0.1um/1um %.6e grav 1um/10um %.6e turb 1um/10um %.6e"%(brown(A,1e-7,1e-6),grav(A,1e-6,1e-5),turb(A,1e-6,1e-5)))
print("turb equal 1um %.6e"%turb(A,1e-6,1e-6))
