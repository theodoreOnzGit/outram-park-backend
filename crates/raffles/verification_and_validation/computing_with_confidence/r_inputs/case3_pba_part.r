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
#######################################################################
samp = 200;

# Define the component C-boxes:
C1 = KN(23,24);
C2 = KN(23,24);
C3 = KN(14,17);
C4 = KN(14,17);
C5 = KN(12,12);

# Plot the C-boxes of the components:
rbyc(2,3)
sh(C1,,'Component 1',c(0,1))
sh(C2,,'Component 2',c(0,1))
sh(C3,,'Component 3',c(0,1))
sh(C4,,'Component 4',c(0,1))
sh(C5,,'Component 5',c(0,1))

# Define the Event C-boxes under Independence via Boolean logic (Path-set):
phi1 = andI(C1,C3);
phi2 = andI(C2,C4);
phi3 = andI(C1,andI(C4,C5));
phi4 = andI(C2,andI(C3,C5));
R_S1_ps_Boolean = orI(orI(phi1,phi2),orI(phi3,phi4));


# ADDED by the V&V driver (not in upstream): 95% interval of the pba Boolean-function c-box
conf_lb = 0.025; conf_ub = 1 - conf_lb;
interval_R_S1_indep = interval(cut(R_S1_ps_Boolean, conf_lb), cut(R_S1_ps_Boolean, conf_ub))
