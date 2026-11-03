//! The Gruvbox palette (MIT, morhetz/gruvbox: only the published hex values
//! are reproduced) as `no_std` [`Rgb`] constants, and its colours assigned to
//! the semantic roles a GUI asks for ([`Roles`]).

use crate::Rgb;

pub const DARK0_HARD: Rgb = Rgb::hex(0x1d2021);
pub const DARK0: Rgb = Rgb::hex(0x282828);
pub const DARK0_SOFT: Rgb = Rgb::hex(0x32302f);
pub const DARK1: Rgb = Rgb::hex(0x3c3836);
pub const DARK2: Rgb = Rgb::hex(0x504945);
pub const DARK3: Rgb = Rgb::hex(0x665c54);
pub const DARK4: Rgb = Rgb::hex(0x7c6f64);
pub const GRAY: Rgb = Rgb::hex(0x928374);
pub const LIGHT0_HARD: Rgb = Rgb::hex(0xf9f5d7);
pub const LIGHT0: Rgb = Rgb::hex(0xfbf1c7);
pub const LIGHT0_SOFT: Rgb = Rgb::hex(0xf2e5bc);
pub const LIGHT1: Rgb = Rgb::hex(0xebdbb2);
pub const LIGHT2: Rgb = Rgb::hex(0xd5c4a1);
pub const LIGHT3: Rgb = Rgb::hex(0xbdae93);
pub const LIGHT4: Rgb = Rgb::hex(0xa89984);

pub const BRIGHT_RED: Rgb = Rgb::hex(0xfb4934);
pub const BRIGHT_GREEN: Rgb = Rgb::hex(0xb8bb26);
pub const BRIGHT_YELLOW: Rgb = Rgb::hex(0xfabd2f);
pub const BRIGHT_BLUE: Rgb = Rgb::hex(0x83a598);
pub const BRIGHT_PURPLE: Rgb = Rgb::hex(0xd3869b);
pub const BRIGHT_AQUA: Rgb = Rgb::hex(0x8ec07c);
pub const BRIGHT_ORANGE: Rgb = Rgb::hex(0xfe8019);

pub const NEUTRAL_RED: Rgb = Rgb::hex(0xcc241d);
pub const NEUTRAL_GREEN: Rgb = Rgb::hex(0x98971a);
pub const NEUTRAL_YELLOW: Rgb = Rgb::hex(0xd79921);
pub const NEUTRAL_BLUE: Rgb = Rgb::hex(0x458588);
pub const NEUTRAL_PURPLE: Rgb = Rgb::hex(0xb16286);
pub const NEUTRAL_AQUA: Rgb = Rgb::hex(0x689d6a);
pub const NEUTRAL_ORANGE: Rgb = Rgb::hex(0xd65d0e);

pub const FADED_RED: Rgb = Rgb::hex(0x9d0006);
pub const FADED_GREEN: Rgb = Rgb::hex(0x79740e);
pub const FADED_YELLOW: Rgb = Rgb::hex(0xb57614);
pub const FADED_BLUE: Rgb = Rgb::hex(0x076678);
pub const FADED_PURPLE: Rgb = Rgb::hex(0x8f3f71);
pub const FADED_AQUA: Rgb = Rgb::hex(0x427b58);
pub const FADED_ORANGE: Rgb = Rgb::hex(0xaf3a03);

/// A theme's colours by role, so a GUI asks for "the accent" or "a string in
/// code" rather than a hex value.
///
/// **The role layout is Aura UI's** (`aura-gui` 0.3.0, `src/view/themes.rs`
/// `Palette`: 7 base, 8 state, 12 syntax and 16 ANSI colours; AGPL-3.0-only,
/// Camilo Grajales Holguín), and [`Roles::GRUVBOX_DARK`]'s assignments are
/// its `gruvbox_dark()` (`src/view/themes/families.rs`). Aura has no Gruvbox
/// light; [`Roles::GRUVBOX_LIGHT`] mirrors the dark assignments onto
/// Gruvbox's light tones (the faded accents, which read on a light ground),
/// new work here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roles {
    pub is_dark: bool,
    pub canvas: Rgb,
    pub surface: Rgb,
    pub elevated: Rgb,
    pub border: Rgb,
    pub border_subtle: Rgb,
    pub text: Rgb,
    pub text_muted: Rgb,
    pub accent: Rgb,
    pub accent_hover: Rgb,
    pub danger: Rgb,
    pub danger_hover: Rgb,
    pub warning: Rgb,
    pub success: Rgb,
    pub hover: Rgb,
    pub active: Rgb,
    /// keyword, function, type, string, comment, constant, attribute,
    /// operator, property, tag, title, link.
    pub syntax: [Rgb; 12],
    /// The 16 terminal colours, black..white then bright black..bright white.
    pub ansi: [Rgb; 16],
}

impl Roles {
    /// Gruvbox Dark, as Aura UI assigns it.
    pub const GRUVBOX_DARK: Roles = Roles {
        is_dark: true,
        canvas: DARK0,
        surface: DARK0_HARD,
        elevated: DARK0_SOFT,
        border: DARK2,
        border_subtle: DARK1,
        text: LIGHT1,
        text_muted: LIGHT4,
        accent: BRIGHT_BLUE,
        accent_hover: Rgb::hex(0x9cb8ad),
        danger: BRIGHT_RED,
        danger_hover: Rgb::hex(0xfc6b5b),
        warning: BRIGHT_YELLOW,
        success: BRIGHT_GREEN,
        hover: DARK1,
        active: DARK2,
        syntax: [
            BRIGHT_RED, BRIGHT_GREEN, BRIGHT_YELLOW, BRIGHT_GREEN, GRAY, BRIGHT_PURPLE, BRIGHT_AQUA, BRIGHT_ORANGE,
            BRIGHT_BLUE, BRIGHT_RED, BRIGHT_YELLOW, BRIGHT_BLUE,
        ],
        ansi: [
            DARK0, NEUTRAL_RED, NEUTRAL_GREEN, NEUTRAL_YELLOW, NEUTRAL_BLUE, NEUTRAL_PURPLE, NEUTRAL_AQUA, LIGHT4,
            GRAY, BRIGHT_RED, BRIGHT_GREEN, BRIGHT_YELLOW, BRIGHT_BLUE, BRIGHT_PURPLE, BRIGHT_AQUA, LIGHT1,
        ],
    };

    /// Gruvbox Light: the dark assignments mirrored onto the light tones.
    pub const GRUVBOX_LIGHT: Roles = Roles {
        is_dark: false,
        canvas: LIGHT0,
        surface: LIGHT0_HARD,
        elevated: LIGHT0_SOFT,
        border: LIGHT2,
        border_subtle: LIGHT1,
        text: DARK1,
        text_muted: DARK4,
        accent: FADED_BLUE,
        accent_hover: NEUTRAL_BLUE,
        danger: FADED_RED,
        danger_hover: NEUTRAL_RED,
        warning: FADED_YELLOW,
        success: FADED_GREEN,
        hover: LIGHT1,
        active: LIGHT2,
        syntax: [
            FADED_RED, FADED_GREEN, FADED_YELLOW, FADED_GREEN, GRAY, FADED_PURPLE, FADED_AQUA, FADED_ORANGE,
            FADED_BLUE, FADED_RED, FADED_YELLOW, FADED_BLUE,
        ],
        ansi: [
            LIGHT0, NEUTRAL_RED, NEUTRAL_GREEN, NEUTRAL_YELLOW, NEUTRAL_BLUE, NEUTRAL_PURPLE, NEUTRAL_AQUA, DARK4,
            GRAY, FADED_RED, FADED_GREEN, FADED_YELLOW, FADED_BLUE, FADED_PURPLE, FADED_AQUA, DARK1,
        ],
    };

    /// The roles for a dark or a light ground.
    pub const fn of(dark: bool) -> Roles {
        if dark {
            Self::GRUVBOX_DARK
        } else {
            Self::GRUVBOX_LIGHT
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dark roles carry Aura UI's `gruvbox_dark()` hex values exactly
    /// (base, states and the first syntax and ANSI entries, transcribed from
    /// `aura-gui` 0.3.0 `src/view/themes/families.rs`).
    #[test]
    fn the_dark_roles_are_auras_gruvbox_dark() {
        let r = Roles::GRUVBOX_DARK;
        let base = [r.canvas, r.surface, r.elevated, r.border, r.border_subtle, r.text, r.text_muted].map(Rgb::to_hex);
        assert_eq!(base, [0x282828, 0x1d2021, 0x32302f, 0x504945, 0x3c3836, 0xebdbb2, 0xa89984]);
        let states = [r.accent, r.accent_hover, r.danger, r.danger_hover, r.warning, r.success, r.hover, r.active]
            .map(Rgb::to_hex);
        assert_eq!(states, [0x83a598, 0x9cb8ad, 0xfb4934, 0xfc6b5b, 0xfabd2f, 0xb8bb26, 0x3c3836, 0x504945]);
        assert_eq!(r.syntax.map(Rgb::to_hex), [
            0xfb4934, 0xb8bb26, 0xfabd2f, 0xb8bb26, 0x928374, 0xd3869b, 0x8ec07c, 0xfe8019, 0x83a598, 0xfb4934,
            0xfabd2f, 0x83a598
        ]);
        assert_eq!(r.ansi.map(Rgb::to_hex), [
            0x282828, 0xcc241d, 0x98971a, 0xd79921, 0x458588, 0xb16286, 0x689d6a, 0xa89984, 0x928374, 0xfb4934,
            0xb8bb26, 0xfabd2f, 0x83a598, 0xd3869b, 0x8ec07c, 0xebdbb2
        ]);
        assert!(Roles::of(true).is_dark && !Roles::of(false).is_dark);
    }
}
