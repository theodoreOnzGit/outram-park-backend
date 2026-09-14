//! WMP data types: the minimal complex number, the evaluated cross-section
//! triple, and the windowed-multipole record itself.
//!
//! Split out of `wmp.rs` (see the module doc there for provenance and status).

/// Minimal complex number for the multipole sum. (The WMP inner loop needs only
/// a handful of operations; a full complex-number dependency is overkill.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cf64 {
    pub re: f64,
    pub im: f64,
}

impl Cf64 {
    /// Construct `re + i·im`.
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }
    /// Complex addition.
    pub fn add(self, o: Cf64) -> Cf64 {
        Cf64::new(self.re + o.re, self.im + o.im)
    }
    /// Complex subtraction `self − o`.
    pub fn sub(self, o: Cf64) -> Cf64 {
        Cf64::new(self.re - o.re, self.im - o.im)
    }
    /// Complex multiplication.
    pub fn mul(self, o: Cf64) -> Cf64 {
        Cf64::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
    /// Complex division `self / o`.
    pub fn div(self, o: Cf64) -> Cf64 {
        let d = o.re * o.re + o.im * o.im;
        Cf64::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
    /// Scale by a real factor.
    pub fn scale(self, s: f64) -> Cf64 {
        Cf64::new(self.re * s, self.im * s)
    }
    /// Complex conjugate `re − i·im`.
    pub fn conj(self) -> Cf64 {
        Cf64::new(self.re, -self.im)
    }
    /// Negation `−self`.
    pub fn neg(self) -> Cf64 {
        Cf64::new(-self.re, -self.im)
    }
    /// Multiply by the imaginary unit: `i·z = −im + i·re`.
    pub fn mul_i(self) -> Cf64 {
        Cf64::new(-self.im, self.re)
    }
}

/// The three base cross sections a windowed-multipole evaluation yields, in barn.
///
/// The multipole residues carry **scattering**, **absorption**, and **fission**
/// directly (this is the library's channel layout). Total is their sensible sum
/// `scatter + absorption`; radiative capture is `absorption − fission`. ν̄ is
/// intentionally absent — WMP carries no secondary data. Combine with
/// [`crate::nuclear_data::secondary::NuBar`] to form the full
/// [`crate::nuclear_data::MicroXs`] the transport kernel consumes.
#[derive(Debug, Clone, Copy, Default)]
pub struct WmpXs {
    /// Elastic scattering σ_s(E, T) \[barn\].
    pub scatter: f64,
    /// Absorption (capture + fission) σ_a(E, T) \[barn\].
    pub absorption: f64,
    /// Fission σ_f(E, T) \[barn\] (0 for non-fissile nuclides).
    pub fission: f64,
}

impl WmpXs {
    /// Total microscopic cross section σ_t = σ_s + σ_a \[barn\].
    pub fn total(&self) -> f64 {
        self.scatter + self.absorption
    }
    /// Radiative capture σ_γ = σ_a − σ_f \[barn\] — the U-238 Doppler target.
    pub fn capture(&self) -> f64 {
        (self.absorption - self.fission).max(0.0)
    }
}

/// Base reaction channels carried by the WMP residues, in library order.
#[derive(Debug, Clone, Copy)]
pub enum WmpReaction {
    Scatter = 0,
    Absorption = 1,
    Fission = 2,
}

/// One energy window in `√E` space: the inclusive range of pole indices that
/// contribute inside it, plus whether its curve-fit background is Doppler-broadened.
#[derive(Debug, Clone, Copy)]
pub struct WmpWindow {
    /// First pole index contributing to this window (inclusive).
    pub start: usize,
    /// Last pole index contributing to this window (inclusive).
    pub end: usize,
    /// Whether the curve-fit polynomial is Doppler-broadened (vs. evaluated raw).
    pub broaden_poly: bool,
}

impl WmpWindow {
    /// A window with no poles (`end < start`) contributes only its curve-fit.
    pub(super) fn is_empty(&self) -> bool {
        self.end < self.start
    }
}

/// Windowed-multipole representation of one nuclide's resonance-range cross
/// sections. Field layout mirrors the MIT `WMP_Library` HDF5.
///
/// The evaluation splits the `√E` axis into equal-spacing **windows**; within a
/// window the cross section is a smooth **curve-fit** polynomial background plus
/// the Faddeeva contribution of the poles assigned to that window. Temperature
/// enters only through the Faddeeva argument, giving analytic Doppler broadening.
#[derive(Debug, Clone, Default)]
pub struct WindowedMultipole {
    /// Nuclide name, e.g. `"U238"`.
    pub name: String,
    /// Atomic weight ratio (target mass / neutron mass) — sets the Doppler width.
    pub awr: f64,
    /// Lower / upper energy bounds of the multipole representation \[eV\].
    pub e_min: f64,
    pub e_max: f64,
    /// Whether this nuclide carries fission residues (the third channel).
    pub fissionable: bool,
    /// Poles (complex, in `√E` space), one per resonance-like term.
    pub poles: Vec<Cf64>,
    /// Residues per pole per base reaction, order `[scatter, absorption, fission]`.
    pub residues: Vec<[Cf64; 3]>,
    /// Curve-fit coefficients `[window][poly_order][channel]`, channel order
    /// `[scatter, absorption, fission]`.
    pub curvefit: Vec<Vec<[f64; 3]>>,
    /// The energy windows (pole ranges + broaden flag), ascending in `√E`.
    pub windows: Vec<WmpWindow>,
    /// Inverse window spacing in `√E`.
    pub inv_spacing: f64,
    /// Curve-fit polynomial order (number of coefficients = `fit_order + 1`).
    pub fit_order: usize,
}
