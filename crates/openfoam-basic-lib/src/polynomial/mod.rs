pub mod roots;
pub mod linear_eqn;
pub mod quadratic_eqn;
pub mod cubic_eqn;
pub mod polynomial;

pub use roots::{RootType, Roots};
pub use linear_eqn::LinearEqn;
pub use quadratic_eqn::QuadraticEqn;
pub use cubic_eqn::CubicEqn;
pub use polynomial::Polynomial;
