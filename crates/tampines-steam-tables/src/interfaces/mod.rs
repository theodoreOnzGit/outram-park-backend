/// this set of interfaces allows the user
/// to interact using a more functional programming
/// style (no objects)
///
/// this keeps things simple.
pub mod functional_programming;

/// Bounds-checked, `Result`-returning facade over the panicking flash
/// internals: validates `(T,p)` / `(p,h)` input against the IAPWS-IF97
/// validity envelope BEFORE calling the unchecked functions, returning
/// [`checked::SteamTablesError`] instead of panicking on out-of-range or
/// non-finite input (bead `op-t647`).
pub mod checked;

/// for OOP users who want to make a struct (class)
/// and then use that for extracting data,
/// this is where the stuff is stored
///
/// this is basically a simple control volume
pub mod object_oriented_programming;

/// these tests show you how to use the interfaces
///
/// i may attempt to produce part or whole of the
/// steam tables here
#[cfg(test)]
pub mod tests_and_examples;
