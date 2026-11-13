/// six group PRKE struct
///
/// uses implicit calculation
pub mod six_group_precursor_prke;

/// N-group point kinetics (gh:#1082): the one PRKE core, which
/// `SixGroupPRKE` delegates to.
pub mod n_group_prke;

/// Circulating-fuel point kinetics (MSRE T-C1, gh:#1131): precursors drift
/// out of the core with the fuel and return after a loop transit.
pub mod circulating;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod six_group_bit_identity_pin;
