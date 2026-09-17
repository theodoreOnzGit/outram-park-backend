//! Periodic-table element symbols, indexed by atomic number Z.
//!
//! A small, static lookup table (`Z` 1-118) used only to translate between a
//! nuclide's element symbol (e.g. `"U"`) and its atomic number (e.g. `92`) for
//! the fuzzy nuclide finder ([`crate::nuclides`]). This is display/search
//! plumbing only — it carries no cross-section data.

/// Element symbols indexed by atomic number `Z` (`SYMBOLS[0]` is `Z=1`, `"H"`).
pub const SYMBOLS: [&str; 118] = [
    "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S", "Cl",
    "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge", "As",
    "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd", "In",
    "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd", "Pm", "Sm", "Eu", "Gd", "Tb",
    "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir", "Pt", "Au", "Hg", "Tl",
    "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu", "Am", "Cm", "Bk",
    "Cf", "Es", "Fm", "Md", "No", "Lr", "Rf", "Db", "Sg", "Bh", "Hs", "Mt", "Ds", "Rg", "Cn", "Nh",
    "Fl", "Mc", "Lv", "Ts", "Og",
];

/// Atomic number `Z` for an element symbol, case-insensitively (e.g. `"u"` or
/// `"U"` -> `92`). Returns `None` for a string that is not a recognised
/// 1-118 element symbol.
pub fn z_for_symbol(symbol: &str) -> Option<u32> {
    SYMBOLS
        .iter()
        .position(|s| s.eq_ignore_ascii_case(symbol))
        .map(|i| (i + 1) as u32)
}

/// Element symbol for atomic number `z` (1-118), e.g. `92` -> `"U"`. Returns
/// `None` outside that range.
pub fn symbol_for_z(z: u32) -> Option<&'static str> {
    if z == 0 {
        return None;
    }
    SYMBOLS.get((z - 1) as usize).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_uranium() {
        assert_eq!(z_for_symbol("U"), Some(92));
        assert_eq!(z_for_symbol("u"), Some(92));
        assert_eq!(symbol_for_z(92), Some("U"));
    }

    #[test]
    fn rejects_unknown_symbol() {
        assert_eq!(z_for_symbol("Xx"), None);
        assert_eq!(symbol_for_z(0), None);
        assert_eq!(symbol_for_z(119), None);
    }
}
