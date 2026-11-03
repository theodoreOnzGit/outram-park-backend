<!-- Moved verbatim from the workspace CLAUDE.md on 2026-09-21 (maintainer direction: physics
     rules stay in the root, everything else here behind a pointer). STILL BINDING. -->

## Rust design rules (mandatory)

### No trait objects — use enums for dispatch

Do not use `Box<dyn Trait>`, `&dyn Trait`, or `Arc<dyn Trait>` for dispatch.
Use enums instead. The set of physics models (EOS, turbulence models, numerical
schemes, boundary conditions) is closed and known at compile time — enums are
the right tool.

Benefits over trait objects:
- **Exhaustiveness** — adding a new variant forces every `match` site to handle it; a missing case is a compile error, not a runtime surprise
- **Zero heap allocation** — the enum lives inline in its containing struct
- **rust-analyzer navigability** — Go-to-definition works on enum variants; it often fails on `dyn Trait` implementations

Traits are still useful as a **compiler-enforced contract** on each concrete
struct — the compiler verifies every model implements the right methods. They
are just not used for runtime dispatch. The pattern:

```rust
// Trait enforces the interface — compiler checks every model satisfies it
pub trait TurbulenceKernel {
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix;
    fn correct(&mut self);
}

// Enum dispatches without Box or dyn
pub enum TurbulenceModel {
    Laminar(LaminarModel),
    KOmegaSST(KOmegaSSTModel),
    KEpsilon(KEpsilonModel),
}

impl TurbulenceModel {
    pub fn correct(&mut self) {
        match self {
            Self::Laminar(m)   => m.correct(),
            Self::KOmegaSST(m) => m.correct(),
            Self::KEpsilon(m)  => m.correct(),
        }
    }
}
```

### Crate-level interfaces: traits consumed generically (SOLID), added 2026-11-03

**Maintainer direction, 2026-11-03:** "At the crate level I am ok with
interfaces using SOLID principles … the reason is so we don't introduce
lifetimes unnecessarily." A crate may expose a trait as the seam other crates
implement, consumed generically, never through `dyn`:

```rust
// raffles states what it needs (dependency inversion) ...
// The physical quantities cross it as uom types (associated types), never bare f64.
pub trait HighFidelityModel {
    type Input: Quantities;   // e.g. a struct of uom fields: Length, ThermalConductivity, ...
    type Output: Quantities;
    fn evaluate(&self, x: &Self::Input) -> Result<Self::Output>;
}
// ... and owns the implementer by value: no `&'a dyn`, so no lifetime.
pub fn build<M: HighFidelityModel>(hf: &M /* or M, or Arc<M> */)
    -> Result<Surrogate<M::Input, M::Output>> { .. }
```

A closed set of variants the crate provides itself is still an enum, as above.
Full statement: the root [`AGENTS.md`](../../AGENTS.md), "Crate-level
interfaces may follow SOLID principles".

### No `Box<T>`

Do not use `Box<T>`. Own data by value or share it with `Arc<T>`.
`Box<T>` is only justified for recursive data structures (trees, linked lists),
which do not appear in this codebase.

### No lifetime parameters

Do not add lifetime parameters (`'a`) to structs, trait definitions, or impl
blocks. Own data by value, or share it with `Arc<T>`.

| Instead of | Use |
|---|---|
| `&'a FvMesh` in a struct | `Arc<FvMesh>` |
| `&'a f64` / uom quantity in a struct | own by value — all uom types are `Copy` |
| `Box<dyn Fn(&'a T) -> U>` | newtype struct that owns its captured state |
| `&'a Cell` for graph/topology links | `CellId(usize)` — index into a `Vec` |

### Shared state: `Arc<RwLock<T>>` over channels

For shared mutable simulation state (fields, solver coefficients), use
`Arc<RwLock<T>>`. For data that is read-only after construction (mesh topology,
lookup tables, material constants), use `Arc<T>` with no lock.

Prefer `RwLock<T>` over `Mutex<T>` — `RwLock` allows concurrent reads from
multiple threads; `Mutex` serialises even read-only access, which defeats
parallelism during the compute phase of a timestep.

Do not use channels (`mpsc`, `crossbeam`) for simulation state. Channels suit
pipeline patterns where data is produced, consumed, and discarded. The simulation
timestep loop is a shared-state pattern — threads compute over non-overlapping
regions of the same fields, then synchronise.

