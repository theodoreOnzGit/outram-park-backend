# Case Study - Radiation Release Nuclear Reactor

source("pba BETTER.r")

#######################################################################
readkeygraph <- function(prompt) {
getGraphicsEvent(prompt=prompt,onMouseDown=NULL,onMouseMove=NULL,onMouseUp=NULL,onKeybd=onKeybd,consolePrompt="[click on graph then follow top prompt to continue]")
Sys.sleep(0.01)
return(keyPressed)}

onKeybd <- function(key) keyPressed <<- key
wait = function(msg='') invisible(readkeygraph(paste(msg,'[press any key to continue]')))
rbyc = function(r=1,c=1) par(mfrow=c(r,c))
pl = function(..., xlab='',ylab='Cumulative probability',ylim=c(0,1)) plot(NULL,ylim=ylim,xlim=range(...),xlab=xlab,ylab=ylab)
sh = function(w, wi=NULL, m, pl=range(w)) {plot(w,xlim=pl); if (!missing(wi)) green(wi); blue(w); title(m)}

c_L <- function(k,n) {beta(k, n - k + 1)} # Left-bounding edge of C-box
c_R <- function(k,n) {beta(k + 1, n - k)} # Right-bounding edge of C-box
#######################################################################

# Define the component C-boxes:
Pbox$steps = 1000; samp = 1000;

c1 = KN(1, 100);  # C-box of basic event 1
c2 = KN(1, 200);  # C-box of basic event 2
c3 = KN(0, 1000); # C-box of basic event 3
c4 = KN(3, 500);  # C-box of basic event 4
c5 = KN(1, 380);  # C-box of basic event 5
c6 = KN(0, 460);  # C-box of basic event 6
c7 = KN(2, 500);  # C-box of basic event 7
c8 = KN(1, 300);  # C-box of basic event 8
c9 = KN(2, 150);  # C-box of basic event 9

# Plot the C-boxes of the components:
rbyc(3,3)
sh(c1,,'Basic event 1',c(0, 8.0e-2))
sh(c2,,'Basic event 2',c(0, 2.8e-2))
sh(c3,,'Basic event 3',c(0, 5.0e-3))
sh(c4,,'Basic event 4',c(0, 1.6e-2))
sh(c5,,'Basic event 5',c(0, 1.6e-2))
sh(c6,,'Basic event 6',c(0, 1e-2))
sh(c7,,'Basic event 7',c(0, 1.8e-2))
sh(c8,,'Basic event 8',c(0, 1.8e-2))
sh(c9,,'Basic event 9',c(0, 5e-2))
wait()

# Define the Event C-boxes under uncertain dependencies:
e4f = and(c1, c2); cbox_vec = array(c(e4f@d, e4f@u),dim = c(samp,2)); write.csv(cbox_vec, "E4_pbox_frechet_RR.csv") 
e3af = or(e4f, c3); cbox_vec = array(c(e3af@d, e3af@u),dim = c(samp,2)); write.csv(cbox_vec, "E3a_pbox_frechet_RR.csv") 
e3bf = and(c4, c5); cbox_vec = array(c(e3bf@d, e3bf@u),dim = c(samp,2)); write.csv(cbox_vec, "E3b_pbox_frechet_RR.csv") 
e2af = or(e3af, e3bf); cbox_vec = array(c(e2af@d, e2af@u),dim = c(samp,2)); write.csv(cbox_vec, "E2a_pbox_frechet_RR.csv") 
e3cf = or(c5, c7); cbox_vec = array(c(e3cf@d, e3cf@u),dim = c(samp,2)); write.csv(cbox_vec, "E3c_pbox_frechet_RR.csv") 
e3df = or(c8, c9); cbox_vec = array(c(e3df@d, e3df@u),dim = c(samp,2)); write.csv(cbox_vec, "E3d_pbox_frechet_RR.csv") 
e2bf = and(c6, and(e3cf, e3df)); cbox_vec = array(c(e2bf@d, e2bf@u),dim = c(samp,2)); write.csv(cbox_vec, "E2b_pbox_frechet_RR.csv") 

c1_L = c_L(1, 100);   c1_R = c_R(1, 100);  
c2_L = c_L(1, 200);   c2_R = c_R(1, 200); 
c3_L = c_L(0, 1000);  c3_R = c_R(0, 1000); 
c4_L = c_L(3, 500);   c4_R = c_R(3, 500);
c5_L = c_L(1, 380);   c5_R = c_R(1, 380); 
c6_L = c_L(0, 460);   c6_R = c_R(0, 460); 
c7_L = c_L(2, 500);   c7_R = c_R(2, 500);  
c8_L = c_L(1, 300);   c8_R = c_R(1, 300); 
c9_L = c_L(2, 150);   c9_R = c_R(2, 150);

e4f_L = and(c1_L, c2_L);                 e4f_R = and(c1_R, c2_R);
e3af_L = or(e4f_L, c3_L);                e3af_R = or(e4f_R, c3_R);
e3bf_L = and(c4_L, c5_L);                e3bf_R = and(c4_R, c5_R);
e2af_L = or(e3af_L, e3bf_L);             e2af_R = or(e3af_R, e3bf_R);
e3cf_L = or(c5_L, c7_L);                 e3cf_R = or(c5_R, c7_R);
e3df_L = or(c8_L, c9_L);                 e3df_R = or(c8_R, c9_R);
e2bf_L = and(c6_L, and(e3cf_L, e3df_L)); e2bf_R = and(c6_R, and(e3cf_R, e3df_R));
e1f_L = or(e2af_L, e2bf_L);              e1f_R = or(e2af_R, e2bf_R);
e1f = env.pbox(e1f_L, e1f_R) # Event E1 computed considering repeated variables
cbox_vec = array(c(e1f@d, e1f@u),dim = c(samp,2)); write.csv(cbox_vec, "E1_pbox_frechet_RR.csv") 

# Define the Event C-boxes under independence:
e4i = andI(c1, c2); cbox_vec = array(c(e4i@d, e4i@u),dim = c(samp,2)); write.csv(cbox_vec, "E4_pbox_indep_RR.csv") 
e3ai = orI(e4i, c3); cbox_vec = array(c(e3ai@d, e3ai@u),dim = c(samp,2)); write.csv(cbox_vec, "E3a_pbox_indep_RR.csv")
e3bi = andI(c4, c5); cbox_vec = array(c(e3bi@d, e3bi@u),dim = c(samp,2)); write.csv(cbox_vec, "E3b_pbox_indep_RR.csv")
e2ai = orI(e3ai, e3bi); cbox_vec = array(c(e2ai@d, e2ai@u),dim = c(samp,2)); write.csv(cbox_vec, "E2a_pbox_indep_RR.csv")
e3ci = orI(c5, c7); cbox_vec = array(c(e3ci@d, e3ci@u),dim = c(samp,2)); write.csv(cbox_vec, "E3c_pbox_indep_RR.csv")
e3di = orI(c8, c9); cbox_vec = array(c(e3di@d, e3di@u),dim = c(samp,2)); write.csv(cbox_vec, "E3d_pbox_indep_RR.csv")
e2bi = andI(c6, andI(e3ci, e3di)); cbox_vec = array(c(e2bi@d, e2bi@u),dim = c(samp,2)); write.csv(cbox_vec, "E2b_pbox_indep_RR.csv")
e1i = orI(e2ai, e2bi); cbox_vec = array(c(e1i@d, e1i@u),dim = c(samp,2)); write.csv(cbox_vec, "E1_pbox_indep_RR.csv")

# Plot the C-boxes of the Events
rbyc(2,4)
sh(e1f,e1i,'E1 (Top event)',c(0, 0.1))
sh(e2af,e2ai,'E2a',c(0, 0.08))
sh(e2bf,e2bi,'E2b',c(0, 0.012))
sh(e3af,e3ai,'E3a',c(0, 0.05))
sh(e3bf,e3bi,'E3b',c(0, 0.025))
sh(e3cf,e3ci,'E3c',c(0, 0.05))
sh(e3df,e3di,'E3d',c(0, 0.1))
sh(e4f,e4i,'E4',c(0, 0.04))
wait()

## Results of the 95% confidence intervals:
conf_lb = 0.025; conf_ub = 1 - conf_lb;

# Under independence:
interval_e1i = interval(cut(e1i, conf_lb), cut(e1i, conf_ub))
interval_e2ai = interval(cut(e2ai, conf_lb), cut(e2ai, conf_ub))
interval_e2bi = interval(cut(e2bi, conf_lb), cut(e2bi, conf_ub))
interval_e3ai = interval(cut(e3ai, conf_lb), cut(e3ai, conf_ub))
interval_e3bi = interval(cut(e3bi, conf_lb), cut(e3bi, conf_ub))
interval_e3ci = interval(cut(e3ci, conf_lb), cut(e3ci, conf_ub))
interval_e3di = interval(cut(e3di, conf_lb), cut(e3di, conf_ub))
interval_e4i = interval(cut(e4i, conf_lb), cut(e4i, conf_ub))

# Under uncertain dependencies:
interval_e1f = interval(cut(e1f, conf_lb), cut(e1f, conf_ub))
interval_e2af = interval(cut(e2af, conf_lb), cut(e2af, conf_ub))
interval_e2bf = interval(cut(e2bf, conf_lb), cut(e2bf, conf_ub))
interval_e3af = interval(cut(e3af, conf_lb), cut(e3af, conf_ub))
interval_e3bf = interval(cut(e3bf, conf_lb), cut(e3bf, conf_ub))
interval_e3cf = interval(cut(e3cf, conf_lb), cut(e3cf, conf_ub))
interval_e3df = interval(cut(e3df, conf_lb), cut(e3df, conf_ub))
interval_e4f = interval(cut(e4f, conf_lb), cut(e4f, conf_ub))