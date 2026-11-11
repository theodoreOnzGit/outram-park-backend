# Source this to get the OpenFOAM-v2506 (dummy Pstream, no MPI) environment.
export FOAM_INST_DIR=/home/user/upstream-build
set +u
. /home/user/upstream-build/OpenFOAM-v2506/etc/bashrc WM_MPLIB=dummy WM_COMPILE_OPTION=Opt WM_LABEL_SIZE=32 WM_PRECISION_OPTION=DP WM_NCOMPPROCS=3
export WM_NCOMPPROCS=3
export OMP_NUM_THREADS=1
