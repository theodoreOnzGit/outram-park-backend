// Transcription driver for PRISMS-Plasticity crystal-plasticity kernels
// (gh:#1434, case 1, kernel level).  Upstream: prisms-center/plasticity
// ffdf4eb67b55b84f8b20cbb21407cf310ec3a7e4 (LGPL-2.1-or-later).
//
// Every line marked UPSTREAM is copied verbatim from the pinned tree (checked
// by check_verbatim.sh, whitespace-normalised).  Everything else is shim:
// a minimal FullMatrix / Vector standing in for deal.II (mmult, mTmult and
// trace use plain sequential sums in k; a deal.II built with BLAS may use a
// different summation order, which this driver does NOT reproduce).
//
// Build:  g++ -O2 -ffp-contract=off -o dump_cp_kernels dump_cp_kernels.cpp
// Run:    ./dump_cp_kernels <fcc deck dir> > reference_cp_kernels.txt
// Floats are printed as u64 bit patterns in hex.
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iostream>
#include <sstream>
#include <string>
#include <vector>

static const unsigned int dim = 3;

struct IdentityMatrix { unsigned int n; explicit IdentityMatrix(unsigned int n_) : n(n_) {} };

struct FullMatrix {
  unsigned int r = 0, c = 0;
  std::vector<double> v;
  FullMatrix() {}
  FullMatrix(unsigned int r_, unsigned int c_) { reinit(r_, c_); }
  void reinit(unsigned int r_, unsigned int c_) { r = r_; c = c_; v.assign(r * c, 0.0); }
  double* operator[](unsigned int i) { return &v[i * c]; }
  const double* operator[](unsigned int i) const { return &v[i * c]; }
  double& operator()(unsigned int i, unsigned int j) { return v[i * c + j]; }
  double operator()(unsigned int i, unsigned int j) const { return v[i * c + j]; }
  FullMatrix& operator=(const IdentityMatrix& I) {
    reinit(I.n, I.n);
    for (unsigned int i = 0; i < I.n; i++) v[i * c + i] = 1.0;
    return *this;
  }
  FullMatrix& operator=(double x) { for (auto& e : v) e = x; return *this; }
  // C = this * B
  void mmult(FullMatrix& C, const FullMatrix& B) const {
    FullMatrix out(r, B.c);
    for (unsigned int i = 0; i < r; i++)
      for (unsigned int j = 0; j < B.c; j++) {
        double sum = 0.0;
        for (unsigned int k = 0; k < c; k++) sum += (*this)(i, k) * B(k, j);
        out(i, j) = sum;
      }
    C = out;
  }
  // C = this * B^T
  void mTmult(FullMatrix& C, const FullMatrix& B) const {
    FullMatrix out(r, B.r);
    for (unsigned int i = 0; i < r; i++)
      for (unsigned int j = 0; j < B.r; j++) {
        double sum = 0.0;
        for (unsigned int k = 0; k < c; k++) sum += (*this)(i, k) * B(j, k);
        out(i, j) = sum;
      }
    C = out;
  }
  double trace() const {
    double s = 0.0;
    for (unsigned int i = 0; i < r; i++) s += (*this)(i, i);
    return s;
  }
};

struct Vector {
  std::vector<double> v;
  Vector() {}
  explicit Vector(unsigned int n) : v(n, 0.0) {}
  double& operator()(unsigned int i) { return v[i]; }
  double operator()(unsigned int i) const { return v[i]; }
  double& operator[](unsigned int i) { return v[i]; }
  double operator[](unsigned int i) const { return v[i]; }
  Vector& operator=(double x) { for (auto& e : v) e = x; return *this; }
};

static std::string hex(double x) {
  uint64_t b;
  std::memcpy(&b, &x, 8);
  char buf[32];
  std::snprintf(buf, sizeof buf, "%016llx", (unsigned long long)b);
  return buf;
}

static uint64_t lcg_state = 0x9E3779B97F4A7C15ULL;
static double u01() {
  lcg_state = lcg_state * 6364136223846793005ULL + 1442695040888963407ULL;
  return (double)(lcg_state >> 11) / 9007199254740992.0;
}

// ---- K2: odfpoint (rotationOperations.cc) -------------------------------
void odfpoint(FullMatrix& OrientationMatrix, Vector r) {
  double rdotr = 0.0;
  for(unsigned int i=0;i<dim;i++){ // UPSTREAM
    rdotr = rdotr + r(i)*r(i); // UPSTREAM
  } // UPSTREAM
  double term1 = 1.0 - (rdotr); // UPSTREAM
  double term2 = 1.0 + (rdotr); // UPSTREAM
  OrientationMatrix.reinit(dim,dim);OrientationMatrix = IdentityMatrix(dim); // UPSTREAM
  for(unsigned int i=0;i<dim;i++){ // UPSTREAM
    OrientationMatrix[i][i]=OrientationMatrix[i][i]*term1; // UPSTREAM
  } // UPSTREAM
  for(unsigned int i=0;i<dim;i++){ // UPSTREAM
    for(unsigned int j=0;j<dim;j++){ // UPSTREAM
      OrientationMatrix[i][j] = OrientationMatrix[i][j] + 2.0*(r(i)*r(j)); // UPSTREAM
    } // UPSTREAM
  } // UPSTREAM
  OrientationMatrix[0][1] = OrientationMatrix[0][1]-2.0*r(2); // UPSTREAM
  OrientationMatrix[0][2] =  OrientationMatrix[0][2]+2.0*r(1); // UPSTREAM
  OrientationMatrix[1][2] = OrientationMatrix[1][2]-2.0*r(0); // UPSTREAM
  OrientationMatrix[1][0] =  OrientationMatrix[1][0]+2.0*r(2); // UPSTREAM
  OrientationMatrix[2][0] = OrientationMatrix[2][0]-2.0*r(1); // UPSTREAM
  OrientationMatrix[2][1] =  OrientationMatrix[2][1]+2.0*r(0); // UPSTREAM
  for(unsigned int i=0;i<dim;i++){ // UPSTREAM
    for(unsigned int j=0;j<dim;j++){ // UPSTREAM
      OrientationMatrix[i][j] = OrientationMatrix[i][j]*1.0/term2; // UPSTREAM
    } // UPSTREAM
  } // UPSTREAM
}

// ---- K3: elasticmoduli (matrixOperations.cc) ----------------------------
void elasticmoduli(FullMatrix& Ar, FullMatrix R, FullMatrix Av) {
  Ar.reinit(2 * dim, 2 * dim); // UPSTREAM
  FullMatrix newr(2 * dim, 2 * dim), temp(2 * dim, 2 * dim); // shim: upstream declares FullMatrix<double>
  newr[0][0] = R[0][0]*R[0][0]; // UPSTREAM
  newr[0][1] = R[0][1]*R[0][1]; // UPSTREAM
  newr[0][2] = R[0][2]*R[0][2]; // UPSTREAM
  newr[0][3] = 2.0*R[0][1]*R[0][2]; // UPSTREAM
  newr[0][4] = 2.0*R[0][2]*R[0][0]; // UPSTREAM
  newr[0][5] = 2.0*R[0][0]*R[0][1]; // UPSTREAM
  newr[1][0] = R[1][0]*R[1][0]; // UPSTREAM
  newr[1][1] = R[1][1]*R[1][1]; // UPSTREAM
  newr[1][2] = R[1][2]*R[1][2]; // UPSTREAM
  newr[1][3] = 2.0*R[1][1]*R[1][2]; // UPSTREAM
  newr[1][4] = 2.0*R[1][2]*R[1][0]; // UPSTREAM
  newr[1][5] = 2.0*R[1][0]*R[1][1]; // UPSTREAM
  newr[2][0] = R[2][0]*R[2][0]; // UPSTREAM
  newr[2][1] = R[2][1]*R[2][1]; // UPSTREAM
  newr[2][2] = R[2][2]*R[2][2]; // UPSTREAM
  newr[2][3] = 2.0*R[2][1]*R[2][2]; // UPSTREAM
  newr[2][4] = 2.0*R[2][2]*R[2][0]; // UPSTREAM
  newr[2][5] = 2.0*R[2][0]*R[2][1]; // UPSTREAM
  newr[3][0] = R[1][0]*R[2][0]; // UPSTREAM
  newr[3][1] = R[1][1]*R[2][1]; // UPSTREAM
  newr[3][2] = R[1][2]*R[2][2]; // UPSTREAM
  newr[3][3] = R[1][1]*R[2][2] + R[1][2]*R[2][1]; // UPSTREAM
  newr[3][4] = R[1][2]*R[2][0] + R[1][0]*R[2][2]; // UPSTREAM
  newr[3][5] = R[1][0]*R[2][1] + R[1][1]*R[2][0]; // UPSTREAM
  newr[4][0] = R[2][0]*R[0][0]; // UPSTREAM
  newr[4][1] = R[2][1]*R[0][1]; // UPSTREAM
  newr[4][2] = R[2][2]*R[0][2]; // UPSTREAM
  newr[4][3] = R[0][1]*R[2][2] + R[0][2]*R[2][1]; // UPSTREAM
  newr[4][4] = R[0][2]*R[2][0] + R[0][0]*R[2][2]; // UPSTREAM
  newr[4][5] = R[0][0]*R[2][1] + R[0][1]*R[2][0]; // UPSTREAM
  newr[5][0] = R[0][0]*R[1][0]; // UPSTREAM
  newr[5][1] = R[0][1]*R[1][1]; // UPSTREAM
  newr[5][2] = R[0][2]*R[1][2]; // UPSTREAM
  newr[5][3] = R[0][1]*R[1][2] + R[0][2]*R[1][1]; // UPSTREAM
  newr[5][4] = R[0][2]*R[1][0] + R[0][0]*R[1][2]; // UPSTREAM
  newr[5][5] = R[0][0]*R[1][1] + R[0][1]*R[1][0]; // UPSTREAM
  newr.mmult(temp, Av); // UPSTREAM
  temp.mTmult(Ar, newr); // UPSTREAM
}

// ---- K1: slip normalisation (init.cc) ------------------------------------
// The two blocks below are the per-row bodies of the upstream read loops.
void normalise_n(FullMatrix& n_alpha_SinglePhase, unsigned int id) {
  double n_norm;
  n_norm = 0 ; // UPSTREAM
  n_norm = n_norm + n_alpha_SinglePhase[id][0]*n_alpha_SinglePhase[id][0] ; // UPSTREAM
  n_norm = n_norm + n_alpha_SinglePhase[id][1]*n_alpha_SinglePhase[id][1] ; // UPSTREAM
  n_norm = n_norm + n_alpha_SinglePhase[id][2]*n_alpha_SinglePhase[id][2] ; // UPSTREAM
  n_norm = sqrt(n_norm) ; // UPSTREAM
  n_alpha_SinglePhase[id][0] = n_alpha_SinglePhase[id][0]/n_norm ; // UPSTREAM
  n_alpha_SinglePhase[id][1] = n_alpha_SinglePhase[id][1]/n_norm ; // UPSTREAM
  n_alpha_SinglePhase[id][2] = n_alpha_SinglePhase[id][2]/n_norm ; // UPSTREAM
}
void normalise_m(FullMatrix& m_alpha_SinglePhase, unsigned int id) {
  double m_norm;
  m_norm = 0 ; // UPSTREAM
  m_norm = m_norm + m_alpha_SinglePhase[id][0]*m_alpha_SinglePhase[id][0] ; // UPSTREAM
  m_norm = m_norm + m_alpha_SinglePhase[id][1]*m_alpha_SinglePhase[id][1] ; // UPSTREAM
  m_norm = m_norm + m_alpha_SinglePhase[id][2]*m_alpha_SinglePhase[id][2] ; // UPSTREAM
  m_norm = sqrt(m_norm) ; // UPSTREAM
  m_alpha_SinglePhase[id][0] = m_alpha_SinglePhase[id][0]/m_norm ; // UPSTREAM
  m_alpha_SinglePhase[id][1] = m_alpha_SinglePhase[id][1]/m_norm ; // UPSTREAM
  m_alpha_SinglePhase[id][2] = m_alpha_SinglePhase[id][2]/m_norm ; // UPSTREAM
}

static const double MILLER[12][6] = {
    {1, 1, 1, 0, 1, -1},   {1, 1, 1, -1, 0, 1},   {1, 1, 1, 1, -1, 0},
    {-1, -1, 1, 0, -1, -1}, {-1, -1, 1, 1, 0, 1},  {-1, -1, 1, -1, 1, 0},
    {-1, 1, 1, 0, 1, -1},  {-1, 1, 1, 1, 0, 1},   {-1, 1, 1, -1, -1, 0},
    {1, -1, 1, 0, -1, -1}, {1, -1, 1, -1, 0, 1},  {1, -1, 1, 1, 1, 0}};

int main(int argc, char** argv) {
  std::string deck = argc > 1 ? argv[1] : ".";
  const unsigned int n_slip_systems_SinglePhase = 12;
  const unsigned int n_Tslip_systems = 12;
  const unsigned int n_slip_systems = 12;

  // ---- K1a: read the deck exactly as init.cc does --------------------------
  FullMatrix n_deck(12, 3), m_deck(12, 3), n_raw(12, 3), m_raw(12, 3);
  {
    FullMatrix n_alpha_SinglePhase(12, 3), m_alpha_SinglePhase(12, 3);
    std::string line;
    unsigned int id = 0;
    std::ifstream slipNormalsDataFile(deck + "/slipNormals.txt");
    while (getline (slipNormalsDataFile,line) && id<n_slip_systems_SinglePhase){ // UPSTREAM
      std::stringstream ss(line); // UPSTREAM
      ss >> n_alpha_SinglePhase[id][0]; // UPSTREAM
      ss >> n_alpha_SinglePhase[id][1]; // UPSTREAM
      ss >> n_alpha_SinglePhase[id][2]; // UPSTREAM
      for (int k = 0; k < 3; k++) n_raw[id][k] = n_alpha_SinglePhase[id][k];
      normalise_n(n_alpha_SinglePhase, id);
      id=id+1; // UPSTREAM
    }
    id = 0;
    std::ifstream slipDirectionsDataFile(deck + "/slipDirections.txt");
    while (getline (slipDirectionsDataFile,line)&& id<n_slip_systems_SinglePhase){ // UPSTREAM
      std::stringstream ss(line); // UPSTREAM
      ss >> m_alpha_SinglePhase[id][0]; // UPSTREAM
      ss >> m_alpha_SinglePhase[id][1]; // UPSTREAM
      ss >> m_alpha_SinglePhase[id][2]; // UPSTREAM
      for (int k = 0; k < 3; k++) m_raw[id][k] = m_alpha_SinglePhase[id][k];
      normalise_m(m_alpha_SinglePhase, id);
      id=id+1; // UPSTREAM
    }
    n_deck = n_alpha_SinglePhase;
    m_deck = m_alpha_SinglePhase;
    // q matrix as the deck file holds it, read as init.cc does
    FullMatrix q_phase1(12, 12);
    unsigned int n_Tslip_systems_Real_SinglePhase = 12;
    id = 0;
    std::ifstream latentHardeningratioFile(deck + "/LatentHardeningRatio.txt");
    while (getline (latentHardeningratioFile,line) && id<n_Tslip_systems_Real_SinglePhase){ // UPSTREAM
      std::stringstream ss(line); // UPSTREAM
      for (unsigned int i=0; i<n_Tslip_systems_Real_SinglePhase; i++){ // UPSTREAM
        ss >> q_phase1[id][i]; // UPSTREAM
      } // UPSTREAM
      id=id+1; // UPSTREAM
    } // UPSTREAM
    for (unsigned int a = 0; a < 12; a++)
      for (unsigned int b = 0; b < 12; b++) std::printf("Q %u %u %s\n", a, b, hex(q_phase1[a][b]).c_str());
    for (unsigned int s = 0; s < 12; s++)
      std::printf("K1 %u %s %s %s %s %s %s %s %s %s %s %s %s\n", s, hex(n_raw[s][0]).c_str(),
                  hex(n_raw[s][1]).c_str(), hex(n_raw[s][2]).c_str(), hex(m_raw[s][0]).c_str(),
                  hex(m_raw[s][1]).c_str(), hex(m_raw[s][2]).c_str(), hex(n_deck[s][0]).c_str(),
                  hex(n_deck[s][1]).c_str(), hex(n_deck[s][2]).c_str(), hex(m_deck[s][0]).c_str(),
                  hex(m_deck[s][1]).c_str(), hex(m_deck[s][2]).c_str());
  }

  // Slip tables from integer Miller indices through the SAME upstream
  // normalisation lines (used by K4 and K7 so the table is not a variable).
  FullMatrix n_alpha(12, 3), m_alpha(12, 3);
  for (unsigned int s = 0; s < 12; s++) {
    for (int k = 0; k < 3; k++) { n_alpha[s][k] = MILLER[s][k]; m_alpha[s][k] = MILLER[s][3 + k]; }
    normalise_n(n_alpha, s);
    normalise_m(m_alpha, s);
    std::printf("K1I %u %s %s %s %s %s %s\n", s, hex(n_alpha[s][0]).c_str(), hex(n_alpha[s][1]).c_str(),
                hex(n_alpha[s][2]).c_str(), hex(m_alpha[s][0]).c_str(), hex(m_alpha[s][1]).c_str(),
                hex(m_alpha[s][2]).c_str());
  }

  // ---- K2, K3, K4 over 200 orientations ------------------------------------
  FullMatrix elasticStiffnessMatrix(6, 6);
  for (int i = 0; i < 3; i++)
    for (int j = 0; j < 3; j++) elasticStiffnessMatrix[i][j] = (i == j) ? 170.0e3 : 124.0e3;
  for (int i = 3; i < 6; i++) elasticStiffnessMatrix[i][i] = 75.0e3;

  for (int o = 0; o < 200; o++) {
    double scale = (o == 0) ? 0.0 : (o < 150 ? 1.0 : 50.0);
    Vector rot1(dim);
    for (unsigned int k = 0; k < 3; k++) rot1(k) = (u01() - 0.5) * 2.0 * scale;
    FullMatrix rotmat(dim, dim);
    rotmat = 0.0;
    odfpoint(rotmat, rot1);
    std::printf("R %d", o);
    for (unsigned int k = 0; k < 3; k++) std::printf(" %s", hex(rot1(k)).c_str());
    for (unsigned int i = 0; i < 3; i++)
      for (unsigned int j = 0; j < 3; j++) std::printf(" %s", hex(rotmat[i][j]).c_str());
    std::printf("\n");

    FullMatrix Dmat2(2 * dim, 2 * dim);
    elasticmoduli(Dmat2, rotmat, elasticStiffnessMatrix); // UPSTREAM
    std::printf("C %d", o);
    for (unsigned int i = 0; i < 6; i++)
      for (unsigned int j = 0; j < 6; j++) std::printf(" %s", hex(Dmat2[i][j]).c_str());
    std::printf("\n");

    // K4: symmetric stress, 12 resolved shear stresses as the rate-dependent
    // model forms them (Schmid tensor rotated with R S R^T, tau = trace(T S^T)).
    double sv[6];
    for (int k = 0; k < 6; k++) sv[k] = (u01() - 0.5) * 600.0;
    FullMatrix T_star_iter(dim, dim);
    T_star_iter[0][0] = sv[0]; T_star_iter[1][1] = sv[1]; T_star_iter[2][2] = sv[2];
    T_star_iter[1][2] = T_star_iter[2][1] = sv[3];
    T_star_iter[0][2] = T_star_iter[2][0] = sv[4];
    T_star_iter[0][1] = T_star_iter[1][0] = sv[5];
    FullMatrix SCHMID_TENSOR1(n_Tslip_systems * dim, dim);
    FullMatrix temp(dim, dim), temp1(dim, dim), temp2(dim, dim), temp3(dim, dim), temp4(dim, dim);
    Vector m1(dim), n1(dim);
    std::printf("T %d", o);
    for (int k = 0; k < 6; k++) std::printf(" %s", hex(sv[k]).c_str());
    for (unsigned int i = 0;i<n_Tslip_systems;i++) { // UPSTREAM
      for (unsigned int j = 0;j<dim;j++) { // UPSTREAM
        m1(j) = m_alpha[i][j]; // (upstream: m_alpha[i][j]; table is a FullMatrix here)
        n1(j) = n_alpha[i][j];
      } // UPSTREAM
      temp = 0.0; // UPSTREAM
      temp2 = 0.0; // UPSTREAM
      for (unsigned int j = 0;j<dim;j++) { // UPSTREAM
        for (unsigned int k = 0;k<dim;k++) { // UPSTREAM
          temp[j][k] = m1(j)*n1(k);         // Construct Schmid tensor matrix relative to the crystal reference frame // UPSTREAM
        } // UPSTREAM
      } // UPSTREAM
      rotmat.mmult(temp2, temp); // UPSTREAM
      temp2.mTmult(temp, rotmat); // UPSTREAM
      for (unsigned int j = 0;j<dim;j++) { // UPSTREAM
        for (unsigned int k = 0;k<dim;k++) { // UPSTREAM
          SCHMID_TENSOR1[dim*i + j][k] = temp[j][k]; // Store rotated Schmid tensor matrix // UPSTREAM
        } // UPSTREAM
      } // UPSTREAM
      for (unsigned int j = 0;j<dim;j++) { // UPSTREAM
        for (unsigned int k = 0;k<dim;k++) { // UPSTREAM
          temp[j][k]=SCHMID_TENSOR1[dim*i + j][k]; // UPSTREAM
        } // UPSTREAM
      } // UPSTREAM
      T_star_iter.mTmult(temp2,temp); // UPSTREAM
      double sctmp1=temp2.trace(); // (upstream: sctmp1=temp2.trace();)
      std::printf(" %s", hex(sctmp1).c_str());
    } // UPSTREAM
    std::printf("\n");
  }

  // ---- K5: power-law slip increment ---------------------------------------
  {
    Vector resolved_shear_tau(1), W_kh_tau(1), s_alpha_it(1), delgam_tau(1);
    unsigned int i = 0;
    double sgnm;
    const double ms[] = {0.02, 0.04, 0.1, 0.2, 0.5};
    const double ss_[] = {16.0, 40.0, 100.0};
    for (double strexp : ms) {
      for (double s : ss_) {
        double delgam_ref = 1.0e-3 * 0.1; // UserMatConstants(0)*this->userInputs.delT
        for (int k = -1; k < 61; k++) {
          double ratio = (k < 0) ? 0.0 : std::pow(10.0, -3.0 + 3.5 * k / 60.0);
          for (int sign = 0; sign < 2; sign++) {
            resolved_shear_tau(i) = (sign ? -1.0 : 1.0) * ratio * s;
            W_kh_tau(i) = 0.0;
            s_alpha_it(i) = s;
            if((resolved_shear_tau(i) - W_kh_tau(i))<0) // UPSTREAM
            sgnm=-1; // UPSTREAM
            else // UPSTREAM
            sgnm=1 ; // UPSTREAM
            delgam_tau(i)=delgam_ref*pow(fabs((resolved_shear_tau(i)-W_kh_tau(i))/s_alpha_it(i)),(1.0/strexp))*sgnm; // UPSTREAM
            std::printf("G %s %s %s %s\n", hex(strexp).c_str(), hex(resolved_shear_tau(i)).c_str(),
                        hex(s).c_str(), hex(delgam_tau(i)).c_str());
          }
        }
      }
    }
  }

  // ---- K6/K7: hardening ----------------------------------------------------
  {
    std::vector<double> initialHardeningModulus(12, 180.0), saturationStress(12, 148.0), powerLawExponent(12, 2.25);
    FullMatrix q(12, 12);
    for (unsigned int a = 0; a < 12; a++)
      for (unsigned int b = 0; b < 12; b++) {
        double dotnn = n_alpha[a][0] * n_alpha[b][0] + n_alpha[a][1] * n_alpha[b][1] + n_alpha[a][2] * n_alpha[b][2];
        q[a][b] = (std::fabs(dotnn) > 1.0 - 1e-9) ? 1.0 : 1.4;
      }
    for (int c = 0; c < 400; c++) {
      Vector s_alpha_it(12), s_alpha_t(12), s_alpha_iterp(12), h_beta(12), delgam_tau(12);
      FullMatrix h_alpha_beta_t(12, 12);
      // 300 cases well below saturation, 100 close to or at saturation.
      for (unsigned int i = 0; i < 12; i++) {
        s_alpha_t(i) = (c < 300) ? 16.0 + (u01() * 120.0) : 140.0 + u01() * 8.0;
        s_alpha_it(i) = s_alpha_t(i);
        delgam_tau(i) = (u01() - 0.5) * 2.0e-2;
      }
      for(unsigned int i=0;i<n_slip_systems;i++){ // UPSTREAM
        h_beta(i)=initialHardeningModulus[i]*pow((1-s_alpha_it(i)/saturationStress[i]),powerLawExponent[i]); // UPSTREAM
      } // UPSTREAM
      for(unsigned int i=0;i<n_Tslip_systems;i++){ // UPSTREAM
        for(unsigned int j=0;j<n_Tslip_systems;j++){ // UPSTREAM
          h_alpha_beta_t[i][j] = q[i][j]*h_beta(j); // UPSTREAM
        } // UPSTREAM
      } // UPSTREAM
      s_alpha_iterp = s_alpha_t;
      for (unsigned int i = 0;i<n_Tslip_systems;i++){ // UPSTREAM
        for (unsigned int j = 0;j<n_Tslip_systems;j++){ // UPSTREAM
          s_alpha_iterp(j)=s_alpha_iterp(j)+h_alpha_beta_t[j][i]*fabs(delgam_tau(i)); // UPSTREAM
        } // UPSTREAM
      } // UPSTREAM
      for(unsigned int i=0;i<n_slip_systems;i++){ // UPSTREAM
        if(s_alpha_iterp[i] >= saturationStress[i]) // UPSTREAM
        s_alpha_iterp[i] =  saturationStress[i] ; // UPSTREAM
      } // UPSTREAM
      std::printf("W %d", c);
      for (unsigned int i = 0; i < 12; i++) std::printf(" %s", hex(s_alpha_t(i)).c_str());
      for (unsigned int i = 0; i < 12; i++) std::printf(" %s", hex(delgam_tau(i)).c_str());
      for (unsigned int i = 0; i < 12; i++) std::printf(" %s", hex(h_beta(i)).c_str());
      for (unsigned int i = 0; i < 12; i++) std::printf(" %s", hex(s_alpha_iterp(i)).c_str());
      std::printf("\n");
    }
  }
  return 0;
}
