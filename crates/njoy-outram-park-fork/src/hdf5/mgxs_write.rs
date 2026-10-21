// SPDX-License-Identifier: GPL-3.0

//! **Writing OpenMC's `mgxs.h5`** — the smallest useful HDF5 round trip.
//! GitHub #270, scope item 2.
//!
//! # Why this crate and not `outram-mc-libs`
//!
//! Maintainer direction on #270: HDF5 belongs on the njoy side, for both
//! reading and writing, because it is nuclear data. `outram-mc-libs` stays
//! data-free and file-I/O-free in its inner transport loop and hands structured
//! data to this codec at the run boundary. That also means the note in
//! `outram-mc-libs`' scope ("HDF5 I/O — out of scope") is still **correct for
//! that crate**: the capability is in the workspace, just not in the transport
//! crate.
//!
//! # Why this format first
//!
//! `mgxs.h5` is the smallest OpenMC format that is still a real interchange
//! test: it has nested groups, scalar and 1-D datasets, and attributes of four
//! distinct types (int, float array, byte string, bool). Getting it read back
//! by OpenMC itself exercises every mechanism a nuclide `.h5` needs, at a
//! fraction of the size.
//!
//! # Survey result (#270 scope item 1), recorded because the issue asks
//!
//! `hdf5-pure` 0.20.1's **write** side covers everything this needs:
//!
//! | capability | API | needed here |
//! |---|---|---|
//! | groups, nested | `create_group` | yes |
//! | datasets | `create_dataset().with_f64_data(..)` | yes |
//! | attributes | `set_attr(name, AttrValue::..)` | yes |
//! | compound types | `with_compound_data`, `with_compound_values<T>` | not here |
//! | compression | `with_deflate(level)`, `with_shuffle()` | not here |
//! | strings | `AttrValue::{String, AsciiString, ..}` | yes |
//!
//! So the plan in #270 stands as written and nothing has to change. That is the
//! finding; it was not obvious beforehand, and a read-only or write-limited
//! answer would have changed the rest of the issue.
//!
//! **One correction against a first reading of the crate.** Grepping the
//! builder modules for `attribute` finds only `extract_attributes*`, which are
//! read-side, and it is easy to conclude attributes cannot be written. They
//! can: the writer spells it `set_attr` with an `AttrValue` enum. Recorded
//! because concluding otherwise would have blocked this issue on a false
//! premise.

use std::path::Path;

use hdf5_pure::{AttrValue, FileBuilder};

use crate::NjoyError;

/// One material's multigroup data, at one temperature.
#[derive(Debug, Clone)]
pub struct MgxsMaterial {
    /// Material name; becomes the top-level group.
    pub name: String,
    /// Temperature label, e.g. `"294K"`. OpenMC keys the inner group on this
    /// and cross-checks it against `kTs/<label>`.
    pub temperature_label: String,
    /// `kT` \[eV\] for that temperature.
    pub kt_ev: f64,
    pub total: Vec<f64>,
    pub absorption: Vec<f64>,
    pub fission: Vec<f64>,
    pub nu_fission: Vec<f64>,
    pub chi: Vec<f64>,
    /// Row-major `[G][G'][Order]`; with `order = 0` this is `G * G'` long.
    pub scatter_matrix: Vec<f64>,
    /// Per outgoing-group first and last scattering group, 1-based, as OpenMC
    /// stores them.
    pub g_min: Vec<i64>,
    pub g_max: Vec<i64>,
}

/// Write an OpenMC-format `mgxs.h5`.
///
/// `group_edges` is the energy group structure \[eV\], ascending, `n_groups + 1`
/// long.
///
/// # The layout, which is not guessable and was read off a real file
///
/// ```text
/// /                     @filetype = b"mgxs"   @version = [1, 0]
///                       @energy_groups = G    @delayed_groups = 0
///                       @"group structure" = [E0 .. EG]
/// /<mat>                @fissionable @order @representation
///                       @scatter_format @scatter_shape
/// /<mat>/kTs/<label>    scalar f64, kT in eV
/// /<mat>/<label>/{total,absorption,fission,nu-fission,chi}
/// /<mat>/<label>/scatter_data/{g_min,g_max,scatter_matrix}
/// ```
///
/// Note `nu-fission` is spelled with a **hyphen** while `nu_fission` elsewhere
/// uses an underscore, and the root attribute is `"group structure"` **with a
/// space**. Both are OpenMC's, both are load-bearing, and neither is something
/// a reasonable person would invent.
pub fn write_mgxs<P: AsRef<Path>>(
    path: P,
    group_edges: &[f64],
    materials: &[MgxsMaterial],
) -> Result<(), NjoyError> {
    let n_groups = group_edges.len().saturating_sub(1);
    if n_groups == 0 {
        return Err(NjoyError::Hdf5(
            "mgxs needs at least one energy group (two group edges)".to_string(),
        ));
    }
    if materials.is_empty() {
        return Err(NjoyError::Hdf5(
            "mgxs needs at least one material".to_string(),
        ));
    }

    let mut b = FileBuilder::new();
    b.set_attr("filetype", AttrValue::AsciiString("mgxs".into()));
    b.set_attr("version", AttrValue::I64Array(vec![1, 0]));
    b.set_attr("energy_groups", AttrValue::I64(n_groups as i64));
    b.set_attr("delayed_groups", AttrValue::I64(0));
    b.set_attr(
        "group structure",
        AttrValue::F64Array(group_edges.to_vec()),
    );

    for m in materials {
        for (label, v) in [
            ("total", &m.total),
            ("absorption", &m.absorption),
            ("fission", &m.fission),
            ("nu-fission", &m.nu_fission),
            ("chi", &m.chi),
        ] {
            if v.len() != n_groups {
                return Err(NjoyError::Hdf5(format!(
                    "material {}: {label} has {} values, expected {n_groups}",
                    m.name,
                    v.len()
                )));
            }
        }

        let mut g = b.create_group(&m.name);
        g.set_attr("fissionable", AttrValue::I32(1));
        g.set_attr("order", AttrValue::I64(0));
        g.set_attr(
            "representation",
            AttrValue::AsciiString("isotropic".into()),
        );
        g.set_attr("scatter_format", AttrValue::AsciiString("legendre".into()));
        g.set_attr(
            "scatter_shape",
            AttrValue::AsciiString("[G][G'][Order]".into()),
        );

        {
            let mut kts = g.create_group("kTs");
            kts.create_dataset(&m.temperature_label)
                .with_f64_data(&[m.kt_ev]);
            g.add_group(kts.finish());
        }

        let mut t = g.create_group(&m.temperature_label);
        t.create_dataset("total").with_f64_data(&m.total);
        t.create_dataset("absorption").with_f64_data(&m.absorption);
        t.create_dataset("fission").with_f64_data(&m.fission);
        t.create_dataset("nu-fission").with_f64_data(&m.nu_fission);
        t.create_dataset("chi").with_f64_data(&m.chi);
        {
            let mut sd = t.create_group("scatter_data");
            sd.create_dataset("g_min").with_i64_data(&m.g_min);
            sd.create_dataset("g_max").with_i64_data(&m.g_max);
            sd.create_dataset("scatter_matrix")
                .with_f64_data(&m.scatter_matrix);
            t.add_group(sd.finish());
        }
        g.add_group(t.finish());
        b.add_group(g.finish());
    }

    b.write(path.as_ref())
        .map_err(|e| NjoyError::Hdf5(format!("writing mgxs.h5: {e}")))
}
