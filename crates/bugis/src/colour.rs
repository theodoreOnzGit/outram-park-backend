//! The crate's colour type: 8-bit sRGB, `no_std`.

/// An opaque 8-bit sRGB colour. Converts into `egui::Color32` under the
/// `egui` feature.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// From channels.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// From a `0xRRGGBB` literal, as palettes are published.
    pub const fn hex(v: u32) -> Self {
        Self { r: ((v >> 16) & 0xff) as u8, g: ((v >> 8) & 0xff) as u8, b: (v & 0xff) as u8 }
    }

    /// Back to `0xRRGGBB`.
    pub const fn to_hex(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }

    /// `self` moved toward `other` by `amount` in \[0, 1\] (clamped), per
    /// channel, rounded to nearest. The idea is Aura UI's `Rgba::mix`
    /// (`aura-gui` 0.3.0, `src/core/color.rs`, AGPL-3.0-only, Camilo Grajales
    /// Holguín), on 8-bit channels here.
    pub fn mix(self, other: Rgb, amount: f32) -> Rgb {
        use petir::real::Real;
        let t = amount.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| -> u8 { Real::round(a as f32 + (b as f32 - a as f32) * t) as u8 };
        Rgb::new(m(self.r, other.r), m(self.g, other.g), m(self.b, other.b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_mix_hits_its_ends() {
        let a = Rgb::hex(0x282828);
        assert_eq!(a, Rgb::new(0x28, 0x28, 0x28));
        assert_eq!(a.to_hex(), 0x282828);
        let b = Rgb::hex(0xfbf1c7);
        assert_eq!(a.mix(b, 0.0), a);
        assert_eq!(a.mix(b, 1.0), b);
        assert_eq!(a.mix(b, 7.0), b);
        assert_eq!(Rgb::new(0, 0, 0).mix(Rgb::new(255, 255, 255), 0.5), Rgb::new(128, 128, 128));
    }
}
