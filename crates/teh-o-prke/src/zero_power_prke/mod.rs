/// six group PRKE struct
///
/// uses implicit calculation
pub mod six_group_precursor_prke;

/// N-group point kinetics (gh:#1082): the one PRKE core, which
/// `SixGroupPRKE` delegates to.
pub mod n_group_prke;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod six_group_bit_identity_pin;
