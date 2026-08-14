# Crate Documentation

**Version:** 0.0.0

**Format Version:** 60

# Module `kovan_literature`

# kovan-literature

The nuclear-engineering knowledge archive. It turns source PDFs into the
canonical [`KovanDocument`] and generates derived artifacts (Markdown,
BibTeX, extracted assets).

## Canonical workflow

```text
PDF → Markdown → KovanDocument → BibTeX → generated knowledge artifacts
```

Implements the pipeline described in `docs/kovan.md` sections
"Literature Workflow", "Canonical Representation" and "PDF Processing".
The [`KovanDocument`] struct is authoritative; BibTeX and generated Markdown
are always derived from it, never the other way round.

## Determinism & offline guarantees

Every function here is **deterministic** (same input bytes → same output
bytes) and runs **fully offline** — no network, no cloud, no OCR service.
PDF text extraction uses the pure-Rust [`pdf_extract`] crate; the low-level
object model (metadata, assets) uses pure-Rust [`lopdf`]. Both build for
Android (`aarch64-linux-android`), matching KOVAN's Android-first mandate
(`docs/kovan.md`, "Android First").

## Storage layout

Content lives on disk next to this crate (`docs/kovan.md`, "Storage Layout"):

- `open/{papers,reports,standards,benchmarks,theses}/` — redistributable
  content, may be committed.
- `proprietary/{…}/` — user-owned content; **gitignored**, never committed.
- `generated/{markdown,bibtex,assets}/{open,proprietary}/` — reproducible
  outputs, split by [`Visibility`] so the proprietary half can be kept out of
  both git and the published crate. See [`storage::generated_dir_for`].

Three distribution tiers follow from that split: generated **open BibTeX** is
committed *and* published to crates.io; open PDFs and generated open Markdown
are committed but **not** published (licence scope and size); everything
proprietary is neither.

## What is real vs. best-effort

- [`pdf_to_markdown`], [`markdown_outline`], [`to_bibtex`] — fully
  implemented and tested.
- [`extract_metadata`] — best-effort heuristics (PDF Info dictionary first,
  then conservative text scanning). Unknown fields are left `None`/empty
  rather than guessed.
- [`extract_assets`] — extracts embedded raster images whose codec is already
  a standalone file format (JPEG via `DCTDecode`, JPEG-2000 via `JPXDecode`).
  Images stored under other filters are reported-skipped, not re-encoded.
- [`digitiser`] — graph digitiser: recover `(x, y)` data points from plot
  images, with mandatory calibration/provenance records. Verified against
  synthetic fixtures only (see its module doc for the honest limits).
  Ships three binaries over the one engine: `kovan-digitise` (automatic
  CLI), `kovan-digitise-tui` (hybrid terminal review), and
  `kovan-digitise-gui` (hybrid egui review; desktop-only, non-default
  `digitise-gui` feature).

## Modules

## Module `digitiser`

# Graph digitiser — extract `(x, y)` data points from plot images

Several validation targets in this project exist **only as figures in
papers** (HTR-10 safety-demonstration transients, MSRE reactivity-insertion
curves, the Tobias decay-heat plots). This module turns a raster image of a
published plot into numeric data points *with the provenance record that
makes them usable as validation evidence* (`DATA_POLICY.md`: digitisation
is a processing step and must be documented as one).

## What belongs in this module

- [`raster`] — loading a plot image into an owned RGB buffer (pure-Rust
  decoding via the `image` crate; PNG and JPEG).
- [`calibration`] — mapping pixel coordinates to data coordinates, with
  **linear and logarithmic axes independently per axis**. Log axes are
  calibrated in log10 space, never by linear pixel interpolation.
- [`detect`] — automatic detection of the plot frame (axis box) from dark
  line runs. Deterministic; no ML, no OCR.
- [`trace`] — automatic curve tracing by column scan, with enum-dispatched
  strategies ([`trace::TraceStrategy`]) and colour selectors
  ([`trace::CurveSelector`]).
- [`dataset`] — the output types. [`dataset::DigitisedDataset`] is
  deliberately impossible to construct or export without its
  [`calibration::PlotCalibration`] and [`dataset::FigureSource`] attached.
- [`auto`] — the one-shot automatic pipeline shared by all front ends.
- [`synthetic`] — deterministic rendering of known curves to images, used
  as self-consistency test fixtures (and later to cross-check the
  maintainer-supplied golden oracle, bead `op-amfh`).
- [`frontend`] *(feature-gated)* — the shared `clap` argument surface used
  by the `kovan-digitise` CLI and `kovan-digitise-tui` binaries.

## What does not belong here

- OCR / reading printed tick labels. KOVAN is deterministic and offline
  (no ML), so **numeric axis values must be supplied by the caller** (they
  are stated in the figure's caption/axes and are facts, not guesses); the
  pixel geometry is what gets automated.
- Network access of any kind.
- PDF page rendering. Extract the figure to PNG/JPEG first (e.g. with
  [`crate::extract_assets`] when the PDF stores it as an embedded raster).

## Units and `uom`

Digitised axes carry whatever units the source figure printed — often
non-SI, arbitrary, or normalised (e.g. "% of operating power",
"MeV/fission·s"). The engine therefore works in plain `f64` *document
units* and records the axis label text verbatim in
[`dataset::DigitisedDataset::x_label`]/`y_label`; converting into `uom`
quantities is the consumer's job, at the point where the unit is actually
interpreted. Forcing `uom` here would require inventing dimensions for
axes the engine cannot know.

## Verification status (honest limits)

The engine is verified by **synthetic self-consistency tests only**
(`tests/digitiser_synthetic.rs`): known curves are rendered to images at
known pixel positions, digitised, and compared against the analytic
values, for linear-linear, log-linear and log-log axes. Measured accuracy
figures live in that test file's doc comments. **No accuracy claim is made
against real published figures** — the hand-digitised golden oracle
(Tobias decay-heat points, bead `op-amfh`) does not exist yet. When it
lands, compare with [`synthetic`]-style tolerance checks against
[`dataset::DigitisedDataset`] output over the real scans.

```rust
pub mod digitiser { /* ... */ }
```

### Modules

## Module `auto`

One-shot automatic digitisation — the pipeline every front end shares.

Belongs here: [`AxisValueSpec`], [`AutoDigitiseConfig`], [`AxisPixelRefs`]
and [`auto_digitise`], which chain frame detection → calibration → trace →
dataset in one deterministic call. The CLI runs exactly this and nothing
more; the TUI/GUI run it as their "automatic pass first" and then let a
human correct the result.

Does not belong here: the individual algorithms (see [`super::detect`],
[`super::calibration`], [`super::trace`]) or any interactivity.

```rust
pub mod auto { /* ... */ }
```

### Types

#### Enum `AxisPixelRefs`

How the numeric axis values are anchored to pixels for one axis. Closed
set, enum-dispatched.

Tick-label OCR is deliberately out of scope (see the [`super`] module
doc), so the *values* always come from the caller; what varies is whether
the *pixels* they attach to come from automatic frame detection or are
given explicitly.

```rust
pub enum AxisPixelRefs {
    FrameEdges {
        min_value: f64,
        max_value: f64,
    },
    Explicit {
        r1: super::calibration::AxisRef,
        r2: super::calibration::AxisRef,
    },
}
```

##### Variants

###### `FrameEdges`

Anchor the values to the detected frame edges: `min_value` at the
frame's left (x axis) / bottom (y axis), `max_value` at its right /
top. The fully automatic path — correct whenever the figure's axis
extremes are labelled, which is the common case.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `min_value` | `f64` | Data value at the left/bottom frame edge. |
| `max_value` | `f64` | Data value at the right/top frame edge. |

###### `Explicit`

Two explicit pixel↔value pairs, e.g. read off gridline intersections.
Use when the curve is cropped oddly or the frame edges are unlabelled.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `r1` | `super::calibration::AxisRef` | First reference (pixel coordinate along this axis + its value). |
| `r2` | `super::calibration::AxisRef` | Second reference. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisPixelRefs { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AxisPixelRefs) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `AxisValueSpec`

Full specification of one axis: scale plus pixel anchoring.

```rust
pub struct AxisValueSpec {
    pub scale: super::calibration::AxisScale,
    pub refs: AxisPixelRefs,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `scale` | `super::calibration::AxisScale` | Linear or logarithmic. |
| `refs` | `AxisPixelRefs` | Where the values sit in pixel space. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisValueSpec { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AxisValueSpec) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `AutoDigitiseConfig`

Everything the automatic pipeline needs besides the image and the
provenance strings.

```rust
pub struct AutoDigitiseConfig {
    pub x: AxisValueSpec,
    pub y: AxisValueSpec,
    pub detect: super::detect::DetectConfig,
    pub trace: super::trace::TraceConfig,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `AxisValueSpec` | x-axis specification. |
| `y` | `AxisValueSpec` | y-axis specification. |
| `detect` | `super::detect::DetectConfig` | Frame-detection tuning. |
| `trace` | `super::trace::TraceConfig` | Curve-trace tuning. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AutoDigitiseConfig { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AutoDigitiseConfig) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `auto_digitise`

Run the full automatic pipeline: detect (or derive) the frame, build the
calibration, trace the curve, and package a [`DigitisedDataset`] with the
complete provenance record. Deterministic: same raster + config +
provenance strings → identical dataset.

Frame detection is skipped only when **both** axes use
[`AxisPixelRefs::Explicit`] *and* automatic detection fails — in that case
the trace region falls back to the rectangle spanned by the explicit
reference pixels. When either axis anchors to
[`AxisPixelRefs::FrameEdges`], detection must succeed.

`digitised_by`/`digitised_at` are recorded verbatim; pass
[`super::dataset::utc_now_iso8601`] for `digitised_at` unless a
reproducible stamp is required. The returned dataset is always
[`super::dataset::ReviewStatus::Unreviewed`].

# Errors

Any [`DigitiserError`] from detection, calibration, or tracing.

```rust
pub fn auto_digitise</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(raster: &super::raster::PlotRaster, config: &AutoDigitiseConfig, source: super::dataset::FigureSource, x_label: impl Into<String>, y_label: impl Into<String>, digitised_by: impl Into<String>, digitised_at: impl Into<String>) -> Result<super::dataset::DigitisedDataset, super::DigitiserError> { /* ... */ }
```

## Module `calibration`

Axis calibration — mapping pixel coordinates to data coordinates.

Belongs here: [`AxisScale`], [`AxisRef`], [`AxisCalibration`],
[`PlotCalibration`], and the pixel ↔ data-value maps. Logarithmic axes are
interpolated in **log10 space** — the pixel position of a value on a log
axis is affine in `log10(value)`, not in the value itself, and getting
this wrong is the classic digitisation error this module exists to avoid.

Does not belong here: image handling ([`super::raster`]), curve extraction
([`super::trace`]), output formats ([`super::dataset`]).

```rust
pub mod calibration { /* ... */ }
```

### Types

#### Enum `AxisScale`

Whether an axis is linear or logarithmic. Closed set — enum-dispatched per
the workspace Rust design rules.

```rust
pub enum AxisScale {
    Linear,
    Logarithmic,
}
```

##### Variants

###### `Linear`

Value is an affine function of pixel position.

###### `Logarithmic`

`log10(value)` is an affine function of pixel position (decade-ruled
axis). Reference values must be strictly positive.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisScale { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AxisScale) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `AxisRef`

One axis reference point: a pixel coordinate along the axis direction
(column index for the x axis, row index for the y axis) paired with the
data value the figure assigns to that pixel.

`pixel` is an `f64` because reference points may be placed with sub-pixel
precision (e.g. the centre of a 2-px-thick axis line). `value` is in
*document units* — whatever the source figure's axis label says (see the
module doc of [`super`] for why `uom` is not used here).

```rust
pub struct AxisRef {
    pub pixel: f64,
    pub value: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pixel` | `f64` | Pixel coordinate along this axis (x axis → column, y axis → row;<br>image rows increase downward). |
| `value` | `f64` | Data value at that pixel, in the figure's own units. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisRef { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AxisRef) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `AxisCalibration`

Calibration of a single axis from two reference points.

Construct with [`AxisCalibration::new`], which validates the references;
the fields stay public so a deserialised calibration can be inspected, but
prefer the constructor for anything built at runtime.

```rust
pub struct AxisCalibration {
    pub scale: AxisScale,
    pub r1: AxisRef,
    pub r2: AxisRef,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `scale` | `AxisScale` | Linear or logarithmic interpolation between the reference points. |
| `r1` | `AxisRef` | First reference point. |
| `r2` | `AxisRef` | Second reference point. Must differ from `r1` in both pixel and value. |

##### Implementations

###### Methods

- ```rust
  pub fn new(scale: AxisScale, r1: AxisRef, r2: AxisRef) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Build a validated axis calibration.

- ```rust
  pub fn value_at(self: &Self, pixel: f64) -> f64 { /* ... */ }
  ```
  Data value at pixel coordinate `pixel`, in the figure's own units.

- ```rust
  pub fn pixel_at(self: &Self, value: f64) -> Option<f64> { /* ... */ }
  ```
  Pixel coordinate at which `value` sits on this axis — the inverse of

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisCalibration { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AxisCalibration) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PlotCalibration`

Full two-axis calibration of a plot: an [`AxisCalibration`] for x (pixel
columns) and one for y (pixel rows; rows increase *downward*, which the
two-point form handles with no special casing — the bottom-of-plot
reference simply has the larger row index).

```rust
pub struct PlotCalibration {
    pub x: AxisCalibration,
    pub y: AxisCalibration,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `AxisCalibration` | Horizontal axis (pixel columns → data x). |
| `y` | `AxisCalibration` | Vertical axis (pixel rows → data y). |

##### Implementations

###### Methods

- ```rust
  pub fn point_at(self: &Self, x_px: f64, y_px: f64) -> (f64, f64) { /* ... */ }
  ```
  Map an image pixel `(column, row)` to data coordinates `(x, y)`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PlotCalibration { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PlotCalibration) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `dataset`

Digitised datasets — the output type, with mandatory provenance.

Belongs here: [`FigureSource`], [`PointOrigin`], [`ReviewStatus`],
[`DigitisedPoint`], [`TraceRecord`], [`DigitisedDataset`], and their
JSON/CSV export. The design rule (from `DATA_POLICY.md`: digitisation is
a processing step and must be documented as one) is that **a dataset
cannot exist, be serialised, or be exported without its calibration and
source record** — [`DigitisedDataset`]'s calibration and source are plain
required fields, there is no points-only constructor, and both exporters
read them from the struct itself.

Does not belong here: pixel scanning ([`super::trace`]), calibration math
([`super::calibration`]), or interactive editing (the TUI/GUI binaries own
that, and record their edits *into* these types).

```rust
pub mod dataset { /* ... */ }
```

### Types

#### Struct `FigureSource`

Where the digitised figure came from — the document-level half of the
provenance record.

`document_id`/`document_title` should reference the figure's
[`crate::KovanDocument`] (its `id` and `title`) when the source has been
catalogued into the KOVAN literature archive; they stay `None` for a
not-yet-catalogued source, in which case `image_path` at least pins the
file that was digitised.

```rust
pub struct FigureSource {
    pub document_id: Option<String>,
    pub document_title: Option<String>,
    pub figure: String,
    pub page: Option<u32>,
    pub image_path: Option<String>,
    pub image_sha256: Option<String>,
    pub notes: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `document_id` | `Option<String>` | [`crate::KovanDocument::id`] of the catalogued source document, if<br>catalogued. |
| `document_title` | `Option<String>` | [`crate::KovanDocument::title`] (or a free-text citation) of the<br>source document. |
| `figure` | `String` | Figure designation as printed, e.g. `"Fig. 7"` or `"Figure 3(b)"`.<br>Required — a digitisation that cannot say which figure it read is not<br>usable as evidence. |
| `page` | `Option<u32>` | Page number the figure appears on, if known. |
| `image_path` | `Option<String>` | Path of the image file that was digitised (as given by the caller). |
| `image_sha256` | `Option<String>` | Lowercase-hex SHA-256 of the image file's bytes, so the exact raster<br>this dataset was read from can be re-identified. Filled automatically<br>when the raster was loaded from a file. |
| `notes` | `Option<String>` | Free-text notes (e.g. "curve labelled '235U thermal'", crop applied,<br>known scan skew). |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(figure: impl Into<String>) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Minimal source record: just the figure designation. Fill the optional

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FigureSource { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FigureSource) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `PointOrigin`

How a single point came to be — automatic, hand-placed, or hand-corrected.
Closed set, enum-dispatched; recorded per point so a reviewer can see
exactly which values a human touched.

```rust
pub enum PointOrigin {
    AutoTraced,
    HandPlaced {
        by: String,
    },
    HandCorrected {
        by: String,
    },
}
```

##### Variants

###### `AutoTraced`

Emitted by the automatic tracer, untouched by a human.

###### `HandPlaced`

Placed by a human (TUI/GUI editing), never produced by the tracer.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `by` | `String` | Who placed it (operator name as given to the front end). |

###### `HandCorrected`

Auto-traced, then moved by a human.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `by` | `String` | Who corrected it. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PointOrigin { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PointOrigin) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ReviewInterface`

Which front end a human review happened in. Closed set.

```rust
pub enum ReviewInterface {
    Tui,
    Gui,
    External,
}
```

##### Variants

###### `Tui`

`kovan-digitise-tui` (ratatui).

###### `Gui`

`kovan-digitise-gui` (egui).

###### `External`

Reviewed outside the shipped front ends (e.g. plotted and inspected by
hand); the reviewer takes responsibility for the method.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReviewInterface { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReviewInterface) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ReviewStatus`

Whether a human has verified this dataset. The automatic CLI always emits
[`ReviewStatus::Unreviewed`]; only the hybrid front ends (or an external
reviewer) may record a review, and the record says who, when, and where —
**confirmation is recorded, never assumed**.

```rust
pub enum ReviewStatus {
    Unreviewed,
    Reviewed {
        by: String,
        at: String,
        interface: ReviewInterface,
    },
}
```

##### Variants

###### `Unreviewed`

No human has checked the points against the figure.

###### `Reviewed`

A human inspected the points overlaid on the figure and accepted them.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `by` | `String` | Reviewer name. |
| `at` | `String` | UTC timestamp, ISO 8601 (`YYYY-MM-DDTHH:MM:SSZ`). |
| `interface` | `ReviewInterface` | Front end the review happened in. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReviewStatus { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReviewStatus) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DigitisedPoint`

One digitised data point, in the figure's own units, with its reading
uncertainty and per-point origin.

Uncertainties are stored as separate `minus`/`plus` magnitudes (both
`>= 0`) because on a logarithmic axis the pixel reading error maps to an
**asymmetric, value-dependent** interval — collapsing it to one symmetric
number would misstate exactly the case (log-log decay-heat curves) this
tool exists for.

```rust
pub struct DigitisedPoint {
    pub x: f64,
    pub y: f64,
    pub x_minus: f64,
    pub x_plus: f64,
    pub y_minus: f64,
    pub y_plus: f64,
    pub x_px: Option<f64>,
    pub y_px: Option<f64>,
    pub origin: PointOrigin,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` | Data x value, in the figure's x-axis units. |
| `y` | `f64` | Data y value, in the figure's y-axis units. |
| `x_minus` | `f64` | Magnitude of the downward x reading uncertainty: the value could be as<br>low as `x - x_minus`. |
| `x_plus` | `f64` | Magnitude of the upward x reading uncertainty. |
| `y_minus` | `f64` | Magnitude of the downward y reading uncertainty. |
| `y_plus` | `f64` | Magnitude of the upward y reading uncertainty. |
| `x_px` | `Option<f64>` | Pixel column this point sits at (kept so the TUI/GUI can re-overlay<br>the point on the image; `None` only for hand-placed points created in<br>data space). |
| `y_px` | `Option<f64>` | Pixel row this point sits at. |
| `origin` | `PointOrigin` | How the point came to be. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DigitisedPoint { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DigitisedPoint) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `TraceRecord`

Record of the automatic pass that produced the auto-traced points: the
exact configuration, so the run can be reproduced bit-for-bit.

```rust
pub struct TraceRecord {
    pub engine: String,
    pub config: super::trace::TraceConfig,
    pub frame: super::detect::PixelRect,
    pub frame_auto_detected: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `engine` | `String` | Engine identifier and version, e.g.<br>`"kovan-literature graph digitiser 0.0.0"`. |
| `config` | `super::trace::TraceConfig` | The full trace configuration used. |
| `frame` | `super::detect::PixelRect` | The pixel frame the trace ran inside. |
| `frame_auto_detected` | `bool` | `true` when the frame came from automatic detection,<br>`false` when the caller supplied it. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TraceRecord { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TraceRecord) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DigitisedDataset`

A complete digitised dataset: points **plus** the calibration, source,
operator, and review records that make them usable as validation evidence.

There is deliberately no way to build or export one without calibration
and source — they are required fields of the only constructors
([`DigitisedDataset::from_pixel_trace`] and deserialisation of a
previously exported record), and both exporters embed them.

```rust
pub struct DigitisedDataset {
    pub schema_version: u32,
    pub source: FigureSource,
    pub calibration: super::calibration::PlotCalibration,
    pub x_label: String,
    pub y_label: String,
    pub digitised_by: String,
    pub digitised_at: String,
    pub trace: Option<TraceRecord>,
    pub review: ReviewStatus,
    pub points: Vec<DigitisedPoint>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `schema_version` | `u32` | Schema version of this record ([`DATASET_SCHEMA_VERSION`]). |
| `source` | `FigureSource` | Which document and figure the points were read from. |
| `calibration` | `super::calibration::PlotCalibration` | The axis calibration every point was computed with (reference points,<br>linear/log per axis). |
| `x_label` | `String` | x-axis label as printed on the figure, units included, e.g.<br>`"Time after fission burst (s)"`. |
| `y_label` | `String` | y-axis label as printed on the figure, units included. |
| `digitised_by` | `String` | Who ran the digitisation (a person, or e.g.<br>`"kovan-digitise (automatic)"` for the unattended CLI). |
| `digitised_at` | `String` | UTC timestamp of the digitisation, ISO 8601. |
| `trace` | `Option<TraceRecord>` | The automatic pass that produced the auto-traced points; `None` for a<br>dataset built entirely by hand in a front end. |
| `review` | `ReviewStatus` | Human verification state. Starts [`ReviewStatus::Unreviewed`]. |
| `points` | `Vec<DigitisedPoint>` | The points, in increasing-x order as traced. |

##### Implementations

###### Methods

- ```rust
  pub fn from_pixel_trace</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(source: FigureSource, calibration: PlotCalibration, x_label: impl Into<String>, y_label: impl Into<String>, digitised_by: impl Into<String>, digitised_at: impl Into<String>, trace_record: TraceRecord, trace_points: &[PixelTracePoint]) -> Self { /* ... */ }
  ```
  Convert a pixel-space trace into a data-space dataset.

- ```rust
  pub fn to_json_string(self: &Self) -> String { /* ... */ }
  ```
  Serialise to pretty-printed JSON — the canonical on-disk form; feed it

- ```rust
  pub fn from_json_str(json: &str) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Parse a dataset previously written by [`DigitisedDataset::to_json_string`].

- ```rust
  pub fn write_json(self: &Self, path: &Path) -> Result<(), DigitiserError> { /* ... */ }
  ```
  Write the JSON form to `path`.

- ```rust
  pub fn read_json(path: &Path) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Read a JSON dataset from `path`.

- ```rust
  pub fn to_csv_string(self: &Self) -> String { /* ... */ }
  ```
  Serialise to CSV with the **full provenance record embedded** as `#`

- ```rust
  pub fn write_csv(self: &Self, path: &Path) -> Result<(), DigitiserError> { /* ... */ }
  ```
  Write the CSV form to `path`.

- ```rust
  pub fn record_review</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(self: &mut Self, by: impl Into<String>, at: impl Into<String>, interface: ReviewInterface) { /* ... */ }
  ```
  Record a human review — called by the hybrid front ends after the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DigitisedDataset { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DigitisedDataset) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `uncertainty_interval`

Map a `± half_pixels` pixel reading error at `pixel` through an axis
calibration, returning `(minus, plus)` magnitudes in data units (both
`>= 0`).

On a linear axis the two magnitudes are equal; on a logarithmic axis they
are asymmetric and grow with the value — which is why they are computed by
evaluating the calibration at `pixel ± half_pixels` rather than by a
constant scale factor.

```rust
pub fn uncertainty_interval(axis: &super::calibration::AxisCalibration, pixel: f64, half_pixels: f64) -> (f64, f64) { /* ... */ }
```

#### Function `utc_now_iso8601`

Current UTC time as an ISO 8601 string (`YYYY-MM-DDTHH:MM:SSZ`), from the
system clock and pure `std` (no chrono dependency). Used by the binaries
to stamp `digitised_at` / review times; pass an explicit string instead
when reproducible output is needed (the CLI's `--timestamp` flag).

```rust
pub fn utc_now_iso8601() -> String { /* ... */ }
```

### Constants and Statics

#### Constant `DATASET_SCHEMA_VERSION`

Version stamp written into every serialised dataset so future readers can
tell what they are looking at. Bump on breaking schema changes.

```rust
pub const DATASET_SCHEMA_VERSION: u32 = 1;
```

## Module `detect`

Automatic plot-frame detection — finding the axis box in pixel space.

Belongs here: [`PixelRect`], [`DetectConfig`], and
[`detect_plot_frame`], which locates the rectangle bounded by the plot's
axis lines by scanning for long dark horizontal/vertical pixel runs.
Deterministic; no ML, no OCR — it finds *where* the axes are, never what
their tick labels say (the caller supplies the numeric axis values, see
the [`super`] module doc).

Does not belong here: calibration values ([`super::calibration`]) or curve
pixels ([`super::trace`]).

```rust
pub mod detect { /* ... */ }
```

### Types

#### Struct `PixelRect`

An axis-aligned pixel rectangle, inclusive on all four edges.

Rows increase downward, so `top < bottom` numerically while `top` is the
visually upper edge.

```rust
pub struct PixelRect {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `left` | `u32` | Leftmost column (inclusive). |
| `right` | `u32` | Rightmost column (inclusive). Always `> left`. |
| `top` | `u32` | Topmost row (inclusive; visually the upper edge). |
| `bottom` | `u32` | Bottommost row (inclusive; visually the lower edge). Always `> top`. |

##### Implementations

###### Methods

- ```rust
  pub fn width(self: &Self) -> u32 { /* ... */ }
  ```
  Width in pixels (inclusive of both edges).

- ```rust
  pub fn height(self: &Self) -> u32 { /* ... */ }
  ```
  Height in pixels (inclusive of both edges).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PixelRect { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PixelRect) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DetectConfig`

Tuning knobs for [`detect_plot_frame`]. [`DetectConfig::default`] suits
typical black-on-white published figures.

```rust
pub struct DetectConfig {
    pub dark_threshold: u8,
    pub min_line_fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dark_threshold` | `u8` | A pixel with Rec. 709 luminance strictly below this counts as "dark"<br>(axis-line ink). Default 128 — the midpoint, tolerant of grey<br>anti-aliasing and scan noise. |
| `min_line_fraction` | `f64` | A row/column is an axis-line candidate when its longest contiguous<br>dark run covers at least this fraction of the image's<br>width/height. Default 0.4 — axis lines span most of a cropped figure;<br>curve segments and tick marks do not. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DetectConfig { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DetectConfig) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **ReadPrimitive**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `detect_plot_frame`

Detect the plot frame (axis box) of a black-on-white figure.

**Method (deterministic).** Every row's and column's longest contiguous
dark run is measured. Rows/columns whose run covers at least
[`DetectConfig::min_line_fraction`] of the image dimension are axis-line
candidates. With two or more candidate rows *and* columns (a fully boxed
plot), the frame is their outermost members. With exactly one of either
(an L-shaped plot: one x axis, one y axis), the missing top/right edges
are taken from the dark extent of the detected axis lines themselves.

# Errors

[`DigitiserError::Detection`] when no candidate row or column exists, or
the resulting rectangle is degenerate (under 10 px in either direction) —
in that case supply explicit pixel reference points instead (see
[`super::auto::AxisPixelRefs`]).

```rust
pub fn detect_plot_frame(raster: &super::raster::PlotRaster, config: &DetectConfig) -> Result<PixelRect, super::DigitiserError> { /* ... */ }
```

## Module `frontend`

**Attributes:**

- `Other("#[attr = CfgTrace([Any([NameValue { name: \"feature\", value: Some(\"digitise-cli\"), span: crates/kovan-literature/src/digitiser/mod.rs:69:11: 69:35 (#0) }, NameValue { name: \"feature\", value: Some(\"digitise-tui\"), span: crates/kovan-literature/src/digitiser/mod.rs:69:37: 69:61 (#0) }], crates/kovan-literature/src/digitiser/mod.rs:69:10: 69:62 (#0))])]")`

Shared command-line surface for the digitiser binaries.

Belongs here: [`AutoArgs`] — the `clap` argument set that fully describes
one automatic digitisation run — and [`AutoArgs::run`], which executes it.
Both the fully automatic `kovan-digitise` CLI and the hybrid
`kovan-digitise-tui` parse exactly these arguments, so a TUI session can
be re-run headlessly by pasting the same flags onto the CLI.

Does not belong here: any interactivity (the TUI binary owns that) or the
pipeline itself ([`super::auto`]).

Compiled only when a front end that needs it is enabled
(`digitise-cli` or `digitise-tui` features), so the plain library build
carries no `clap` dependency.

```rust
pub mod frontend { /* ... */ }
```

### Types

#### Struct `AutoArgs`

Arguments for one automatic digitisation pass.

Axis values are supplied by the caller (read from the figure's printed
labels — tick-label OCR is deliberately out of scope, see the
[`super`] module doc); pixel geometry is automatic unless explicit
`--x-ref`/`--y-ref` pairs are given.

```rust
pub struct AutoArgs {
    pub image: String,
    pub x_scale: String,
    pub y_scale: String,
    pub x_range: Option<String>,
    pub y_range: Option<String>,
    pub x_ref: Vec<String>,
    pub y_ref: Vec<String>,
    pub figure: String,
    pub document_id: Option<String>,
    pub document_title: Option<String>,
    pub page: Option<u32>,
    pub notes: Option<String>,
    pub x_label: String,
    pub y_label: String,
    pub operator: String,
    pub timestamp: Option<String>,
    pub strategy: String,
    pub step: u32,
    pub threshold: u8,
    pub curve_rgb: Option<String>,
    pub curve_tolerance: u16,
    pub inset: u32,
    pub max_column_fill: f64,
    pub dark_threshold: u8,
    pub min_line_fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `image` | `String` | Path to the plot image (PNG or JPEG). |
| `x_scale` | `String` | x-axis scale: `linear` or `log`. |
| `y_scale` | `String` | y-axis scale: `linear` or `log`. |
| `x_range` | `Option<String>` | Data values at the detected frame's left and right edges, as<br>`min,max` (e.g. `--x-range 1,1e6`). Mutually exclusive with `--x-ref`. |
| `y_range` | `Option<String>` | Data values at the detected frame's bottom and top edges, as<br>`min,max`. Mutually exclusive with `--y-ref`. |
| `x_ref` | `Vec<String>` | Explicit x reference point as `pixel=value`; give exactly twice<br>(e.g. `--x-ref 57=1 --x-ref 462=1000`). Overrides `--x-range`. |
| `y_ref` | `Vec<String>` | Explicit y reference point as `pixel=value` (pixel row, growing<br>downward); give exactly twice. Overrides `--y-range`. |
| `figure` | `String` | Figure designation as printed, e.g. `"Fig. 7"`. Required provenance. |
| `document_id` | `Option<String>` | `KovanDocument` id of the catalogued source, if any. |
| `document_title` | `Option<String>` | Source document title / free-text citation. |
| `page` | `Option<u32>` | Page the figure appears on. |
| `notes` | `Option<String>` | Free-text provenance notes (crop, curve label, known skew…). |
| `x_label` | `String` | x-axis label as printed (units included). |
| `y_label` | `String` | y-axis label as printed (units included). |
| `operator` | `String` | Operator recorded as `digitised_by`. |
| `timestamp` | `Option<String>` | Override the `digitised_at` timestamp (ISO 8601) for byte-reproducible<br>output; defaults to the current UTC time. |
| `strategy` | `String` | Trace strategy: `continuity` (default), `largest-run`, or `centroid`. |
| `step` | `u32` | Sample every Nth pixel column. |
| `threshold` | `u8` | Curve-ink luminance threshold (0–255); ignored with `--curve-rgb`. |
| `curve_rgb` | `Option<String>` | Trace a specific curve colour, as `r,g,b` (0–255 each). |
| `curve_tolerance` | `u16` | RGB distance tolerance for `--curve-rgb`. |
| `inset` | `u32` | Pixels to shrink the frame inward before tracing. |
| `max_column_fill` | `f64` | Skip columns whose ink fill exceeds this fraction (vertical gridlines). |
| `dark_threshold` | `u8` | Frame detection: luminance below this is axis ink. |
| `min_line_fraction` | `f64` | Frame detection: min dark-run fraction of the image dimension. |

##### Implementations

###### Methods

- ```rust
  pub fn run(self: &Self) -> Result<(PlotRaster, DigitisedDataset), DigitiserError> { /* ... */ }
  ```
  Load the image and run the automatic pipeline, returning the raster

- ```rust
  pub fn pipeline_config(self: &Self) -> Result<AutoDigitiseConfig, DigitiserError> { /* ... */ }
  ```
  Build the [`AutoDigitiseConfig`] these arguments describe.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Args**
  - ```rust
    fn group_id() -> Option<clap::Id> { /* ... */ }
    ```

  - ```rust
    fn augment_args<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn augment_args_for_update<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AutoArgs { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **CommandFactory**
  - ```rust
    fn command<''b>() -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn command_for_update<''b>() -> clap::Command { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **FromArgMatches**
  - ```rust
    fn from_arg_matches(__clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn from_arg_matches_mut(__clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches(self: &mut Self, __clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches_mut(self: &mut Self, __clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Parser**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `parse_scale`

Parse `linear` / `log` (also accepts `lin` / `logarithmic`).

```rust
pub fn parse_scale(s: &str) -> Result<super::calibration::AxisScale, super::DigitiserError> { /* ... */ }
```

#### Function `parse_strategy`

Parse a trace strategy name.

```rust
pub fn parse_strategy(s: &str) -> Result<super::trace::TraceStrategy, super::DigitiserError> { /* ... */ }
```

## Module `raster`

Plot image loading — an owned RGB pixel buffer decoded with pure Rust.

Belongs here: [`PlotRaster`] (the in-memory image the whole digitiser
works on) and its constructors. Decoding uses the `image` crate's
pure-Rust PNG/JPEG decoders — no C toolchain, no system libraries, so the
engine builds natively on Termux/Android.

Does not belong here: axis geometry ([`super::detect`]), curve pixels
([`super::trace`]), or any pixel *interpretation* beyond luminance.

```rust
pub mod raster { /* ... */ }
```

### Types

#### Struct `PlotRaster`

An owned, row-major RGB8 plot image.

The public API deliberately does not expose `image`-crate types, so a
caller only needs this struct and plain integers to work with the
digitiser (workspace "human interface layer" rule).

```rust
pub struct PlotRaster {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn from_path(path: &Path) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Decode a plot image from a file on disk (PNG or JPEG).

- ```rust
  pub fn from_bytes(bytes: &[u8]) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Decode a plot image from in-memory encoded bytes (PNG or JPEG).

- ```rust
  pub fn from_rgb_fn</* synthetic */ impl Fn(u32, u32) -> [u8; 3]: Fn(u32, u32) -> [u8; 3]>(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Self { /* ... */ }
  ```
  Build a raster from a pixel generator function — used by

- ```rust
  pub fn width(self: &Self) -> u32 { /* ... */ }
  ```
  Image width in pixels (number of columns).

- ```rust
  pub fn height(self: &Self) -> u32 { /* ... */ }
  ```
  Image height in pixels (number of rows).

- ```rust
  pub fn rgb(self: &Self, x: u32, y: u32) -> [u8; 3] { /* ... */ }
  ```
  RGB triple at column `x`, row `y` (row 0 is the top of the image).

- ```rust
  pub fn luminance(self: &Self, x: u32, y: u32) -> u8 { /* ... */ }
  ```
  Rec. 709 luminance of the pixel at `(x, y)`, 0 (black) – 255 (white).

- ```rust
  pub fn source_sha256(self: &Self) -> Option<&str> { /* ... */ }
  ```
  Lowercase-hex SHA-256 of the encoded source bytes, when this raster

- ```rust
  pub fn to_png_bytes(self: &Self) -> Result<Vec<u8>, DigitiserError> { /* ... */ }
  ```
  Encode this raster as PNG bytes (pure Rust). Used to write synthetic

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PlotRaster { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PlotRaster) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `synthetic`

Synthetic plot rendering — deterministic ground-truth fixtures.

Belongs here: [`SyntheticPlotSpec`] and [`render_synthetic_plot`], which
draw a *known analytic curve* into a [`PlotRaster`] at known pixel
positions and return the exact [`PlotCalibration`] used. The
self-consistency tests (`tests/digitiser_synthetic.rs`) digitise these
images and compare the recovered points against the analytic function —
the only ground truth available until the maintainer-supplied golden
oracle (bead `op-amfh`) lands. Keeping the renderer public also lets that
future oracle comparison reuse the same tolerance machinery.

Does not belong here: any digitising. This module only *makes* images.

```rust
pub mod synthetic { /* ... */ }
```

### Types

#### Struct `SyntheticPlotSpec`

Description of a synthetic plot: image size, frame placement, axis ranges
and scales, and the curve to draw.

The curve is a plain function pointer (`fn(f64) -> f64`), not a closure
trait object, per the workspace no-trait-objects rule; every fixture curve
is a free function anyway.

```rust
pub struct SyntheticPlotSpec {
    pub width: u32,
    pub height: u32,
    pub frame: super::detect::PixelRect,
    pub x_scale: super::calibration::AxisScale,
    pub x_min: f64,
    pub x_max: f64,
    pub y_scale: super::calibration::AxisScale,
    pub y_min: f64,
    pub y_max: f64,
    pub curve: fn(f64) -> f64,
    pub curve_half_thickness: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `width` | `u32` | Total image width in pixels. |
| `height` | `u32` | Total image height in pixels. |
| `frame` | `super::detect::PixelRect` | Where the axis frame is drawn. Must fit inside the image with at<br>least 1 px margin. |
| `x_scale` | `super::calibration::AxisScale` | x-axis scale and the data values at the frame's left and right edges. |
| `x_min` | `f64` | Data x at `frame.left`. |
| `x_max` | `f64` | Data x at `frame.right`. |
| `y_scale` | `super::calibration::AxisScale` | y-axis scale and the data values at the frame's bottom and top edges. |
| `y_min` | `f64` | Data y at `frame.bottom` (rows grow downward, so the bottom edge is<br>the *smaller* y for a conventional plot). |
| `y_max` | `f64` | Data y at `frame.top`. |
| `curve` | `fn(f64) -> f64` | The curve to draw: `y = curve(x)` in data units. |
| `curve_half_thickness` | `u32` | Half-thickness of the drawn curve in pixels (the drawn band spans<br>`centre ± half`, so thickness is `2*half + 1`). 1 gives a 3-px line,<br>typical of published figures. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SyntheticPlotSpec { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `render_synthetic_plot`

Render the spec to an image, returning the raster **and the exact
calibration** implied by the frame/ranges (which is also the ground-truth
calibration a digitising test should use).

**Method (deterministic).** White background; 1-px black frame on the
spec's rectangle; then for every pixel column strictly inside the frame,
`x = cal.x.value_at(col)` and the curve pixel row is
`cal.y.pixel_at(curve(x))`. A vertical band of `2*half+1` px is inked at
the rounded row, and consecutive columns are connected by filling the row
interval between them, so steep curves have no gaps. Curve values that
fall outside the frame (or are non-finite / non-positive on a log axis)
are simply not drawn for that column.

# Errors

[`DigitiserError::Calibration`] if the axis ranges are invalid for their
scale (via [`AxisCalibration::new`]), or the frame does not fit in the
image.

```rust
pub fn render_synthetic_plot(spec: &SyntheticPlotSpec) -> Result<(super::raster::PlotRaster, super::calibration::PlotCalibration), super::DigitiserError> { /* ... */ }
```

## Module `trace`

Automatic curve tracing — extracting curve pixel positions by column scan.

Belongs here: [`CurveSelector`] (which pixels count as curve ink),
[`TraceStrategy`] (which vertical run to keep when a column has several),
[`TraceConfig`], [`PixelTracePoint`], and [`trace_curve`]. All strategy
dispatch is by enum `match` — no trait objects, per the workspace Rust
design rules. The trace is deterministic: the same raster and config
always produce the same points.

Does not belong here: converting pixels to data values (that is
[`super::calibration`], applied in [`super::dataset`]) and axis-box
finding ([`super::detect`]).

```rust
pub mod trace { /* ... */ }
```

### Types

#### Enum `CurveSelector`

Which pixels count as "curve ink". Closed set, enum-dispatched.

```rust
pub enum CurveSelector {
    DarkestBand {
        max_luminance: u8,
    },
    Rgb {
        rgb: [u8; 3],
        tolerance: u16,
    },
}
```

##### Variants

###### `DarkestBand`

Any pixel with Rec. 709 luminance strictly below `max_luminance` is
curve ink. The right default for black-on-white published figures.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `max_luminance` | `u8` | Luminance cut, 0–255. 128 tolerates anti-aliasing and scan grey. |

###### `Rgb`

Pixels within `tolerance` of a target colour (Euclidean RGB distance,
0–441). Use for a coloured curve that must be separated from black
gridlines or from other curves.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `rgb` | `[u8; 3]` | Target curve colour as `[r, g, b]`. |
| `tolerance` | `u16` | Maximum Euclidean RGB distance from `rgb` that still counts. |

##### Implementations

###### Methods

- ```rust
  pub fn matches(self: &Self, raster: &PlotRaster, x: u32, y: u32) -> bool { /* ... */ }
  ```
  Does the pixel at `(x, y)` count as curve ink under this selector?

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CurveSelector { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CurveSelector) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `TraceStrategy`

When a scanned column holds several disjoint vertical runs of curve ink
(curve + gridline, or two curves), which one is the curve? Closed set,
enum-dispatched. Ties always resolve to the topmost run (deterministic).

```rust
pub enum TraceStrategy {
    ColumnCentroid,
    LargestRun,
    ContinuityNearest,
}
```

##### Variants

###### `ColumnCentroid`

Centroid of *all* matching pixels in the column. Cheapest; correct
only when the column contains nothing but the one curve.

###### `LargestRun`

Centroid of the longest contiguous run. Robust against thin
horizontal gridlines crossing the column.

###### `ContinuityNearest`

Centroid of the run nearest (vertically) to the previous column's
accepted point; the first accepted column uses the longest run. Tracks
one curve through crossings with other curves or gridlines. The
default.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TraceStrategy { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TraceStrategy) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `TraceConfig`

Tuning for [`trace_curve`]. [`TraceConfig::default`] suits a clean
black-on-white single-curve figure.

```rust
pub struct TraceConfig {
    pub selector: CurveSelector,
    pub strategy: TraceStrategy,
    pub column_step: u32,
    pub inset: u32,
    pub max_column_fill: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `selector` | `CurveSelector` | What counts as curve ink. Default: luminance < 128. |
| `strategy` | `TraceStrategy` | Run-choice strategy. Default: [`TraceStrategy::ContinuityNearest`]. |
| `column_step` | `u32` | Sample every `column_step`-th pixel column (≥ 1). Default 1. |
| `inset` | `u32` | Pixels to shrink the frame inward on every side before scanning, so<br>the frame lines and their anti-aliasing halo are not traced as curve.<br>Default 3. |
| `max_column_fill` | `f64` | Skip a column when the matched fraction of its scanned height exceeds<br>this (it is a vertical gridline or axis, not curve). Default 0.6. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TraceConfig { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TraceConfig) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **ReadPrimitive**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PixelTracePoint`

One traced curve sample, still in pixel coordinates.

```rust
pub struct PixelTracePoint {
    pub x_px: f64,
    pub y_px: f64,
    pub thickness_px: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x_px` | `f64` | Column index of the sample (whole pixel, stored as `f64` so hand<br>corrections can be sub-pixel). |
| `y_px` | `f64` | Centroid row of the accepted ink run in this column. |
| `thickness_px` | `f64` | Vertical extent (pixel count) of the accepted run — the local curve<br>line thickness, which [`super::dataset`] turns into the per-point<br>reading uncertainty. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PixelTracePoint { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PixelTracePoint) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `trace_curve`

Trace the curve inside `frame`, one sample per scanned column.

**Method (deterministic).** For each sampled column inside the frame
(shrunk by [`TraceConfig::inset`]), the contiguous vertical runs of pixels
matching [`TraceConfig::selector`] are collected. Columns whose matched
fraction exceeds [`TraceConfig::max_column_fill`] are skipped as vertical
gridlines. One run is accepted per remaining column according to
[`TraceConfig::strategy`], and its centroid row becomes the sample.
Columns with no matching pixels yield no sample (gaps are permitted —
dashed curves still trace).

Returns the samples in strictly increasing `x_px` order; possibly empty
(e.g. an empty plot region) — emptiness is the *caller's* signal to warn,
not an error, because a legitimately empty sub-range can occur when
tracing a figure region-by-region.

# Errors

[`DigitiserError::Trace`] if `frame` (after inset) leaves no columns or
rows to scan, or `column_step == 0`.

```rust
pub fn trace_curve(raster: &super::raster::PlotRaster, frame: &super::detect::PixelRect, config: &TraceConfig) -> Result<Vec<PixelTracePoint>, super::DigitiserError> { /* ... */ }
```

### Types

#### Enum `DigitiserError`

Errors produced by the graph digitiser.

Enum-dispatched per the workspace Rust design rules (no trait objects).
Every variant carries a human-readable message describing what failed.

```rust
pub enum DigitiserError {
    Image(String),
    Calibration(String),
    Detection(String),
    Trace(String),
    Io(String),
}
```

##### Variants

###### `Image`

The image file could not be read or decoded (bad path, unsupported
format, corrupt data).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Calibration`

Axis calibration is invalid — coincident reference pixels, coincident
reference values, or non-positive values on a logarithmic axis.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Detection`

The plot frame (axis box) could not be detected automatically.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Trace`

Curve tracing failed (e.g. no curve pixels found inside the frame).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Io`

A dataset file could not be read, written, or parsed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DigitiserError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DigitiserError) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `storage`

Roots of the on-disk storage tree, relative to the crate directory.

Implements `docs/kovan.md`, "Storage Layout".

```rust
pub mod storage { /* ... */ }
```

### Functions

#### Function `generated_dir_for`

Directory a generated artifact of `kind` and `visibility` belongs in,
joined onto `base` (usually the `kovan-literature` crate directory).

The generated tree is split by [`Visibility`] one level below each artifact
kind — `generated/bibtex/open/`, `generated/bibtex/proprietary/`, and so on
— because the two halves have different distribution rules:

- **open** — committed to the repository, and for BibTeX also *published*
  in the packaged crate (citation entries are small bibliographic facts).
- **proprietary** — never committed and never published; an artifact
  derived from user-owned content is equally user-owned.

Both rules are enforced outside this function (the root `.gitignore` and
the `exclude` list in this crate's `Cargo.toml`); this is the single place
that decides *which* directory a writer should target, so the two
mechanisms and the code cannot drift apart.

`kind` should be one of [`BIBTEX_DIR`], [`MARKDOWN_DIR`], [`ASSETS_DIR`].

```rust
pub fn generated_dir_for(base: &std::path::Path, kind: &str, visibility: super::Visibility) -> std::path::PathBuf { /* ... */ }
```

#### Function `root_for`

Return the storage root for a given [`Visibility`], joined onto `base`
(usually the `kovan-literature` crate directory).

```rust
pub fn root_for(base: &std::path::Path, visibility: super::Visibility) -> std::path::PathBuf { /* ... */ }
```

#### Function `visibility_from_path`

Infer a document's [`Visibility`] from where its source file lives.

**Closed by default.** A document is [`Visibility::Open`] only when its
path explicitly contains an `open/` component. Everything else —
including `proprietary/`, and including any path with neither marker —
is [`Visibility::Proprietary`].

# Why the default is closed

The two ways of being wrong here are not symmetric:

- Mislabelling an **open** document as proprietary costs a reviewer a
  minute and keeps a committable file out of git. Recoverable.
- Mislabelling a **proprietary** document as open invites it into
  `open/`, which `.gitignore` deliberately un-ignores for PDFs, and from
  there into a public repository. That is a licence violation, and
  pushed history is not something you can quietly take back.

So the rule fails towards the recoverable error. This matches the
instruction in `kovan_import/README.md` — "unsure -> treat as
proprietary and ask" — and `DATA_POLICY.md`.

# The bug this replaced

Until 2026-08-11 this defaulted to [`Visibility::Open`] and only
special-cased `proprietary/`, so a source file staged anywhere else —
notably `kovan_import/`, the gitignored drop area where documents sit
*before* their access tier has been decided — was silently labelled
Open. That is precisely the unrecoverable direction. It was found when
Tobias (1980), a Pergamon Press work with all rights reserved, imported
as `visibility: Open` despite being written to proprietary output paths
(bead `op-nv6g`). The old doc comment claimed the function existed "so
proprietary material never gets an open label by accident", which is
what it should have done and did not.

Note this is a *storage-layout* inference, not a licence determination.
The access tier is decided by a human reading the document's own
copyright page, then expressed by choosing where to put the file.

```rust
pub fn visibility_from_path(path: &std::path::Path) -> super::Visibility { /* ... */ }
```

#### Function `document_type_from_path`

Infer a [`super::DocumentType`] from a storage sub-directory name in the
source path (`papers/`, `reports/`, `standards/`, `benchmarks/`,
`manuals/`, `theses/` or `dissertations/`), falling back to
[`super::DocumentType::Other`] when none is present.

```rust
pub fn document_type_from_path(path: &std::path::Path) -> super::DocumentType { /* ... */ }
```

### Constants and Statics

#### Constant `OPEN_ROOT`

Directory for redistributable content that may be committed.

```rust
pub const OPEN_ROOT: &str = "open";
```

#### Constant `PROPRIETARY_ROOT`

Directory for user-owned content that must never be committed.

```rust
pub const PROPRIETARY_ROOT: &str = "proprietary";
```

#### Constant `GENERATED_ROOT`

Directory for reproducible generated artifacts.

```rust
pub const GENERATED_ROOT: &str = "generated";
```

#### Constant `BIBTEX_DIR`

Sub-directory of [`GENERATED_ROOT`] holding generated BibTeX entries.

```rust
pub const BIBTEX_DIR: &str = "bibtex";
```

#### Constant `MARKDOWN_DIR`

Sub-directory of [`GENERATED_ROOT`] holding generated Markdown bodies.

```rust
pub const MARKDOWN_DIR: &str = "markdown";
```

#### Constant `ASSETS_DIR`

Sub-directory of [`GENERATED_ROOT`] holding extracted image assets.

```rust
pub const ASSETS_DIR: &str = "assets";
```

## Types

### Enum `LiteratureError`

Errors produced by the literature pipeline.

```rust
pub enum LiteratureError {
    Unimplemented(&'static str),
    Io(String),
}
```

#### Variants

##### `Unimplemented`

The requested operation is not implemented yet (placeholder stage).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

##### `Io`

A source file could not be read, parsed, or was malformed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

#### Implementations

##### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LiteratureError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LiteratureError) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Constants and Statics

### Constant `MAX_MARKDOWN_PAGES`

Target maximum number of pages per generated Markdown document. Larger
documents should be split with [`split_markdown_by_page_limit`]. See
`docs/kovan.md`, "PDF Processing" (`≤ 30 pages` per Markdown document).

```rust
pub const MAX_MARKDOWN_PAGES: u32 = 30;
```

## Re-exports

### Re-export `Author`

```rust
pub use kovan_common::Author;
```

### Re-export `DocumentType`

```rust
pub use kovan_common::DocumentType;
```

### Re-export `KovanBenchmark`

```rust
pub use kovan_common::KovanBenchmark;
```

### Re-export `KovanDocument`

```rust
pub use kovan_common::KovanDocument;
```

### Re-export `Visibility`

```rust
pub use kovan_common::Visibility;
```

### Re-export `to_bibtex`

```rust
pub use bibtex::to_bibtex;
```

### Re-export `markdown_outline`

```rust
pub use markdown::markdown_outline;
```

### Re-export `split_markdown_by_page_limit`

```rust
pub use markdown::split_markdown_by_page_limit;
```

### Re-export `text_to_markdown`

```rust
pub use markdown::text_to_markdown;
```

### Re-export `Heading`

```rust
pub use markdown::Heading;
```

### Re-export `PAGE_SEPARATOR`

```rust
pub use markdown::PAGE_SEPARATOR;
```

### Re-export `extract_metadata`

```rust
pub use metadata::extract_metadata;
```

### Re-export `extract_assets`

```rust
pub use pdf_import::extract_assets;
```

### Re-export `extract_pdf_text`

```rust
pub use pdf_import::extract_pdf_text;
```

### Re-export `pdf_to_markdown`

```rust
pub use pdf_import::pdf_to_markdown;
```

