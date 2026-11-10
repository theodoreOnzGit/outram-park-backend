//! **A hexagonal fast-reactor subassembly** (gh #1327, epic #1312): a hex
//! duct holding a triangular-pitch bundle of pins, each a fuel slug, a
//! bond (or gas gap) annulus and cladding, with coolant between them and an
//! optional helical wire wrap smeared into the coolant.
//!
//! It is built from the beginner API's own exact solids and nothing else: two
//! [`HexPrism`]s (the duct's outside, filled with the duct material, and its
//! inside, filled with coolant) and three coaxial finite [`Cylinder`]s per
//! pin. So [`super::Model::assemble`] checks it with the same exact
//! relations as any other model (every pin enclosed, none overlapping), and
//! it lowers to the same CSG.
//!
//! ```text
//!        ____________            duct: HexPrism, faces normal to y,
//!       /  o  o  o   \           flat-to-flat F_in + 2 t
//!      / o  o  o  o   \          coolant: HexPrism, flat-to-flat F_in
//!     (  o  o  o  o  o )         pins: rows parallel to the flats,
//!      \ o  o  o  o   /          pitch p, r rings, 3r(r-1)+1 pins
//!       \__o__o__o___/
//! ```
//!
//! **Pin centres** come from the OpenMC `HexLattice` port
//! ([`outram_blender::csg::lattice::HexLattice`], `x` orientation, so the
//! neighbours of a pin sit at `(+-p, 0)` and the rows of pins run parallel to
//! the duct's flats), in OpenMC's ring order: outermost ring first, each ring
//! clockwise from its first element, the centre pin last.
//!
//! **Axially uniform.** Every pin runs the subassembly's whole height: this
//! is a section of the fuelled length (or, with reflective ends, an
//! axially infinite one), not the plenum, the reflectors and the end
//! fittings. Stack sections for an axially detailed model.
//!
//! **Wire wrap, smeared.** A wire of diameter `s` wound with lead `H` around
//! a pin of outer diameter `D` follows a helix of diameter `D + s`; its
//! circular section, normal to the helix, cuts an axial plane in an ellipse
//! of area `(pi s^2 / 4) / cos(theta)` with
//! `cos(theta) = H / sqrt(H^2 + (pi (D + s))^2)` (geometry, no correlation).
//! That volume is taken out of the coolant and mixed back into it as wire
//! material, by atom density ([`HexSubassembly::build`]). (Some subchannel
//! correlations drop the `1/cos(theta)` factor; it is about 0.5 % for a
//! lead of 50 pin diameters.) With a wire the bundle must leave room for it:
//! `p >= D + s` and the outer pins `s` clear of the duct, which
//! [`HexSubassembly::check`] tests with the exact relations.
//!
//! **Inputs are the caller's.** No dimension or composition is built in; the
//! tests use generic round numbers.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use outram_blender::csg::lattice::{HexLattice, HexOrientation};
use outram_blender::csg::position::Position;
use petir::mathf::RealMath;
use uom::si::f64::{Length, Ratio};
use uom::si::length::centimeter;
use uom::si::ratio::ratio;

use super::hex::HexPrism;
use super::material::{to_atoms_per_barn_cm, atoms_per_barn_cm, Composition, Material};
use super::model::ModelError;
use super::shape::{Cylinder, Shape, Solid};
use super::isotope::Species;

/// **Pins in a hexagonal bundle of `rings` rings**: `3 r (r - 1) + 1` (the
/// centre pin, then `6 k` in ring `k`). Zero rings is zero pins.
pub fn pins_in_rings(rings: usize) -> usize {
    if rings == 0 {
        0
    } else {
        3 * rings * (rings - 1) + 1
    }
}

/// **One pin's radii**: the fuel slug, the inside of the cladding (the bond
/// or gas gap lies between the two) and the outside of the cladding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PinDesign {
    /// Fuel slug radius.
    pub fuel_radius: Length,
    /// Cladding inner radius (`>=` the fuel radius; equal means no gap).
    pub clad_inner_radius: Length,
    /// Cladding outer radius.
    pub clad_outer_radius: Length,
}

/// **A helical wire wrap**: its diameter and its lead (the axial length of
/// one turn).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WireWrap {
    /// Wire diameter `s`.
    pub diameter: Length,
    /// Lead (helical pitch) `H`.
    pub lead: Length,
}

/// **The materials of a subassembly.** `bond` fills the annulus between the
/// fuel and the cladding (sodium for a sodium-bonded metal pin, helium for a
/// gas gap) and is unused when there is no gap. `wire` is required when the
/// subassembly has a wire wrap; it and `coolant` must then both be given as
/// atom densities ([`Material::from_atom_densities`]), so they can be mixed
/// by volume.
#[derive(Debug, Clone, PartialEq)]
pub struct SubassemblyMaterials {
    /// Fuel slug.
    pub fuel: Material,
    /// Bond or gap.
    pub bond: Material,
    /// Cladding.
    pub clad: Material,
    /// Coolant.
    pub coolant: Material,
    /// Duct wall.
    pub duct: Material,
    /// Wire wrap (smeared into the coolant).
    pub wire: Option<Material>,
}

/// **Volume fractions of a subassembly**, over the duct's outer hexagon
/// (the axial direction cancels). They sum to one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubassemblyVolumeFractions {
    /// Fuel slugs.
    pub fuel: Ratio,
    /// Bond or gap annuli.
    pub bond: Ratio,
    /// Cladding.
    pub clad: Ratio,
    /// Wire wrap.
    pub wire: Ratio,
    /// Coolant (inside the duct, outside the pins, less the wire).
    pub coolant: Ratio,
    /// Duct wall.
    pub duct: Ratio,
}

/// **A hexagonal subassembly's design**: the duct, the bundle and the pin.
/// Build the [`Shape`] with [`Self::build`].
///
/// ```
/// use outram_mc_libs::model::hex_subassembly::{HexSubassembly, PinDesign};
/// use uom::si::{f64::Length, length::centimeter};
/// let cm = Length::new::<centimeter>;
/// let pin = PinDesign {
///     fuel_radius: cm(0.25),
///     clad_inner_radius: cm(0.28),
///     clad_outer_radius: cm(0.33),
/// };
/// // 3 rings (19 pins) at 0.8 cm pitch in a duct 4.0 cm across the flats
/// // inside, with a 0.2 cm wall, 10 cm tall: generic numbers.
/// let sa = HexSubassembly::new(cm(4.0), cm(0.2), cm(10.0), cm(0.8), 3, pin);
/// assert_eq!(sa.pin_count(), 19);
/// sa.check().unwrap();
/// let f = sa.volume_fractions();
/// let total = f.fuel + f.bond + f.clad + f.wire + f.coolant + f.duct;
/// assert!((total.value - 1.0).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HexSubassembly {
    duct_inner_flat_to_flat_cm: f64,
    duct_wall_cm: f64,
    height_cm: f64,
    pin_pitch_cm: f64,
    rings: usize,
    fuel_radius_cm: f64,
    clad_inner_radius_cm: f64,
    clad_outer_radius_cm: f64,
    wire: Option<(f64, f64)>,
}

impl HexSubassembly {
    /// A subassembly: the duct's inside flat-to-flat distance and its wall
    /// thickness, the height, the pin pitch, the number of rings of pins
    /// (`1` is a single pin) and the pin's radii. No wire wrap; add one
    /// with [`Self::wire_wrap`]. Nothing is checked until [`Self::check`]
    /// or [`Self::build`].
    pub fn new(
        duct_inner_flat_to_flat: Length,
        duct_wall: Length,
        height: Length,
        pin_pitch: Length,
        rings: usize,
        pin: PinDesign,
    ) -> Self {
        Self {
            duct_inner_flat_to_flat_cm: duct_inner_flat_to_flat.get::<centimeter>(),
            duct_wall_cm: duct_wall.get::<centimeter>(),
            height_cm: height.get::<centimeter>(),
            pin_pitch_cm: pin_pitch.get::<centimeter>(),
            rings,
            fuel_radius_cm: pin.fuel_radius.get::<centimeter>(),
            clad_inner_radius_cm: pin.clad_inner_radius.get::<centimeter>(),
            clad_outer_radius_cm: pin.clad_outer_radius.get::<centimeter>(),
            wire: None,
        }
    }

    /// Wind a helical wire round every pin, smeared into the coolant.
    pub fn wire_wrap(mut self, wire: WireWrap) -> Self {
        self.wire = Some((
            wire.diameter.get::<centimeter>(),
            wire.lead.get::<centimeter>(),
        ));
        self
    }

    /// Number of pins, `3 r (r - 1) + 1`.
    pub fn pin_count(&self) -> usize {
        pins_in_rings(self.rings)
    }

    /// Number of rings.
    pub fn rings(&self) -> usize {
        self.rings
    }

    /// **The pin centres** `(x, y)` \[cm\], in OpenMC's ring order
    /// (outermost ring first, the centre pin last), from the OpenMC
    /// `HexLattice` port's own tile centres.
    pub fn pin_centres_cm(&self) -> Vec<[f64; 2]> {
        if self.rings == 0 {
            return Vec::new();
        }
        let n = self.rings;
        let rings: Vec<Vec<usize>> = (0..n)
            .map(|k| vec![0; if k == n - 1 { 1 } else { 6 * (n - 1 - k) }])
            .collect();
        let lattice = HexLattice::from_rings(
            0,
            HexOrientation::X,
            Position::new(0.0, 0.0, 0.0),
            self.pin_pitch_cm,
            &rings,
            None,
        );
        let side = lattice.n_side();
        lattice
            .ring_order_flat_indices()
            .into_iter()
            .map(|flat| {
                let c = lattice.tile_center([(flat % side) as i32, (flat / side) as i32, 0]);
                [c.x, c.y]
            })
            .collect()
    }

    /// The duct's outside \[a prism\].
    fn duct_prism(&self) -> HexPrism {
        HexPrism::flat_to_flat(
            Length::new::<centimeter>(self.duct_inner_flat_to_flat_cm + 2.0 * self.duct_wall_cm),
            Length::new::<centimeter>(self.height_cm),
        )
    }

    /// The duct's inside, the coolant region.
    fn coolant_prism(&self) -> HexPrism {
        HexPrism::flat_to_flat(
            Length::new::<centimeter>(self.duct_inner_flat_to_flat_cm),
            Length::new::<centimeter>(self.height_cm),
        )
    }

    /// A finite cylinder of `radius_cm` on pin centre `c`, the
    /// subassembly's height.
    fn pin_cylinder(&self, c: [f64; 2], radius_cm: f64) -> Cylinder {
        let cm = Length::new::<centimeter>;
        Cylinder::radius(cm(radius_cm))
            .height(cm(self.height_cm))
            .centred_at(cm(c[0]), cm(c[1]), cm(0.0))
    }

    /// `(diameter, cos theta)` of the wire, if any.
    fn wire_geometry(&self) -> Option<(f64, f64)> {
        self.wire.map(|(s, lead)| {
            let helix = std::f64::consts::PI * (2.0 * self.clad_outer_radius_cm + s);
            (s, lead / lead.r_hypot(helix))
        })
    }

    /// One wire's cross-section in an axial plane \[cm²\],
    /// `(pi s^2 / 4) / cos(theta)`; zero without a wire.
    pub fn wire_area_cm2(&self) -> f64 {
        self.wire_geometry().map_or(0.0, |(s, cos_theta)| {
            0.25 * std::f64::consts::PI * s * s / cos_theta
        })
    }

    /// **Check the design**: every dimension positive and finite, the radii
    /// ordered (`fuel <= clad inner < clad outer`), at least one ring, and,
    /// with the exact solid relations, every pin (with its wire, when it
    /// has one) inside the duct and clear of every other pin's cladding.
    /// Touching is allowed (a wire that touches its neighbour, `p = D + s`,
    /// is the usual design).
    ///
    /// # Errors
    /// [`ModelError::BadValue`] naming the first violation.
    pub fn check(&self) -> Result<(), ModelError> {
        let bad = |s: String| Err(ModelError::BadValue(format!("hex subassembly: {s}")));
        let dims = [
            ("duct inner flat-to-flat", self.duct_inner_flat_to_flat_cm),
            ("duct wall", self.duct_wall_cm),
            ("height", self.height_cm),
            ("pin pitch", self.pin_pitch_cm),
            ("fuel radius", self.fuel_radius_cm),
            ("clad inner radius", self.clad_inner_radius_cm),
            ("clad outer radius", self.clad_outer_radius_cm),
        ];
        for (name, v) in dims {
            if !(v.is_finite() && v > 0.0) {
                return bad(format!("{name} must be positive and finite, got {v} cm"));
            }
        }
        if let Some((s, lead)) = self.wire {
            if !(s.is_finite() && s > 0.0 && lead.is_finite() && lead > 0.0) {
                return bad(format!(
                    "wire diameter and lead must be positive and finite, got {s} and {lead} cm"
                ));
            }
        }
        if self.rings == 0 {
            return bad("needs at least one ring of pins".into());
        }
        if !(self.fuel_radius_cm <= self.clad_inner_radius_cm
            && self.clad_inner_radius_cm < self.clad_outer_radius_cm)
        {
            return bad(format!(
                "radii must satisfy fuel <= clad inner < clad outer, got {} / {} / {} cm",
                self.fuel_radius_cm, self.clad_inner_radius_cm, self.clad_outer_radius_cm
            ));
        }
        let wire_cm = self.wire.map_or(0.0, |(s, _)| s);
        let coolant = Solid::HexPrism(self.coolant_prism());
        let centres = self.pin_centres_cm();
        let clads: Vec<Solid> = centres
            .iter()
            .map(|c| Solid::Cylinder(self.pin_cylinder(*c, self.clad_outer_radius_cm)))
            .collect();
        for (i, c) in centres.iter().enumerate() {
            let envelope =
                Solid::Cylinder(self.pin_cylinder(*c, self.clad_outer_radius_cm + wire_cm));
            if !coolant.encloses(&envelope) {
                return bad(format!(
                    "pin {i} at ({:.6}, {:.6}) cm{} pokes out of the duct",
                    c[0],
                    c[1],
                    if wire_cm > 0.0 { " with its wire" } else { "" }
                ));
            }
            for (j, other) in clads.iter().enumerate() {
                if j != i && envelope.overlaps(other) {
                    return bad(format!(
                        "pin {i}{} overlaps pin {j}'s cladding (pitch {} cm)",
                        if wire_cm > 0.0 { " with its wire" } else { "" },
                        self.pin_pitch_cm
                    ));
                }
            }
        }
        Ok(())
    }

    /// **Volume fractions, analytic**: hexagon areas `(sqrt 3 / 2) F^2`,
    /// circle areas `pi r^2` times the pin count, and the wire's
    /// [`Self::wire_area_cm2`], over the duct's outer hexagon.
    pub fn volume_fractions(&self) -> SubassemblyVolumeFractions {
        let hex = |f: f64| 0.5 * 3.0_f64.sqrt() * f * f;
        let circle = |r: f64| std::f64::consts::PI * r * r;
        let n = self.pin_count() as f64;
        let outer = hex(self.duct_inner_flat_to_flat_cm + 2.0 * self.duct_wall_cm);
        let inner = hex(self.duct_inner_flat_to_flat_cm);
        let fuel = n * circle(self.fuel_radius_cm);
        let bond = n * (circle(self.clad_inner_radius_cm) - circle(self.fuel_radius_cm));
        let clad = n * (circle(self.clad_outer_radius_cm) - circle(self.clad_inner_radius_cm));
        let wire = n * self.wire_area_cm2();
        let coolant = inner - n * circle(self.clad_outer_radius_cm) - wire;
        let f = |a: f64| Ratio::new::<ratio>(a / outer);
        SubassemblyVolumeFractions {
            fuel: f(fuel),
            bond: f(bond),
            clad: f(clad),
            wire: f(wire),
            coolant: f(coolant),
            duct: f(outer - inner),
        }
    }

    /// The wire's share of the space between the pins (the volume fraction
    /// of the smeared coolant that is wire).
    pub fn wire_fraction_of_coolant(&self) -> f64 {
        let v = self.volume_fractions();
        let wire = v.wire.get::<ratio>();
        wire / (wire + v.coolant.get::<ratio>())
    }

    /// **The subassembly as one [`Shape`]**: the duct (outer prism, duct
    /// material) holding the coolant (inner prism) holding the pins, each a
    /// cladding cylinder holding a bond cylinder (left out when there is no
    /// gap) holding the fuel. Centred at the origin, faces normal to y.
    /// Add it to a model as it is, or give it a boundary
    /// (`.boundary(Boundary::Reflective)` makes an infinite array of
    /// subassemblies, a k-infinity).
    ///
    /// With a wire wrap, the coolant is the volume mix of
    /// `materials.coolant` and `materials.wire`, by atom density, at the
    /// wire's share [`Self::wire_fraction_of_coolant`].
    ///
    /// # Errors
    /// [`ModelError::BadValue`]: [`Self::check`] fails; a wire wrap without a
    /// wire material (or the reverse); or a wire whose coolant or wire
    /// material is not given as atom densities, or carries contaminants.
    pub fn build(&self, materials: SubassemblyMaterials) -> Result<Shape, ModelError> {
        self.check()?;
        let coolant = match (&self.wire, materials.wire) {
            (None, None) => materials.coolant,
            (Some(_), Some(wire)) => {
                smear_by_volume(&materials.coolant, &wire, self.wire_fraction_of_coolant())?
            }
            (Some(_), None) => {
                return Err(ModelError::BadValue(
                    "hex subassembly: a wire wrap needs a wire material".into(),
                ))
            }
            (None, Some(_)) => {
                return Err(ModelError::BadValue(
                    "hex subassembly: a wire material was given but no wire wrap".into(),
                ))
            }
        };
        let gap = self.clad_inner_radius_cm > self.fuel_radius_cm;
        let mut region = self.coolant_prism().filled_with(coolant);
        for c in self.pin_centres_cm() {
            let fuel = self
                .pin_cylinder(c, self.fuel_radius_cm)
                .filled_with(materials.fuel.clone());
            let inside_clad = if gap {
                self.pin_cylinder(c, self.clad_inner_radius_cm)
                    .filled_with(materials.bond.clone())
                    .containing(fuel)
            } else {
                fuel
            };
            region = region.containing(
                self.pin_cylinder(c, self.clad_outer_radius_cm)
                    .filled_with(materials.clad.clone())
                    .containing(inside_clad),
            );
        }
        Ok(self
            .duct_prism()
            .filled_with(materials.duct)
            .containing(region))
    }
}

/// **Mix two atom-density materials by volume**: `(1 - f) N_a + f N_b` for
/// every species, the same species merged. Named `"<a> + <b> (smeared)"`.
fn smear_by_volume(a: &Material, b: &Material, f: f64) -> Result<Material, ModelError> {
    let parts = |m: &Material| match &m.composition {
        Composition::AtomDensities(v) if m.contaminants.is_empty() => Ok(v.clone()),
        _ => Err(ModelError::BadValue(format!(
            "hex subassembly: to smear the wire into the coolant, give '{}' as atom densities \
             (Material::from_atom_densities) without contaminants",
            m.name
        ))),
    };
    let mut mixed: Vec<(Species, f64)> = Vec::new();
    for (weight, list) in [(1.0 - f, parts(a)?), (f, parts(b)?)] {
        for (s, n) in list {
            let add = weight * to_atoms_per_barn_cm(n);
            match mixed.iter_mut().find(|(t, _)| *t == s) {
                Some(e) => e.1 += add,
                None => mixed.push((s, add)),
            }
        }
    }
    Ok(
        Material::from_atom_densities(mixed.into_iter().map(|(s, n)| (s, atoms_per_barn_cm(n))))
            .named(&format!("{} + {} (smeared)", a.name, b.name)),
    )
}

#[cfg(test)]
mod tests;
