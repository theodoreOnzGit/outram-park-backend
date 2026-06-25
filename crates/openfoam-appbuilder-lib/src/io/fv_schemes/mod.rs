use std::path::Path;
use crate::error::AppBuilderError;

/// Parsed `system/fvSchemes` — numerical scheme selection for each operator.
#[derive(Debug, Clone)]
pub struct FvSchemes {
    pub ddt:         DdtScheme,
    pub default_grad: GradScheme,
    pub default_div:  DivScheme,
    pub default_laplacian: LaplacianScheme,
    pub default_sn_grad:   SnGradScheme,
    pub default_interpolation: InterpolationScheme,
}

/// Time-stepping scheme (ddtSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum DdtScheme {
    Euler,
    Backward,
    CrankNicolson(f64),  // off-centring coefficient ψ ∈ [0,1]
    LocalEuler,
    SteadyState,
}

/// Gradient scheme (gradSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum GradScheme {
    GaussLinear,
    LeastSquares,
    FourthOrder,
}

/// Divergence / convection scheme (divSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum DivScheme {
    GaussLinear,
    GaussUpwind,
    GaussLinearUpwind(String),  // e.g. "Gauss linearUpwind grad(U)"
    GaussVanLeer,
    GaussMUSCL,
    GaussLimitedLinear(f64),
}

/// Laplacian scheme (laplacianSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum LaplacianScheme {
    GaussLinearCorrected,
    GaussLinearUncorrected,
    GaussLinearLimited(f64),  // limiter coefficient ∈ [0,1]
}

/// Surface-normal gradient scheme (snGradSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum SnGradScheme {
    Corrected,
    Uncorrected,
    Limited(f64),
}

/// Face interpolation scheme (interpolationSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum InterpolationScheme {
    Linear,
    Upwind(String),  // e.g. "upwind phi"
    Harmonic,
}

impl FvSchemes {
    pub fn read(path: &Path) -> Result<Self, AppBuilderError> {
        let _ = path;
        todo!("FvSchemes::read — parse system/fvSchemes")
    }
}

impl Default for FvSchemes {
    fn default() -> Self {
        Self {
            ddt: DdtScheme::Euler,
            default_grad: GradScheme::GaussLinear,
            default_div:  DivScheme::GaussLinear,
            default_laplacian: LaplacianScheme::GaussLinearCorrected,
            default_sn_grad:   SnGradScheme::Corrected,
            default_interpolation: InterpolationScheme::Linear,
        }
    }
}
